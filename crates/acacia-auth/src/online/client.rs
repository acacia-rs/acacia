use std::sync::Mutex;

use p256::ecdsa::SigningKey as XboxKey;
use p384::ecdsa::SigningKey;
use serde_json::Value;

use super::cache::{CachedCredentials, CachedTokens};
use super::config::{AuthConfig, endpoints as ep};
use super::http::Http;
use super::live::{self, DeviceCodePrompt, MsaToken};
use super::minecraft::{self, AuthEnvironment, Discovery, PlayFabSession, QosBeacon, ServiceToken, SignalingEnvironment};
use super::nsal;
use super::xbox::{self, SisuTokens, XboxToken};
use crate::jwt::{self, now_unix, public_key_der_b64};
use crate::{Error, LoginCredentials, Result};

/// Async auth HTTP client. Cheap to share (`Arc`); one per proxy is the intended shape.
pub struct AuthClient {
    http: Http,
    cfg: AuthConfig,
    discovery: Mutex<Option<Discovery>>,
    /// `(multiplayer_rp, playfab_rp)` resolved from NSAL; the same for every user of a title.
    relying_parties: Mutex<Option<(String, String)>>,
}

impl AuthClient {
    pub fn new(cfg: AuthConfig) -> Result<Self> {
        Ok(Self {
            http: Http::new(&cfg)?,
            cfg,
            discovery: Mutex::new(None),
            relying_parties: Mutex::new(None),
        })
    }

    pub fn config(&self) -> &AuthConfig {
        &self.cfg
    }

    pub(crate) fn http(&self) -> &Http {
        &self.http
    }

    pub async fn start_device_code(&self) -> Result<DeviceCodePrompt> {
        live::start_device_code(&self.http, self.cfg.title).await
    }

    /// Polls until the user signs in, the code expires or the user declines.
    pub async fn poll_device_code(&self, prompt: &DeviceCodePrompt) -> Result<MsaToken> {
        live::poll_device_code(&self.http, self.cfg.title, prompt).await
    }

    pub async fn refresh_msa(&self, token: &MsaToken) -> Result<MsaToken> {
        live::refresh(&self.http, self.cfg.title, token).await
    }

    /// Full chain from an MSA token, with a fresh Xbox device key and no caching. Prefer
    /// [`crate::Account`] for repeated logins.
    pub async fn bedrock_credentials(
        &self,
        msa: &MsaToken,
        client_key: &SigningKey,
    ) -> Result<LoginCredentials> {
        let mut state = CachedTokens::new(Some(msa.clone()));
        self.credentials_with(&mut state, client_key).await
    }

    /// Brings `state` up to date (refreshing only expired tokens) and returns credentials bound
    /// to `client_key`: MSA → device token → SISU → XSTS (multiplayer + PlayFab RPs) → Mojang
    /// chain, PlayFab → session/start → multiplayer token.
    pub async fn credentials_with(
        &self,
        state: &mut CachedTokens,
        client_key: &SigningKey,
    ) -> Result<LoginCredentials> {
        let client_pub = public_key_der_b64(client_key);
        if let Some(c) = &state.credentials
            && c.client_public_key == client_pub
            && c.credentials.expires_at - 60 > now_unix()
        {
            return Ok(c.credentials.clone());
        }

        let (key, _, sisu) = self.xbox_session(state).await?;
        let env = self.environment().await?;
        let (mp_rp, _) = self.relying_parties(&key, &sisu.authorization, &env).await;
        let mp_xsts = self.xsts_with(state, &mp_rp).await?;

        let chain =
            minecraft::mojang_chain(&self.http, &mp_xsts, &client_pub, &self.cfg.game_version).await?;
        let identity = chain_identity(&chain)?;

        let playfab = self.playfab_with(state).await?;
        let service = self.service_token_with(state).await?;
        let (mp_token, mp_valid_until) =
            minecraft::multiplayer_token(&self.http, &env, &service, &client_pub).await?;

        let creds = LoginCredentials {
            expires_at: identity.chain_expiry.min(mp_valid_until),
            chain,
            multiplayer_token: Some(mp_token),
            xuid: identity.xuid,
            display_name: identity.display_name,
            identity: identity.identity,
            playfab_id: Some(playfab.playfab_id),
        };
        state.credentials =
            Some(CachedCredentials { client_public_key: client_pub, credentials: creds.clone() });
        Ok(creds)
    }

    /// The Minecraft services token (`MCToken`) from session/start, e.g. for NetherNet signaling.
    pub async fn service_token_with(&self, state: &mut CachedTokens) -> Result<ServiceToken> {
        if let Some(s) = state.service_token.clone().filter(ServiceToken::is_valid) {
            return Ok(s);
        }
        let playfab = self.playfab_with(state).await?;
        let env = self.environment().await?;
        let s = minecraft::session_start(&self.http, &env, &playfab, &state.device_id, &self.cfg.game_version)
            .await?;
        state.service_token = Some(s.clone());
        Ok(s)
    }

    /// The signaling service environment from discovery, if this game version lists one.
    pub async fn signaling_environment(&self) -> Result<Option<SignalingEnvironment>> {
        Ok(self.discovery().await?.signaling)
    }

    /// The QoS beacons to measure for a realm join's `pingRegions`, in the order to send them.
    pub async fn qos_beacons(&self) -> Result<Vec<QosBeacon>> {
        Ok(self.discovery().await?.qos_beacons)
    }

    async fn playfab_with(&self, state: &mut CachedTokens) -> Result<PlayFabSession> {
        if let Some(p) = state.playfab.clone().filter(PlayFabSession::is_valid) {
            return Ok(p);
        }
        let (key, _, sisu) = self.xbox_session(state).await?;
        let env = self.environment().await?;
        let (_, pf_rp) = self.relying_parties(&key, &sisu.authorization, &env).await;
        let pf_xsts = self.xsts_with(state, &pf_rp).await?;
        let p = minecraft::playfab_login(&self.http, &env, &pf_xsts, &key, self.cfg.title.user_agent()).await?;
        state.playfab = Some(p.clone());
        Ok(p)
    }

    /// An XSTS token for `relying_party` (e.g. Realms), reusing cached tokens while valid.
    pub async fn xsts_with(&self, state: &mut CachedTokens, relying_party: &str) -> Result<XboxToken> {
        if let Some(t) = state.xsts.get(relying_party).filter(|t| t.is_valid()) {
            return Ok(t.clone());
        }
        let (key, device, sisu) = self.xbox_session(state).await?;
        let t = xbox::xsts(&self.http, self.cfg.title, &key, relying_party, &device, &sisu).await?;
        state.xsts.insert(relying_party.to_owned(), t.clone());
        Ok(t)
    }

    /// Proof key and SISU tokens, refreshing the MSA, device and SISU tokens as needed.
    async fn xbox_session(
        &self,
        state: &mut CachedTokens,
    ) -> Result<(XboxKey, XboxToken, SisuTokens)> {
        let title = self.cfg.title;
        let msa = self.valid_msa(state).await?;
        let key = state.xbox_key();
        let device = match state.device_token.clone().filter(XboxToken::is_valid) {
            Some(d) => d,
            None => {
                let d = xbox::device_token(&self.http, title, &key).await?;
                state.device_token = Some(d.clone());
                d
            }
        };
        let sisu = match state.sisu.clone().filter(|s| s.is_valid()) {
            Some(s) => s,
            None => {
                let s = xbox::sisu_authorize(&self.http, title, &key, &msa.access_token, &device)
                    .await?;
                state.xsts.insert(ep::RP_XBOXLIVE.into(), s.authorization.clone());
                state.sisu = Some(s.clone());
                s
            }
        };
        Ok((key, device, sisu))
    }

    async fn valid_msa(&self, state: &mut CachedTokens) -> Result<MsaToken> {
        let msa = state.msa.clone().ok_or_else(|| Error::OAuth {
            code: "no_msa_token".into(),
            description: "account has no Microsoft token; run the device code flow".into(),
        })?;
        if msa.is_valid() {
            return Ok(msa);
        }
        let fresh = self.refresh_msa(&msa).await?;
        state.msa = Some(fresh.clone());
        Ok(fresh)
    }

    async fn environment(&self) -> Result<AuthEnvironment> {
        Ok(self.discovery().await?.auth)
    }

    async fn discovery(&self) -> Result<Discovery> {
        if let Some(d) = self.discovery.lock().unwrap_or_else(|e| e.into_inner()).clone() {
            return Ok(d);
        }
        let d = minecraft::discover(&self.http, &self.cfg.game_version).await?;
        *self.discovery.lock().unwrap_or_else(|e| e.into_inner()) = Some(d.clone());
        Ok(d)
    }

    /// NSAL lookups are best-effort: on failure the hardcoded relying parties are used and the
    /// result is not cached, so the next login retries.
    async fn relying_parties(
        &self,
        key: &p256::ecdsa::SigningKey,
        xboxlive: &XboxToken,
        env: &AuthEnvironment,
    ) -> (String, String) {
        if let Some(rps) = self.relying_parties.lock().unwrap_or_else(|e| e.into_inner()).clone() {
            return rps;
        }
        let current = nsal::fetch_current(&self.http, key, xboxlive).await;
        let default = nsal::fetch_default(&self.http).await;
        let complete = current.is_ok() && default.is_ok();
        let titles: Vec<_> = [current, default].into_iter().filter_map(Result::ok).collect();
        let rps = nsal::resolve(&titles, &env.playfab_host());
        if complete {
            *self.relying_parties.lock().unwrap_or_else(|e| e.into_inner()) = Some(rps.clone());
        }
        rps
    }
}

struct ChainIdentity {
    xuid: String,
    display_name: String,
    identity: String,
    chain_expiry: i64,
}

/// Reads `extraData` and the earliest `exp` from the Mojang chain (unverified: it came straight
/// from Mojang over TLS; servers do the verification).
fn chain_identity(chain: &[String]) -> Result<ChainIdentity> {
    let mut extra: Option<Value> = None;
    let mut expiry = i64::MAX;
    for token in chain {
        let claims = jwt::decode(token)?.claims;
        if let Some(exp) = claims.get("exp").and_then(Value::as_i64) {
            expiry = expiry.min(exp);
        }
        if let Some(e) = claims.get("extraData") {
            extra = Some(e.clone());
        }
    }
    let extra = extra.ok_or_else(|| Error::Protocol {
        endpoint: ep::MOJANG_CHAIN.into(),
        reason: "chain has no extraData".into(),
    })?;
    let field = |k: &str| extra.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    Ok(ChainIdentity {
        xuid: field("XUID"),
        display_name: field("displayName"),
        identity: field("identity"),
        chain_expiry: expiry,
    })
}
