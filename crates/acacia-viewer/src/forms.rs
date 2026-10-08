//! Server forms in the window: the bot's open [`Form`] as an `acacia_ui` form, the player's answer
//! back as a [`FormReply`], and button images from the pack or the web.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use acacia_bot::forms::{Element, Form, FormKind, FormReply, FormValue};
use acacia_render::assets::image_file;
use acacia_ui::form::{Outcome, Spec};
use acacia_ui::widget::{TextEdit, Value, Widget};
use image::RgbaImage;

/// Bedrock's edit box `max_length`.
const INPUT_MAX: usize = 100;
/// Button images are drawn 32×32 (Bedrock) or 16×16 (Java); kept at 32 in the atlas.
const IMAGE_SIDE: u32 = 32;
const FETCH_TIMEOUT: Duration = Duration::from_secs(5);
const FETCH_MAX_BYTES: usize = 1 << 20;

/// The form as widgets, and each action-form button's image source in button order.
pub fn spec(form: &Form) -> (Spec, Vec<Option<String>>) {
    match &form.kind {
        FormKind::Simple { content, buttons, elements, positions } => {
            let mut items = Vec::with_capacity(buttons.len() + elements.len());
            let mut placed = elements.iter().zip(positions).peekable();
            for (i, b) in buttons.iter().enumerate() {
                while let Some((e, _)) = placed.next_if(|&(_, &at)| at <= i) {
                    items.push(widget(e));
                }
                items.push(Widget::Button { text: b.text.clone(), image: None });
            }
            items.extend(placed.map(|(e, _)| widget(e)));
            (Spec::Action { content: content.clone(), items }, buttons.iter().map(|b| b.image.clone()).collect())
        }
        FormKind::Modal { content, yes, no } => (Spec::Modal { content: content.clone(), yes: yes.clone(), no: no.clone() }, Vec::new()),
        FormKind::Custom { elements } => (Spec::Custom { elements: elements.iter().map(widget).collect() }, Vec::new()),
    }
}

fn widget(element: &Element) -> Widget {
    match element.clone() {
        Element::Label { text } => Widget::Text { text },
        Element::Header { text } => Widget::Header { text },
        Element::Divider => Widget::Divider,
        Element::Input { text, placeholder, default } => Widget::Input { label: text, placeholder, edit: TextEdit::new(&default, INPUT_MAX) },
        Element::Toggle { text, default } => Widget::Toggle { label: text, on: default },
        Element::Slider { text, min, max, step, default } => Widget::Slider { label: text, min, max, step, value: default },
        Element::StepSlider { text, steps, default } => Widget::Steps { label: text, options: steps, index: default },
        Element::Dropdown { text, options, default } => Widget::Dropdown { label: text, options, index: default },
    }
}

pub fn reply(outcome: Outcome) -> FormReply {
    match outcome {
        Outcome::Button(i) => FormReply::Button(i),
        Outcome::Modal(yes) => FormReply::Modal(yes),
        Outcome::Close => FormReply::Close,
        Outcome::Submit(values) => FormReply::Custom(
            values
                .into_iter()
                .map(|v| match v {
                    Value::Text(s) => FormValue::Text(s),
                    Value::Toggle(b) => FormValue::Toggle(b),
                    Value::Number(n) => FormValue::Slider(n),
                    Value::Choice(i) => FormValue::Choice(i),
                })
                .collect(),
        ),
    }
}

/// Button images by source: pack paths read at once, URLs fetched on a thread of their own.
pub struct Images {
    pack: PathBuf,
    loaded: HashMap<String, Option<RgbaImage>>,
    fetched: Receiver<(String, Option<RgbaImage>)>,
    fetch: Sender<(String, Option<RgbaImage>)>,
}

impl Images {
    /// `pack` is the Bedrock resource pack that texture paths name.
    pub fn new(pack: &Path) -> Images {
        let (fetch, fetched) = channel();
        Images { pack: pack.to_owned(), loaded: HashMap::new(), fetched, fetch }
    }

    /// The image for `source`, `None` while a URL is loading or when it failed.
    pub fn get(&mut self, source: &str) -> Option<&RgbaImage> {
        while let Ok((url, image)) = self.fetched.try_recv() {
            self.loaded.insert(url, image);
        }
        if !self.loaded.contains_key(source) {
            let image = if is_url(source) {
                self.start_fetch(source);
                None
            } else {
                image_file(&self.pack, source.trim_start_matches('/')).and_then(|f| image::open(f).ok()).map(|i| icon(i.to_rgba8()))
            };
            self.loaded.insert(source.to_owned(), image);
        }
        self.loaded.get(source)?.as_ref()
    }

    /// Whether a fetch is still out for one of `sources`.
    pub fn pending(&self, sources: &[Option<String>]) -> bool {
        sources.iter().flatten().any(|s| is_url(s) && self.loaded.get(s).is_some_and(Option::is_none))
    }

    fn start_fetch(&self, url: &str) {
        let (url, done) = (url.to_owned(), self.fetch.clone());
        let spawned = std::thread::Builder::new().name("form-image".into()).spawn(move || {
            let image = fetch(&url).map(icon);
            if image.is_none() {
                tracing::debug!(%url, "form button image not loaded");
            }
            let _ = done.send((url, image));
        });
        if let Err(e) = spawned {
            tracing::warn!(%e, "form image thread");
        }
    }
}

fn is_url(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://")
}

/// PNG at `url`, at most [`FETCH_MAX_BYTES`] and [`FETCH_TIMEOUT`].
fn fetch(url: &str) -> Option<RgbaImage> {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
    let bytes = runtime.block_on(async {
        let client = reqwest::Client::builder().timeout(FETCH_TIMEOUT).build().ok()?;
        let response = client.get(url).send().await.ok()?.error_for_status().ok()?;
        if response.content_length().is_some_and(|n| n > FETCH_MAX_BYTES as u64) {
            return None;
        }
        let bytes = response.bytes().await.ok()?;
        (bytes.len() <= FETCH_MAX_BYTES).then_some(bytes)
    })?;
    image::load_from_memory(&bytes).ok().map(|i| i.to_rgba8())
}

/// `image` (its first frame when it's a flipbook strip) scaled to the atlas size.
fn icon(image: RgbaImage) -> RgbaImage {
    let side = image.width().min(image.height());
    let frame = image::imageops::crop_imm(&image, 0, 0, image.width(), side).to_image();
    image::imageops::resize(&frame, IMAGE_SIDE, IMAGE_SIDE, image::imageops::FilterType::Nearest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_answers_map_onto_the_bots_values() {
        let out = Outcome::Submit(vec![Value::Text("a".into()), Value::Toggle(true), Value::Number(2.5), Value::Choice(1)]);
        let want = FormReply::Custom(vec![FormValue::Text("a".into()), FormValue::Toggle(true), FormValue::Slider(2.5), FormValue::Choice(1)]);
        assert_eq!(reply(out), want);
    }

    #[test]
    fn action_items_interleave_and_buttons_keep_their_images() {
        let button = |text: &str, image: Option<&str>| acacia_bot::forms::Button { text: text.into(), image: image.map(str::to_owned) };
        let form = Form {
            id: 1,
            title: "t".into(),
            kind: FormKind::Simple {
                content: "c".into(),
                buttons: vec![button("x", Some("textures/items/apple")), button("y", None)],
                elements: vec![Element::Header { text: "h".into() }, Element::Divider, Element::Label { text: "end".into() }],
                positions: vec![0, 1, 2],
            },
        };
        let (spec, images) = spec(&form);
        assert_eq!(images, [Some("textures/items/apple".to_owned()), None]);
        let Spec::Action { items, .. } = spec else { panic!() };
        assert!(matches!(items[..], [Widget::Header { .. }, Widget::Button { .. }, Widget::Divider, Widget::Button { .. }, Widget::Text { .. }]));
    }
}
