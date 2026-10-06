//! What the fuzz targets share: a generator of programs that mostly compile, and the comparison
//! with molangx. See the crate README, "Fuzzing".

pub mod differ;

use std::fmt;

use arbitrary::{Arbitrary, Result, Unstructured};

/// A generated program, as source. It stays within what BDS is known to accept at the latest engine
/// version, and leaves out randomness and trigonometry, which two implementations cannot agree on.
pub struct Source(pub String);

impl fmt::Debug for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `v.a` and `v.b` are set before a program runs ([`differ::PRESET`]); `v.none` only if a program does it.
pub const VARIABLES: [&str; 8] = ["v.a", "v.b", "v.c", "v.none", "v.s.x", "v.s.y", "t.a", "t.b"];
const NUMBERS: [&str; 10] = ["0", "1", "2", "3", "-2", "0.5", "7.25", "100", "0.001", "16777216"];
/// No `-`: BDS cancels a `variable.` against itself in a sum without reading it, by rules of its own
/// (`v.none - v.none` is 0, `1 + v.none - v.none` too) that this crate does not copy (README, "Limits").
const BINARY: [&str; 11] = ["+", "*", "/", "<", "<=", ">", ">=", "==", "!=", "&&", "||"];
/// The left of `??`: only plain variables, where BDS neither complains nor depends on the context.
const COALESCED: [&str; 6] = ["v.a", "v.b", "v.c", "v.none", "t.a", "t.b"];
const MATH: [(&str, usize); 12] = [
    ("abs", 1),
    ("ceil", 1),
    ("floor", 1),
    ("round", 1),
    ("trunc", 1),
    ("sign", 1),
    ("sqrt", 1),
    ("min", 2),
    ("max", 2),
    ("mod", 2),
    ("clamp", 3),
    ("lerp", 3),
];

struct Generator<'a, 'b> {
    bytes: &'a mut Unstructured<'b>,
    out: String,
}

impl Generator<'_, '_> {
    fn pick(&mut self, from: &[&'static str]) -> Result<()> {
        self.out.push_str(self.bytes.choose(from)?);
        Ok(())
    }

    /// Either side of `text`, in parentheses or bare: bare is where precedence gets tested.
    fn grouped(&mut self, depth: u8, parts: &[&str]) -> Result<()> {
        let bare = self.bytes.ratio(1, 3)?;
        self.out.push_str(if bare { "" } else { "(" });
        for (index, part) in parts.iter().enumerate() {
            if index > 0 {
                self.expr(depth)?;
            }
            self.out.push_str(part);
        }
        self.out.push_str(if bare { "" } else { ")" });
        Ok(())
    }

    fn expr(&mut self, depth: u8) -> Result<()> {
        let kinds = if depth == 0 { 2 } else { 10 };
        let depth = depth.saturating_sub(1);
        match self.bytes.int_in_range(0..=kinds)? {
            0 => self.pick(&NUMBERS),
            1 => self.pick(&VARIABLES),
            // Strings only where they are compared: under `-`, BDS reads one as a meaningless number.
            2 => self.pick(&["this", "(v.c == 'a')", "('a' != 'b')", "('a' == 'a')"]),
            3 => {
                self.out.push('(');
                self.pick(&COALESCED)?;
                self.out.push_str(" ?? ");
                self.expr(depth)?;
                self.out.push(')');
                Ok(())
            }
            // Always grouped: BDS rejects `- -x`.
            4 => {
                self.pick(&["-(", "!("])?;
                self.expr(depth)?;
                self.out.push(')');
                Ok(())
            }
            5 => self.grouped(depth, &["", " ? ", " : ", ""]),
            6 => {
                let (name, count) = *self.bytes.choose(&MATH)?;
                self.out.push_str("math.");
                self.out.push_str(name);
                for index in 0..count {
                    self.out.push_str(if index == 0 { "(" } else { ", " });
                    self.expr(depth)?;
                }
                self.out.push(')');
                Ok(())
            }
            _ => {
                let operator = format!(" {} ", self.bytes.choose(&BINARY)?);
                self.grouped(depth, &["", &operator, ""])
            }
        }
    }

    fn block(&mut self, depth: u8, in_loop: bool) -> Result<()> {
        self.out.push_str("{ ");
        self.statements(depth, in_loop)?;
        // Last in its block: BDS rejects a statement after one of these.
        if in_loop && self.bytes.ratio(1, 4)? {
            self.expr(1)?;
            self.pick(&[" ? break; ", " ? continue; "])?;
        }
        self.out.push('}');
        Ok(())
    }

    fn statements(&mut self, depth: u8, in_loop: bool) -> Result<()> {
        for _ in 0..self.bytes.int_in_range(1..=4)? {
            match self.bytes.int_in_range(0..=if depth == 0 { 3 } else { 5 })? {
                0..=2 => {
                    self.pick(&VARIABLES)?;
                    self.out.push_str(" = ");
                    self.expr(3)?;
                }
                3 => self.expr(3)?,
                4 => {
                    self.out.push_str("loop(");
                    self.pick(&["0", "1", "2", "3", "2.5", "v.a", "v.none"])?;
                    self.out.push_str(", ");
                    self.block(depth - 1, true)?;
                    self.out.push(')');
                }
                _ => {
                    self.expr(2)?;
                    self.out.push_str(" ? ");
                    self.block(depth - 1, in_loop)?;
                }
            }
            self.out.push_str("; ");
        }
        Ok(())
    }
}

impl<'b> Arbitrary<'b> for Source {
    fn arbitrary(bytes: &mut Unstructured<'b>) -> Result<Source> {
        let mut generator = Generator { bytes, out: String::new() };
        if generator.bytes.ratio(1, 3)? {
            generator.expr(4)?;
        } else {
            generator.statements(2, false)?;
            if generator.bytes.ratio(2, 3)? {
                generator.out.push_str("return ");
                generator.expr(3)?;
                generator.out.push(';');
            }
        }
        Ok(Source(generator.out))
    }
}
