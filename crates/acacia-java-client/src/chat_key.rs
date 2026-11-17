//! Profile key refresh: `AccountProfileKeyPairManager` as `ClientPacketListener.tick` drives it. A key past its
//! `refreshed_after` is fetched again (at most once an hour); a different key starts a new chat session.

use std::sync::mpsc::{Receiver, TryRecvError, channel};

use acacia_auth::{Account, JavaProfileKey};
use acacia_java_proto::ids::play::serverbound::CHAT_SESSION_UPDATE;
use tokio::runtime::Handle;

use crate::chat::now_millis;
use crate::frame::Error;
use crate::join::Client;
use crate::signing::{ChatKey, Session};

/// `MINIMUM_PROFILE_KEY_REFRESH_INTERVAL`.
const MIN_INTERVAL_MILLIS: i64 = 3_600_000;

/// The fetching half of `AccountProfileKeyPairManager`.
#[derive(Default)]
pub(crate) struct KeyRefresh {
    /// The account and the runtime the join ran on; `None` offline.
    fetcher: Option<(Account, Handle)>,
    /// `nextProfileKeyRefreshTime`, unix milliseconds.
    next_check: i64,
    /// The `keyPair` future while it runs: a fetched key, or `None` when the fetch failed.
    pending: Option<Receiver<Option<JavaProfileKey>>>,
}

impl KeyRefresh {
    pub(crate) fn new(account: Account) -> Self {
        Self { fetcher: Handle::try_current().ok().map(|handle| (account, handle)), ..Self::default() }
    }

    /// `shouldRefreshKeyPair`.
    fn due(&self, key: Option<&JavaProfileKey>, now: i64) -> bool {
        self.pending.is_none() && now > self.next_check && key.is_none_or(|k| k.refreshed_after < now)
    }

    /// `prepareKeyPair`: no new check for an hour, and a fetch unless the key isn't due (`readOrFetchProfileKeyPair`).
    pub(crate) fn prepare(&mut self, key: Option<&JavaProfileKey>, now: i64) {
        self.next_check = now + MIN_INTERVAL_MILLIS;
        let Some((account, handle)) = &self.fetcher else { return };
        if self.pending.is_some() || key.is_some_and(|k| k.refreshed_after >= now) {
            return;
        }
        let (send, receive) = channel();
        let account = account.clone();
        handle.spawn(async move {
            let key = match account.java_profile().await {
                Ok(player) => account.java_profile_key(&player).await.ok(),
                Err(_) => None,
            };
            let _ = send.send(key);
        });
        self.pending = Some(receive);
    }

    /// The finished fetch's key; a failed one keeps the cached key, as vanilla does.
    fn take(&mut self) -> Option<JavaProfileKey> {
        let key = match self.pending.as_ref()?.try_recv() {
            Err(TryRecvError::Empty) => return None,
            Ok(key) => key,
            Err(TryRecvError::Disconnected) => None,
        };
        self.pending = None;
        key
    }
}

impl Client {
    /// The key refresh in `ClientPacketListener.tick`, which `step` runs: with a chat session, a due key is fetched
    /// again, and a fetched key that differs (`setKeyPair`) gets a new session announced to the server.
    pub fn tick_profile_key(&self) -> Result<(), Error> {
        let now = now_millis();
        let mut chat = self.chat_state();
        let current = chat.key.as_ref().ok().map(ChatKey::profile).cloned();
        if chat.session.is_some() && chat.refresh.due(current.as_ref(), now) {
            chat.refresh.prepare(current.as_ref(), now);
        }
        let Some(fetched) = chat.refresh.take() else { return Ok(()) };
        if chat.session.is_some() && current.as_ref() == Some(&fetched) {
            return Ok(());
        }
        let Ok(key) = ChatKey::new(fetched) else { return Ok(()) };
        let session = Session::new(self.uuid);
        self.send(CHAT_SESSION_UPDATE, &session.update(&key))?;
        chat.key = Ok(key);
        chat.session = Some(session);
        Ok(())
    }

    /// Fetches the profile key now, as if it were due: lets a live check run the refresh.
    pub fn refresh_profile_key(&self) {
        self.chat_state().refresh.prepare(None, now_millis());
    }

    /// Whether a profile key fetch is under way.
    pub fn refreshing_profile_key(&self) -> bool {
        self.chat_state().refresh.pending.is_some()
    }

    /// The announced chat session's id.
    pub fn chat_session(&self) -> Option<u128> {
        self.chat_state().session.as_ref().map(|s| s.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(refreshed_after: i64) -> JavaProfileKey {
        JavaProfileKey { private_key: Vec::new(), public_key: Vec::new(), signature: Vec::new(), expires_at: 0, refreshed_after }
    }

    #[test]
    fn refresh_waits_for_the_key_and_the_hour() {
        let mut refresh = KeyRefresh::default();
        assert!(refresh.due(None, 1), "no key: due at once");
        assert!(!refresh.due(Some(&key(10)), 5), "the key isn't due");
        assert!(refresh.due(Some(&key(10)), 11));
        refresh.prepare(Some(&key(10)), 11);
        assert!(!refresh.due(Some(&key(10)), 12), "an hour between checks");
        assert!(refresh.due(Some(&key(10)), 12 + MIN_INTERVAL_MILLIS));
        assert!(refresh.pending.is_none(), "no account: nothing fetched");
    }
}
