//! What the bot thread reports, applied to the window's state.

use std::time::Instant;

use super::App;
use crate::net::NetEvent;

impl App {
    pub(super) fn poll_net(&mut self) {
        while let Ok(event) = self.net.events.try_recv() {
            match event {
                NetEvent::World(world) => {
                    self.show_world(world);
                    if let Some(shot) = &mut self.shot {
                        shot.world_at.get_or_insert_with(Instant::now);
                    }
                }
                NetEvent::Biomes(colors) => {
                    if let Some(r) = &mut self.renderer {
                        r.set_biomes(colors);
                    }
                }
                NetEvent::Player(p) => {
                    self.player = Some(p);
                    if !self.camera_placed {
                        self.camera.position = p;
                        self.camera_placed = true;
                    }
                }
                NetEvent::Me(me) => {
                    if !me.alive && self.menu.is_none() && self.mode == super::Mode::Play {
                        self.open_menu(super::menu::Menu::Death);
                    }
                    self.bob.tick(&me, std::time::Instant::now());
                    self.play.tick(me);
                }
                NetEvent::Players(names) => self.players = names,
                NetEvent::Inventory(inventory) => self.set_inventory(inventory),
                NetEvent::Title(title) => self.ui.show_title(title),
                NetEvent::Broken { pos, block } => {
                    if let Some(r) = &mut self.renderer {
                        r.break_particles(pos, block);
                    }
                }
                NetEvent::Sound(cue) => self.audio.play(self.settings.look, &cue, &self.camera, self.settings.volume as f32 / 100.0),
                NetEvent::Form(form) => self.show_form(form),
                NetEvent::Sidebar(sidebar) => self.ui.sidebar = sidebar,
                NetEvent::Chat { sender, message, params } => self.ui.push_chat(sender.as_deref(), &message, &params),
                NetEvent::EntityModels(models) => {
                    self.entities.set_models(models.clone());
                    if let Some(r) = &mut self.renderer {
                        r.set_entity_models(models);
                    }
                }
                NetEvent::Entities(snapshot) => self.entities.push(snapshot),
                NetEvent::Time(time) => self.time = Some(time),
                NetEvent::BlockData(data) => {
                    if let Some(r) = &mut self.renderer {
                        r.set_block_data(data);
                    }
                }
                NetEvent::Status(s) => {
                    tracing::info!("{s}");
                    self.status = s;
                }
                NetEvent::Ended(reason) => {
                    tracing::warn!("session ended: {reason}");
                    if self.shot.as_ref().is_some_and(|s| !s.taken) {
                        self.failed = Some(reason.clone());
                    }
                    self.status = reason;
                }
            }
        }
    }
}
