use serde::{Deserialize, Serialize};

/// Everything the Login packet needs from online auth. Bound to one P-384 client key: the
/// chain's `identityPublicKey` and the multiplayer token's `cpk` both name that key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoginCredentials {
    /// Mojang-signed chain from `multiplayer.minecraft.net/authentication` (2 JWTs), without the
    /// client's self-signed head; [`crate::build_connection_request`] prepends that.
    pub chain: Vec<String>,
    /// OIDC multiplayer token (`cpk` = client key). Required by 1.26.10+ servers.
    pub multiplayer_token: Option<String>,
    pub xuid: String,
    pub display_name: String,
    /// Player UUID (`extraData.identity` of the chain).
    pub identity: String,
    /// PlayFab master player ID (`PlayFabId` in the client data), when PlayFab login succeeded.
    pub playfab_id: Option<String>,
    /// Unix seconds after which the chain or multiplayer token must be re-requested.
    pub expires_at: i64,
}
