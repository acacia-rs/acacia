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
        let template = match self.lookup(text) {
            Some(t) => t.to_owned(),
            None => text.split(' ').map(|w| self.lookup(w).unwrap_or(w)).collect::<Vec<_>>().join(" "),
        };
        fill(&template, &params)
    }

    fn lookup<'a>(&'a self, word: &'a str) -> Option<&'a str> {
        self.0.get(word.strip_prefix('%')?).map(String::as_str)
    }
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
        assert_eq!(lang.translate("%multiplayer.player.joined", &["Steve".into()]), "§eSteve joined the game");
        assert_eq!(lang.translate("%swap", &["a".into(), "b".into()]), "b before a");
        assert_eq!(lang.translate("§e%multiplayer.player.joined", &["X".into()]), "§e%multiplayer.player.joined", "a code glued on is not a key");
        assert_eq!(lang.translate("plain 100% text", &[]), "plain 100% text");
        assert_eq!(lang.translate("%chat.type.announcement", &["Server".into(), "%commands.time.set".into()]), "[Server] Set the time to ");
    }
}
