//! Offline check: items captured from a server survive ItemStack -> to_wire unchanged.
//! `cargo run -p acacia-bot --example item_roundtrip -- <capture dir with InventoryContent/InventorySlot .bin files>`
use acacia_bot::interact::to_wire;
use acacia_bot::proto::packets::{InventoryContent, InventorySlot};
use acacia_bot::proto::types::ItemV4;
use acacia_bot::proto::{Packet, RawPacket};
use acacia_bot::state::ItemStack;
use bytes::BytesMut;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).ok_or("capture dir required")?;
    let (mut checked, mut differ) = (0, 0);
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        let raw = RawPacket::parse(std::fs::read(&path)?.into())?;
        let items: Vec<ItemV4> = match raw.id {
            InventoryContent::ID => raw.decode::<InventoryContent>()?.input,
            InventorySlot::ID => vec![raw.decode::<InventorySlot>()?.item],
            _ => continue,
        };
        for orig in items.into_iter().filter(|i| i.network_id != 0) {
            checked += 1;
            if std::env::var("SHOW_ITEMS").is_ok() {
                let stack = ItemStack::from(orig.clone());
                println!("id {} x{} meta {} stack_id {:?} name {:?} nbt {:?}", orig.network_id, orig.count, orig.metadata, stack.stack_network_id, stack.custom_name, stack.nbt.as_ref().map(|n| format!("{n:?}").chars().take(300).collect::<String>()));
            }
            let back = to_wire(&ItemStack::from(orig.clone()));
            let (mut a, mut b) = (BytesMut::new(), BytesMut::new());
            orig.write(&mut a);
            back.write(&mut b);
            if a != b {
                differ += 1;
                if differ <= 3 {
                    println!("{}: id {} differs\n  orig {:02x?}\n  ours {:02x?}", path.display(), orig.network_id, &a[..a.len().min(96)], &b[..b.len().min(96)]);
                }
            }
        }
    }
    println!("checked {checked} items, {differ} differ");
    Ok(())
}
