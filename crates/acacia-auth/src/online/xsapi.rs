//! Xbox Live service requests shaped like the game's XSAPI (`XblHttpCall`): docs/research/friends-join.md §2.

use p256::ecdsa::SigningKey;
use reqwest::{Method, RequestBuilder};

use super::http::Http;
use super::xbox::XboxToken;

/// TODO: the Android build's XSAPI version is unknown (capture); this pairs with the XAL User-Agent's.
const XSAPI_VERSION: &str = "2025.04.20250326.0";
/// TODO: XSAPI sends the device locales; matches ClientData's `en_GB`, unverified.
const ACCEPT_LANGUAGE: &str = "en-GB";

/// The `http://xboxlive.com` XSTS token and the proof key that signs requests made with it.
pub(crate) struct XboxLiveAuth {
    pub token: XboxToken,
    pub key: SigningKey,
}

impl XboxLiveAuth {
    pub fn xuid(&self) -> Option<&str> {
        self.token.xuid.as_deref()
    }

    /// Headers for an XSAPI WebSocket (RTA): no ` c` suffix on the User-Agent, unlike HTTP calls.
    pub(crate) fn websocket_headers(&self, http: &Http, url: &str) -> Vec<(&'static str, String)> {
        let authorization = self.token.authorization();
        let signature = http.signature(&self.key, "GET", url, &authorization, b"");
        vec![
            ("authorization", authorization),
            ("signature", signature),
            ("accept-language", ACCEPT_LANGUAGE.to_owned()),
            ("user-agent", format!("XboxServicesAPI/{XSAPI_VERSION}")),
        ]
    }

    /// A signed request in XSAPI's header order; `body` is empty for GETs.
    pub(crate) fn request(&self, http: &Http, method: Method, url: &str, contract: &str, body: Vec<u8>) -> RequestBuilder {
        let authorization = self.token.authorization();
        let signature = http.signature(&self.key, method.as_str(), url, &authorization, &body);
        http.client
            .request(method, url)
            .header("x-xbl-contract-version", contract)
            .header("content-type", "application/json; charset=utf-8")
            .header("accept-language", ACCEPT_LANGUAGE)
            .header("user-agent", format!("XboxServicesAPI/{XSAPI_VERSION} c"))
            .header("authorization", authorization)
            .header("signature", signature)
            .body(body)
    }
}
