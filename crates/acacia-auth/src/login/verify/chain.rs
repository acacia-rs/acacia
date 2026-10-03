//! The legacy certificate chain: one self-signed JWT (offline), or three rooted at Mojang's key
//! (client head → Mojang → identity). Rules follow gophertunnel `login.parseLegacyChain`.

use serde_json::Value;
use uuid::Uuid;

use super::jws::{self, check_time, key_claim, str_claim};
use super::{Identity, Part, Subject, VerifyError};
use crate::jwt;

/// Mojang's current (rotated) chain-signing key, P-384 SPKI DER in base64; from gophertunnel
/// `minecraft/protocol/login/request.go`.
pub const MOJANG_ROOT_KEY: &str = "MHYwEAYHKoZIzj0CAQYFK4EEACIDYgAECRXueJeTDqNRRgJi/vlRufByu/2G0i2Ebt6YMar5QX/R0DIIyrJMcUpruK4QveTfJSTp3Shlq4Gk34cD/4GUWwkv0DVuzeuB+tXija7HBxii03NHDbPAD0AKnLr2wdAp";

const MOJANG_ISSUER: &str = "Mojang";

pub(super) fn verify(root: &p384::PublicKey, chain: &[String], now: i64, leeway: i64) -> Result<Subject, VerifyError> {
    let rooted_len = match chain.len() {
        1 => false,
        3 => true,
        n => return Err(VerifyError::ChainLength(n)),
    };
    let head = jws::decode(Part::Chain(0), &chain[0])?;
    let mut key = head
        .x5u()
        .and_then(|x5u| jwt::parse_public_key_der_b64(x5u).ok())
        .ok_or(VerifyError::Claim { part: Part::Chain(0), claim: "x5u" })?;

    let mut trusted = false;
    let mut last = Value::Null;
    for (i, token) in chain.iter().enumerate() {
        let part = Part::Chain(i);
        let token = jws::decode(part, token)?;
        jws::verify_es384(part, &token, &key)?;
        check_time(part, &token.claims, now, leeway, false)?;
        if i > 0 {
            let issuer = token.claims.get("iss").and_then(Value::as_str);
            if issuer != Some(MOJANG_ISSUER) {
                return Err(VerifyError::Issuer { part, found: issuer.map(str::to_owned) });
            }
        }
        // Every link of a rooted chain must name the next key; the offline JWT may omit its own.
        if rooted_len || token.claims.get("identityPublicKey").is_some() {
            key = key_claim(part, &token.claims, "identityPublicKey")?;
        }
        if i == 0 {
            trusted = rooted_len && key == *root;
        }
        last = token.claims;
    }

    let part = Part::Chain(chain.len() - 1);
    let extra = last.get("extraData").filter(|v| v.is_object()).ok_or(VerifyError::Claim { part, claim: "extraData" })?;
    let xuid = str_claim(extra, "XUID");
    if rooted_len && !trusted && !xuid.is_empty() {
        return Err(VerifyError::UntrustedChain);
    }
    if trusted && !jws::is_xuid(xuid) {
        return Err(VerifyError::Claim { part, claim: "XUID" });
    }
    let display_name = str_claim(extra, "displayName");
    if display_name.is_empty() {
        return Err(VerifyError::Claim { part, claim: "displayName" });
    }
    let uuid = Uuid::parse_str(str_claim(extra, "identity"))
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or(VerifyError::Claim { part, claim: "identity" })?;
    let identity = Identity {
        xuid: if trusted { xuid.to_owned() } else { String::new() },
        display_name: display_name.to_owned(),
        uuid,
        title_id: Some(str_claim(extra, "titleId")).filter(|id| trusted && !id.is_empty()).map(str::to_owned),
        playfab_id: None,
    };
    Ok(Subject { identity, key, authenticated: trusted })
}
