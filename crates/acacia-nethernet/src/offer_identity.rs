//! The `a=identity` of a client's offer, as the answering side reads it.

use p384::ecdsa::VerifyingKey;

use crate::assertion::{self, parse_cpk, split_jwt, verify_es384, verify_fingerprints, Reason};
use crate::Error;

/// What an offer asserted. The key is proven to own the offer's DTLS certificate; whether the token
/// is to be believed is the host's policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferIdentity {
    /// The token's `cpk`. The Login that follows must be signed by the same key.
    pub key: VerifyingKey,
    pub token: String,
    /// [`crate::AUTH_DOMAIN`] for a MultiplayerToken.
    pub domain: String,
    /// True if `key` signed the token itself: it vouches for nothing but the key. A MultiplayerToken
    /// is RS256 by the auth service and is not checked here (the Login carries it again).
    pub self_signed: bool,
}

/// Reads an offer's assertion and checks its fingerprint signature. `None` if the offer has none.
pub fn verify_offer(offer: &str) -> Result<Option<OfferIdentity>, Error> {
    check(offer).map_err(|reason| Error::ClientIdentity(reason.to_owned()))
}

fn check(offer: &str) -> Result<Option<OfferIdentity>, Reason> {
    let Some(assertion) = assertion::read(offer)? else { return Ok(None) };
    let jwt = split_jwt(&assertion.token)?;
    let key = parse_cpk(&jwt.claims["cpk"])?;
    verify_fingerprints(offer, &key, &assertion.fingerprints)?;
    let self_signed = jwt.alg == "ES384" && verify_es384(&key, jwt.signing_input.as_bytes(), &jwt.signature).is_ok();
    Ok(Some(OfferIdentity { key, token: assertion.token, domain: assertion.domain, self_signed }))
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use p384::ecdsa::SigningKey;
    use serde_json::json;

    use super::*;
    use crate::identity::{identity_line, insert_session_line};
    use crate::Identity;

    const SDP: &str = "v=0\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\na=fingerprint:sha-256 AB:CD\r\n";

    fn offer(identity: &Identity) -> String {
        insert_session_line(SDP, &identity_line(SDP, identity))
    }

    /// A token shaped like the auth service's: RS256, `cpk` naming the client key.
    fn service_token(key: &SigningKey) -> String {
        let point = key.verifying_key().to_encoded_point(false);
        let coord = |c: Option<&_>| URL_SAFE_NO_PAD.encode(c.unwrap());
        let claims = json!({ "cpk": { "kty": "EC", "crv": "P-384", "x": coord(point.x()), "y": coord(point.y()) } });
        let part = |v: serde_json::Value| URL_SAFE_NO_PAD.encode(v.to_string());
        format!("{}.{}.{}", part(json!({ "alg": "RS256" })), part(claims), URL_SAFE_NO_PAD.encode([1u8; 8]))
    }

    #[test]
    fn reads_a_service_token_and_a_self_signed_one() {
        let key = SigningKey::from_slice(&[5; 48]).unwrap();
        let online = verify_offer(&offer(&Identity::multiplayer(key.clone(), service_token(&key)))).unwrap().unwrap();
        assert_eq!((online.key, online.self_signed, online.domain.as_str()), (*key.verifying_key(), false, crate::AUTH_DOMAIN));
        let offline = verify_offer(&offer(&Identity::self_signed(key.clone()))).unwrap().unwrap();
        assert_eq!((offline.key, offline.self_signed), (*key.verifying_key(), true));
    }

    #[test]
    fn refuses_a_borrowed_token_or_changed_fingerprint() {
        let (key, thief) = (SigningKey::from_slice(&[5; 48]).unwrap(), SigningKey::from_slice(&[6; 48]).unwrap());
        // Someone else's token, fingerprints signed with a key the token does not name.
        assert!(verify_offer(&offer(&Identity::multiplayer(thief, service_token(&key)))).is_err());
        let sdp = offer(&Identity::multiplayer(key.clone(), service_token(&key)));
        assert!(verify_offer(&sdp.replace("AB:CD", "AB:CE")).is_err());
        assert_eq!(verify_offer(SDP).unwrap(), None);
    }
}
