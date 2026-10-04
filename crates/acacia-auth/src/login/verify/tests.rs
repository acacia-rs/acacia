use serde_json::{Value, json};

use super::fixtures::*;
use super::{Part, SigningKeys, Verifier, VerifyError};
use crate::jwt::{self, public_key_der_b64};
use crate::login::{
    OfflineIdentity, build_offline_connection_request, build_offline_connection_request_for, offline_identity, xuid_identity,
};

fn with(mut claims: Value, key: &str, value: Value) -> Value {
    claims[key] = value;
    claims
}

fn without(mut claims: Value, key: &str) -> Value {
    claims.as_object_mut().unwrap().remove(key);
    claims
}

/// Verifies a token login whose token carries `claims`.
fn verify_claims(claims: &Value, client: &p384::ecdsa::SigningKey) -> Result<super::VerifiedLogin, VerifyError> {
    verifier().verify(&token_request(rs256(claims), client), NOW)
}

#[test]
fn client_built_token_login_verifies() {
    let client = ec_key();
    let login = verify_claims(&token_claims(&client), &client).unwrap();
    assert!(login.authenticated);
    assert_eq!(login.identity.xuid, XUID);
    assert_eq!(login.identity.display_name, "Steve");
    assert_eq!(login.identity.uuid, xuid_identity(XUID));
    assert_eq!(login.identity.playfab_id.as_deref(), Some("A1B2C3D4E5F60718"));
    assert_eq!(login.identity.title_id, None);
    assert_eq!(login.client_key, public(&client));
    assert_eq!(login.client_data, client_data());
    assert_eq!(login.client_claims["ThirdPartyName"], "Steve");
}

#[test]
fn xuid_identity_matches_the_reference_derivation() {
    // Computed independently: md5("pocket-auth-1-xuid:" + xuid) with the version/variant bits set.
    assert_eq!(xuid_identity(XUID).to_string(), "f5bd986c-ca46-3e54-93de-227d4b24503c");
}

#[test]
fn issuer_and_audience_must_match() {
    let client = ec_key();
    let claims = token_claims(&client);
    let no_slash = "https://authorization.franchise.minecraft-services.net";
    assert_eq!(
        verify_claims(&with(claims.clone(), "iss", json!(no_slash)), &client).unwrap_err(),
        VerifyError::Issuer { part: Part::Token, found: Some(no_slash.into()) }
    );
    assert_eq!(verify_claims(&without(claims.clone(), "iss"), &client).unwrap_err(), VerifyError::Issuer { part: Part::Token, found: None });
    for aud in [json!("api://auth-minecraft-services/realms"), json!(["a", "b"]), json!(7)] {
        assert_eq!(verify_claims(&with(claims.clone(), "aud", aud), &client).unwrap_err(), VerifyError::Audience);
    }
    assert_eq!(verify_claims(&without(claims.clone(), "aud"), &client).unwrap_err(), VerifyError::Audience);
    let listed = json!(["other", crate::login::MULTIPLAYER_AUDIENCE]);
    assert!(verify_claims(&with(claims, "aud", listed), &client).unwrap().authenticated);
}

#[test]
fn validity_window_is_enforced_with_leeway() {
    let client = ec_key();
    let claims = token_claims(&client);
    let expired = |exp: i64| verify_claims(&with(claims.clone(), "exp", json!(exp)), &client);
    assert_eq!(expired(NOW - 61).unwrap_err(), VerifyError::Expired { part: Part::Token });
    assert!(expired(NOW - 30).is_ok(), "inside the 60 s leeway");
    let strict = verifier().with_leeway(0).verify(&token_request(rs256(&with(claims.clone(), "exp", json!(NOW - 30))), &client), NOW);
    assert_eq!(strict.unwrap_err(), VerifyError::Expired { part: Part::Token });

    let early = with(claims.clone(), "nbf", json!(NOW + 600));
    assert_eq!(verify_claims(&early, &client).unwrap_err(), VerifyError::NotYetValid { part: Part::Token });
    assert_eq!(verify_claims(&without(claims.clone(), "exp"), &client).unwrap_err(), VerifyError::Claim { part: Part::Token, claim: "exp" });
    assert_eq!(verify_claims(&with(claims, "exp", json!("soon")), &client).unwrap_err(), VerifyError::Claim { part: Part::Token, claim: "exp" });
}

#[test]
fn token_signature_must_come_from_a_known_key() {
    let client = ec_key();
    let genuine = rs256(&token_claims(&client));
    // The genuine signature over other claims: someone else's XUID.
    let other = rs256(&with(token_claims(&client), "xid", json!("1")));
    let parts: Vec<&str> = genuine.split('.').collect();
    let forged = format!("{}.{}.{}", parts[0], other.split('.').nth(1).unwrap(), parts[2]);
    assert_eq!(verifier().verify(&token_request(forged, &client), NOW).unwrap_err(), VerifyError::Signature { part: Part::Token });

    let short_signature = format!("{}.{}.AAAA", parts[0], parts[1]);
    assert_eq!(verifier().verify(&token_request(short_signature, &client), NOW).unwrap_err(), VerifyError::Signature { part: Part::Token });

    let rotated = issuer_jwt(&json!({"alg": "RS256", "kid": "NEWKID"}), &token_claims(&client));
    let unknown = VerifyError::UnknownSigningKey { kid: Some("NEWKID".into()) };
    assert_eq!(verifier().verify(&token_request(rotated, &client), NOW).unwrap_err(), unknown);
    assert!(!verifier().keys().contains("NEWKID") && verifier().keys().contains(KID));

    let keyless = Verifier::new(SigningKeys::empty());
    assert_eq!(keyless.verify(&token_request(genuine.clone(), &client), NOW).unwrap_err(), VerifyError::UnknownSigningKey { kid: Some(KID.into()) });

    let no_kid = issuer_jwt(&json!({"alg": "RS256"}), &token_claims(&client));
    assert!(verifier().verify(&token_request(no_kid, &client), NOW).unwrap().authenticated, "no kid: every key is tried");
}

#[test]
fn only_rs256_and_es384_tokens_are_accepted() {
    let client = ec_key();
    for alg in ["none", "HS256", "RS384", ""] {
        let token = issuer_jwt(&json!({"alg": alg, "kid": KID}), &token_claims(&client));
        let err = verifier().verify(&token_request(token, &client), NOW).unwrap_err();
        assert_eq!(err, VerifyError::Algorithm { part: Part::Token, alg: alg.into() });
    }
}

#[test]
fn client_data_must_be_signed_by_the_token_key() {
    let (client, thief) = (ec_key(), ec_key());
    // A stolen token: valid, but bound to the victim's key.
    let err = verifier().verify(&token_request(rs256(&token_claims(&client)), &thief), NOW).unwrap_err();
    assert_eq!(err, VerifyError::Signature { part: Part::ClientData });
}

#[test]
fn offline_login_verifies_but_is_never_authenticated() {
    let client = ec_key();
    let now = jwt::now_unix();
    let login = verifier().verify(&build_offline_connection_request("Bot_1", &client, &client_data()), now).unwrap();
    assert!(!login.authenticated);
    assert_eq!(login.identity.xuid, "");
    assert_eq!(login.identity.display_name, "Bot_1");
    assert_eq!(login.identity.uuid.to_string(), offline_identity("Bot_1"));
    assert_eq!(login.client_key, public(&client));

    // A self-signed token may claim any XUID; nobody vouches for it.
    let claimed = OfflineIdentity { name: "Steve", xuid: XUID, uuid: "not-a-uuid" };
    let login = verifier().verify(&build_offline_connection_request_for(&claimed, &client, &client_data()), now).unwrap();
    assert!(!login.authenticated);
    assert_eq!(login.identity.xuid, "");
    assert_eq!(login.identity.uuid.to_string(), offline_identity("Steve"));

    let expired = verifier().verify(&build_offline_connection_request("Bot_1", &client, &client_data()), now + 7 * 3600);
    assert_eq!(expired.unwrap_err(), VerifyError::Expired { part: Part::Token });
}

#[test]
fn self_signed_token_must_be_signed_by_its_own_cpk() {
    let (attacker, victim) = (ec_key(), ec_key());
    let claims = json!({"aud": crate::login::MULTIPLAYER_AUDIENCE, "exp": NOW + 60, "cpk": public_key_der_b64(&victim), "xname": "Steve", "xid": ""});
    let envelope = json!({"AuthenticationType": 2, "Token": client_jwt(&attacker, &claims)}).to_string();
    let request = raw_request(envelope.as_bytes(), client_jwt(&attacker, &client_data()).as_bytes());
    assert_eq!(verifier().verify(&request, NOW).unwrap_err(), VerifyError::Signature { part: Part::Token });
}

#[test]
fn issued_token_without_xuid_is_unauthenticated() {
    let client = ec_key();
    let login = verify_claims(&with(token_claims(&client), "xid", json!("")), &client).unwrap();
    assert!(!login.authenticated);
    assert_eq!(login.identity.uuid.to_string(), offline_identity("Steve"));

    let err = verify_claims(&with(token_claims(&client), "xid", json!("12ab")), &client).unwrap_err();
    assert_eq!(err, VerifyError::Claim { part: Part::Token, claim: "xid" });
    let err = verify_claims(&with(token_claims(&client), "xname", json!("")), &client).unwrap_err();
    assert_eq!(err, VerifyError::Claim { part: Part::Token, claim: "xname" });
    let err = verify_claims(&with(token_claims(&client), "cpk", json!("AAAA")), &client).unwrap_err();
    assert_eq!(err, VerifyError::Claim { part: Part::Token, claim: "cpk" });
}

#[test]
fn client_data_from_other_platforms_parses_leniently() {
    let client = ec_key();
    let envelope = json!({"AuthenticationType": 0, "Token": rs256(&token_claims(&client))}).to_string();
    let verify = |claims: Value| verifier().verify(&raw_request(envelope.as_bytes(), client_jwt(&client, &claims).as_bytes()), NOW);

    let sparse = json!({"DeviceOS": 7, "GameVersion": "1.26.52", "PlayFabId": "abc", "IsEditorMode": false, "SkinId": "s"});
    let login = verify(sparse.clone()).unwrap();
    assert_eq!((login.client_data.device_os, login.client_data.skin.skin_id.as_str()), (7, "s"));
    assert_eq!(login.client_data.language_code, "");
    assert_eq!(login.client_claims, sparse, "unmodelled claims stay available");
    assert!(verify(json!({})).is_ok());

    let wrong_type = verify(json!({"DeviceOS": "seven"})).unwrap_err();
    assert!(matches!(wrong_type, VerifyError::Malformed { part: Part::ClientData, .. }), "{wrong_type:?}");
}
