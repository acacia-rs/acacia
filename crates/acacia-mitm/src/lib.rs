//! A man-in-the-middle proxy for Minecraft: Bedrock Edition over RakNet or NetherNet direct
//! connect: the game joins the [`Proxy`], the proxy joins the server as the same player. Only the
//! encryption handshake is terminated (each side gets its own key) and Login re-signed; the rest
//! can be recorded ([`Recorder`]), and dropped, rewritten or added to ([`Interceptor`],
//! [`Injector`]). The `acacia-mitm` binary is the recording CLI on top (main.rs).

mod intercept;
mod login;
mod nethernet;
mod pair;
mod proxy;
mod raknet;
mod record;
mod relay;

pub use acacia_proto as proto;
pub use intercept::{encode, Direction, Injector, Interceptor, Session, Verdict};
pub use nethernet::host_key;
pub use proxy::{BoundProxy, Proxy};
pub use record::Recorder;
