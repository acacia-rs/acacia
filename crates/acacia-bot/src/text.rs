//! Minecraft `§` formatting codes (colours, bold, reset, ...).

pub const FORMAT_CHAR: char = '§';

/// Removes every `§` and the character following it.
pub fn strip_formatting(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == FORMAT_CHAR {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

/// Strips formatting and splits into lines.
pub fn plain_lines(s: &str) -> Vec<String> {
    strip_formatting(s).lines().map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_codes() {
        assert_eq!(strip_formatting("§6§lMoney §r§a$1.5K"), "Money $1.5K");
        assert_eq!(strip_formatting("plain"), "plain");
        assert_eq!(strip_formatting(""), "");
    }

    #[test]
    fn trailing_format_char_is_dropped() {
        assert_eq!(strip_formatting("abc§"), "abc");
    }

    #[test]
    fn code_char_may_be_multibyte() {
        assert_eq!(strip_formatting("a§éb§§c"), "abc");
    }

    #[test]
    fn plain_lines_splits_after_stripping() {
        assert_eq!(plain_lines("§aone\n§btwo"), vec!["one", "two"]);
    }
}
