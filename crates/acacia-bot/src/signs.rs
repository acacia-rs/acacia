//! Writing signs. The server opens the editor (`OpenSign`) after a sign is placed or clicked, one
//! side at a time, and the client answers with the whole sign block entity in `BlockActorData`
//! when the editor closes. NBT layout, sources and captured timing: docs/research/survival-signs-beds.md §3.

use std::time::Duration;

use acacia_client::proto::nbt::{Nbt, Str, Value};
use acacia_client::proto::packets::BlockEntityData;
use acacia_client::proto::RawPacket;
use acacia_physics::BlockPos;

use crate::human::CLICK;
use crate::interact::facing_face;
use crate::interact::wire::block_coordinates;
use crate::state::SignEditor;
use crate::{ActionError, Bot};

const EDITOR_TIMEOUT: Duration = Duration::from_secs(2);
/// Dragonfly rejects longer text; PowerNukkitX keeps at most 4 lines.
const MAX_TEXT_BYTES: usize = 256;
const MAX_LINES: usize = 4;
/// Opaque black, the default text colour (ARGB).
const BLACK: i32 = -16_777_216;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignSide {
    Front,
    Back,
}

impl Bot {
    /// Writes `text` (lines separated by `\n`) on one side of the sign at `pos`. Uses the editor
    /// the server opened when the sign was placed; otherwise clicks the sign (with an item that is
    /// not dye, ink or honeycomb) and waits for one. The server picks the side from where the
    /// player stands: if it opens the other side, that editor is closed unchanged and the result
    /// is `NotPossible`.
    pub async fn write_sign(&mut self, pos: BlockPos, side: SignSide, text: &str) -> Result<(), ActionError> {
        if text.len() > MAX_TEXT_BYTES || text.split('\n').count() > MAX_LINES {
            return Err(ActionError::NotPossible(format!("sign text is over {MAX_LINES} lines or {MAX_TEXT_BYTES} bytes")));
        }
        if self.state.block_entities.sign(pos).is_some_and(|n| matches!(n.value.get("IsWaxed"), Some(Value::Byte(1)))) {
            return Err(ActionError::NotPossible(format!("the sign at {pos:?} is waxed")));
        }
        let editor = match self.state.signs.editor.filter(|e| e.position == pos) {
            Some(editor) => editor,
            None => self.open_sign_editor(pos).await?,
        };
        let front = side == SignSide::Front;
        if editor.front != front {
            let pause = self.human.between(CLICK);
            self.pause(pause).await?;
            let current = self.state.block_entities.sign_text(pos, editor.front).unwrap_or_default().to_owned();
            self.close_sign_editor(pos, editor.front, &current);
            let opened = if editor.front { "front" } else { "back" };
            return Err(ActionError::NotPossible(format!("the server opened the {opened} side of the sign at {pos:?}")));
        }
        let typing = self.human.typing(text.chars().count());
        self.pause(typing).await?;
        self.close_sign_editor(pos, front, text);
        Ok(())
    }

    /// Closes the editor the server opened with `text` on its side, at once: for a player who
    /// typed it, where [`Self::write_sign`] takes a person's time.
    pub fn write_open_sign(&mut self, text: &str) -> Result<(), ActionError> {
        let editor = self.state.signs.editor.ok_or_else(|| ActionError::NotPossible("no sign editor is open".into()))?;
        self.close_sign_editor(editor.position, editor.front, text);
        Ok(())
    }

    async fn open_sign_editor(&mut self, pos: BlockPos) -> Result<SignEditor, ActionError> {
        self.state.signs.editor = None;
        let face = facing_face(self.eye_position(), pos);
        self.use_item_on_block(pos, face).await?;
        let opened = |bot: &Bot, _: &RawPacket| bot.state.signs.editor.filter(|e| e.position == pos);
        match self.wait_until(EDITOR_TIMEOUT, opened).await {
            Err(ActionError::Timeout) => Err(ActionError::Rejected(format!("no sign editor opened for {pos:?}"))),
            other => other,
        }
    }

    /// Sends the sign with `text` on the editor's side.
    fn close_sign_editor(&mut self, pos: BlockPos, front: bool, text: &str) {
        let hanging = self.block_at(pos).is_some_and(|(_, s)| s.name.contains("hanging_sign"));
        let edit = SideEdit { front, text };
        let nbt = sign_nbt(self.state.block_entities.sign(pos), pos, hanging, &edit, self.state.player.unique_entity_id);
        self.client.send(&BlockEntityData { position: block_coordinates(pos), nbt: (&nbt).into() });
        self.reflexes.next_legacy_id();
        self.state.signs.editor = None;
        self.state.block_entities.insert(pos, nbt);
    }
}

/// One side's new text.
pub(crate) struct SideEdit<'a> {
    pub front: bool,
    pub text: &'a str,
}

/// The sign block entity the client sends: the server's copy (from `BlockActorData` or chunk data,
/// see [`crate::state::BlockEntityTracking`]) with only the edited side's `Text` changed (vanilla
/// leaves `TextOwner` and `LockedForEditingBy` as the server sent them), or a fresh sign shaped
/// like BDS's, keys in byte order. `editor`: the unique id written to `LockedForEditingBy` on a fresh sign.
pub(crate) fn sign_nbt(base: Option<&Nbt>, pos: BlockPos, hanging: bool, edit: &SideEdit, editor: i64) -> Nbt {
    let mut root = match base.map(|n| &n.value) {
        Some(Value::Compound(entries)) => entries.clone(),
        _ => fresh_sign(pos, hanging, editor),
    };
    let side = side_mut(&mut root, if edit.front { "FrontText" } else { "BackText" });
    set(side, "Text", Value::String(edit.text.into()));
    Nbt { name: String::new(), value: sorted(Value::Compound(root)) }
}

fn fresh_side() -> Value {
    Value::Compound(vec![
        ("FilteredText".into(), Value::String(Str::default())),
        ("HideGlowOutline".into(), Value::Byte(0)),
        ("IgnoreLighting".into(), Value::Byte(0)),
        ("PersistFormatting".into(), Value::Byte(1)),
        ("SignTextColor".into(), Value::Int(BLACK)),
        ("Text".into(), Value::String(Str::default())),
        ("TextOwner".into(), Value::String(Str::default())),
    ])
}

fn fresh_sign([x, y, z]: BlockPos, hanging: bool, editor: i64) -> Vec<(Str, Value)> {
    let id = if hanging { "HangingSign" } else { "Sign" };
    vec![
        ("BackText".into(), fresh_side()),
        ("BlockEntityVersion".into(), Value::Int(0)),
        ("FrontText".into(), fresh_side()),
        ("IsWaxed".into(), Value::Byte(0)),
        ("LockedForEditingBy".into(), Value::Long(editor)),
        ("id".into(), Value::String(id.into())),
        ("x".into(), Value::Int(x)),
        ("y".into(), Value::Int(y)),
        ("z".into(), Value::Int(z)),
    ]
}

fn side_mut<'a>(root: &'a mut Vec<(Str, Value)>, key: &str) -> &'a mut Vec<(Str, Value)> {
    let index = match root.iter().position(|(k, v)| k == key && matches!(v, Value::Compound(_))) {
        Some(i) => i,
        None => {
            root.retain(|(k, _)| k != key);
            root.push((key.into(), fresh_side()));
            root.len() - 1
        }
    };
    let Value::Compound(side) = &mut root[index].1 else { unreachable!("found or inserted as a compound") };
    side
}

fn set(entries: &mut Vec<(Str, Value)>, key: &str, value: Value) {
    match entries.iter_mut().find(|(k, _)| k == key) {
        Some(entry) => entry.1 = value,
        None => entries.push((key.into(), value)),
    }
}

/// Bedrock's `CompoundTag` is a sorted map, so the client writes compound keys in byte order.
fn sorted(value: Value) -> Value {
    match value {
        Value::Compound(mut entries) => {
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Compound(entries.into_iter().map(|(k, v)| (k, sorted(v))).collect())
        }
        Value::List(mut list) => {
            list.items = list.items.into_iter().map(sorted).collect();
            Value::List(list)
        }
        other => other,
    }
}

#[cfg(test)]
mod tests;
