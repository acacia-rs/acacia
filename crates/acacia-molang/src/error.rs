use std::fmt;

/// Why a source did not compile. `at` is a byte offset into it.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub kind: ErrorKind,
    pub at: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    UnexpectedChar(char),
    UnterminatedString,
    BadNumber,
    /// What was found, as written.
    UnexpectedToken(String),
    UnexpectedEnd,
    /// A name outside `query.`, `variable.`, `temp.`, `context.`, `math.`, `array.` and the resource prefixes.
    UnknownName(String),
    UnknownMath(String),
    /// A `query.` name outside the documented ones, or one the pack's engine version has dropped;
    /// see [`crate::Compiler::documented_queries_only`].
    UnknownQuery(String),
    UnknownArray(String),
    ArgumentCount { name: &'static str, expected: u8, found: usize },
    /// Call arguments on something that takes none (`v.x(1)`).
    NotCallable(String),
    /// The left of `=` is not a `variable.` or `temp.` name.
    NotAssignable,
    /// The left of `??` is not a plain `variable.`, `temp.` or `context.` name.
    CoalesceTarget,
    /// Statements after a `return`, `break` or `continue` in the same scope.
    Unreachable,
    /// An expression with a `;` or an assignment has to end with `;`.
    MissingSemicolon,
    /// A string literal under an operator other than `==` and `!=`.
    StringOperand,
    /// The second argument of `loop` is not a `{}` block.
    LoopBody,
    /// Nested more than 64 levels deep.
    TooDeep,
    /// `- -x`: BDS rejects a negation of a negation.
    DoubleNegation,
    /// Valid Molang this crate does not evaluate yet.
    Unsupported(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::UnexpectedChar(c) => write!(f, "unexpected character {c:?}"),
            ErrorKind::UnterminatedString => write!(f, "string without a closing quote"),
            ErrorKind::BadNumber => write!(f, "malformed number"),
            ErrorKind::UnexpectedToken(token) => write!(f, "unexpected `{token}`"),
            ErrorKind::UnexpectedEnd => write!(f, "expression ends early"),
            ErrorKind::UnknownName(name) => write!(f, "unknown name `{name}`"),
            ErrorKind::UnknownMath(name) => write!(f, "unknown math function `{name}`"),
            ErrorKind::UnknownQuery(name) => write!(f, "unknown query `{name}`"),
            ErrorKind::UnknownArray(name) => write!(f, "unknown array `{name}`"),
            ErrorKind::ArgumentCount { name, expected, found } => write!(f, "`math.{name}` takes {expected} arguments, found {found}"),
            ErrorKind::NotCallable(name) => write!(f, "`{name}` takes no arguments"),
            ErrorKind::NotAssignable => write!(f, "only `variable.` and `temp.` names can be assigned"),
            ErrorKind::CoalesceTarget => write!(f, "the left of `??` must be a plain variable"),
            ErrorKind::Unreachable => write!(f, "statements after `return`, `break` or `continue`"),
            ErrorKind::MissingSemicolon => write!(f, "an expression with `;` or `=` must end with `;`"),
            ErrorKind::StringOperand => write!(f, "strings only support `==` and `!=`"),
            ErrorKind::LoopBody => write!(f, "the body of `loop` must be a `{{}}` block"),
            ErrorKind::TooDeep => write!(f, "nested too deeply"),
            ErrorKind::DoubleNegation => write!(f, "negation of a negation"),
            ErrorKind::Unsupported(what) => write!(f, "{what} is not supported"),
        }?;
        write!(f, " at byte {}", self.at)
    }
}

impl std::error::Error for Error {}
