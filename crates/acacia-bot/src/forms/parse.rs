//! `ModalFormRequest.data` JSON into [`Form`]s. Lenient: missing fields take their defaults, as
//! servers (PocketMine plugins, Geyser's Cumulus, the script API) each omit different ones.

use serde_json::{Map, Value};

use super::{Button, Element, Form, FormKind};

pub(super) fn form(id: u32, data: &str) -> Option<Form> {
    let v: Value = serde_json::from_str(data).ok()?;
    let o = v.as_object()?;
    let title = text(o, "title");
    let kind = match o.get("type")?.as_str()? {
        "form" => {
            // Older servers list `buttons`; newer ones mix buttons into `elements` (forms.md).
            let is_button = |e: &&Map<String, Value>| e.get("type").and_then(Value::as_str) == Some("button");
            let mixed = list(o, "elements").iter().filter_map(Value::as_object).filter(is_button);
            let buttons = list(o, "buttons").iter().filter_map(Value::as_object).chain(mixed).map(button).collect();
            let (mut positions, mut elements, mut seen) = (Vec::new(), Vec::new(), list(o, "buttons").len());
            for e in list(o, "elements").iter().filter_map(Value::as_object) {
                if is_button(&e) {
                    seen += 1;
                } else if let Some(e) = element(e) {
                    positions.push(seen);
                    elements.push(e);
                }
            }
            FormKind::Simple { content: text(o, "content"), buttons, elements, positions }
        }
        "modal" => FormKind::Modal { content: text(o, "content"), yes: text(o, "button1"), no: text(o, "button2") },
        "custom_form" => {
            FormKind::Custom { elements: list(o, "content").iter().filter_map(Value::as_object).filter_map(element).collect() }
        }
        _ => return None,
    };
    Some(Form { id, title, kind })
}

fn button(o: &Map<String, Value>) -> Button {
    let image = o.get("image").and_then(Value::as_object).map(|i| text(i, "data")).filter(|d| !d.is_empty());
    Button { text: text(o, "text"), image }
}

fn element(o: &Map<String, Value>) -> Option<Element> {
    let label = text(o, "text");
    Some(match o.get("type")?.as_str()? {
        "label" => Element::Label { text: label },
        "header" => Element::Header { text: label },
        "divider" => Element::Divider,
        "input" => Element::Input { text: label, placeholder: text(o, "placeholder"), default: text(o, "default") },
        "toggle" => Element::Toggle { text: label, default: o.get("default").and_then(Value::as_bool).unwrap_or(false) },
        "slider" => {
            let min = number(o, "min").unwrap_or(0.0);
            Element::Slider {
                text: label,
                min,
                max: number(o, "max").unwrap_or(min),
                step: number(o, "step").unwrap_or(1.0),
                default: number(o, "default").unwrap_or(min),
            }
        }
        "step_slider" => Element::StepSlider { text: label, steps: strings(o, "steps"), default: index(o) },
        "dropdown" => Element::Dropdown { text: label, options: strings(o, "options"), default: index(o) },
        _ => return None,
    })
}

fn text(o: &Map<String, Value>, key: &str) -> String {
    match o.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        // Script-API forms may send rawtext objects.
        Some(v @ Value::Object(_)) => crate::events::rawtext_value(v),
        _ => String::new(),
    }
}

fn list<'a>(o: &'a Map<String, Value>, key: &str) -> &'a [Value] {
    o.get(key).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn strings(o: &Map<String, Value>, key: &str) -> Vec<String> {
    list(o, key).iter().map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned)).collect()
}

fn number(o: &Map<String, Value>, key: &str) -> Option<f64> {
    o.get(key).and_then(Value::as_f64)
}

fn index(o: &Map<String, Value>) -> usize {
    o.get("default").and_then(Value::as_u64).unwrap_or(0) as usize
}
