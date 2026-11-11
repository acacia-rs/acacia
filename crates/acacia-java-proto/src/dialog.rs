//! Dialogs (`ClientboundShowDialogPacket`) read from their inline NBT, with vanilla's defaults. Only what a bot
//! answers is kept: text, inputs, buttons and their actions. Research: docs/java-edition.md "Forms".

use crate::tag::Tag;
use crate::{Error, Wire, read_varint, write_varint};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    Notice,
    Confirmation,
    MultiAction,
    DialogList,
    ServerLinks,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dialog {
    pub kind: DialogKind,
    pub title: String,
    /// The plain-message bodies' text.
    pub body: Vec<String>,
    pub inputs: Vec<Input>,
    /// What a reply picks from: notice's one button, confirmation's yes then no, multi_action's actions, one per
    /// sub-dialog of a dialog_list; server_links has none (its links are client-only).
    pub buttons: Vec<Button>,
    /// What Escape runs: notice's button, confirmation's no, else the exit action; absent when Escape does nothing.
    pub cancel: Option<Button>,
    pub can_close_with_escape: bool,
    /// `after_action: none`: the dialog stays open after a button.
    pub stays_open: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Button {
    pub label: String,
    pub action: Option<Action>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    RunCommand(String),
    /// A command template with `$(key)` macros for the inputs.
    DynamicRunCommand(String),
    Custom { id: String, payload: Option<Tag> },
    /// Sends `additions` plus every input's value.
    DynamicCustom { id: String, additions: Option<Tag> },
    /// Opens a dialog locally; nothing is sent.
    ShowDialog(Box<Dialog>),
    /// Links, clipboard, suggestions and registry dialogs the bot cannot open: nothing is sent.
    ClientOnly,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub key: String,
    pub label: String,
    pub control: Control,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Control {
    Text { initial: String, max_length: usize },
    Boolean { initial: bool, on_true: String, on_false: String },
    /// `(id, shown text)` per option.
    SingleOption { options: Vec<(String, String)>, initial: usize },
    NumberRange { start: f32, end: f32, initial: f32, step: Option<f32> },
}

/// `ServerboundCustomClickActionPacket`: the payload goes as a length-prefixed root tag, 0 for none.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomClickAction {
    pub id: String,
    pub payload: Option<Tag>,
}

impl Wire for CustomClickAction {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let id = String::read(input)?;
        let len = crate::wire::read_len(input)?;
        let mut payload = crate::wire::take(input, len)?;
        Ok(Self { id, payload: if len == 0 { None } else { Tag::read_root(&mut payload)? } })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.id.write(out);
        let mut nbt = Vec::new();
        if let Some(tag) = &self.payload {
            tag.write_root(&mut nbt);
        }
        write_varint(nbt.len() as i32, out);
        out.extend(nbt);
    }
}

/// A play-phase `ShowDialog`: inline, or the `minecraft:dialog` registry entry by index.
#[derive(Debug, Clone, PartialEq)]
pub enum Shown {
    Inline(Dialog),
    Registry(usize),
}

/// The play-phase packet body: a holder varint (0 inline), then the dialog's NBT.
pub fn read_show_dialog(input: &mut &[u8]) -> Result<Shown, Error> {
    match read_varint(input)? {
        0 => read_inline(input).map(Shown::Inline),
        n => usize::try_from(n - 1).map(Shown::Registry).map_err(|_| Error::Invalid("dialog holder")),
    }
}

pub fn read_inline(input: &mut &[u8]) -> Result<Dialog, Error> {
    let tag = Tag::read_root(input)?.ok_or(Error::Invalid("dialog NBT"))?;
    Dialog::from_tag(&tag).ok_or(Error::Invalid("dialog"))
}

impl Dialog {
    pub fn from_tag(tag: &Tag) -> Option<Dialog> {
        let kind = tag.get("type")?.as_str()?.trim_start_matches("minecraft:");
        let field = |key: &str| tag.get(key).and_then(button);
        let exit = field("exit_action");
        let (kind, buttons, cancel) = match kind {
            "notice" => {
                let ok = field("action").unwrap_or(Button { label: "OK".into(), action: None });
                (DialogKind::Notice, vec![ok.clone()], Some(ok))
            }
            "confirmation" => {
                let (yes, no) = (field("yes")?, field("no")?);
                (DialogKind::Confirmation, vec![yes, no.clone()], Some(no))
            }
            "multi_action" => (DialogKind::MultiAction, list(tag.get("actions")).iter().filter_map(button).collect(), exit),
            "dialog_list" => (DialogKind::DialogList, list(tag.get("dialogs")).iter().map(sub_dialog).collect(), exit),
            "server_links" => (DialogKind::ServerLinks, Vec::new(), exit),
            _ => return None,
        };
        Some(Dialog {
            kind,
            title: text(tag.get("title")?),
            body: bodies(tag.get("body")),
            inputs: list(tag.get("inputs")).iter().filter_map(input).collect(),
            buttons,
            cancel,
            can_close_with_escape: tag.get("can_close_with_escape").and_then(Tag::as_bool).unwrap_or(true),
            stays_open: tag.get("after_action").and_then(Tag::as_str) == Some("none"),
        })
    }
}

/// A text component's plain text: a string, or `text`/`translate` plus `extra`.
pub fn text(tag: &Tag) -> String {
    match tag {
        Tag::String(s) => s.clone(),
        Tag::List(parts) => parts.iter().map(text).collect(),
        Tag::Compound(_) => {
            let own = tag.get("text").or_else(|| tag.get("translate")).and_then(Tag::as_str).unwrap_or_default();
            own.to_owned() + &list(tag.get("extra")).iter().map(text).collect::<String>()
        }
        other => other.as_f64().map(|v| v.to_string()).unwrap_or_default(),
    }
}

fn list(tag: Option<&Tag>) -> &[Tag] {
    match tag {
        Some(Tag::List(items)) => items,
        _ => &[],
    }
}

/// `body` is one body or a list; a bare component is a plain message.
fn bodies(tag: Option<&Tag>) -> Vec<String> {
    let one = |b: &Tag| match b.get("type").and_then(Tag::as_str) {
        Some(t) if t.ends_with("plain_message") => b.get("contents").map(text),
        Some(_) => b.get("description").map(|d| d.get("contents").map_or_else(|| text(d), text)),
        None => Some(text(b)),
    };
    match tag {
        Some(Tag::List(items)) => items.iter().filter_map(one).collect(),
        Some(b) => one(b).into_iter().collect(),
        None => Vec::new(),
    }
}

fn button(tag: &Tag) -> Option<Button> {
    Some(Button { label: text(tag.get("label")?), action: tag.get("action").map(action) })
}

fn sub_dialog(tag: &Tag) -> Button {
    match Dialog::from_tag(tag) {
        Some(d) => Button { label: d.title.clone(), action: Some(Action::ShowDialog(Box::new(d))) },
        None => Button { label: tag.as_str().unwrap_or_default().to_owned(), action: Some(Action::ClientOnly) },
    }
}

fn action(tag: &Tag) -> Action {
    let string = |key: &str| tag.get(key).and_then(Tag::as_str).map(str::to_owned);
    let kind = tag.get("type").and_then(Tag::as_str).unwrap_or_default().trim_start_matches("minecraft:");
    let made = match kind {
        "run_command" => string("command").map(Action::RunCommand),
        "dynamic/run_command" => string("template").map(Action::DynamicRunCommand),
        "custom" => string("id").map(|id| Action::Custom { id, payload: tag.get("payload").cloned() }),
        "dynamic/custom" => string("id").map(|id| Action::DynamicCustom { id, additions: tag.get("additions").cloned() }),
        "show_dialog" => tag.get("dialog").and_then(Dialog::from_tag).map(|d| Action::ShowDialog(Box::new(d))),
        _ => None,
    };
    made.unwrap_or(Action::ClientOnly)
}

fn input(tag: &Tag) -> Option<Input> {
    let string = |key: &str, default: &str| tag.get(key).and_then(Tag::as_str).unwrap_or(default).to_owned();
    let number = |key: &str| tag.get(key).and_then(Tag::as_f64).map(|v| v as f32);
    let control = match tag.get("type")?.as_str()?.trim_start_matches("minecraft:") {
        "text" => Control::Text { initial: string("initial", ""), max_length: number("max_length").map_or(32, |n| n as usize) },
        "boolean" => Control::Boolean {
            initial: tag.get("initial").and_then(Tag::as_bool).unwrap_or(false),
            on_true: string("on_true", "true"),
            on_false: string("on_false", "false"),
        },
        "single_option" => {
            let options = list(tag.get("options"));
            let entry = |o: &Tag| match o {
                Tag::String(id) => Some((id.clone(), id.clone())),
                _ => {
                    let id = o.get("id")?.as_str()?.to_owned();
                    Some((id.clone(), o.get("display").map_or(id, text)))
                }
            };
            let initial = options.iter().position(|o| o.get("initial").and_then(Tag::as_bool) == Some(true)).unwrap_or(0);
            Control::SingleOption { options: options.iter().filter_map(entry).collect(), initial }
        }
        "number_range" => {
            let (start, end) = (number("start")?, number("end")?);
            Control::NumberRange { start, end, initial: number("initial").unwrap_or((start + end) / 2.0), step: number("step") }
        }
        _ => return None,
    };
    Some(Input { key: tag.get("key")?.as_str()?.to_owned(), label: tag.get("label").map(text).unwrap_or_default(), control })
}
