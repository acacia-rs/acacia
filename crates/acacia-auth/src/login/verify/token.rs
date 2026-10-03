//! The multiplayer token: RS256 from the authorization service, or ES384 self-signed by the
//! client's own key (offline logins).

use serde_json::Value;
use uuid::Uuid;

use super::jws::{self, check_time, key_claim, str_claim};
use super::{Identity, Part, Subject, Verifier, VerifyError};
use crate::login::request::{name_uuid, xuid_identity};

const PART: Part = Part::Token;

pub(super) fn verify(verifier: &Verifier, token: &str, now: i64) -> Result<Subject, VerifyError> {
    let jwt = jws::decode(PART, token)?;
    let claims = &jwt.claims;
    let issued = match jwt.alg() {
        Some("RS256") => {
            verifier.keys.verify(&jwt)?;
            let issuer = claims.get("iss").and_then(Value::as_str);
            if issuer != Some(verifier.issuer.as_str()) {
                return Err(VerifyError::Issuer { part: PART, found: issuer.map(str::to_owned) });
            }
            true
        }
        Some("ES384") => false,
        _ => return Err(jws::wrong_alg(PART, &jwt)),
    };
    let key = key_claim(PART, claims, "cpk")?;
    if !issued {
        jws::verify_es384(PART, &jwt, &key)?;
    }
    if !has_audience(claims, &verifier.audience) {
        return Err(VerifyError::Audience);
    }
    check_time(PART, claims, now, verifier.leeway, true)?;

    let display_name = str_claim(claims, "xname");
    if display_name.is_empty() {
        return Err(VerifyError::Claim { part: PART, claim: "xname" });
    }
    let xuid = str_claim(claims, "xid");
    let authenticated = issued && !xuid.is_empty();
    if authenticated && !jws::is_xuid(xuid) {
        return Err(VerifyError::Claim { part: PART, claim: "xid" });
    }
    let (xuid, uuid) = if authenticated {
        (xuid.to_owned(), xuid_identity(xuid))
    } else {
        let claimed = Uuid::parse_str(str_claim(claims, "leguuid")).ok().filter(|id| !id.is_nil());
        (String::new(), claimed.unwrap_or_else(|| name_uuid(&format!("OfflinePlayer:{display_name}"))))
    };
    let playfab_id = Some(str_claim(claims, "mid")).filter(|id| issued && !id.is_empty()).map(str::to_owned);
    let identity = Identity { xuid, display_name: display_name.to_owned(), uuid, title_id: None, playfab_id };
    Ok(Subject { identity, key, authenticated })
}

/// `aud` is a string or an array of strings.
fn has_audience(claims: &Value, audience: &str) -> bool {
    match claims.get("aud") {
        Some(Value::String(aud)) => aud == audience,
        Some(Value::Array(auds)) => auds.iter().any(|aud| aud.as_str() == Some(audience)),
        _ => false,
    }
}
