//! The `raw·scale + offset` BDS folds `x·c`, `x / c`, `x + c` and `-x` into the node they apply to
//! (`optimise.rs`). Every multiply and add is rounded on its own, as on x86-64.

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Post {
    pub(crate) scale: f32,
    pub(crate) offset: f32,
}

impl Post {
    pub(crate) const IDENTITY: Post = Post { scale: 1.0, offset: 0.0 };

    /// An offset of -0 counts.
    pub(crate) fn is_identity(self) -> bool {
        self.scale == 1.0 && self.offset == 0.0
    }

    /// The identity leaves `raw` untouched, so a -0 stays one.
    #[inline]
    pub(crate) fn apply(self, raw: f32) -> f32 {
        if self.is_identity() { raw } else { raw * self.scale + self.offset }
    }

    /// What a comparison or a logic operator gives: never a multiplication.
    #[inline]
    pub(crate) fn select(self, condition: bool) -> f32 {
        if condition { self.scale + self.offset } else { self.offset }
    }

    /// The post-op of `x·c`, this being `x`'s.
    pub(crate) fn scaled(self, c: f32) -> Post {
        Post { scale: c * self.scale, offset: c * self.offset * 1.0 + 0.0 }
    }

    /// The post-op of `-x`, this being `x`'s.
    pub(crate) fn negated(self) -> Post {
        Post { scale: -self.scale, offset: 0.0 - self.offset }
    }

    /// Two terms of a sum merged into one.
    pub(crate) fn plus(self, other: Post) -> Post {
        Post { scale: self.scale + other.scale, offset: self.offset + other.offset }
    }
}

/// Whether BDS prints two numbers alike when it compares terms (`alike.rs`): to six decimals, so
/// numbers that differ only beyond them count as equal.
pub(crate) fn print_alike(a: f32, b: f32) -> bool {
    // Numbers that print alike are less than 1e-6 apart: most pairs need no printing.
    let near = (a - b).abs() < 1e-5 || (a.is_nan() && b.is_nan());
    a.to_bits() == b.to_bits() || (near && printed(a) == printed(b))
}

fn printed(n: f32) -> String {
    match n {
        _ if n.is_finite() => format!("{:.6}", f64::from(n)),
        _ if n.is_nan() => format!("{}nan", if n.is_sign_negative() { "-" } else { "" }),
        _ => format!("{}inf", if n < 0.0 { "-" } else { "" }),
    }
}
