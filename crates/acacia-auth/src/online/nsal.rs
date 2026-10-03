//! NSAL relying-party resolution (go-xsapi `nsal.Resolver`): gophertunnel picks the XSTS relying
//! party for `multiplayer.minecraft.net` and `20ca2.playfabapi.com` from the title's endpoint
//! table (`titles/current`, then `titles/default`) rather than hardcoding it.

use p256::ecdsa::SigningKey;
use serde::Deserialize;

use super::config::endpoints as ep;
use super::http::Http;
use super::sign;
use super::xbox::XboxToken;
use crate::Result;

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct TitleData {
    #[serde(rename = "EndPoints", default)]
    endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Endpoint {
    protocol: String,
    host: String,
    host_type: String,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    relying_party: Option<String>,
}

impl Endpoint {
    fn matches(&self, scheme: &str, host: &str, path: &str) -> bool {
        let host_ok = match self.host_type.as_str() {
            "fqdn" => self.host == host,
            "wildcard" => wildcard_match(&self.host, host),
            _ => false,
        };
        self.relying_party.as_deref().is_some_and(|rp| !rp.is_empty())
            && self.protocol == scheme
            && host_ok
            && self.port.is_none_or(|p| p == 443 && scheme == "https")
            && self.path.as_deref().is_none_or(|p| p.is_empty() || p == path)
    }
}

/// `*` matches any run of characters (go-xsapi turns the pattern into `.*`).
fn wildcard_match(pattern: &str, host: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or("");
    let Some(mut rest) = host.strip_prefix(first) else { return false };
    let parts: Vec<&str> = parts.collect();
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            return rest.ends_with(part);
        }
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.is_empty()
}

impl TitleData {
    /// Relying party for an https URL; an fqdn match wins over wildcards, else the last wildcard.
    pub fn relying_party(&self, host: &str, path: &str) -> Option<String> {
        let mut found = None;
        for e in &self.endpoints {
            if e.matches("https", host, path) {
                found = e.relying_party.clone();
                if e.host_type == "fqdn" {
                    break;
                }
            }
        }
        found
    }
}

pub(crate) async fn fetch_default(http: &Http) -> Result<TitleData> {
    let url = format!("{}/default/endpoints?type=1", ep::NSAL_TITLES);
    let req = http.client.get(&url).header("x-xbl-contract-version", "1");
    http.json(req, &url).await
}

/// `GET titles/current/endpoints`, authorized with the `http://xboxlive.com` XSTS token.
pub(crate) async fn fetch_current(
    http: &Http,
    key: &SigningKey,
    xboxlive: &XboxToken,
) -> Result<TitleData> {
    let url = format!("{}/current/endpoints", ep::NSAL_TITLES);
    let auth = xboxlive.authorization();
    let ft = sign::filetime(http.server_now_nanos());
    let signature = sign::signature_header(key, ft, "GET", "/titles/current/endpoints", &auth, b"");
    let req = http
        .client
        .get(&url)
        .header("x-xbl-contract-version", "1")
        .header("Authorization", auth)
        .header("Signature", signature);
    http.json(req, &url).await
}

/// Resolves `(multiplayer_rp, playfab_rp)`: current title first, default title next, then the
/// hardcoded fallbacks.
pub(crate) fn resolve(titles: &[TitleData], playfab_host: &str) -> (String, String) {
    let lookup = |host: &str, path: &str| titles.iter().find_map(|t| t.relying_party(host, path));
    (
        lookup("multiplayer.minecraft.net", "/authentication")
            .unwrap_or_else(|| ep::RP_MULTIPLAYER_FALLBACK.into()),
        lookup(playfab_host, "/Client/LoginWithXbox")
            .unwrap_or_else(|| ep::RP_PLAYFAB_FALLBACK.into()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_rules() {
        let t: TitleData = serde_json::from_str(
            r#"{"EndPoints":[
            {"Protocol":"https","Host":"*.xboxlive.com","HostType":"wildcard","RelyingParty":"http://xboxlive.com"},
            {"Protocol":"https","Host":"*.playfabapi.com","HostType":"wildcard","RelyingParty":"rp://wild/"},
            {"Protocol":"https","Host":"20ca2.playfabapi.com","HostType":"fqdn","RelyingParty":"rp://exact/"},
            {"Protocol":"https","Host":"multiplayer.minecraft.net","HostType":"fqdn","RelyingParty":"https://multiplayer.minecraft.net/"},
            {"Protocol":"http","Host":"*.xboxlive.com","HostType":"wildcard"}]}"#,
        )
        .unwrap();
        assert_eq!(t.relying_party("sisu.xboxlive.com", "/").as_deref(), Some("http://xboxlive.com"));
        assert_eq!(t.relying_party("20ca2.playfabapi.com", "/x").as_deref(), Some("rp://exact/"));
        assert_eq!(t.relying_party("abc.playfabapi.com", "/x").as_deref(), Some("rp://wild/"));
        assert_eq!(t.relying_party("example.com", "/"), None);
        let (mp, pf) = resolve(&[TitleData::default(), t], "20ca2.playfabapi.com");
        assert_eq!(mp, "https://multiplayer.minecraft.net/");
        assert_eq!(pf, "rp://exact/");
        assert!(!wildcard_match("*.xboxlive.com", "xboxlive.com"));
    }
}
