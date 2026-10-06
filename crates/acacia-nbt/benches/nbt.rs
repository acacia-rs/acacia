//! Throughput of read, skip and write over the real documents in corpus/, per flavour and kind.
//! `cargo bench -p acacia-nbt [-- <filter>]`, e.g. `-- le/item/read`.

use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Duration;

use acacia_nbt::{Flavor, LittleEndian, Nbt, Network, read, skip, write};
use bytes::BytesMut;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};

#[path = "../tests/common/corpus.rs"]
mod corpus;

fn flavour<F: Flavor>(c: &mut Criterion, name: &str) {
    let mut kinds: BTreeMap<String, Vec<Vec<u8>>> = BTreeMap::new();
    for (file, bytes) in corpus::load(name) {
        let kind = file.rsplit_once('-').map_or(file.as_str(), |(kind, _)| kind).to_owned();
        kinds.entry(kind).or_default().push(bytes);
    }
    for (kind, docs) in &kinds {
        let trees: Vec<Nbt> = docs.iter().map(|d| read::<F>(&mut &d[..]).unwrap()).collect();
        let bytes: usize = docs.iter().map(Vec::len).sum();
        let mut group = c.benchmark_group(format!("{name}/{kind}"));
        group.throughput(Throughput::Bytes(bytes as u64));
        group.warm_up_time(Duration::from_millis(500)).measurement_time(Duration::from_secs(2));
        group.bench_function("read", |b| {
            b.iter(|| docs.iter().for_each(|d| drop(black_box(read::<F>(&mut &d[..]).unwrap()))))
        });
        group.bench_function("skip", |b| b.iter(|| docs.iter().for_each(|d| skip::<F>(&mut black_box(&d[..])).unwrap())));
        group.bench_function("write", |b| {
            let mut w = BytesMut::with_capacity(bytes);
            b.iter(|| {
                w.clear();
                trees.iter().for_each(|t| write::<F>(&mut w, t));
                black_box(w.len())
            })
        });
        group.finish();
    }
}

fn benches(c: &mut Criterion) {
    flavour::<Network>(c, "network");
    flavour::<LittleEndian>(c, "le");
}

criterion_group!(nbt, benches);
criterion_main!(nbt);
