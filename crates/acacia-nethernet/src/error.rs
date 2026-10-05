#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("signaling HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("server refused the offer: NetherNet error {code} ({})", signaling_error_name(*.code))]
    Signaling { code: u32 },
    #[error("malformed HTTP response")]
    MalformedHttp,
    #[error("SDP: {0}")]
    Sdp(String),
    #[error("signal: {0}")]
    Signal(String),
    #[error("LAN discovery: {0}")]
    Lan(String),
    #[error("server identity: {0}")]
    ServerIdentity(String),
    #[error("client identity: {0}")]
    ClientIdentity(String),
    #[error("the host has no room for another connection")]
    HostFull,
    #[error("message framing: {0}")]
    Framing(&'static str),
    #[error("WebRTC: {0}")]
    Rtc(String),
}

impl From<str0m::RtcError> for Error {
    fn from(e: str0m::RtcError) -> Self {
        Self::Rtc(e.to_string())
    }
}

/// Names of NetherNet signaling error codes (df-mc/go-nethernet `signal.go`).
pub fn signaling_error_name(code: u32) -> &'static str {
    const NAMES: [&str; 38] = [
        "None", "DestinationNotLoggedIn", "NegotiationTimeout", "WrongTransportVersion",
        "FailedToCreatePeerConnection", "ICE", "ConnectRequest", "ConnectResponse", "CandidateAdd",
        "InactivityTimeout", "FailedToCreateOffer", "FailedToCreateAnswer", "FailedToSetLocalDescription",
        "FailedToSetRemoteDescription", "NegotiationTimeoutWaitingForResponse",
        "NegotiationTimeoutWaitingForAccept", "IncomingConnectionIgnored", "SignalingParsingFailure",
        "SignalingUnknownError", "SignalingUnicastMessageDeliveryFailed", "SignalingBroadcastDeliveryFailed",
        "SignalingMessageDeliveryFailed", "SignalingTurnAuthFailed", "SignalingFallbackToBestEffortDelivery",
        "NoSignalingChannel", "NotLoggedIn", "SignalingFailedToSend", "RelayServerConfigurationResultFailure",
        "RelayConfigNoURLs", "RelayConfigNoCreds", "RelayConfigNoServers", "RelayConfigNoExpiration",
        "DataChannelClosed", "InternalErrorJSONSerialization", "InvalidArgument", "GenericFailure",
        "FailedToCreateIdentityAssertion", "IdentityNotAllowed",
    ];
    NAMES.get(code as usize).copied().unwrap_or("Unknown")
}
