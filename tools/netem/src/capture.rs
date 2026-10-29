//! `--capture FILE`: one line per datagram the proxy took in, for raknet_trace.py: microseconds
//! since the proxy started, `up` or `down`, what became of it (`sent`, `lost`, `full`) and its
//! bytes in hex.

use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::sync::Mutex;
use std::time::Instant;

use crate::link::Fate;

pub struct Capture {
    epoch: Instant,
    out: Mutex<BufWriter<File>>,
}

impl Capture {
    pub fn create(path: &str, epoch: Instant) -> io::Result<Self> {
        Ok(Self { epoch, out: Mutex::new(BufWriter::new(File::create(path)?)) })
    }

    pub fn record(&self, up: bool, fate: Fate, data: &[u8]) {
        let fate = match fate {
            Fate::Lost => "lost",
            Fate::Overflow => "full",
            Fate::Arrives(_) => "sent",
        };
        let mut line = format!("{} {} {fate} ", self.epoch.elapsed().as_micros(), if up { "up" } else { "down" });
        data.iter().for_each(|byte| {
            let _ = write!(line, "{byte:02x}");
        });
        let _ = writeln!(self.out.lock().expect("capture lock"), "{line}");
    }

    /// The proxy is ended by a signal, so what is buffered is written out as it goes.
    pub fn flush(&self) {
        let _ = self.out.lock().expect("capture lock").flush();
    }
}
