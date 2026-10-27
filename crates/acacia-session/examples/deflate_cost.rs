//! Time and size of compressing one batch: `Algorithm::Deflate` against a reused zlib state reset
//! per batch (what it did before small_deflate.rs), at batch sizes a bot sends.
//! `cargo run --release -p acacia-session --example deflate_cost`
use std::hint::black_box;
use std::time::Instant;

use acacia_session::compression::Algorithm;
use bytes::BytesMut;

const ROUNDS: u32 = 20_000;

/// Packet-like bytes: small varints and floats with repeats, some noise.
fn batch(len: usize) -> Vec<u8> {
    let mut seed = 0x2545_f491u32;
    (0..len)
        .map(|i| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            if i % 7 < 3 { (seed & 0x0f) as u8 } else { [0x90, 0x01, 0x3f, 0x80, 0x00, 0x00][i % 6] }
        })
        .collect()
}

fn ns_per_call(mut f: impl FnMut()) -> f64 {
    let start = Instant::now();
    for _ in 0..ROUNDS {
        f();
    }
    start.elapsed().as_nanos() as f64 / f64::from(ROUNDS)
}

fn main() {
    let mut zlib = flate2::Compress::new(flate2::Compression::fast(), false);
    println!("{:>6} {:>10} {:>10} {:>8} {:>8}", "bytes", "ours ns", "zlib ns", "ours B", "zlib B");
    for len in [32, 64, 128, 256, 512, 1024, 2048, 4096] {
        let input = batch(len);
        let mut out = BytesMut::with_capacity(2 * len + 64);
        let ours = ns_per_call(|| {
            out.clear();
            Algorithm::Deflate.compress(black_box(&input), &mut out);
        });
        let ours_len = out.len();
        let mut zout = Vec::with_capacity(2 * len + 64);
        let theirs = ns_per_call(|| {
            zout.clear();
            zlib.reset();
            zlib.compress_vec(black_box(&input), &mut zout, flate2::FlushCompress::Finish).unwrap();
        });
        println!("{len:>6} {ours:>10.0} {theirs:>10.0} {ours_len:>8} {:>8}", zout.len());
    }
}
