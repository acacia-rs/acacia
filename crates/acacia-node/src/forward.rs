//! The forwarder thread: the host's record stream into the JS sink, one call per batch (contract: the crate docs).

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use acacia_host::Outbound;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode, UnknownReturnValue};
use napi::Status;

/// Counts calls, not records: a full queue means JS is far behind, and the forwarder merges until it catches up.
pub const MAX_QUEUE: usize = 1024;

/// `onRecords(batch | null)`: weak, callee not handed errors, bounded.
pub type Sink = ThreadsafeFunction<Option<String>, UnknownReturnValue, Option<String>, Status, false, true, MAX_QUEUE>;

const RETRY: Duration = Duration::from_millis(1);

pub fn spawn(mut outbound: Outbound, sink: Arc<Sink>) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new().name("acacia-node-forward".into()).spawn(move || {
        while let Some(mut held) = outbound.blocking_recv() {
            loop {
                while let Some(next) = outbound.try_recv() {
                    held = merge(held, &next);
                }
                // napi drops a payload it refuses, hence the clone.
                match sink.call(Some(held.clone()), ThreadsafeFunctionCallMode::NonBlocking) {
                    Status::Ok => break,
                    Status::QueueFull => std::thread::sleep(RETRY),
                    // Closing: the env is being torn down, nobody is left to read.
                    _ => return,
                }
            }
        }
        while sink.call(None, ThreadsafeFunctionCallMode::NonBlocking) == Status::QueueFull {
            std::thread::sleep(RETRY);
        }
    })
}

/// Two JSON arrays as one, in order.
fn merge(mut a: String, b: &str) -> String {
    match (a.as_str(), b) {
        (_, "[]") => a,
        ("[]", _) => b.to_owned(),
        _ => {
            a.pop();
            a.push(',');
            a.push_str(&b[1..]);
            a
        }
    }
}
