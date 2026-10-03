//! Prints registry data for blocks: `cargo run -p acacia-world --example block_info -- leaf_litter short_grass`
use acacia_world::BlockRegistry;

fn main() {
    let registry = BlockRegistry::vanilla();
    for name in std::env::args().skip(1) {
        let name = if name.contains(':') { name } else { format!("minecraft:{name}") };
        let states: Vec<_> = registry.states_of(&name).collect();
        println!("{name}: {} states", states.len());
        let mut seen = Vec::new();
        for (id, s) in states {
            let key = format!("{:?}", s.boxes);
            if !seen.contains(&key) {
                println!("  #{id} [{}] friction {} flags {:#06x} boxes {key}", s.properties, s.friction, s.flags.0);
                seen.push(key);
            }
        }
    }
}
