//! `CommandSyntaxException` with the messages Minecraft's `BrigadierExceptions` gives them, and
//! `CommandSuggestions.getExceptionMessage`.

use std::collections::HashMap;

use crate::java::text::{Lang, Text};

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    IntegerTooLow { found: i32, min: i32 },
    IntegerTooHigh { found: i32, max: i32 },
    FloatTooLow { found: f32, min: f32 },
    FloatTooHigh { found: f32, max: f32 },
    LiteralIncorrect(String),
    ExpectedEndOfQuote,
    InvalidEscape(String),
    InvalidBool(String),
    ExpectedBool,
    InvalidInt(String),
    ExpectedInt,
    InvalidFloat(String),
    ExpectedFloat,
    UnknownCommand,
    UnknownArgument,
    ExpectedSeparator,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub kind: Kind,
    /// The whole input and the cursor the error points at (`createWithContext`).
    pub input: Option<Vec<u16>>,
    pub cursor: i32,
}

/// `String.valueOf(float)` for the values a command range holds.
pub fn java_float(v: f32) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1.0e7 {
        return format!("{v:.1}");
    }
    let abs = v.abs();
    if (1.0e-3..1.0e7).contains(&abs) {
        return format!("{v}");
    }
    // Scientific: shortest digits, `d.dddE±n`.
    let s = format!("{v:e}");
    let (mantissa, exp) = s.split_once('e').unwrap_or((&s, "0"));
    let mantissa = if mantissa.contains('.') { mantissa.to_owned() } else { format!("{mantissa}.0") };
    format!("{mantissa}E{exp}")
}

impl Kind {
    /// The `BrigadierExceptions` component: a key and its arguments.
    fn message(&self, lang: Lang) -> Text {
        let t = |key: &str, args: &[String]| Text::translate(key, &args.iter().map(|a| Text::literal(a.as_str())).collect::<Vec<_>>(), lang);
        match self {
            Kind::IntegerTooLow { found, min } => t("argument.integer.low", &[min.to_string(), found.to_string()]),
            Kind::IntegerTooHigh { found, max } => t("argument.integer.big", &[max.to_string(), found.to_string()]),
            Kind::FloatTooLow { found, min } => t("argument.float.low", &[java_float(*min), java_float(*found)]),
            Kind::FloatTooHigh { found, max } => t("argument.float.big", &[java_float(*max), java_float(*found)]),
            Kind::LiteralIncorrect(expected) => t("argument.literal.incorrect", std::slice::from_ref(expected)),
            Kind::ExpectedEndOfQuote => t("parsing.quote.expected.end", &[]),
            Kind::InvalidEscape(c) => t("parsing.quote.escape", std::slice::from_ref(c)),
            Kind::InvalidBool(v) => t("parsing.bool.invalid", std::slice::from_ref(v)),
            Kind::ExpectedBool => t("parsing.bool.expected", &[]),
            Kind::InvalidInt(v) => t("parsing.int.invalid", std::slice::from_ref(v)),
            Kind::ExpectedInt => t("parsing.int.expected", &[]),
            Kind::InvalidFloat(v) => t("parsing.float.invalid", std::slice::from_ref(v)),
            Kind::ExpectedFloat => t("parsing.float.expected", &[]),
            Kind::UnknownCommand => t("command.unknown.command", &[]),
            Kind::UnknownArgument => t("command.unknown.argument", &[]),
            Kind::ExpectedSeparator => t("command.expected.separator", &[]),
        }
    }
}

/// The translations the messages use, kept so they can be formatted whenever the input changes.
#[derive(Clone, Debug, Default)]
pub struct Messages(HashMap<&'static str, String>);

const KEYS: [&str; 17] = [
    "argument.integer.low",
    "argument.integer.big",
    "argument.float.low",
    "argument.float.big",
    "argument.literal.incorrect",
    "parsing.quote.expected.end",
    "parsing.quote.escape",
    "parsing.bool.invalid",
    "parsing.bool.expected",
    "parsing.int.invalid",
    "parsing.int.expected",
    "parsing.float.invalid",
    "parsing.float.expected",
    "command.unknown.command",
    "command.unknown.argument",
    "command.expected.separator",
    "command.context.parse_error",
];

impl Messages {
    pub fn new(lang: Lang) -> Messages {
        Messages(KEYS.iter().filter_map(|&k| lang(k).map(|v| (k, v))).collect())
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
}

impl Error {
    /// `CommandSyntaxException.getContext`: the ten units before the cursor, then `<--[HERE]`.
    pub fn context(&self) -> Option<String> {
        let input = self.input.as_ref().filter(|_| self.cursor >= 0)?;
        let cursor = input.len().min(self.cursor as usize);
        let mut out = String::new();
        if cursor > 10 {
            out.push_str("...");
        }
        out.push_str(&String::from_utf16_lossy(&input[cursor.saturating_sub(10)..cursor]));
        out.push_str("<--[HERE]");
        Some(out)
    }

    /// `CommandSuggestions.getExceptionMessage`, before `getVisualOrderText`.
    pub fn display(&self, lang: Lang) -> Text {
        let message = self.kind.message(lang);
        match self.context() {
            None => message,
            Some(context) => Text::translate("command.context.parse_error", &[message, Text::literal(self.cursor.to_string()), Text::literal(context)], lang),
        }
    }
}
