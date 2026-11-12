//! Shulker lids that shove the local player or its vehicle. Both tick after it, as vanilla's later entity and block entity ticks do: a
//! shulker's peek (`Shulker.onPeekAmountChange`, `MoverType.SHULKER`) and an opening shulker box
//! (`ShulkerBoxBlockEntity.updateAnimation` and `moveCollidedEntities`, `MoverType.SHULKER_BOX`), whose lid is also a
//! collision box (`ShulkerBoxBlock.getShape`).

use std::collections::HashMap;

use acacia_java_physics::{Aabb, Blocks, shove};
use acacia_java_proto::actions::BlockPos;
use acacia_java_proto::entity_types::ENTITY_SHAPES;
use acacia_java_proto::{Error, Wire, ids};
use acacia_registry::value::Direction;
use acacia_registry::{BlockTag, prop};

use crate::driven::Driven;
use crate::entity::Entity;
use crate::join::Client;
use crate::lid_shape::{opposite, physical_peek, progress_box, step};
use crate::mount_data::short_name;
use crate::mount_drive::controlled_by;
use crate::world::{Mover, World};

use ids::play::clientbound as from;

/// `triggerEvent`'s lid event; its data is how many have the box open.
const LID_EVENT: u8 = 1;
const STEP: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Animation {
    Closed,
    Opening,
    Opened,
    Closing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BoxLid {
    animation: Animation,
    progress: f32,
    old: f32,
}

/// The shulker boxes whose lid an event set moving, by block position.
#[derive(Clone, Debug, Default)]
pub(crate) struct Lids {
    boxes: HashMap<[i32; 3], BoxLid>,
}

/// The direction a shulker box at `pos` opens towards, if one stands there.
fn box_facing(world: &World, pos: [i32; 3]) -> Option<i32> {
    let state = world.block_state(pos);
    if !state.block().is(BlockTag::ShulkerBoxes) {
        return None;
    }
    Some(match state.get(prop::Facing)? {
        Direction::Down => 0,
        Direction::Up => 1,
        Direction::North => 2,
        Direction::South => 3,
        Direction::West => 4,
        Direction::East => 5,
    })
}

fn bottom_center(pos: [i32; 3]) -> [f64; 3] {
    [f64::from(pos[0]) + 0.5, f64::from(pos[1]), f64::from(pos[2]) + 0.5]
}

impl BoxLid {
    /// `updateAnimation`; true while it opens, when it pushes.
    fn tick(&mut self) -> bool {
        self.old = self.progress;
        match self.animation {
            Animation::Closed => self.progress = 0.0,
            Animation::Opened => self.progress = 1.0,
            Animation::Opening => {
                self.progress += STEP;
                if self.progress >= 1.0 {
                    (self.animation, self.progress) = (Animation::Opened, 1.0);
                }
                return true;
            }
            Animation::Closing => {
                self.progress -= STEP;
                if self.progress <= 0.0 {
                    (self.animation, self.progress) = (Animation::Closed, 0.0);
                }
            }
        }
        false
    }
}

impl Lids {
    /// `ClientboundBlockEventPacket` on a shulker box: `level.blockEvent` asks the block at the position, not the
    /// packet's block.
    pub(crate) fn apply(&mut self, world: &World, id: i32, mut input: &[u8]) -> Result<(), Error> {
        if id == from::LOGIN {
            self.boxes.clear();
        }
        if id != from::BLOCK_EVENT {
            return Ok(());
        }
        let input = &mut input;
        let (BlockPos(pos), event, open) = (BlockPos::read(input)?, u8::read(input)?, u8::read(input)?);
        if event != LID_EVENT || box_facing(world, pos).is_none() {
            return Ok(());
        }
        let lid = self.boxes.entry(pos).or_insert(BoxLid { animation: Animation::Closed, progress: 0.0, old: 0.0 });
        match open {
            0 => lid.animation = Animation::Closing,
            1 => lid.animation = Animation::Opening,
            _ => {}
        }
        Ok(())
    }

    /// The open lids' block shapes (`getBoundingBox`, at `getProgress(1.0F)`), for the player's collisions.
    pub(crate) fn boxes(&self, world: &World) -> Vec<Aabb> {
        let shown = |l: &BoxLid| l.old + (l.progress - l.old);
        self.boxes
            .iter()
            .filter(|(_, l)| shown(l) > 0.0)
            .filter_map(|(&pos, l)| Some(progress_box(1.0, box_facing(world, pos)?, -1.0, 0.5 * shown(l), bottom_center(pos))))
            .collect()
    }
}

impl Client {
    /// After the player's tick and report: shulkers' lids, then shulker boxes', each shoving the player where it
    /// sweeps. A seated player is left to its seat.
    pub(crate) fn move_lids(&mut self) {
        let mut pushes = Vec::new();
        for e in self.world.entities.values_mut().filter(|e| e.is("minecraft:shulker")) {
            if !e.shape.update_peek() {
                continue;
            }
            let (peek, scale) = (e.shape.peek, e.scale());
            let (now, before) = (physical_peek(peek.current), physical_peek(peek.old));
            let push = (now - before) * scale;
            if push > 0.0 {
                let direction = opposite(peek.face);
                let sweep = progress_box(scale, direction, before, now, e.position);
                pushes.push((sweep, step(direction).map(|s| f64::from(push * s as f32))));
            }
        }
        let lids = &mut self.lids.boxes;
        lids.retain(|&pos, _| box_facing(&self.world, pos).is_some());
        for (&pos, lid) in lids.iter_mut() {
            if !lid.tick() {
                continue;
            }
            let Some(direction) = box_facing(&self.world, pos) else { continue };
            let sweep = progress_box(1.0, direction, lid.old, lid.progress, bottom_center(pos));
            let s = step(direction);
            pushes.push((sweep, [0, 1, 2].map(|a| (sweep.max[a] - sweep.min[a] + 0.01) * s[a])));
        }
        lids.retain(|_, l| l.animation != Animation::Closed);
        for (sweep, delta) in pushes {
            self.shove(&sweep, delta);
        }
    }

    /// `Entity.move(SHULKER…)` on each entity in `sweep` whose move the client keeps: the player, and its vehicle.
    /// A driven vehicle moves as simulated (and reports it); another moves where it shows, the seat following next
    /// tick. A pushed passenger moves too, until `positionRider` seats it again.
    fn shove(&mut self, sweep: &Aabb, delta: [f64; 3]) {
        let player_hit = self.movement.player.bounding_box().intersects(sweep);
        let Some(vehicle) = self.movement.riding.as_ref().map(|r| r.vehicle) else {
            if player_hit {
                self.movement.player.move_pushed(&self.world, delta);
            }
            return;
        };
        let riders = self.world.entities.get(&vehicle).map_or(Vec::new(), |e| e.passengers.clone());
        let skip = [&[vehicle][..], &riders].concat();
        let mut solid = self.world.collision_boxes(Mover::Vehicle, &skip);
        solid.extend(self.lids.boxes(&self.world));
        match self.movement.riding.as_mut().and_then(|r| r.driven.as_mut()) {
            Some(Driven::Boat(boat)) if boat.bounding_box().intersects(sweep) => boat.move_pushed(&self.world, delta),
            Some(Driven::Mount(mount)) if mount.body.bounding_box().intersects(sweep) => mount.body.move_pushed(&self.world, delta),
            Some(_) => {}
            None => {
                if let Some(e) = self.world.entities.get(&vehicle) {
                    if let Some((w, h, _)) = e.dimensions().filter(|&(w, h, _)| Aabb::of_entity(e.position, w, h).intersects(sweep)) {
                        let moved = shove(&self.world, e.position, (w, h), e.on_ground, step_height(&self.world, e), &solid, delta);
                        self.world.entities.entry(vehicle).and_modify(|e| e.position = moved);
                    }
                }
            }
        }
        if player_hit {
            let feet = self.movement.player.pos[1];
            let mut solid = self.world.collision_boxes(Mover::Player { feet }, &skip);
            solid.extend(self.lids.boxes(&self.world));
            self.movement.player.solid_entities = solid;
            self.movement.player.move_pushed(&self.world, delta);
        }
    }
}

/// `LivingEntity.maxUpStep` of an entity the client only shows: its synced step attribute (else the type's default),
/// at least 1 while another player controls it.
fn step_height(world: &World, e: &Entity) -> f32 {
    if !ENTITY_SHAPES.get(usize::try_from(e.kind).unwrap_or(usize::MAX)).is_some_and(|s| s.living) {
        return 0.0;
    }
    let step = e.mount.step_height.map_or_else(
        || match short_name(e.kind) {
            "camel" | "camel_husk" => 1.5,
            "horse" | "donkey" | "mule" | "skeleton_horse" | "zombie_horse" | "llama" | "trader_llama" => 1.0,
            _ => 0.6,
        },
        |v| v as f32,
    );
    let driver = e.passengers.first().and_then(|p| world.entities.get(p)).filter(|p| short_name(p.kind) == "player");
    if driver.is_some_and(|p| controlled_by(e, p.mount.hands)) { step.max(1.0) } else { step }
}
