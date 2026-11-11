//! A connection in play (and in the configuration a server sends it back to) on two spawned tasks, so sending
//! never waits and receiving is cancel-safe.

use acacia_java_proto::packets::{ChunkBatchReceived, Empty, KeepAlive};
use acacia_java_proto::{Wire, ids};
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

use crate::configuration::{self, Cookies};
use crate::frame::{Connection, Error, FrameReader, FrameWriter, encode};

use ids::play::{clientbound as from, serverbound as to};

/// Chunks per tick the client says it can take, as a client with time to spare does.
const CHUNKS_PER_TICK: f32 = 20.0;

/// A packet in the play state.
#[derive(Debug, Clone)]
pub struct Packet {
    pub id: i32,
    pub body: Vec<u8>,
}

/// What the reader hands over: a play packet, or one of a reconfiguration (already answered).
pub(crate) enum Received {
    Play(Packet),
    Configuration(i32, Vec<u8>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Play,
    Configuration,
}

/// A packet to write, in the phase it belongs to; `then` is the phase the connection enters once it is written.
struct Outgoing {
    phase: Phase,
    bytes: Vec<u8>,
    then: Option<Phase>,
}

/// The spawned connection. Keep-alives and chunk-batch acks are answered on the reader task, as vanilla's
/// network thread does; pings are not, because anticheats order them against movement. Configuration is answered
/// whole on the reader too, and play packets queued while it runs are dropped, as the client then has no player to
/// send them. Dropping the link closes the connection.
pub struct Link {
    commands: mpsc::UnboundedSender<Outgoing>,
    events: mpsc::UnboundedReceiver<Result<Received, Error>>,
    tasks: [AbortHandle; 2],
}

impl Link {
    pub(crate) fn spawn(connection: Connection, cookies: Cookies) -> Link {
        let (reader, writer) = connection.into_halves();
        let (commands, outgoing) = mpsc::unbounded_channel();
        let (deliver, events) = mpsc::unbounded_channel();
        let writing = tokio::spawn(write_loop(writer, outgoing)).abort_handle();
        let reading = tokio::spawn(read_loop(reader, Replies(commands.clone()), deliver, cookies)).abort_handle();
        Link { commands, events, tasks: [writing, reading] }
    }

    /// Drops the socket at once; [`Link::recv`] then returns `None` once queued packets are read.
    pub fn close(&self) {
        self.tasks.iter().for_each(AbortHandle::abort);
    }

    /// Queues a play packet; false once the connection has closed.
    pub fn send(&self, id: i32, body: &impl Wire) -> bool {
        self.commands.send(Outgoing { phase: Phase::Play, bytes: encode(id, body), then: None }).is_ok()
    }

    /// The next packet; `None` after the connection has ended and its last error was delivered. Cancel-safe.
    pub(crate) async fn recv(&mut self) -> Option<Result<Received, Error>> {
        self.events.recv().await
    }

    pub(crate) fn try_recv(&mut self) -> Option<Result<Received, Error>> {
        self.events.try_recv().ok()
    }
}

async fn write_loop(mut writer: FrameWriter, mut outgoing: mpsc::UnboundedReceiver<Outgoing>) {
    let mut phase = Phase::Play;
    while let Some(packet) = outgoing.recv().await {
        if packet.phase != phase {
            continue;
        }
        if writer.write(packet.bytes).await.is_err() {
            return;
        }
        phase = packet.then.unwrap_or(phase);
    }
}

struct Replies(mpsc::UnboundedSender<Outgoing>);

impl Replies {
    fn send(&self, phase: Phase, bytes: Vec<u8>, then: Option<Phase>) -> Result<(), Error> {
        self.0.send(Outgoing { phase, bytes, then }).map_err(|_| Error::Closed)
    }
}

async fn read_loop(mut reader: FrameReader, replies: Replies, deliver: mpsc::UnboundedSender<Result<Received, Error>>, mut cookies: Cookies) {
    let mut phase = Phase::Play;
    loop {
        let received = reader.read().await.and_then(|(id, body)| {
            let received = match phase {
                Phase::Play => Received::Play(Packet { id, body }),
                Phase::Configuration => Received::Configuration(id, body),
            };
            phase = reply(phase, &received, &replies, &mut cookies)?;
            Ok(received)
        });
        let failed = received.is_err();
        if deliver.send(received).is_err() || failed {
            return;
        }
    }
}

/// Answers what the reader answers itself, returning the phase the connection is in after `received`.
fn reply(phase: Phase, received: &Received, replies: &Replies, cookies: &mut Cookies) -> Result<Phase, Error> {
    match received {
        Received::Play(Packet { id, body }) => {
            let mut input = &body[..];
            match *id {
                from::KEEP_ALIVE => replies.send(phase, encode(to::KEEP_ALIVE, &KeepAlive::read(&mut input)?), None)?,
                from::CHUNK_BATCH_FINISHED => replies.send(phase, encode(to::CHUNK_BATCH_RECEIVED, &ChunkBatchReceived { chunks_per_tick: CHUNKS_PER_TICK }), None)?,
                from::START_CONFIGURATION => {
                    replies.send(phase, encode(to::CONFIGURATION_ACKNOWLEDGED, &Empty {}), Some(Phase::Configuration))?;
                    return Ok(Phase::Configuration);
                }
                id => {
                    for bytes in configuration::answer_common(&configuration::PLAY, id, body, cookies)? {
                        replies.send(phase, bytes, None)?;
                    }
                }
            }
            Ok(phase)
        }
        Received::Configuration(id, body) => {
            let finished = *id == ids::configuration::clientbound::FINISH_CONFIGURATION;
            for bytes in configuration::answer(*id, body, cookies)? {
                replies.send(phase, bytes, finished.then_some(Phase::Play))?;
            }
            Ok(if finished { Phase::Play } else { phase })
        }
    }
}
