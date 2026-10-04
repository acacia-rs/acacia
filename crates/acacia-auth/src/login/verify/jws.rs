//! JWT checks shared by the token, chain and client-data verifiers, with typed errors.

use serde_json::Value;

use super::{Part, VerifyError};
use crate::jwt::{self, Unverified};

pub(super) fn decode(part: Part, token: &str) -> Result<Unverified<'_>, VerifyError> {
    let decoded = jwt::decode(token).map_err(|e| VerifyError::Malformed { part, reason: e.to_string() })?;
    if !decoded.claims.is_object() {
        return Err(VerifyError::Malformed { part, reason: "claims are not an object".into() });
    }
    Ok(decoded)
}

pub(super) fn wrong_alg(part: Part, token: &Unverified) -> VerifyError {
    VerifyError::Algorithm { part, alg: token.alg().unwrap_or_default().to_owned() }
}

pub(super) fn verify_es384(part: Part, token: &Unverified, key: &p384::PublicKey) -> Result<(), VerifyError> {
    if token.alg() != Some("ES384") {
        return Err(wrong_alg(part, token));
    }
    token.verify(key).map_err(|_| VerifyError::Signature { part })
}

/// `exp`/`nbf` against `now` (unix seconds). An absent `exp` passes unless `exp_required`.
pub(super) fn check_time(part: Part, claims: &Value, now: i64, leeway: i64, exp_required: bool) -> Result<(), VerifyError> {
    match numeric_date(part, claims, "exp")? {
        Some(exp) if now.saturating_sub(leeway) >= exp => return Err(VerifyError::Expired { part }),
        None if exp_required => return Err(VerifyError::Claim { part, claim: "exp" }),
        _ => {}
    }
    match numeric_date(part, claims, "nbf")? {
        Some(nbf) if now.saturating_add(leeway) < nbf => Err(VerifyError::NotYetValid { part }),
        _ => Ok(()),
    }
}

fn numeric_date(part: Part, claims: &Value, claim: &'static str) -> Result<Option<i64>, VerifyError> {
    match claims.get(claim) {
        None | Some(Value::Null) => Ok(None),
        // `as` saturates, so absurd floats cannot wrap into the valid range.
        Some(v) => v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).map(Some).ok_or(VerifyError::Claim { part, claim }),
    }
}

/// A string claim; absent, null and non-string all read as `""`.
pub(super) fn str_claim<'a>(claims: &'a Value, claim: &str) -> &'a str {
    claims.get(claim).and_then(Value::as_str).unwrap_or_default()
}

pub(super) fn key_claim(part: Part, claims: &Value, claim: &'static str) -> Result<p384::PublicKey, VerifyError> {
    jwt::parse_public_key_der_b64(str_claim(claims, claim)).map_err(|_| VerifyError::Claim { part, claim })
}

/// XUIDs are decimal integers (gophertunnel `IdentityData.Validate`).
pub(super) fn is_xuid(xuid: &str) -> bool {
    !xuid.is_empty() && xuid.len() <= 20 && xuid.bytes().all(|b| b.is_ascii_digit())
}
