//! A play-state connection on two spawned tasks, so sending never waits and receiving is cancel-safe.

use acacia_java_proto::packets::{ChunkBatchReceived, KeepAlive};
use acacia_java_proto::{Wire, ids};
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

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

/// The spawned connection. Keep-alives and chunk-batch acks are answered on the reader task, as vanilla's
/// network thread does; pings are not, because anticheats order them against movement. Dropping the link
/// closes the connection.
pub struct Link {
    commands: mpsc::UnboundedSender<Vec<u8>>,
    events: mpsc::UnboundedReceiver<Result<Packet, Error>>,
    tasks: [AbortHandle; 2],
}

impl Link {
    pub(crate) fn spawn(connection: Connection) -> Link {
        let (reader, writer) = connection.into_halves();
        let (commands, outgoing) = mpsc::unbounded_channel();
        let (deliver, events) = mpsc::unbounded_channel();
        let writing = tokio::spawn(write_loop(writer, outgoing)).abort_handle();
        let reading = tokio::spawn(read_loop(reader, commands.clone(), deliver)).abort_handle();
        Link { commands, events, tasks: [writing, reading] }
    }

    /// Drops the socket at once; [`Link::recv`] then returns `None` once queued packets are read.
    pub fn close(&self) {
        self.tasks.iter().for_each(AbortHandle::abort);
    }

    /// Queues a packet; false once the connection has closed.
    pub fn send(&self, id: i32, body: &impl Wire) -> bool {
        self.commands.send(encode(id, body)).is_ok()
    }

    /// The next packet; `None` after the connection has ended and its last error was delivered. Cancel-safe.
    pub async fn recv(&mut self) -> Option<Result<Packet, Error>> {
        self.events.recv().await
    }

    pub fn try_recv(&mut self) -> Option<Result<Packet, Error>> {
        self.events.try_recv().ok()
    }
}

async fn write_loop(mut writer: FrameWriter, mut outgoing: mpsc::UnboundedReceiver<Vec<u8>>) {
    while let Some(packet) = outgoing.recv().await {
        if writer.write(packet).await.is_err() {
            return;
        }
    }
}

async fn read_loop(mut reader: FrameReader, replies: mpsc::UnboundedSender<Vec<u8>>, deliver: mpsc::UnboundedSender<Result<Packet, Error>>) {
    loop {
        let packet = match reader.read().await.and_then(|(id, body)| reply(id, &body, &replies).map(|()| Packet { id, body })) {
            Ok(packet) => packet,
            Err(e) => {
                deliver.send(Err(e)).ok();
                return;
            }
        };
        if deliver.send(Ok(packet)).is_err() {
            return;
        }
    }
}

fn reply(id: i32, body: &[u8], replies: &mpsc::UnboundedSender<Vec<u8>>) -> Result<(), Error> {
    let mut input = body;
    let packet = match id {
        from::KEEP_ALIVE => encode(to::KEEP_ALIVE, &KeepAlive::read(&mut input)?),
        from::CHUNK_BATCH_FINISHED => encode(to::CHUNK_BATCH_RECEIVED, &ChunkBatchReceived { chunks_per_tick: CHUNKS_PER_TICK }),
        _ => return Ok(()),
    };
    replies.send(packet).map_err(|_| Error::Closed)
}
