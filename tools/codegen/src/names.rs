//! Identifier conversion: ProtoDef names → Rust identifiers.

const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "do", "dyn",
    "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl", "in", "let",
    "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return",
    "static", "struct", "trait", "true", "try", "type", "typeof", "unsafe", "unsized", "use",
    "virtual", "where", "while", "yield",
];

/// Splits on non-alphanumerics and camel-case humps: `HasOverrideURI` → [Has, Override, URI].
fn words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in s
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
    {
        let chars: Vec<char> = part.chars().collect();
        let mut cur = String::new();
        for (i, &c) in chars.iter().enumerate() {
            let prev = i.checked_sub(1).map(|j| chars[j]);
            let next = chars.get(i + 1).copied();
            let boundary = c.is_ascii_uppercase()
                && prev.is_some_and(|p| {
                    p.is_ascii_lowercase()
                        || p.is_ascii_digit()
                        || (p.is_ascii_uppercase() && next.is_some_and(|n| n.is_ascii_lowercase()))
                });
            if boundary && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(c);
        }
        out.push(cur);
    }
    out
}

pub fn pascal(s: &str) -> String {
    let mut out: String = s
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            let first = c.next().unwrap().to_ascii_uppercase();
            std::iter::once(first).chain(c).collect::<String>()
        })
        .collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'V');
    }
    if out == "Self" {
        out.push('_');
    }
    out
}

pub fn snake_raw(s: &str) -> String {
    let mut out = words(s)
        .iter()
        .map(|w| w.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("_");
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// A snake_case identifier usable as a field name.
pub fn field(s: &str) -> String {
    let raw = snake_raw(s);
    match raw.as_str() {
        "self" | "super" | "crate" => format!("{raw}_"),
        k if KEYWORDS.contains(&k) => format!("r#{raw}"),
        _ => raw,
    }
}

pub fn upper(s: &str) -> String {
    let mut out = snake_raw(s).to_ascii_uppercase();
    if out.starts_with('_') {
        out.insert(0, 'F');
    }
    out
}

/// Returns `base`, or `base2`, `base3`, … if `taken` already holds it.
pub fn dedupe(base: String, taken: &mut Vec<String>) -> String {
    let mut name = base.clone();
    let mut n = 2;
    while taken.contains(&name) {
        name = format!("{base}{n}");
        n += 1;
    }
    taken.push(name.clone());
    name
}
