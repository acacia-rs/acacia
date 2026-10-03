use std::path::Path;

use serde_json::Value;

use crate::Error;

/// Reads a resource pack JSON file; vanilla files carry `//` and `/* */` comments.
pub fn read(path: &Path) -> Result<Value, Error> {
    let text = std::fs::read_to_string(path).map_err(|source| Error::Io { path: path.display().to_string(), source })?;
    serde_json::from_str(&strip_comments(&text)).map_err(|source| Error::Json { path: path.display().to_string(), source })
}

pub fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next()),
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => {
                while chars.next_if(|&n| n != '\n').is_some() {}
            }
            ('/', Some('*')) => {
                chars.next();
                let mut prev = ' ';
                for n in chars.by_ref() {
                    if prev == '*' && n == '/' {
                        break;
                    }
                    prev = n;
                }
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn comments_outside_strings_are_removed() {
        let src = "// head\n{\"a\": \"x//y\", /* c */ \"b\": 1 // tail\n}";
        let v: serde_json::Value = serde_json::from_str(&super::strip_comments(src)).unwrap();
        assert_eq!(v["a"], "x//y");
        assert_eq!(v["b"], 1);
    }
}
