//! The pause menu (Esc), the options it leads to, and the death and disconnected screens. Options
//! change the saved settings at once. The Bedrock look's menus are here; the Java look draws and
//! routes Java's own screens (java_menu.rs).

use acacia_ui::menu;

use super::App;
use crate::control::Command;
use crate::settings::LookChoice;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Pause,
    Options,
    /// Shown while the player is dead; Esc does not close it.
    Death,
    /// Java's "quit to title?" over the death screen.
    TitleConfirm,
    /// The session ended (the Java look's; the reason is in `JavaScreens`).
    Disconnected,
}

/// Largest GUI scale the Bedrock options cycle through before auto.
const MAX_GUI_SCALE: u32 = 6;

impl App {
    pub(super) fn open_menu(&mut self, menu: Menu) {
        self.play.release_all();
        self.java.opened(menu);
        self.menu = Some(menu);
        self.grab(false);
    }

    /// Esc in a menu: options go back to the pause menu, which goes back to the game.
    pub(super) fn menu_back(&mut self) {
        match self.menu {
            Some(Menu::Options) if self.java.pages.len() > 1 => {
                self.java.pages.pop();
                self.java.interaction = Default::default();
            }
            Some(Menu::Options) => self.open_menu(Menu::Pause),
            Some(Menu::Death | Menu::TitleConfirm | Menu::Disconnected) => {}
            _ => {
                self.menu = None;
                self.grab(true);
            }
        }
    }

    /// The Bedrock look's menu: its title and buttons.
    pub(super) fn menu_content(&self, menu: Menu) -> (&'static str, Vec<String>) {
        let on = |b: bool| if b { "On" } else { "Off" };
        match menu {
            // The pause screen has the logo for a title (an empty one, acacia_ui::menu).
            Menu::Pause => ("", vec!["Resume Game".into(), "Settings".into(), "Save & Quit".into()]),
            Menu::Death | Menu::TitleConfirm => ("You Died!", vec!["Respawn".into(), "Disconnect".into()]),
            Menu::Disconnected => ("Disconnected", vec!["Quit".into()]),
            Menu::Options => {
                let s = &self.settings;
                let scale = if s.gui_scale == 0 { "Auto".to_owned() } else { s.gui_scale.to_string() };
                ("Options", vec![
                    format!("Look: {:?}", s.look),
                    format!("GUI Scale: {scale}"),
                    format!("VSync: {}", on(s.vsync)),
                    format!("Cave Culling: {}", on(s.cave_culling)),
                    format!("Sound: {}%", s.volume),
                    format!("FOV: {}", s.fov),
                    format!("View Bobbing: {}", on(s.view_bobbing)),
                    format!("Clouds: {}", if !s.clouds { "OFF" } else if s.fancy_clouds { "Fancy" } else { "Fast" }),
                    "Done".into(),
                ])
            }
        }
    }

    /// A left click in the Bedrock look's menu.
    pub(super) fn menu_click(&mut self) {
        let Some(open) = self.menu else { return };
        let (title, buttons) = self.menu_content(open);
        let ([w, h], scale) = self.gui();
        let size = [(w / scale) as f32, (h / scale) as f32];
        let theme = self.ui.theme(LookChoice::Bedrock);
        let Some(i) = menu::hit(theme, title, buttons.len(), size, self.gui_mouse()) else { return };
        match (open, i) {
            (Menu::Pause, 0) => self.menu_back(),
            (Menu::Pause, 1) => self.menu = Some(Menu::Options),
            (Menu::Pause | Menu::Disconnected, _) | (Menu::Death | Menu::TitleConfirm, 1) => self.quit = true,
            // The screen stays until the server has the player alive again (events.rs).
            (Menu::Death | Menu::TitleConfirm, _) => self.respawn(),
            (Menu::Options, 0) => self.switch_look(),
            (Menu::Options, 1) => self.settings.change_and_save(|s| s.gui_scale = (s.gui_scale + 1) % (MAX_GUI_SCALE + 1)),
            (Menu::Options, 2) => self.toggle_vsync(),
            (Menu::Options, 3) => self.settings.change_and_save(|s| s.cave_culling = !s.cave_culling),
            (Menu::Options, 4) => self.settings.change_and_save(|s| s.volume = (s.volume / 25 * 25 + 25) % 125),
            // Java's range, 30 to 110, in steps of 10.
            (Menu::Options, 5) => self.settings.change_and_save(|s| s.fov = (s.fov.clamp(30, 110) / 10 * 10 - 20) % 90 + 30),
            (Menu::Options, 6) => self.settings.change_and_save(|s| s.view_bobbing = !s.view_bobbing),
            (Menu::Options, 7) => self.settings.change_and_save(|s| (s.clouds, s.fancy_clouds) = match (s.clouds, s.fancy_clouds) {
                // Java's order: fancy, fast, off.
                (true, true) => (true, false),
                (true, false) => (false, true),
                (false, _) => (true, true),
            }),
            (Menu::Options, _) => self.menu = Some(Menu::Pause),
        }
    }

    pub(super) fn respawn(&mut self) {
        let _ = self.net.commands.send(Command::Respawn);
    }

    /// The other look, saved; the world is meshed anew in it.
    pub(super) fn switch_look(&mut self) {
        self.settings.change_and_save(|s| s.look = s.look.next());
        if let Some(world) = self.renderer.as_ref().and_then(|r| r.world().cloned()) {
            self.show_world(world);
        }
    }

    pub(super) fn toggle_vsync(&mut self) {
        self.settings.change_and_save(|s| s.vsync = !s.vsync);
        self.apply_vsync();
    }

    pub(super) fn apply_vsync(&mut self) {
        if let Some(r) = &mut self.renderer {
            r.set_vsync(self.settings.vsync);
        }
    }
}
