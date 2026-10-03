//! The SDP `a=identity` assertion: a token naming the client key (`cpk`) plus a detached ES384 JWS
//! over the SDP's DTLS fingerprints. See docs/research/nethernet-wire.md §2.

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use p384::ecdsa::signature::Signer;
use p384::ecdsa::{Signature, SigningKey};

/// `idp.domain` for a MultiplayerToken issued by the Minecraft auth service.
pub const AUTH_DOMAIN: &str = "https://authorization.franchise.minecraft-services.net/";

pub struct Identity {
    /// The login key; the token's `cpk` must name it.
    pub key: SigningKey,
    /// MultiplayerToken (`LoginCredentials::multiplayer_token`). BDS rejects self-signed tokens
    /// with error 37 even when `online-mode=false`.
    pub token: String,
    pub domain: String,
}

impl Identity {
    pub fn multiplayer(key: SigningKey, token: String) -> Self {
        Self { key, token, domain: AUTH_DOMAIN.to_owned() }
    }
}

pub(crate) const FINGERPRINT: &str = "a=fingerprint:";
pub(crate) const IDENTITY: &str = "a=identity:";

/// The `a=identity` line asserting `identity` over the SDP's `a=fingerprint` lines.
pub(crate) fn identity_line(sdp: &str, identity: &Identity) -> String {
    format!("{IDENTITY}{}", envelope(sdp, identity))
}

/// The canonical JSON both sides sign over the SDP's `a=fingerprint` lines, in order.
pub(crate) fn fingerprint_payload<'a>(sdp_lines: impl IntoIterator<Item = &'a str>) -> String {
    let entries: Vec<serde_json::Value> = sdp_lines
        .into_iter()
        .filter_map(|l| l.strip_prefix(FINGERPRINT)?.split_once(' '))
        .map(|(alg, digest)| serde_json::json!({"algorithm": alg, "digest": digest.trim().to_uppercase()}))
        .collect();
    // Keys are inserted in sorted order, as canonical JSON requires, whatever serde_json's map does.
    serde_json::json!({ "fingerprint": entries }).to_string()
}

fn envelope(sdp: &str, identity: &Identity) -> String {
    let payload = fingerprint_payload(sdp.lines());
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"ES384"}"#);
    let sig: Signature = identity.key.sign(format!("{header}.{}", URL_SAFE_NO_PAD.encode(&payload)).as_bytes());
    let detached = format!("{header}..{}", URL_SAFE_NO_PAD.encode(sig.to_bytes()));
    let assertion = serde_json::json!({ "token": identity.token, "fingerprints": detached }).to_string();
    let envelope = serde_json::json!({
        "idp": { "domain": identity.domain, "protocol": "default" },
        "assertion": assertion,
    });
    STANDARD.encode(envelope.to_string())
}

/// Inserts `line` at session level, just before the first `m=` (where BDS puts its assertion).
pub(crate) fn insert_session_line(sdp: &str, line: &str) -> String {
    let at = sdp.find("m=").unwrap_or(sdp.len());
    format!("{}{line}\r\n{}", &sdp[..at], &sdp[at..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use p384::ecdsa::signature::Verifier;

    #[test]
    fn signs_the_fingerprints_with_the_login_key() {
        let key = SigningKey::from_slice(&[7; 48]).unwrap();
        let sdp = "v=0\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\na=fingerprint:sha-256 ab:cd\r\n";
        let line = identity_line(sdp, &Identity::multiplayer(key.clone(), "tok".into()));
        let env: serde_json::Value = serde_json::from_slice(&STANDARD.decode(&line[IDENTITY.len()..]).unwrap()).unwrap();
        let assertion: serde_json::Value = serde_json::from_str(env["assertion"].as_str().unwrap()).unwrap();
        assert_eq!(assertion["token"], "tok");
        let (header, sig) = assertion["fingerprints"].as_str().unwrap().split_once("..").unwrap();
        let payload = URL_SAFE_NO_PAD.encode(r#"{"fingerprint":[{"algorithm":"sha-256","digest":"AB:CD"}]}"#);
        let sig = Signature::from_slice(&URL_SAFE_NO_PAD.decode(sig).unwrap()).unwrap();
        key.verifying_key().verify(format!("{header}.{payload}").as_bytes(), &sig).unwrap();
    }
}
