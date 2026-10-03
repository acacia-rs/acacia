//! JsonCpp's `StyledWriter` layout, which the game uses for skin geometry and resource patches in
//! Login (in-game `PlayerSkin` carries the same JSON compact). Scalars keep their exact text.

enum Node<'a> {
    Scalar(&'a str),
    Array(Vec<Node<'a>>),
    Object(Vec<(&'a str, Node<'a>)>),
}

const RIGHT_MARGIN: usize = 74;

/// Re-lays out `json` (any whitespace) as StyledWriter would, with its trailing newline.
pub fn styled(json: &str) -> String {
    let compact = strip_whitespace(json);
    let (root, _) = parse(&compact, 0);
    let mut w = Writer::default();
    w.value(&root);
    w.doc.push('\n');
    w.doc
}

fn strip_whitespace(s: &str) -> String {
    let (mut out, mut in_string, mut escaped) = (String::with_capacity(s.len()), false, false);
    for c in s.chars() {
        if in_string {
            out.push(c);
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if !c.is_whitespace() {
            out.push(c);
        }
    }
    out
}

fn parse(s: &str, mut i: usize) -> (Node<'_>, usize) {
    let b = s.as_bytes();
    match b[i] {
        b'{' | b'[' => {
            let object = b[i] == b'{';
            let (mut members, mut items) = (Vec::new(), Vec::new());
            i += 1;
            if b[i] != b'}' && b[i] != b']' {
                loop {
                    if object {
                        let (Node::Scalar(name), next) = parse(s, i) else { unreachable!() };
                        let (value, next) = parse(s, next + 1);
                        members.push((name, value));
                        i = next;
                    } else {
                        let (value, next) = parse(s, i);
                        items.push(value);
                        i = next;
                    }
                    if b[i] != b',' {
                        break;
                    }
                    i += 1;
                }
            }
            (if object { Node::Object(members) } else { Node::Array(items) }, i + 1)
        }
        b'"' => {
            let mut j = i + 1;
            while b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            (Node::Scalar(&s[i..=j]), j + 1)
        }
        _ => {
            let end = s[i..].find([',', ']', '}']).map_or(s.len(), |n| i + n);
            (Node::Scalar(&s[i..end]), end)
        }
    }
}

#[derive(Default)]
struct Writer {
    doc: String,
    indent: String,
    child_values: Vec<String>,
    collecting: bool,
}

impl Writer {
    fn push(&mut self, v: &str) {
        if self.collecting {
            self.child_values.push(v.to_owned());
        } else {
            self.doc.push_str(v);
        }
    }

    fn write_indent(&mut self) {
        match self.doc.chars().last() {
            Some(' ') => return,
            Some('\n') | None => {}
            Some(_) => self.doc.push('\n'),
        }
        self.doc.push_str(&self.indent);
    }

    fn with_indent(&mut self, v: &str) {
        self.write_indent();
        self.doc.push_str(v);
    }

    fn value(&mut self, node: &Node) {
        match node {
            Node::Scalar(text) => self.push(text),
            Node::Array(items) => self.array(items),
            Node::Object(members) if members.is_empty() => self.push("{}"),
            Node::Object(members) => {
                self.with_indent("{");
                self.indent.push_str("   ");
                for (n, (name, child)) in members.iter().enumerate() {
                    self.with_indent(name);
                    self.doc.push_str(" : ");
                    self.value(child);
                    if n + 1 < members.len() {
                        self.doc.push(',');
                    }
                }
                self.indent.truncate(self.indent.len() - 3);
                self.with_indent("}");
            }
        }
    }

    fn array(&mut self, items: &[Node]) {
        if items.is_empty() {
            return self.push("[]");
        }
        if self.is_multiline(items) {
            let values = std::mem::take(&mut self.child_values);
            self.with_indent("[");
            self.indent.push_str("   ");
            for (n, child) in items.iter().enumerate() {
                if values.is_empty() {
                    self.write_indent();
                    self.value(child);
                } else {
                    self.with_indent(&values[n]);
                }
                if n + 1 < items.len() {
                    self.doc.push(',');
                }
            }
            self.indent.truncate(self.indent.len() - 3);
            self.with_indent("]");
        } else {
            let line = format!("[ {} ]", self.child_values.join(", "));
            self.doc.push_str(&line);
        }
    }

    /// Fills `child_values` when every item is a scalar or empty container (JsonCpp's isMultilineArray).
    fn is_multiline(&mut self, items: &[Node]) -> bool {
        self.child_values.clear();
        let nested = |n: &Node| matches!(n, Node::Array(a) if !a.is_empty()) || matches!(n, Node::Object(m) if !m.is_empty());
        if items.len() * 3 >= RIGHT_MARGIN || items.iter().any(nested) {
            return true;
        }
        self.collecting = true;
        for child in items {
            self.value(child);
        }
        self.collecting = false;
        let line = 4 + (items.len() - 1) * 2 + self.child_values.iter().map(String::len).sum::<usize>();
        line >= RIGHT_MARGIN
    }
}

#[cfg(test)]
mod tests {
    use super::styled;

    #[test]
    fn matches_the_games_layout() {
        let pretty = "{\n   \"geometry\" : {\n      \"animated_face\" : \"geometry.animated_face_x\",\n      \"default\" : \"geometry.x\"\n   }\n}\n";
        assert_eq!(styled(pretty), pretty);
        assert_eq!(styled(r#"{"a":[0.0,24.0,0.0],"b":[],"c":{}}"#), "{\n   \"a\" : [ 0.0, 24.0, 0.0 ],\n   \"b\" : [],\n   \"c\" : {}\n}\n");
        assert_eq!(styled(r#"[{"n":"x"}]"#), "[\n   {\n      \"n\" : \"x\"\n   }\n]\n");
    }
}
