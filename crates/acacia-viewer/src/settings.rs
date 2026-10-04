//! Settings kept between runs as JSON: `$ACACIA_SETTINGS`, else `.viewer.json` in the working directory.

use std::path::PathBuf;
use std::str::FromStr;

use acacia_render::Look;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LookChoice {
    #[default]
    Bedrock,
    Java,
}

impl LookChoice {
    pub fn look(self) -> Look {
        match self {
            LookChoice::Bedrock => Look::BEDROCK,
            LookChoice::Java => Look::JAVA,
        }
    }

    pub fn next(self) -> LookChoice {
        match self {
            LookChoice::Bedrock => LookChoice::Java,
            LookChoice::Java => LookChoice::Bedrock,
        }
    }
}

impl FromStr for LookChoice {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "bedrock" => Ok(LookChoice::Bedrock),
            "java" => Ok(LookChoice::Java),
            _ => Err(format!("unknown look {s:?}: bedrock or java")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub look: LookChoice,
    pub vsync: bool,
    pub cave_culling: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { look: LookChoice::default(), vsync: true, cave_culling: true }
    }
}

impl Settings {
    fn path() -> PathBuf {
        std::env::var_os("ACACIA_SETTINGS").map_or_else(|| PathBuf::from(".viewer.json"), PathBuf::from)
    }

    /// Defaults when the file is missing or unreadable.
    pub fn load() -> Settings {
        let path = Settings::path();
        let Ok(text) = std::fs::read_to_string(&path) else { return Settings::default() };
        serde_json::from_str(&text).unwrap_or_else(|e| {
            tracing::warn!(%e, path = %path.display(), "settings");
            Settings::default()
        })
    }

    pub fn change_and_save(&mut self, change: impl FnOnce(&mut Settings)) {
        change(self);
        let path = Settings::path();
        let text = serde_json::to_string_pretty(self).expect("settings serialize");
        if let Err(e) = std::fs::write(&path, text) {
            tracing::warn!(%e, path = %path.display(), "settings not saved");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_take_defaults() {
        let s: Settings = serde_json::from_str(r#"{"look":"java"}"#).unwrap();
        assert_eq!(s, Settings { look: LookChoice::Java, ..Settings::default() });
    }

    #[test]
    fn look_names_match_the_saved_form() {
        for look in [LookChoice::Bedrock, LookChoice::Java] {
            let saved = serde_json::to_string(&look).unwrap();
            assert_eq!(saved.trim_matches('"').parse::<LookChoice>(), Ok(look));
        }
    }
}
