//! Joining a friend's world: its Xbox session (MPSD over RTA) then the signaling service.
//! Spec: docs/research/friends-join.md.

#[cfg(feature = "online")]
mod join;
#[cfg(feature = "online")]
mod presence;
#[cfg(feature = "online")]
mod rta;

use tokio::sync::oneshot;

use crate::signaling::SignalingTarget;
#[cfg(feature = "online")]
pub use join::{friend_builder, join_friend_world, FriendJoinError};

/// A friend's world joined at the Xbox level, ready to dial with [`crate::ClientBuilder::friend`].
/// Dropping it (or the connection made from it) leaves the friend's Xbox session.
pub struct FriendJoin {
    pub(crate) target: SignalingTarget,
    pub(crate) nonce: Option<String>,
    pub(crate) session: oneshot::Sender<()>,
}

impl FriendJoin {
    pub fn target(&self) -> &SignalingTarget {
        &self.target
    }

    /// The nonce the host published for us (sent as the Login `Nonce`); None if it published none in time.
    pub fn nonce(&self) -> Option<&str> {
        self.nonce.as_deref()
    }
}
