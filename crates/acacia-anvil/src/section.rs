//! Java block states of one 16³ section, kept small while a column waits for its neighbours.

use crate::mapping::JavaState;

pub const VOLUME: usize = 4096;

/// Java's in-section order: `(y << 8) | (z << 4) | x`.
#[inline]
pub fn index(x: usize, y: usize, z: usize) -> usize {
    (y << 8) | (z << 4) | x
}

#[derive(Clone)]
pub enum Blocks {
    Uniform(JavaState),
    Small {
        palette: Vec<JavaState>,
        idx: Box<[u8; VOLUME]>,
    },
    Full(Box<[JavaState; VOLUME]>),
}

impl Blocks {
    /// Packs a full section, choosing the smallest form.
    pub fn from_full(states: &[JavaState; VOLUME]) -> Self {
        let first = states[0];
        if states.iter().all(|&s| s == first) {
            return Self::Uniform(first);
        }
        let mut palette: Vec<JavaState> = Vec::new();
        let mut idx = Box::new([0u8; VOLUME]);
        let mut last = (first, 0u8);
        palette.push(first);
        for (i, &s) in states.iter().enumerate() {
            if s != last.0 {
                let p = match palette.iter().position(|&q| q == s) {
                    Some(p) => p,
                    None if palette.len() < 256 => {
                        palette.push(s);
                        palette.len() - 1
                    }
                    None => return Self::Full(Box::new(*states)),
                };
                last = (s, p as u8);
            }
            idx[i] = last.1;
        }
        Self::Small { palette, idx }
    }

    #[inline]
    pub fn get(&self, i: usize) -> JavaState {
        match self {
            Self::Uniform(s) => *s,
            Self::Small { palette, idx } => palette[idx[i] as usize],
            Self::Full(f) => f[i],
        }
    }

    pub fn copy_to(&self, out: &mut [JavaState; VOLUME]) {
        match self {
            Self::Uniform(s) => out.fill(*s),
            Self::Small { palette, idx } => {
                for (o, &i) in out.iter_mut().zip(idx.iter()) {
                    *o = palette[i as usize];
                }
            }
            Self::Full(f) => out.copy_from_slice(&f[..]),
        }
    }

    pub fn uniform(&self) -> Option<JavaState> {
        match self {
            Self::Uniform(s) => Some(*s),
            _ => None,
        }
    }

    /// Bytes held on the heap (for the memory bound).
    pub fn heap_bytes(&self) -> usize {
        match self {
            Self::Uniform(_) => 0,
            Self::Small { palette, .. } => VOLUME + palette.capacity() * 2,
            Self::Full(_) => VOLUME * 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forms() {
        let mut s = [7 as JavaState; VOLUME];
        assert!(matches!(Blocks::from_full(&s), Blocks::Uniform(7)));
        s[100] = 9;
        let b = Blocks::from_full(&s);
        assert!(matches!(b, Blocks::Small { .. }));
        assert_eq!((b.get(100), b.get(101)), (9, 7));
        for (i, v) in s.iter_mut().enumerate() {
            *v = i as JavaState;
        }
        let b = Blocks::from_full(&s);
        assert!(matches!(b, Blocks::Full(_)));
        let mut out = [0; VOLUME];
        b.copy_to(&mut out);
        assert_eq!(out, s);
    }
}
