use std::time::Duration;

/// The Xbox title the login impersonates. Values from gophertunnel `auth.AndroidConfig` /
/// `auth.NintendoConfig` (client IDs match prismarine-auth `Titles.js`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Title {
    /// Minecraft for Android. gophertunnel's default; the Mojang chain's `titleId` is 1739947436.
    #[default]
    Android,
    /// Minecraft for Nintendo Switch (prismarine-auth / bedrock-protocol default).
    NintendoSwitch,
}

impl Title {
    /// MSA OAuth client ID (also the SISU `AppId`).
    pub fn client_id(self) -> &'static str {
        match self {
            Title::Android => "0000000048183522",
            Title::NintendoSwitch => "00000000441cc96b",
        }
    }

    pub fn title_id(self) -> u64 {
        match self {
            Title::Android => 1_739_947_436,
            Title::NintendoSwitch => 2_047_319_603,
        }
    }

    /// `User-Agent` for live.com refresh and Xbox auth requests.
    pub fn user_agent(self) -> &'static str {
        match self {
            Title::Android => "XAL Android 2025.04.20250326.000",
            Title::NintendoSwitch => "XAL",
        }
    }

    /// `(DeviceType, Version)` for the device token request.
    pub fn device(self) -> (&'static str, &'static str) {
        match self {
            Title::Android => ("Android", "13"),
            Title::NintendoSwitch => ("Nintendo", "0.0.0"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub title: Title,
    /// Proxy URL for every auth request (`http://`, `https://`, or `socks5://` with the `socks`
    /// feature).
    pub proxy: Option<String>,
    pub timeout: Duration,
    /// Game version sent to discovery, `Client-Version` and session/start.
    pub game_version: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            title: Title::default(),
            proxy: None,
            timeout: Duration::from_secs(30),
            game_version: GAME_VERSION.into(),
        }
    }
}

/// The current vanilla release on protocol 2193; keep equal to acacia-proto's `GAME_VERSION`
/// (tools/codegen/data/version.json).
pub const GAME_VERSION: &str = "1.26.52";

pub(crate) mod endpoints {
    pub const LIVE_DEVICE_CODE: &str = "https://login.live.com/oauth20_connect.srf";
    pub const LIVE_TOKEN: &str = "https://login.live.com/oauth20_token.srf";
    pub const LIVE_SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";
    pub const DEVICE_AUTH: &str = "https://device.auth.xboxlive.com/device/authenticate";
    pub const SISU_AUTHORIZE: &str = "https://sisu.xboxlive.com/authorize";
    pub const XSTS_AUTHORIZE: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
    pub const NSAL_TITLES: &str = "https://title.mgt.xboxlive.com/titles";
    pub const MOJANG_CHAIN: &str = "https://multiplayer.minecraft.net/authentication";
    pub const DISCOVERY: &str =
        "https://client.discovery.minecraft-services.net/api/v1.0/discovery/MinecraftPE/builds";

    pub const RP_XBOXLIVE: &str = "http://xboxlive.com";
    /// Fallbacks when NSAL resolution fails (prismarine-auth / default NSAL title values).
    pub const RP_MULTIPLAYER_FALLBACK: &str = "https://multiplayer.minecraft.net/";
    pub const RP_PLAYFAB_FALLBACK: &str = "http://playfab.xboxlive.com/";

    /// User-Agent gophertunnel sends to *.minecraft-services.net (libHttpClient).
    pub const SERVICES_USER_AGENT: &str = "libhttpclient/1.0.0.0";
}
