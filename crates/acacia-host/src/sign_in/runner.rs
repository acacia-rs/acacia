use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use acacia_api::{ApiError, CallId, CallResult, ErrorCode, MsaCode, Out, Reply, SignInParams, SignInResult};
use parking_lot::Mutex;
use tokio::runtime::Runtime;
use tokio::task::{AbortHandle, JoinHandle};

use super::{default_profiles_folder, DeviceCode, MsaAccount, OnlineLogin, Prompt, SignIn};
use crate::outbound::ShardBuf;

/// The host's sign-in side: the provider, the thread it runs on (started at first use, so a device-code wait of
/// up to 15 minutes never holds a shard) and the open `signIn` calls, answered `CLOSED` at shutdown.
pub(crate) struct SignIns {
    provider: Arc<dyn SignIn>,
    profiles_folder: PathBuf,
    /// `None` before the first sign-in and after [`SignIns::close`] (then `closed`).
    runtime: Mutex<Option<Runtime>>,
    closed: Mutex<bool>,
    /// Open `signIn` calls, with their task once it is spawned.
    open: Arc<Mutex<HashMap<CallId, Option<AbortHandle>>>>,
    /// Where `signIn` records go (they name no bot, so no bot's shard).
    buf: Arc<ShardBuf>,
}

impl SignIns {
    pub(crate) fn new(provider: Arc<dyn SignIn>, profiles_folder: Option<PathBuf>, buf: Arc<ShardBuf>) -> Self {
        let profiles_folder = profiles_folder.unwrap_or_else(default_profiles_folder);
        Self { provider, profiles_folder, runtime: Mutex::new(None), closed: Mutex::new(false), open: Arc::default(), buf }
    }

    pub(crate) fn account(&self, username: &str, profiles_folder: Option<&str>) -> MsaAccount {
        let profiles_folder = profiles_folder.map_or_else(|| self.profiles_folder.clone(), PathBuf::from);
        MsaAccount { username: username.to_owned(), profiles_folder }
    }

    /// Runs the provider's login on the sign-in thread; dropping the future abandons it.
    pub(crate) async fn login(&self, account: MsaAccount, prompt: Prompt) -> Result<OnlineLogin, ApiError> {
        let mut task = AbortOnDrop(self.spawn(self.provider.login(account, prompt))?);
        (&mut task.0).await.map_err(|e| ApiError::new(ErrorCode::Internal, format!("the sign-in task failed: {e}")))?
    }

    /// Starts a `signIn` call; its `msaCode` and `done` go out on the host's first shard.
    pub(crate) fn sign_in(&self, call: CallId, params: SignInParams) -> Result<(), ApiError> {
        let account = self.account(&params.username, params.profiles_folder.as_deref());
        let buf = self.buf.clone();
        let login = self.provider.login(account, Box::new(move |code| buf.push(msa_code(call, code))));
        let (open, buf) = (self.open.clone(), self.buf.clone());
        open.lock().insert(call, None);
        let spawned = self.spawn(async move {
            let result = login.await.map(|l| Reply::SignIn(SignInResult { display_name: l.credentials.display_name, xuid: l.credentials.xuid }));
            if open.lock().remove(&call).is_some() {
                buf.push(Out::Done { bot: None, call, result: result.map_or_else(CallResult::Err, |r| CallResult::ok(&r)) });
            }
        });
        match spawned {
            Ok(task) => {
                if let Some(slot) = self.open.lock().get_mut(&call) {
                    *slot = Some(task.abort_handle());
                }
                Ok(())
            }
            Err(e) => {
                self.open.lock().remove(&call);
                Err(e)
            }
        }
    }

    /// Abandons open `signIn` call `call`, answering it `CANCELLED`; false when no such call is open.
    pub(crate) fn cancel(&self, call: CallId) -> bool {
        let Some(task) = self.open.lock().remove(&call) else { return false };
        if let Some(task) = task {
            task.abort();
        }
        self.answer(call, Err(ApiError::new(ErrorCode::Cancelled, "cancelled while signing in")));
        true
    }

    /// A `done` naming no bot, on the sign-in records' shard.
    pub(crate) fn answer(&self, call: CallId, result: Result<Reply, ApiError>) {
        self.buf.push(Out::Done { bot: None, call, result: result.map_or_else(CallResult::Err, |r| CallResult::ok(&r)) });
    }

    /// Answers the open `signIn` calls `CLOSED` and stops the sign-in thread; later sign-ins are refused.
    pub(crate) fn close(&self) {
        *self.closed.lock() = true;
        for call in std::mem::take(&mut *self.open.lock()).into_keys() {
            self.answer(call, Err(ApiError::new(ErrorCode::Closed, "the host shut down during the sign-in")));
        }
        if let Some(runtime) = self.runtime.lock().take() {
            runtime.shutdown_background();
        }
    }

    fn spawn<T: Send + 'static>(&self, task: impl Future<Output = T> + Send + 'static) -> Result<JoinHandle<T>, ApiError> {
        if *self.closed.lock() {
            return Err(ApiError::new(ErrorCode::Closed, "the host is shutting down"));
        }
        let mut runtime = self.runtime.lock();
        let runtime = match &mut *runtime {
            Some(runtime) => runtime,
            slot => slot.insert(
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .thread_name("acacia-sign-in")
                    .enable_all()
                    .build()
                    .map_err(|e| ApiError::new(ErrorCode::Internal, format!("the sign-in thread: {e}")))?,
            ),
        };
        Ok(runtime.spawn(task))
    }
}

impl Drop for SignIns {
    fn drop(&mut self) {
        // Dropping a runtime inside another (a daemon's) panics; shutting it down in the background does not.
        if let Some(runtime) = self.runtime.get_mut().take() {
            runtime.shutdown_background();
        }
    }
}

struct AbortOnDrop<T>(JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// The `msaCode` record for call `call`, worded as prismarine-auth words it.
pub(crate) fn msa_code(call: CallId, code: DeviceCode) -> Out {
    let message = format!(
        "To sign in, use a web browser to open the page {} and enter the code {} to authenticate.",
        code.verification_uri, code.user_code
    );
    Out::MsaCode(MsaCode { call, user_code: code.user_code, verification_uri: code.verification_uri, message, expires_in: code.expires_in })
}
