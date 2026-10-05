use acacia_auth::ClientData;
use acacia_session::proto::GAME_VERSION;
use p384::ecdsa::SigningKey;

use crate::client::Login;

/// Who the player is on the server: gamertag (or offline name) and xuid (empty offline).
pub(crate) struct Identity {
    pub display_name: String,
    pub xuid: String,
}

/// `nonce` is a friend's world's per-player join nonce (ClientData `Nonce`). `offline_key` signs an
/// offline login; an online one brings its own key.
pub(crate) fn build_login(login: Login, offline_key: SigningKey, server: &str, nonce: Option<String>) -> (SigningKey, Vec<u8>, Identity) {
    match login {
        Login::Offline { name } => {
            let key = offline_key;
            let client = ClientData { nonce, ..ClientData::default_for(&name, server, GAME_VERSION) };
            let request = acacia_auth::build_offline_connection_request(&name, &key, &client);
            (key, request, Identity { display_name: name, xuid: String::new() })
        }
        Login::Online { credentials, key } => {
            let client = ClientData { nonce, ..ClientData::default_for(&credentials.display_name, server, GAME_VERSION) };
            let request = acacia_auth::build_connection_request(&credentials, &key, &client);
            let identity = Identity { display_name: credentials.display_name, xuid: credentials.xuid };
            (key, request, identity)
        }
    }
}
