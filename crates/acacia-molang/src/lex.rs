use crate::error::{Error, ErrorKind};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    Num(f32),
    /// Case kept.
    Text(String),
    /// A dotted name, lowercased.
    Name(String),
    Symbol(&'static str),
}

impl Token {
    pub(crate) fn written(&self) -> String {
        match self {
            Token::Num(n) => n.to_string(),
            Token::Text(t) => format!("'{t}'"),
            Token::Name(n) => n.clone(),
            Token::Symbol(s) => (*s).to_owned(),
        }
    }
}

/// Longest first, so `==` is not read as two `=`.
const SYMBOLS: [&str; 26] = [
    "&&", "||", "==", "!=", "<=", ">=", "??", "->", "(", ")", "[", "]", "{", "}", ",", ";", "?", ":", "!", "<", ">", "+", "-", "*", "/", "=",
];

/// Tokens with the byte offset each starts at.
pub(crate) fn tokens(source: &str) -> Result<Vec<(Token, usize)>, Error> {
    let mut out = Vec::new();
    let mut at = source.len() - source.trim_start().len();
    while let Some(first) = source[at..].chars().next() {
        let rest = &source[at..];
        let fail = |kind| Error { kind, at };
        let take = |pred: fn(char) -> bool| rest.find(|c| !pred(c)).unwrap_or(rest.len());
        let len = if first.is_ascii_digit() || (first == '.' && rest[1..].starts_with(|c: char| c.is_ascii_digit())) {
            let mut len = take(|c| c.is_ascii_digit() || c == '.');
            let exponent = rest[len..].strip_prefix(['e', 'E']).map(|e| e.strip_prefix(['+', '-']).unwrap_or(e));
            if let Some(digits) = exponent.filter(|e| e.starts_with(|c: char| c.is_ascii_digit())) {
                len = rest.len() - digits.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            }
            out.push((Token::Num(rest[..len].parse().map_err(|_| fail(ErrorKind::BadNumber))?), at));
            // C-style float suffix: `1.0f`.
            let suffix = rest[len..].starts_with(['f', 'F']) && !rest[len + 1..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_');
            len + usize::from(suffix)
        } else if first.is_ascii_alphabetic() || first == '_' {
            let len = take(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
            out.push((Token::Name(rest[..len].to_ascii_lowercase()), at));
            len
        } else if first == '\'' {
            let end = rest[1..].find('\'').ok_or_else(|| fail(ErrorKind::UnterminatedString))?;
            out.push((Token::Text(rest[1..=end].to_owned()), at));
            end + 2
        } else if let Some(symbol) = SYMBOLS.iter().find(|s| rest.starts_with(**s)) {
            out.push((Token::Symbol(symbol), at));
            symbol.len()
        } else {
            return Err(fail(ErrorKind::UnexpectedChar(first)));
        };
        at += len;
        at += source[at..].len() - source[at..].trim_start().len();
    }
    Ok(out)
}
