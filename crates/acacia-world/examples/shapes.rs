//! Prints every state of a block with its collision boxes: `cargo run -p acacia-world --example shapes -- snow_layer`
fn main() {
    let name = format!("minecraft:{}", std::env::args().nth(1).expect("usage: shapes <block>"));
    for (id, s) in acacia_world::BlockRegistry::vanilla().states_of(&name) {
        let boxes: Vec<_> = s.boxes.iter().map(|b| (b.min, b.max)).collect();
        println!("{id:6} [{}] {boxes:?}", s.properties);
    }
}
