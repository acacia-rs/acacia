//! The pause menu (Esc) and the options it leads to. Options change the saved settings at once.

use acacia_ui::menu;

use super::App;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Pause,
    Options,
}

/// Largest GUI scale the options cycle through before auto.
const MAX_GUI_SCALE: u32 = 6;

impl App {
    pub(super) fn open_menu(&mut self, menu: Menu) {
        self.play.release_all();
        self.menu = Some(menu);
        self.grab(false);
    }

    /// Esc in a menu: options go back to the pause menu, which goes back to the game.
    pub(super) fn menu_back(&mut self) {
        match self.menu {
            Some(Menu::Options) => self.menu = Some(Menu::Pause),
            _ => {
                self.menu = None;
                self.grab(true);
            }
        }
    }

    pub(super) fn menu_content(&self, menu: Menu) -> (&'static str, Vec<String>) {
        let on = |b: bool| if b { "On" } else { "Off" };
        match menu {
            Menu::Pause => ("Game Menu", vec!["Back to Game".into(), "Options...".into(), "Disconnect".into()]),
            Menu::Options => {
                let s = &self.settings;
                let scale = if s.gui_scale == 0 { "Auto".to_owned() } else { s.gui_scale.to_string() };
                ("Options", vec![
                    format!("Look: {:?}", s.look),
                    format!("GUI Scale: {scale}"),
                    format!("VSync: {}", on(s.vsync)),
                    format!("Cave Culling: {}", on(s.cave_culling)),
                    "Done".into(),
                ])
            }
        }
    }

    pub(super) fn menu_click(&mut self) {
        let Some(open) = self.menu else { return };
        let (_, buttons) = self.menu_content(open);
        let ([w, h], scale) = self.gui();
        let size = [(w / scale) as f32, (h / scale) as f32];
        let Some(i) = menu::hit(buttons.len(), size, self.gui_mouse()) else { return };
        match (open, i) {
            (Menu::Pause, 0) => self.menu_back(),
            (Menu::Pause, 1) => self.menu = Some(Menu::Options),
            (Menu::Pause, _) => self.quit = true,
            (Menu::Options, 0) => self.switch_look(),
            (Menu::Options, 1) => self.settings.change_and_save(|s| s.gui_scale = (s.gui_scale + 1) % (MAX_GUI_SCALE + 1)),
            (Menu::Options, 2) => self.toggle_vsync(),
            (Menu::Options, 3) => self.settings.change_and_save(|s| s.cave_culling = !s.cave_culling),
            (Menu::Options, _) => self.menu = Some(Menu::Pause),
        }
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
        if let Some(r) = &mut self.renderer {
            r.set_vsync(self.settings.vsync);
        }
    }
}
