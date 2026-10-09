//! Where the camera is this frame, what it targets, and the item it carries.

use std::time::Instant;

use acacia_render::Outline;
use acacia_render::entity::EntityInstance;
use acacia_render::item::{ItemKey, hand};
use acacia_ui::nametags::Tag;

use super::{App, Mode};
use crate::keyscript::Step;

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
                // The script's clock starts with the player's first tick: joining takes 4 to 9 s.
                let script = self.key_script.as_mut().filter(|_| self.play.me.is_some());
                for step in script.map(|s| s.due(now)).unwrap_or_default() {
                    match step {
                        Step::Key(key, pressed) => self.key(key, pressed),
                        Step::Type(text) => self.text(&text, true),
                        Step::Wheel(notches) => self.scroll(notches),
                        Step::Click(at) => {
                            self.mouse = at;
                            self.button(winit::event::MouseButton::Left, true);
                            self.button(winit::event::MouseButton::Left, false);
                        }
                        // Straight to the player: an unattended window never grabs the mouse.
                        Step::Use(pressed) => self.play.button(winit::event::MouseButton::Right, pressed),
                    }
                }
                self.play.frame(&mut self.camera, world.as_deref(), self.table.as_deref(), &entities, now);
                if self.auto_attack && self.play.target.is_some() && !self.play.attacking() {
                    self.play.button(winit::event::MouseButton::Left, true);
                }
                let mining = self.play.me.as_ref().and_then(|m| m.mining);
                self.play.target.as_ref().map(|t| {
                    let crack = mining.filter(|(block, _)| *block == t.block).map(|(_, progress)| (progress * 10.0).clamp(0.0, 9.0) as u8);
                    Outline { block: t.block, boxes: t.boxes.clone(), crack }
                })
            }
        }
    }

    /// Name tags projected onto the screen (`gui` GUI pixels big, `scale` window pixels each):
    /// Java draws a font pixel 0.025 blocks big in the world; tags beyond 64 blocks or behind
    /// the camera are left out.
    pub(super) fn name_tags(&self, gui: [f32; 2], scale: f32) -> Vec<Tag> {
        let view_proj = self.camera.view_proj();
        let pixels_per_block = gui[1] * scale / 2.0 / (self.camera.fov_y / 2.0).tan();
        self.entities
            .name_tags()
            .into_iter()
            .filter_map(|(text, at)| {
                let relative = (at - self.camera.position).as_vec3();
                let distance = relative.length();
                let clip = view_proj * relative.extend(1.0);
                if clip.w <= 0.1 || distance > 64.0 {
                    return None;
                }
                let (x, y) = (clip.x / clip.w, clip.y / clip.w);
                let size = 0.025 * pixels_per_block / distance / scale;
                Some(Tag { text: text.to_owned(), x: (x + 1.0) / 2.0 * gui[0], y: (1.0 - y) / 2.0 * gui[1], scale: size })
            })
            .collect()
    }

    /// The items in both hands in first person: the main one swings with clicks and is used, and
    /// a shield is raised while sneaking (Bedrock's way to block; the off hand's before the main one's).
    pub(super) fn hand(&mut self, now: Instant) -> Vec<EntityInstance> {
        if self.mode != Mode::Play {
            return Vec::new();
        }
        let [main, off] = self.play.held_first_person().map(|stack| stack.cloned());
        let swing = self.play.swing(now);
        let using = main.as_ref().and_then(|stack| using(&stack.name, self.play.item_use_secs(now)? * 20.0));
        let off_shield = off.as_ref().is_some_and(|stack| stack.name == SHIELD);
        let sneaking = self.play.sneaking();
        let mut out = Vec::new();
        for (left, stack) in [(false, main), (true, off)] {
            let Some(stack) = stack else { continue };
            let Some(model) = self.entities.item(&ItemKey { name: stack.name.clone(), aux: stack.aux, block: stack.block }, stack.enchanted) else { continue };
            let display = hand::Display::first_person(&stack.name, model.form, left, sneaking && (left || !off_shield));
            let (swing, using) = if left { (0.0, None) } else { (swing, using) };
            out.push(hand::first_person(&model, display, &self.camera, left, swing, using));
        }
        out
    }
}

const SHIELD: &str = "minecraft:shield";

/// How an item in use looks held: a bow drawn, or food and drink eaten (again each `duration`).
fn using(name: &str, ticks: f32) -> Option<hand::Using> {
    if matches!(name, "minecraft:bow" | "minecraft:crossbow") {
        return Some(hand::Using::Bow { ticks });
    }
    let duration = acacia_bot::survival::use_ticks(name)? as f32;
    Some(hand::Using::Eat { ticks: ticks % duration, duration })
}
