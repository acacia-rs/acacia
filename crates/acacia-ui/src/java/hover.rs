//! `HoverEvent`, what a style shows while the mouse is over its text, and the tooltip
//! `GuiGraphicsExtractor.componentHoverEffect` sets for it: text split to half the screen (200 at
//! least), an item's tooltip, or with advanced tooltips an entity's name, type and UUID.

use serde_json::Value;

use super::gui::Metrics;
use super::split::split;
use super::text::{Lang, Text};
use super::tooltip::item::Details;
use super::tooltip::stack::ItemStack;
use super::tooltip::{Line, Positioner, Tooltip, bundle};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HoverEvent {
    ShowText(Text),
    /// The stack as the `show_item` JSON gives it (`ItemStackTemplate`'s fields).
    ShowItem(String),
    /// `EntityTooltipInfo`: the type's full id, the UUID as `UUID.toString` writes it.
    ShowEntity { id: String, uuid: String, name: Option<Text> },
}

/// `UUIDUtil.LENIENT_CODEC`: four ints or a string, written back as `UUID.toString` does.
fn uuid(v: &Value) -> Option<String> {
    let (most, least): (u64, u64) = match v {
        Value::Array(a) if a.len() == 4 => {
            let w: Vec<u64> = a.iter().map(|x| x.as_i64().map(|i| i as u32 as u64)).collect::<Option<_>>()?;
            (w[0] << 32 | w[1], w[2] << 32 | w[3])
        }
        Value::String(s) => {
            let p: Vec<u64> = s.split('-').map(|g| u64::from_str_radix(g, 16).ok()).collect::<Option<_>>()?;
            let [a, b, c, d, e] = p.as_slice() else { return None };
            ((a & 0xFFFF_FFFF) << 32 | (b & 0xFFFF) << 16 | (c & 0xFFFF), (d & 0xFFFF) << 48 | (e & 0xFFFF_FFFF_FFFF))
        }
        _ => return None,
    };
    Some(format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", most >> 32, (most >> 16) & 0xFFFF, most & 0xFFFF, least >> 48, least & 0xFFFF_FFFF_FFFF))
}

/// A stack whose item the game facts lack: the template's patch over just a default `item_name`,
/// `block.` when the language has that key (a `BlockItem`'s description id), else `item.`.
fn bare_stack(template: &Value, lang: Lang) -> Option<ItemStack> {
    let mut v = template.clone();
    let id = v.get("id")?.as_str()?.to_owned();
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", &id));
    let block = format!("block.{namespace}.{path}");
    let key = if lang(&block).is_some() { block } else { format!("item.{namespace}.{path}") };
    let components = v.as_object_mut()?.entry("components").or_insert_with(|| Value::Object(Default::default())).as_object_mut()?;
    components.entry("minecraft:item_name").or_insert_with(|| serde_json::json!({"translate": key}));
    ItemStack::from_json(&v)
}

impl HoverEvent {
    /// A style's `hover_event`.
    pub fn from_json(v: &Value, lang: Lang) -> Option<HoverEvent> {
        Some(match v.get("action")?.as_str()? {
            "show_text" => HoverEvent::ShowText(Text::from_json(v.get("value")?, lang)),
            "show_item" => {
                let mut stack = v.as_object()?.clone();
                stack.remove("action");
                HoverEvent::ShowItem(Value::Object(stack).to_string())
            }
            "show_entity" => {
                let id = v.get("id")?.as_str()?;
                HoverEvent::ShowEntity {
                    id: if id.contains(':') { id.to_owned() } else { format!("minecraft:{id}") },
                    uuid: uuid(v.get("uuid")?)?,
                    name: v.get("name").map(|n| Text::from_json(n, lang)),
                }
            }
            _ => return None,
        })
    }

    /// `componentHoverEffect`'s tooltip at `mouse` on a `gui_width`-wide screen; `d` describes
    /// items and whether tooltips are advanced.
    pub fn tooltip(&self, font: &dyn Metrics, gui_width: i32, mouse: (i32, i32), d: &Details) -> Option<Tooltip> {
        let lines: Vec<Line> = match self {
            HoverEvent::ShowItem(json) => {
                let template: Value = serde_json::from_str(json).ok()?;
                let stack = d.game.stack(&template).or_else(|| bare_stack(&template, d.lang))?;
                return Some(bundle::stack_tooltip(&stack, d, font, [mouse.0, mouse.1]));
            }
            HoverEvent::ShowEntity { .. } if !d.advanced => return None,
            HoverEvent::ShowEntity { id, uuid, name } => {
                let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
                let kind = d.tr(&format!("entity.{namespace}.{path}"), &[]);
                let mut lines: Vec<Text> = name.iter().cloned().collect();
                lines.push(d.tr("gui.entity_tooltip.type", &[kind]));
                lines.push(Text::literal(uuid.clone()));
                lines.into_iter().map(Line::Text).collect()
            }
            HoverEvent::ShowText(text) => split(font, text, (gui_width / 2).max(200)).into_iter().map(Line::Text).collect(),
        };
        Some(Tooltip { lines, x: mouse.0, y: mouse.1, positioner: Positioner::Default, style: None, extra_space: false })
    }
}
