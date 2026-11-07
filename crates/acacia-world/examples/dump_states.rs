//! Prints every vanilla state as `name<TAB>properties` in runtime-id order, for acacia-registry's generator:
//! `cargo run -p acacia-world --example dump_states > states.tsv`
use acacia_world::BlockRegistry;

fn main() {
    let registry = BlockRegistry::vanilla();
    for id in 0..registry.len() as u32 {
        let s = registry.get(id).expect("dense ids");
        println!("{}\t{}", s.name, s.properties);
    }
}
