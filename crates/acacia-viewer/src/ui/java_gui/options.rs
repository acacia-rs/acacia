//! The viewer's settings as Java's options (`Options`' getter names): the values the options
//! screens show and the changes they make. An option the viewer has no setting for shows Java's
//! default and changes nothing.

use acacia_ui::java::screen::options::value::V;

use crate::settings::Settings;

/// Option `id`'s value from the settings; `None` where the viewer has no such setting.
pub fn value(s: &Settings, id: &str) -> Option<V> {
    Some(match id {
        "guiScale" => V::Int(s.gui_scale as i32),
        "fov" => V::Int(s.fov as i32),
        "enableVsync" => V::Bool(s.vsync),
        "bobView" => V::Bool(s.view_bobbing),
        "cloudStatus" => V::Name(clouds(s).into()),
        "sound.master" => V::Num(f64::from(s.volume.min(100)) / 100.0),
        "menuBackgroundBlurriness" => V::Int(s.menu_blur as i32),
        _ => return None,
    })
}

/// `CloudStatus`' name for the cloud settings.
fn clouds(s: &Settings) -> &'static str {
    match (s.clouds, s.fancy_clouds) {
        (false, _) => "OFF",
        (true, false) => "FAST",
        (true, true) => "FANCY",
    }
}

/// Sets what option `id` stands for to `v`; false when the viewer has no such setting.
pub fn apply(s: &mut Settings, id: &str, v: &V) -> bool {
    match id {
        "guiScale" => s.gui_scale = v.i().max(0) as u32,
        "fov" => s.fov = v.i().clamp(30, 110) as u32,
        "enableVsync" => s.vsync = v.b(),
        "bobView" => s.view_bobbing = v.b(),
        "cloudStatus" => {
            (s.clouds, s.fancy_clouds) = match v.s() {
                "OFF" => (false, s.fancy_clouds),
                "FAST" => (true, false),
                _ => (true, true),
            }
        }
        "sound.master" => s.volume = (v.f() * 100.0).round().clamp(0.0, 100.0) as u32,
        "menuBackgroundBlurriness" => s.menu_blur = v.i().clamp(0, 10) as u32,
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_through_their_options() {
        let mut s = Settings::default();
        for (id, v) in [
            ("guiScale", V::Int(3)),
            ("fov", V::Int(95)),
            ("enableVsync", V::Bool(false)),
            ("bobView", V::Bool(false)),
            ("cloudStatus", V::Name("FAST".into())),
            ("sound.master", V::Num(0.42)),
            ("menuBackgroundBlurriness", V::Int(0)),
        ] {
            assert!(apply(&mut s, id, &v), "{id}");
            assert_eq!(value(&s, id), Some(v), "{id}");
        }
        assert_eq!((s.gui_scale, s.fov, s.vsync, s.view_bobbing, s.clouds, s.fancy_clouds, s.volume), (3, 95, false, false, true, false, 42));
    }

    #[test]
    fn clouds_off_keeps_the_kind_and_unknown_options_change_nothing() {
        let mut s = Settings::default();
        apply(&mut s, "cloudStatus", &V::Name("OFF".into()));
        assert_eq!((s.clouds, s.fancy_clouds), (false, true));
        assert_eq!(value(&s, "cloudStatus"), Some(V::Name("OFF".into())));
        let before = s.clone();
        assert!(!apply(&mut s, "chatOpacity", &V::Num(0.5)));
        assert_eq!((s, value(&before, "chatOpacity")), (before, None));
    }
}
