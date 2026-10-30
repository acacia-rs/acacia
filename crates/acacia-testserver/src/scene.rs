//! Scene helpers for tests: other players and entities, block changes, and when the client can see them, sent as
//! BDS would (hashed block ids, players at eye height).

use std::collections::{BTreeMap, HashSet};
use std::io;

use acacia_proto::packets::{
    AddEntity, AddPlayer, InventorySlot, ItemRegistry, MovePlayer, RemoveEntity, Respawn, StartGame, SubchunkRequest,
    UpdateSubchunkBlocks,
};
use acacia_proto::types::{BlockCoordinates, BlockUpdate, BlockUpdateTransitionType, Vec3f, WindowIDVarint};
use acacia_proto::{Packet, RawPacket};
use acacia_world::BlockRegistry;
use bytes::Bytes;

use crate::peer::Received;
use crate::FakeServer;

/// Players' wire position is this far above their feet.
const EYE_HEIGHT: f32 = 1.62;
/// `Respawn.state` once the server has placed the player.
const RESPAWN_READY: u8 = 1;

/// The first sample of acacia-proto's byte fixture for `T`.
fn fixture<T: Packet>(hex: &str) -> T {
    let bytes = hex::decode(hex.lines().next().expect("a fixture line")).expect("fixture hex");
    RawPacket::parse(Bytes::from(bytes)).and_then(|p| p.decode()).expect("the fixture decodes")
}

fn vec3([x, y, z]: [f32; 3]) -> Vec3f {
    Vec3f { x, y, z }
}

/// The id BDS sends for block `name`'s default state.
pub fn bds_block_id(name: &str) -> u32 {
    let registry = BlockRegistry::vanilla();
    let id = registry.find(name, "").unwrap_or_else(|| panic!("{name} is not a vanilla block"));
    registry.network_hash(id)
}

impl FakeServer {
    /// Shows another player with its feet at `feet` (`AddPlayer` only: no player-list entry).
    pub fn add_player(&self, name: &str, runtime_id: u64, unique_id: i64, feet: [f32; 3]) {
        let mut p: AddPlayer = fixture(include_str!("../../acacia-proto/tests/fixtures/packets/add_player.hex"));
        (p.runtime_id, p.unique_id, p.username) = (runtime_id, unique_id, name.into());
        p.position = vec3([feet[0], feet[1] + EYE_HEIGHT, feet[2]]);
        (p.velocity, p.yaw, p.pitch, p.head_yaw) = (vec3([0.0; 3]), 0.0, 0.0, 0.0);
        self.send(&p);
    }

    /// Shows an entity of `kind` (`minecraft:zombie`) with its feet at `feet`.
    pub fn add_entity(&self, kind: &str, runtime_id: u64, unique_id: i64, feet: [f32; 3]) {
        let mut p: AddEntity = fixture(include_str!("../../acacia-proto/tests/fixtures/packets/add_entity.hex"));
        (p.runtime_id, p.unique_id, p.entity_type, p.position) = (runtime_id, unique_id, kind.into(), vec3(feet));
        (p.velocity, p.yaw, p.pitch, p.head_yaw, p.body_yaw) = (vec3([0.0; 3]), 0.0, 0.0, 0.0, 0.0);
        self.send(&p);
    }

    pub fn remove_entity(&self, unique_id: i64) {
        self.send(&RemoveEntity { entity_id_self: unique_id });
    }

    /// Sets `cells` to `wire_id` ([`bds_block_id`]), one `UpdateSubchunkBlocks` per sub-chunk. The client drops
    /// updates for chunks it has not loaded: wait for [`FakeServer::columns_sent`] first.
    pub fn set_blocks(&self, cells: &[[i32; 3]], wire_id: u32) {
        let mut by_section: BTreeMap<[i32; 3], Vec<BlockUpdate>> = BTreeMap::new();
        for &[x, y, z] in cells {
            by_section.entry([x >> 4, y >> 4, z >> 4]).or_default().push(BlockUpdate {
                position: BlockCoordinates { x, y, z },
                runtime_id: wire_id,
                flags: 3,
                entity_unique_id: 0,
                transition_type: BlockUpdateTransitionType::Create,
            });
        }
        for ([x, y, z], blocks) in by_section {
            self.send(&UpdateSubchunkBlocks { x, y, z, blocks, extra: vec![] });
        }
    }

    /// A square of `wire_id` `half` cells around `feet`, one below it; returns the cells.
    pub fn lay_floor(&self, feet: [i32; 3], half: i32, wire_id: u32) -> Vec<[i32; 3]> {
        let cells: Vec<_> = (-half..=half).flat_map(|dx| (-half..=half).map(move |dz| [feet[0] + dx, feet[1] - 1, feet[2] + dz])).collect();
        self.set_blocks(&cells, wire_id);
        cells
    }

    /// The script's network id for item `name` (`minecraft:stone`), from the `ItemRegistry` it sent.
    pub fn item_id(&self, name: &str) -> Option<i16> {
        let sent = self.sent();
        let registry: ItemRegistry = sent.iter().find(|r| r.packet.id == ItemRegistry::ID)?.packet.decode().ok()?;
        registry.itemstates.into_iter().find(|i| i.name == name).map(|i| i.runtime_id)
    }

    /// Puts `count` of item `name` in main-inventory slot `slot` (0-8: the hotbar), with stack id `stack_id`.
    pub fn give(&self, slot: u32, name: &str, count: u16, stack_id: i32) {
        let mut p: InventorySlot = fixture(include_str!("../../acacia-proto/tests/fixtures/packets/inventory_slot.hex"));
        let id = self.item_id(name).unwrap_or_else(|| panic!("{name} is not in the script's item registry"));
        (p.window_id, p.slot, p.container, p.storage_item) = (WindowIDVarint::Inventory, slot, None, None);
        let block = BlockRegistry::vanilla().find(name, "").map_or(0, |b| BlockRegistry::vanilla().network_hash(b));
        (p.item.network_id, p.item.count, p.item.metadata) = (id, count, 0);
        (p.item.has_stack_id, p.item.stack_id, p.item.block_runtime_id) = (true, Some(stack_id), block);
        self.send(&p);
    }

    /// The feet position the script put the player at: its last ready `Respawn` or `MovePlayer`, else `StartGame`'s
    /// (BDS's is a y = 32767 placeholder until then).
    pub fn spawn_feet(&self) -> Option<[f32; 3]> {
        let sent = self.sent();
        let start: StartGame = sent.iter().find(|r| r.packet.id == StartGame::ID)?.packet.decode().ok()?;
        let me = start.runtime_entity_id;
        let placed = sent.iter().filter_map(|r| match r.packet.id {
            Respawn::ID => r.packet.decode::<Respawn>().ok().filter(|p| p.state == RESPAWN_READY).map(|p| p.position),
            MovePlayer::ID => r.packet.decode::<MovePlayer>().ok().filter(|p| u64::from(p.runtime_id) == me).map(|p| p.position),
            _ => None,
        });
        let p = placed.last().unwrap_or(start.player_position);
        Some([p.x, p.y - EYE_HEIGHT, p.z])
    }

    /// Waits until the server has answered sub-chunk requests for every chunk column in `columns`.
    pub async fn columns_sent(&self, columns: &[(i32, i32)]) -> io::Result<()> {
        self.wait_for(|s| columns.iter().all(|c| requested(&s.received).contains(c)).then_some(())).await
    }
}

/// The chunk columns the client asked sub-chunks for (each request is answered as it arrives).
fn requested(received: &[Received]) -> HashSet<(i32, i32)> {
    let requests = received.iter().filter(|r| r.packet.id == SubchunkRequest::ID).filter_map(|r| r.packet.decode::<SubchunkRequest>().ok());
    requests.flat_map(|q| q.requests.into_iter().map(move |o| (q.origin.x + i32::from(o.x), q.origin.z + i32::from(o.z)))).collect()
}
