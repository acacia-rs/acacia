//! Where the camera is this frame, what it targets, and the item it carries.

use std::time::Instant;

use acacia_render::Outline;
use acacia_render::entity::EntityInstance;
use acacia_render::item::{ItemKey, hand};

use super::{App, Mode};

impl App {
    /// Moves the camera by the mode's input; in play, finds the targeted block.
    pub(super) fn steer(&mut self, now: Instant, dt: f32) -> Option<Outline> {
        match self.mode {
            Mode::Fly => {
                self.input.step(&mut self.camera, dt);
                None
            }
            Mode::Play => {
                let world = self.renderer.as_ref().and_then(|r| r.world().cloned());
                let entities = self.entities.hitboxes();
                self.play.frame(&mut self.camera, world.as_deref(), self.table.as_deref(), &entities, now);
                let mining = self.play.me.as_ref().and_then(|m| m.mining);
                self.play.target.as_ref().map(|t| {
                    let crack = mining.filter(|(block, _)| *block == t.block).map(|(_, progress)| (progress * 10.0).clamp(0.0, 9.0) as u8);
                    Outline { block: t.block, boxes: t.boxes.clone(), crack }
                })
            }
        }
    }

    /// The held item in first person, swinging with clicks.
    pub(super) fn hand(&mut self, now: Instant) -> Option<EntityInstance> {
        let stack = self.play.held_first_person().filter(|_| self.mode == Mode::Play)?.clone();
        let swing = self.play.swing(now);
        let model = self.entities.item(&ItemKey { name: stack.name, aux: stack.aux, block: stack.block })?;
        Some(hand::first_person(&model, &self.camera, swing))
    }
}
