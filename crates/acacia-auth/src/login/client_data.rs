use serde::{Deserialize, Serialize};

use super::device::Device;
use super::skin::Skin;

/// Claims of the client-data JWT in the Login packet: the key set the vanilla 1.26.52 client sends
/// (no PlayFabId/PartyId/IsPartyLeader), signed with keys sorted (`request::sign_client`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct ClientData {
    #[serde(flatten)]
    pub skin: Skin,
    #[serde(rename = "ClientRandomId")]
    pub client_random_id: i64,
    pub current_input_mode: i32,
    pub default_input_mode: i32,
    pub device_model: String,
    #[serde(rename = "DeviceOS")]
    pub device_os: i32,
    #[serde(rename = "DeviceId")]
    pub device_id: String,
    pub game_version: String,
    pub gui_scale: i32,
    pub filter_profanity: bool,
    pub client_editor_connection_intent: i32,
    pub client_is_editor_capable: bool,
    pub language_code: String,
    #[serde(rename = "PlatformOfflineId")]
    pub platform_offline_id: String,
    #[serde(rename = "PlatformOnlineId")]
    pub platform_online_id: String,
    #[serde(rename = "PlatformUserId", skip_serializing_if = "Option::is_none")]
    pub platform_user_id: Option<String>,
    #[serde(rename = "SelfSignedId")]
    pub self_signed_id: String,
    pub server_address: String,
    pub third_party_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub third_party_name_only: Option<bool>,
    #[serde(rename = "UIProfile")]
    pub ui_profile: i32,
    pub compatible_with_client_side_chunk_gen: bool,
    pub max_view_distance: i32,
    pub memory_tier: i32,
    pub platform_type: i32,
    pub graphics_mode: i32,
    pub profile_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

pub const DEVICE_OS_ANDROID: i32 = 1;
pub const INPUT_MODE_TOUCH: i32 = 2;
const UI_PROFILE_POCKET: i32 = 1;
const PLATFORM_TYPE_MOBILE: i32 = 1;
const GRAPHICS_MODE_FANCY: i32 = 1;
/// What phones report as their render-distance ceiling (Android ClientData dump);
/// also the `max_radius` of RequestChunkRadius.
pub const MAX_VIEW_DISTANCE: i32 = 22;

impl ClientData {
    /// An Android phone (online logins are Android-titled, so everything must agree with that),
    /// the same device and skin for `display_name` on every join.
    pub fn default_for(display_name: &str, server_address: &str, game_version: &str) -> Self {
        let device = Device::for_account(display_name);
        Self {
            skin: Skin::for_account(display_name),
            client_random_id: device.client_random_id,
            current_input_mode: INPUT_MODE_TOUCH,
            default_input_mode: INPUT_MODE_TOUCH,
            device_model: device.model.to_owned(),
            device_os: DEVICE_OS_ANDROID,
            device_id: device.device_id,
            game_version: game_version.to_owned(),
            gui_scale: 0,
            filter_profanity: false,
            client_editor_connection_intent: 0,
            client_is_editor_capable: false,
            language_code: "en_GB".into(),
            platform_offline_id: String::new(),
            platform_online_id: String::new(),
            platform_user_id: None,
            self_signed_id: String::new(),
            server_address: server_address.to_owned(),
            third_party_name: display_name.to_owned(),
            third_party_name_only: None,
            ui_profile: UI_PROFILE_POCKET,
            compatible_with_client_side_chunk_gen: true,
            max_view_distance: MAX_VIEW_DISTANCE,
            memory_tier: device.memory_tier,
            platform_type: PLATFORM_TYPE_MOBILE,
            graphics_mode: GRAPHICS_MODE_FANCY,
            profile_hash: String::new(),
            nonce: None,
        }
    }
}
