//! Real-endpoint smoke tests that need no user account. Ignored by default:
//! `cargo test -p acacia-auth -- --ignored live_`

use super::config::{AuthConfig, GAME_VERSION, Title};
use super::http::Http;
use super::{minecraft, nsal, xbox};

#[tokio::test]
#[ignore]
async fn live_device_token_signature_accepted() {
    let http = Http::new(&AuthConfig::default()).unwrap();
    let key = p256::ecdsa::SigningKey::random(&mut rand_core::OsRng);
    for title in [Title::Android, Title::NintendoSwitch] {
        let t = xbox::device_token(&http, title, &key).await.unwrap();
        assert!(t.is_valid() && !t.token.is_empty(), "{title:?}");
    }
}

#[tokio::test]
#[ignore]
async fn live_discovery_and_default_nsal() {
    let http = Http::new(&AuthConfig::default()).unwrap();
    let d = minecraft::discover(&http, GAME_VERSION).await.unwrap();
    println!("signaling={:?} qos_beacons={}", d.signaling, d.qos_beacons.len());
    let env = d.auth;
    assert_eq!(env.playfab_title_id, "20CA2");
    let default = nsal::fetch_default(&http).await.unwrap();
    let (mp, pf) = nsal::resolve(&[default], &env.playfab_host());
    println!("service={} mp_rp={mp} playfab_rp={pf}", env.service_uri);
}
