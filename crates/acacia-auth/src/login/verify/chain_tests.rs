use p384::ecdsa::SigningKey;
use serde_json::{Value, json};

use super::fixtures::*;
use super::{Part, Verifier, VerifyError};
use crate::jwt::{self, public_key_der_b64};
use crate::login::build_connection_request;

/// A verifier that trusts `root` where production trusts Mojang.
fn test_verifier(root: &SigningKey) -> Verifier {
    verifier().with_chain_root(public(root))
}

/// A legacy chain login built by acacia's own client code.
fn chain_request(chain: Vec<String>, client: &SigningKey) -> Vec<u8> {
    build_connection_request(&credentials(chain, None), client, &client_data())
}

/// A chain login from explicit JWTs, in the legacy top-level `{"chain":[..]}` envelope.
fn legacy_request(chain: &[String], client: &SigningKey) -> Vec<u8> {
    raw_request(json!({ "chain": chain }).to_string().as_bytes(), client_jwt(client, &client_data()).as_bytes())
}

fn head(client: &SigningKey, next: &SigningKey) -> String {
    client_jwt(client, &json!({"identityPublicKey": public_key_der_b64(next), "certificateAuthority": true}))
}

fn self_signed(client: &SigningKey, extra: Value) -> String {
    client_jwt(client, &json!({"exp": jwt::now_unix() + 3600, "identityPublicKey": public_key_der_b64(client), "extraData": extra}))
}

#[test]
fn client_built_chain_login_verifies() {
    let (root, client) = (ec_key(), ec_key());
    let request = chain_request(mojang_chain(&root, &client, XUID), &client);
    let login = test_verifier(&root).verify(&request, jwt::now_unix()).unwrap();
    assert!(login.authenticated);
    assert_eq!(login.identity.xuid, XUID);
    assert_eq!(login.identity.display_name, "Steve");
    assert_eq!(login.identity.uuid.to_string(), "8a5f1c9e-3b7d-3f21-9c4e-6d2b8f0a1e57");
    assert_eq!(login.identity.title_id.as_deref(), Some("896928775"));
    assert_eq!(login.client_key, public(&client));
    assert_eq!(login.client_data, client_data());
}

#[test]
fn chain_not_rooted_at_mojang_is_refused() {
    let (forger, client) = (ec_key(), ec_key());
    // Structurally perfect, but the production verifier only trusts Mojang's key.
    let request = chain_request(mojang_chain(&forger, &client, XUID), &client);
    assert_eq!(verifier().verify(&request, jwt::now_unix()).unwrap_err(), VerifyError::UntrustedChain);

    // Without an XUID the same chain is merely an offline login.
    let request = chain_request(mojang_chain(&forger, &client, ""), &client);
    let login = verifier().verify(&request, jwt::now_unix()).unwrap();
    assert!(!login.authenticated);
    assert_eq!((login.identity.xuid.as_str(), login.identity.title_id.as_deref()), ("", None));
}

#[test]
fn forged_links_break_the_chain() {
    let (root, client, forger) = (ec_key(), ec_key(), ec_key());
    let genuine = mojang_chain(&root, &client, XUID);
    let now = jwt::now_unix();

    // The identity JWT re-signed by a key the chain does not name.
    let identity = json!({"iss": "Mojang", "identityPublicKey": public_key_der_b64(&client), "extraData": extra_data("1")});
    let chain = vec![head(&client, &root), genuine[0].clone(), client_jwt(&forger, &identity)];
    assert_eq!(test_verifier(&root).verify(&legacy_request(&chain, &client), now).unwrap_err(), VerifyError::Signature { part: Part::Chain(2) });

    // The head must be signed by the key in its own x5u.
    let mut parts: Vec<String> = head(&forger, &root).split('.').map(str::to_owned).collect();
    parts[0] = head(&client, &root).split('.').next().unwrap().to_owned();
    let chain = vec![parts.join("."), genuine[0].clone(), genuine[1].clone()];
    assert_eq!(test_verifier(&root).verify(&legacy_request(&chain, &client), now).unwrap_err(), VerifyError::Signature { part: Part::Chain(0) });

    // Rooted correctly, but the links were not issued by Mojang.
    let no_issuer = client_jwt(&root, &json!({"identityPublicKey": public_key_der_b64(&client), "extraData": extra_data(XUID)}));
    let chain = vec![head(&client, &root), no_issuer.clone(), no_issuer];
    assert_eq!(test_verifier(&root).verify(&legacy_request(&chain, &client), now).unwrap_err(), VerifyError::Issuer { part: Part::Chain(1), found: None });
}

#[test]
fn replayed_chain_cannot_be_used_with_another_key() {
    let (root, victim, attacker) = (ec_key(), ec_key(), ec_key());
    // The victim's chain is visible to every server it joined; the attacker adds its own head.
    let request = chain_request(mojang_chain(&root, &victim, XUID), &attacker);
    let err = test_verifier(&root).verify(&request, jwt::now_unix()).unwrap_err();
    assert_eq!(err, VerifyError::Signature { part: Part::ClientData });
}

#[test]
fn expired_chain_is_refused() {
    let (root, client) = (ec_key(), ec_key());
    let request = chain_request(mojang_chain(&root, &client, XUID), &client);
    let err = test_verifier(&root).verify(&request, jwt::now_unix() + 2 * 3600).unwrap_err();
    assert_eq!(err, VerifyError::Expired { part: Part::Chain(1) });
}

#[test]
fn self_signed_chain_is_offline() {
    let client = ec_key();
    // Claims an XUID; a single self-signed JWT proves nothing about it.
    let request = legacy_request(&[self_signed(&client, extra_data(XUID))], &client);
    let login = verifier().verify(&request, jwt::now_unix()).unwrap();
    assert!(!login.authenticated);
    assert_eq!(login.identity.xuid, "");
    assert_eq!(login.identity.display_name, "Steve");
    assert_eq!(login.identity.title_id, None);
    assert_eq!(login.client_key, public(&client));

    let other = ec_key();
    let request = legacy_request(&[self_signed(&client, extra_data(""))], &other);
    assert_eq!(verifier().verify(&request, jwt::now_unix()).unwrap_err(), VerifyError::Signature { part: Part::ClientData });
}

#[test]
fn chain_shape_is_checked() {
    let (root, client) = (ec_key(), ec_key());
    let now = jwt::now_unix();
    let verify = |chain: &[String]| verifier().verify(&legacy_request(chain, &client), now).unwrap_err();

    assert_eq!(verify(&mojang_chain(&root, &client, XUID)), VerifyError::ChainLength(2));
    assert_eq!(verify(&[]), VerifyError::NoCredentials);
    assert!(matches!(verify(&["garbage".into()]), VerifyError::Malformed { part: Part::Chain(0), .. }));

    let missing = |claim: &'static str| VerifyError::Claim { part: Part::Chain(0), claim };
    assert_eq!(verify(&[client_jwt(&client, &json!({}))]), missing("extraData"));
    assert_eq!(verify(&[self_signed(&client, json!({"displayName": "Steve", "identity": "nope"}))]), missing("identity"));
    assert_eq!(verify(&[self_signed(&client, json!({"identity": "8a5f1c9e-3b7d-3f21-9c4e-6d2b8f0a1e57"}))]), missing("displayName"));
}

#[test]
fn token_login_takes_the_title_id_from_a_matching_chain() {
    let (root, client) = (ec_key(), ec_key());
    let token = rs256(&with_wall_clock(token_claims(&client)));
    let envelope = |chain: Vec<String>| {
        let mut chain = chain;
        chain.insert(0, head(&client, &root));
        let certificate = json!({ "chain": chain }).to_string();
        json!({"AuthenticationType": 0, "Certificate": certificate, "Token": token}).to_string()
    };
    let verify = |chain: Vec<String>| {
        let request = raw_request(envelope(chain).as_bytes(), client_jwt(&client, &client_data()).as_bytes());
        test_verifier(&root).verify(&request, jwt::now_unix()).unwrap()
    };
    assert_eq!(verify(mojang_chain(&root, &client, XUID)).identity.title_id.as_deref(), Some("896928775"));
    // Another account's chain, or a broken one, is ignored: the token alone decides.
    let other = verify(mojang_chain(&root, &client, "99"));
    assert!(other.authenticated && other.identity.xuid == XUID && other.identity.title_id.is_none());
    assert!(verify(vec!["junk".into(), "junk".into()]).identity.title_id.is_none());
}

fn with_wall_clock(mut claims: Value) -> Value {
    claims["exp"] = json!(jwt::now_unix() + 3600);
    claims["nbf"] = json!(jwt::now_unix() - 60);
    claims
}
