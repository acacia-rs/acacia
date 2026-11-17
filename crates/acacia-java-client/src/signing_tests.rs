use acacia_auth::JavaProfileKey;
use acacia_java_proto::Wire;
use rsa::pkcs1v15::{Signature, VerifyingKey};
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey};
use rsa::signature::Verifier;
use rsa::{RsaPrivateKey, RsaPublicKey};

use super::*;

const SENDER: u128 = 0x0011_2233_4455_6677_8899_aabb_ccdd_eeff;
const SESSION: u128 = 0xf0e0_d0c0_b0a0_9080_7060_5040_3020_1000;

fn fixture() -> (JavaProfileKey, RsaPublicKey) {
    let key = RsaPrivateKey::from_pkcs8_pem(include_str!("signing_tests_key.pem")).unwrap();
    let public = RsaPublicKey::from(&key);
    let profile = JavaProfileKey {
        private_key: key.to_pkcs8_der().unwrap().as_bytes().to_vec(),
        public_key: public.to_public_key_der().unwrap().into_vec(),
        signature: vec![7; 512],
        expires_at: 1_794_564_000_123,
        refreshed_after: 1_794_500_000_000,
    };
    (profile, public)
}

#[test]
fn signed_bytes_follow_player_chat_message_update_signature() {
    let seen = [MessageSignature([0xab; SIGNATURE_BYTES])];
    let body = Body { content: "hé", timestamp_millis: 1_700_000_000_999, salt: -2, last_seen: &seen };
    let bytes = signed_bytes(SENDER, SESSION, 5, &body);
    let mut expected = vec![0, 0, 0, 1];
    expected.extend(SENDER.to_be_bytes());
    expected.extend(SESSION.to_be_bytes());
    expected.extend([0, 0, 0, 5]);
    expected.extend([0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe]);
    expected.extend(1_700_000_000i64.to_be_bytes());
    expected.extend([0, 0, 0, 3, b'h', 0xc3, 0xa9]);
    expected.extend([0, 0, 0, 1]);
    expected.extend([0xab; SIGNATURE_BYTES]);
    assert_eq!(bytes, expected);
}

#[test]
fn chain_signatures_verify_with_the_public_key_and_advance_the_index() {
    let (profile, public) = fixture();
    let key = ChatKey::new(profile).unwrap();
    let mut session = Session { id: SESSION, sender: SENDER, next: Some(0) };
    let body = Body { content: "hello", timestamp_millis: 1_700_000_000_000, salt: 42, last_seen: &[] };
    let verifier = VerifyingKey::<Sha256>::new(public);
    for index in 0..2 {
        let signature = session.sign(&key, &body).unwrap();
        let signature = Signature::try_from(&signature.0[..]).unwrap();
        verifier.verify(&signed_bytes(SENDER, SESSION, index, &body), &signature).unwrap();
        assert!(verifier.verify(&signed_bytes(SENDER, SESSION, index + 1, &body), &signature).is_err());
    }
    session.next = Some(i32::MAX);
    assert!(session.sign(&key, &body).is_some());
    assert!(session.sign(&key, &body).is_none(), "the chain ends after index i32::MAX");
}

#[test]
fn session_update_is_uuid_expiry_key_and_signature() {
    let (profile, _) = fixture();
    let key = ChatKey::new(profile.clone()).unwrap();
    let session = Session { id: SESSION, sender: SENDER, next: Some(0) };
    let mut out = Vec::new();
    session.update(&key).write(&mut out);
    let mut expected = SESSION.to_be_bytes().to_vec();
    expected.extend(1_794_564_000_123i64.to_be_bytes());
    expected.extend([0xa6, 0x02]);
    expected.extend(&profile.public_key);
    expected.extend([0x80, 0x04]);
    expected.extend([7; 512]);
    assert_eq!(profile.public_key.len(), 294);
    assert_eq!(out, expected);
}

#[test]
fn random_session_ids_are_version_4() {
    let id = random_uuid();
    assert_eq!(id >> 76 & 0xf, 4);
    assert_eq!(id >> 62 & 0b11, 0b10);
}
