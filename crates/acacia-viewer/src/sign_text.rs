//! Sign text: the sides read from the block entities on the bot's thread, and their layout in the
//! shown look's font for the renderer.

use std::collections::HashMap;
use std::sync::Arc;

use acacia_bot::proto::nbt::{Nbt, Value};
use acacia_bot::state::BlockEntities;
use acacia_render::Renderer;
use acacia_render::sign_text::{SignText, SignTextMap};
use acacia_ui::signs::{self, Side};
use acacia_ui::theme::Theme;

use crate::settings::LookChoice;

/// A sign's front and back, and whether it hangs.
#[derive(Debug, Clone, PartialEq)]
pub struct Sign {
    pub sides: [Side; 2],
    pub hanging: bool,
}

/// The written signs by block position.
pub type Texts = HashMap<[i32; 3], Sign>;

pub fn snapshot(entities: &BlockEntities) -> Texts {
    entities.iter().filter_map(|(pos, nbt)| Some((*pos, sign(nbt)?))).collect()
}

/// `None` for other block entities and for signs without text.
fn sign(nbt: &Nbt) -> Option<Sign> {
    let Some(Value::String(id)) = nbt.value.get("id") else { return None };
    let hanging = match &**id {
        "Sign" => false,
        "HangingSign" => true,
        _ => return None,
    };
    // Signs from before 1.20 keep the front's fields in the root.
    let front = nbt.value.get("FrontText").unwrap_or(&nbt.value);
    let sides = [side(front), nbt.value.get("BackText").map(side).unwrap_or_default()];
    sides.iter().any(|s| !s.text.trim().is_empty()).then_some(Sign { sides, hanging })
}

fn side(nbt: &Value) -> Side {
    let flag = |key: &str| matches!(nbt.get(key), Some(Value::Byte(1)));
    let text = match nbt.get("Text") {
        Some(Value::String(text)) => text.to_string(),
        _ => String::new(),
    };
    let colour = match nbt.get("SignTextColor") {
        Some(Value::Int(argb)) => *argb as u32 & 0xFF_FFFF,
        _ => 0,
    };
    Side { text, colour, glowing: flag("IgnoreLighting"), outline: !flag("HideGlowOutline") }
}

/// What the window knows of the signs, and the look they are laid out for.
#[derive(Default)]
pub struct Signs {
    texts: Arc<Texts>,
    /// `None` when the texts changed since.
    laid: Option<LookChoice>,
}

impl Signs {
    pub fn set(&mut self, texts: Arc<Texts>) {
        self.texts = texts;
        self.laid = None;
    }

    /// Hands the renderer the texts in `theme`'s font when they or the look changed.
    pub fn feed(&mut self, look: LookChoice, theme: &Theme, renderer: &mut Renderer) {
        if self.laid == Some(look) {
            return;
        }
        self.laid = Some(look);
        renderer.set_sign_text(Arc::new(lay_out(&self.texts, theme)));
    }
}

fn lay_out(texts: &Texts, theme: &Theme) -> SignTextMap {
    let laid = |sign: &Sign| {
        let metrics = if sign.hanging { signs::HANGING } else { signs::SIGN };
        let [front, back] = &sign.sides;
        Arc::new(SignText { front: signs::layout(theme, front, metrics), back: signs::layout(theme, back, metrics) })
    };
    texts.iter().map(|(pos, sign)| (*pos, laid(sign))).collect()
}

#[cfg(test)]
mod tests {
    use acacia_bot::proto::nbt::Str;

    use super::*;

    fn compound(entries: Vec<(&str, Value)>) -> Value {
        Value::Compound(entries.into_iter().map(|(k, v)| (Str::from(k), v)).collect())
    }

    fn text(text: &str, colour: i32, glow: i8) -> Value {
        compound(vec![("Text", Value::String(text.into())), ("SignTextColor", Value::Int(colour)), ("IgnoreLighting", Value::Byte(glow)), ("HideGlowOutline", Value::Byte(0))])
    }

    #[test]
    fn sides_come_from_the_block_entity() {
        let nbt = |value| Nbt { name: String::new(), value };
        let written = nbt(compound(vec![("id", Value::String("HangingSign".into())), ("FrontText", text("a\nb", -16_777_216, 0)), ("BackText", text("c", -5_231_066, 1))]));
        let sign = sign(&written).unwrap();
        assert!(sign.hanging);
        assert_eq!(sign.sides[0], Side { text: "a\nb".into(), colour: 0, glowing: false, outline: true });
        assert_eq!(sign.sides[1], Side { text: "c".into(), colour: 0xB02E26, glowing: true, outline: true });
        let blank = nbt(compound(vec![("id", Value::String("Sign".into())), ("FrontText", text("", 0, 0)), ("BackText", text(" ", 0, 0))]));
        assert_eq!(super::sign(&blank), None);
        let old = nbt(compound(vec![("id", Value::String("Sign".into())), ("Text", Value::String("old".into()))]));
        assert_eq!(super::sign(&old).unwrap().sides[0].text, "old");
        assert_eq!(super::sign(&nbt(compound(vec![("id", Value::String("Chest".into()))]))), None);
    }
}
