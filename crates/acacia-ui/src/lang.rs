//! Translations from a Bedrock `.lang` file, for server text sent as `%key` with parameters.

use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Default)]
pub struct Lang(HashMap<String, String>);

impl Lang {
    /// `key=value` lines; a tab starts a trailing comment, `##` a comment line.
    pub fn parse(text: &str) -> Lang {
        let entries = text.lines().filter(|l| !l.starts_with('#')).filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            let value = value.split_once('\t').map_or(value, |(v, _)| v);
            Some((key.trim().to_owned(), value.trim_end().to_owned()))
        });
        Lang(entries.collect())
    }

    /// An empty table when the file is missing.
    pub fn load(path: &Path) -> Lang {
        std::fs::read_to_string(path).map(|t| Lang::parse(&t)).unwrap_or_default()
    }

    /// `text` with every `%key` it names (the whole text, or a word of it) translated and `%s`,
    /// `%1$s` and `%d` filled from `params`, which are translated first.
    pub fn translate(&self, text: &str, params: &[String]) -> String {
        let params: Vec<String> = params.iter().map(|p| self.translate(p, &[])).collect();
        // BDS colours whole lines ahead of their key: `§e%multiplayer.player.joined`.
        let (prefix, body) = text.split_at(leading_codes(text));
        // A whole message may also be a bare key (BDS command feedback: `commands.give.successRecipient`).
        let template = match self.lookup(body).or_else(|| self.0.get(body).map(String::as_str)) {
            Some(t) => format!("{prefix}{t}"),
            None => text.split(' ').map(|w| self.lookup(w).unwrap_or(w)).collect::<Vec<_>>().join(" "),
        };
        fill(&template, &params)
    }

    fn lookup<'a>(&'a self, word: &'a str) -> Option<&'a str> {
        self.0.get(word.strip_prefix('%')?).map(String::as_str)
    }

    /// The name shown for item `id` (`minecraft:diamond_sword`): the table's `item.` or `tile.`
    /// entry, else the identifier as a title (most block entries keep pre-flattening keys).
    pub fn item_name(&self, id: &str) -> String {
        let bare = id.rsplit(':').next().unwrap_or(id);
        let named = ["item", "tile"].iter().find_map(|kind| self.0.get(&format!("{kind}.{bare}.name")));
        named.cloned().unwrap_or_else(|| {
            let word = |w: &str| w.chars().take(1).flat_map(char::to_uppercase).chain(w.chars().skip(1)).collect::<String>();
            bare.split('_').map(word).collect::<Vec<_>>().join(" ")
        })
    }

    /// A `{"rawtext":[...]}` message (command feedback, scripted text) as its text: `text` parts
    /// verbatim, `translate` parts through the table with their `with` filled in. `None` when it
    /// is not JSON.
    pub fn rawtext(&self, json: &str) -> Option<String> {
        Some(self.flatten(&serde_json::from_str(json).ok()?))
    }

    fn flatten(&self, v: &serde_json::Value) -> String {
        use serde_json::Value;
        let parts = |v: &Value| v.get("rawtext").and_then(Value::as_array).cloned();
        if let Some(parts) = parts(v) {
            return parts.iter().map(|p| self.flatten(p)).collect();
        }
        if let Some(text) = v.as_str().or_else(|| v.get("text").and_then(Value::as_str)) {
            return text.to_owned();
        }
        let Some(key) = v.get("translate").and_then(Value::as_str) else { return String::new() };
        // `with` is a list of strings, or rawtext whose parts are the arguments.
        let with = v.get("with");
        let args = with.and_then(Value::as_array).cloned().or_else(|| with.and_then(parts)).unwrap_or_default();
        let args: Vec<String> = args.iter().map(|a| self.flatten(a)).collect();
        let key = key.trim_start_matches('%');
        match self.0.contains_key(key) {
            true => self.translate(&format!("%{key}"), &args),
            // An unknown key is its own template (scripts put plain `%s` text here).
            false => fill(key, &args),
        }
    }
}

/// Bytes of the `§x` formatting codes `text` starts with.
fn leading_codes(text: &str) -> usize {
    let mut rest = text;
    while let Some(after) = rest.strip_prefix('§') {
        let Some(code) = after.chars().next() else { break };
        rest = &after[code.len_utf8()..];
    }
    text.len() - rest.len()
}

fn fill(template: &str, params: &[String]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut next = 0;
    let mut rest = template;
    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        let positional = rest[digits..].strip_prefix('$').filter(|_| digits > 0);
        let (index, after) = match positional {
            Some(after) => (rest[..digits].parse::<usize>().ok().map(|n| n.saturating_sub(1)), after),
            None => (None, rest),
        };
        match after.chars().next() {
            Some('s' | 'd') => {
                let index = index.unwrap_or_else(|| {
                    next += 1;
                    next - 1
                });
                out.push_str(params.get(index).map_or("", String::as_str));
                rest = &after[1..];
            }
            Some('%') => {
                out.push('%');
                rest = &after[1..];
            }
            _ => out.push('%'),
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_parameters_are_filled() {
        let lang = Lang::parse("## header\ncommands.time.set=Set the time to %s\t#comment\nchat.type.announcement=[%s] %s\nmultiplayer.player.joined=§e%s joined the game\nswap=%2$s before %1$s\n");
        assert_eq!(lang.translate("%commands.time.set", &["1000".into()]), "Set the time to 1000");
        assert_eq!(lang.translate("commands.time.set", &["1000".into()]), "Set the time to 1000", "a bare key");
        assert_eq!(lang.translate("%multiplayer.player.joined", &["Steve".into()]), "§eSteve joined the game");
        assert_eq!(lang.translate("%swap", &["a".into(), "b".into()]), "b before a");
        assert_eq!(lang.translate("§e%multiplayer.player.joined", &["X".into()]), "§e§eX joined the game", "BDS colours the line ahead of the key");
        assert_eq!(lang.translate("a§e%multiplayer.player.joined", &["X".into()]), "a§e%multiplayer.player.joined", "only leading codes");
        assert_eq!(lang.translate("plain 100% text", &[]), "plain 100% text");
        assert_eq!(lang.translate("%chat.type.announcement", &["Server".into(), "%commands.time.set".into()]), "[Server] Set the time to ");
    }

    #[test]
    fn rawtext_is_translated_with_its_arguments() {
        let lang = Lang::parse("commands.time.set=Set the time to %1$d\ncommands.tp.successVictim=You have been teleported to %1$s\ngameMode.changed=Your game mode has been updated to %s\ngameMode.creative=Creative\n");
        let tp = r#"{"rawtext":[{"translate":"commands.tp.successVictim","with":["1, 2, 3"]}]}"#;
        assert_eq!(lang.rawtext(tp).as_deref(), Some("You have been teleported to 1, 2, 3"));
        // An op's echo: literal parts around a translated one; arguments may be rawtext and keys.
        let echo = r#"{"rawtext":[{"text":"§7§o["},{"text":"Steve"},{"text":": "},{"translate":"commands.time.set","with":{"rawtext":[{"text":"6000"}]}},{"text":"]"}]}"#;
        assert_eq!(lang.rawtext(echo).as_deref(), Some("§7§o[Steve: Set the time to 6000]"));
        let mode = r#"{"rawtext":[{"translate":"gameMode.changed","with":["%gameMode.creative"]}]}"#;
        assert_eq!(lang.rawtext(mode).as_deref(), Some("Your game mode has been updated to Creative"));
        assert_eq!(lang.rawtext("not json"), None);
        let names = Lang::parse("item.stick.name=Stick\ntile.torch.name=Torch\n");
        assert_eq!([names.item_name("minecraft:stick"), names.item_name("minecraft:torch"), names.item_name("minecraft:oak_planks")], ["Stick", "Torch", "Oak Planks"]);
    }
}
