use serde::Deserialize;

use super::VerifyError;
use crate::login::request::take_prefixed;

const AUTH_TYPE_GUEST: u8 = 1;

/// A Login connection request split into its credentials and the client-data JWT.
pub(super) struct LoginRequest<'a> {
    /// The multiplayer token, when the envelope carries one.
    pub token: Option<String>,
    /// The legacy certificate chain without empty placeholders (`[""]` is what token logins send).
    pub chain: Vec<String>,
    pub client_jwt: &'a str,
}

/// Both envelope forms: `{"AuthenticationType","Certificate":"{\"chain\":[..]}","Token"}` and the
/// legacy top-level `{"chain":[..]}`.
#[derive(Deserialize)]
struct Envelope {
    #[serde(rename = "AuthenticationType", default)]
    authentication_type: u8,
    #[serde(rename = "Certificate", default)]
    certificate: Option<String>,
    #[serde(rename = "Token", default)]
    token: Option<String>,
    #[serde(default)]
    chain: Vec<String>,
}

#[derive(Deserialize)]
struct Certificate {
    #[serde(default)]
    chain: Vec<String>,
}

pub(super) fn parse(request: &[u8]) -> Result<LoginRequest<'_>, VerifyError> {
    let (envelope, rest) = take_prefixed(request).ok_or(VerifyError::Framing("envelope length"))?;
    let (client_jwt, trailing) = take_prefixed(rest).ok_or(VerifyError::Framing("client data length"))?;
    if !trailing.is_empty() {
        return Err(VerifyError::Framing("trailing bytes"));
    }
    let client_jwt = std::str::from_utf8(client_jwt).map_err(|_| VerifyError::Framing("client data is not UTF-8"))?;

    // serde would also read a struct from a JSON array, positionally.
    let envelope: serde_json::Value = serde_json::from_slice(envelope).map_err(|e| VerifyError::Envelope(e.to_string()))?;
    if !envelope.is_object() {
        return Err(VerifyError::Envelope("not a JSON object".into()));
    }
    let envelope = Envelope::deserialize(envelope).map_err(|e| VerifyError::Envelope(e.to_string()))?;
    if envelope.authentication_type == AUTH_TYPE_GUEST {
        return Err(VerifyError::GuestLogin);
    }
    let mut chain = match envelope.certificate.as_deref() {
        Some(cert) if !cert.is_empty() => {
            let cert = serde_json::from_str::<serde_json::Value>(cert).ok().filter(serde_json::Value::is_object);
            let cert = cert.ok_or_else(|| VerifyError::Envelope("Certificate is not a JSON object".into()))?;
            Certificate::deserialize(cert).map_err(|e| VerifyError::Envelope(format!("Certificate: {e}")))?.chain
        }
        _ => envelope.chain,
    };
    chain.retain(|jwt| !jwt.is_empty());
    Ok(LoginRequest { token: envelope.token.filter(|t| !t.is_empty()), chain, client_jwt })
}
