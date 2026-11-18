//! `§` formatting codes to a component: one sibling per run of identical formatting.

use super::{Color, Component, NamedColor, Style};

pub(super) fn parse(text: &str) -> Component {
    let mut runs: Vec<Component> = Vec::new();
    let (mut style, mut run) = (Style::default(), String::new());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '§' {
            run.push(c);
            continue;
        }
        let Some(code) = chars.next() else { break };
        if let Some(next) = apply(style, code) {
            flush(&mut runs, &mut run, style);
            style = next;
        }
    }
    flush(&mut runs, &mut run, style);
    match runs.len() {
        0 => Component::empty(),
        1 => runs.remove(0),
        _ => Component { extra: runs, ..Component::empty() },
    }
}

fn flush(runs: &mut Vec<Component>, run: &mut String, style: Style) {
    if !run.is_empty() {
        runs.push(Component::text(std::mem::take(run)).style(style));
    }
}

/// `None` for an unknown code, which vanilla drops along with its `§`.
fn apply(style: Style, code: char) -> Option<Style> {
    if let Some(named) = NamedColor::from_code(code) {
        return Some(Style { color: Some(Color::Named(named)), ..Style::default() });
    }
    let mut next = style;
    match code.to_ascii_lowercase() {
        'k' => next.obfuscated = Some(true),
        'l' => next.bold = Some(true),
        'm' => next.strikethrough = Some(true),
        'n' => next.underlined = Some(true),
        'o' => next.italic = Some(true),
        'r' => next = Style::default(),
        _ => return None,
    }
    Some(next)
}
