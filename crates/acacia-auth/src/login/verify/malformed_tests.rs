//! Garbage in, typed error out: nothing here may panic.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::json;

use super::fixtures::*;
use super::{Part, SigningKeys, VerifyError};

#[test]
fn framing_errors() {
    let v = verifier();
    let framing = |request: &[u8]| match v.verify(request, NOW).unwrap_err() {
        VerifyError::Framing(what) => what,
        other => panic!("expected a framing error, got {other:?}"),
    };
    assert_eq!(framing(b""), "envelope length");
    assert_eq!(framing(&[1, 2, 3]), "envelope length");
    assert_eq!(framing(&(-1i32).to_le_bytes()), "envelope length");
    assert_eq!(framing(&i32::MAX.to_le_bytes()), "envelope length");
    assert_eq!(framing(&[2, 0, 0, 0, b'{', b'}']), "client data length");
    assert_eq!(framing(&[2, 0, 0, 0, b'{', b'}', 0xff, 0xff, 0xff, 0xff]), "client data length");
    assert_eq!(framing(&[2, 0, 0, 0, b'{', b'}', 9, 0, 0, 0, b'x']), "client data length");

    let mut trailing = raw_request(b"{}", b"a.b.c");
    trailing.push(0);
    assert_eq!(framing(&trailing), "trailing bytes");
    assert_eq!(framing(&raw_request(b"{}", &[0xff, 0xfe])), "client data is not UTF-8");
}

#[test]
fn envelope_errors() {
    let v = verifier();
    let verify = |envelope: &str| v.verify(&raw_request(envelope.as_bytes(), b"a.b.c"), NOW).unwrap_err();
    for envelope in ["", "nope", "[]", "42", r#"{"Token":5}"#, r#"{"AuthenticationType":"0"}"#, r#"{"Certificate":"{"}"#, r#"{"chain":"x"}"#] {
        assert!(matches!(verify(envelope), VerifyError::Envelope(_)), "{envelope}");
    }
    assert_eq!(verify(r#"{"AuthenticationType":1,"Token":"a.b.c"}"#), VerifyError::GuestLogin);
    for envelope in ["{}", r#"{"Token":""}"#, r#"{"Certificate":"","Token":""}"#, r#"{"Certificate":"{\"chain\":[\"\"]}"}"#] {
        assert_eq!(verify(envelope), VerifyError::NoCredentials, "{envelope}");
    }
}

#[test]
fn garbage_tokens_and_client_data() {
    let v = verifier();
    let client = ec_key();
    for token in ["x", "a.b.c", "..", "....", "e30.e30.e30", "W10.W10.", "bnVsbA.bnVsbA.AA", "\u{0}.\u{0}.\u{0}"] {
        let envelope = json!({ "Token": token }).to_string();
        let err = v.verify(&raw_request(envelope.as_bytes(), b"a.b.c"), NOW).unwrap_err();
        let expected = matches!(err, VerifyError::Malformed { part: Part::Token, .. } | VerifyError::Algorithm { part: Part::Token, .. });
        assert!(expected, "{token}: {err:?}");
    }

    let envelope = json!({ "Token": rs256(&token_claims(&client)) }).to_string();
    for client_data in ["", "x", "a.b.c", "e30.e30.e30", "W10.W10.AA"] {
        let err = v.verify(&raw_request(envelope.as_bytes(), client_data.as_bytes()), NOW).unwrap_err();
        let expected = matches!(err, VerifyError::Malformed { part: Part::ClientData, .. } | VerifyError::Algorithm { part: Part::ClientData, .. });
        assert!(expected, "{client_data}: {err:?}");
    }
    // ES384 header, but a signature that is not 96 bytes.
    let short = client_jwt(&client, &client_data()).rsplit_once('.').unwrap().0.to_owned() + ".AAAA";
    let err = v.verify(&raw_request(envelope.as_bytes(), short.as_bytes()), NOW).unwrap_err();
    assert_eq!(err, VerifyError::Signature { part: Part::ClientData });
}

#[test]
fn truncated_and_corrupted_requests_never_verify() {
    let v = verifier();
    let client = ec_key();
    let request = token_request(rs256(&token_claims(&client)), &client);
    assert!(v.verify(&request, NOW).is_ok());
    for len in (0..request.len()).step_by(211) {
        assert!(v.verify(&request[..len], NOW).is_err(), "prefix of {len} bytes");
    }
    // Flip one bit at a time across the token and the start of the client-data JWT.
    let envelope_len = i32::from_le_bytes(request[..4].try_into().unwrap()) as usize;
    let token_start = request.windows(9).position(|w| w == br#""Token":""#).unwrap() + 9;
    let client_start = 4 + envelope_len + 4;
    for at in (token_start..4 + envelope_len - 2).chain(client_start..client_start + 400).step_by(7) {
        let mut corrupted = request.clone();
        corrupted[at] ^= 0x01;
        assert!(v.verify(&corrupted, NOW).is_err(), "byte {at} flipped");
    }
    // xorshift noise, with and without a plausible length prefix.
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    for round in 0..500usize {
        let noise: Vec<u8> = (0..round % 97)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect();
        assert!(v.verify(&noise, NOW).is_err());
        assert!(v.verify(&raw_request(&noise, &noise), NOW).is_err());
    }
}

#[test]
fn jwks_parsing() {
    // One key of the live document (2026-10-03), with its hex `x5t`.
    let live = r#"{"keys":[{"kty":"RSA","use":"sig","kid":"472E53B520276E1F3B6BA6F95EBF09D61C2568BE","x5t":"472E53B520276E1F3B6BA6F95EBF09D61C2568BE","n":"-nr2A74AK1-AqrGPw5VCFpznoWDIH9AGinoFRsIdakvsauqblvC30ZkzUi6p1_4009aV_Wn1e4M-V98-Rpj1SEyK1EiQoj5DRMmye3an-3NfHKB8SoXl1TvQSYCYBkLN-TTz27aHYsL3eR3xFRvcmJ0Pr37BFlkf0x3aWp3zuH3-qTY08M5M39IY1mBR0k3DVB7MAeJ6dEcjeVclI3-Fc9-qr6qtG9m8PssWt_Q0jKr2RXPhvAPjd7UOYjjWVySR6sL7IoPUDO5ITElUI9XPpNCrUaP2Ig5ytfAgYqdAMAZsScDdSnJXuQ5gd2p8TkTGB3Y5vD9upnfrtQP8d7PmjQ","e":"AQAB"},{"kty":"EC","crv":"P-256","kid":"ignored"},{"kty":"RSA","use":"enc","kid":"ignored-too"}]}"#;
    let keys = SigningKeys::from_jwks(live).unwrap();
    assert_eq!(keys.len(), 1);
    assert!(keys.contains("472E53B520276E1F3B6BA6F95EBF09D61C2568BE") && !keys.contains("ignored"));
    assert!(SigningKeys::empty().is_empty());

    let jwks_error = |json: &str| match SigningKeys::from_jwks(json) {
        Err(VerifyError::Jwks(reason)) => reason,
        other => panic!("expected a JWKS error, got {other:?}"),
    };
    jwks_error("");
    jwks_error("{}");
    jwks_error(r#"{"keys":[{"kty":"RSA","n":"!!","e":"AQAB"}]}"#);
    jwks_error(r#"{"keys":[{"kty":"RSA","n":"","e":""}]}"#);
    assert_eq!(jwks_error(r#"{"keys":[]}"#), "no RSA signing keys");
    assert_eq!(jwks_error(r#"{"keys":[{"kty":"EC"}]}"#), "no RSA signing keys");
    assert_eq!(jwks_error(r#"{"keys":[{"kty":"RSA","e":"AQAB"}]}"#), "RSA key without n");
    let weak = json!({"keys": [{"kty": "RSA", "n": URL_SAFE_NO_PAD.encode([0xff; 128]), "e": "AQAB"}]}).to_string();
    assert_eq!(jwks_error(&weak), "1024-bit RSA key is too short");
}
