use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tokio::task::{JoinSet, LocalSet};

use super::spec::BotId;

/// Built on the caller's thread, run on the shard's: the future it returns need not be `Send`.
pub type Job = Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()>>> + Send>;

/// Told which bot's task panicked (user task code included), with the panic message.
pub type OnPanic = Arc<dyn Fn(&BotId, String) + Send + Sync>;

/// One OS thread with a single-threaded runtime: its bots never migrate between threads. The swarm runs its
/// supervisors on these; a host with its own per-bot loop can use one directly.
pub struct Shard {
    jobs: mpsc::UnboundedSender<(BotId, Job)>,
    exited: oneshot::Receiver<()>,
}

impl Shard {
    /// Starts thread `swarm-shard-{index}`; `on_panic` hears of each job that panics.
    pub fn start(index: usize, on_panic: OnPanic) -> std::io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let (jobs, queue) = mpsc::unbounded_channel();
        let (exit, exited) = oneshot::channel();
        std::thread::Builder::new().name(format!("swarm-shard-{index}")).spawn(move || {
            LocalSet::new().block_on(&runtime, run(queue, on_panic));
            let _ = exit.send(());
        })?;
        Ok(Self { jobs, exited })
    }

    /// False once the shard has stopped.
    pub fn submit(&self, id: BotId, job: Job) -> bool {
        self.jobs.send((id, job)).is_ok()
    }

    /// Stops taking jobs and waits for its bots to finish.
    pub async fn stop(self) {
        drop(self.jobs);
        let _ = self.exited.await;
    }
}

async fn run(mut queue: mpsc::UnboundedReceiver<(BotId, Job)>, on_panic: OnPanic) {
    let mut bots = JoinSet::new();
    let mut ids = HashMap::new();
    loop {
        tokio::select! {
            job = queue.recv() => match job {
                Some((id, job)) => {
                    ids.insert(bots.spawn_local(job()).id(), id);
                }
                None => break,
            },
            Some(done) = bots.join_next_with_id(), if !bots.is_empty() => {
                let (task, error) = match done {
                    Ok((task, ())) => (task, None),
                    Err(e) => (e.id(), Some(e.to_string())),
                };
                if let (Some(id), Some(error)) = (ids.remove(&task), error) {
                    on_panic(&id, error);
                }
            }
        }
    }
    while bots.join_next().await.is_some() {}
}
