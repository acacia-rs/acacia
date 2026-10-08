//! Window input as the UI sees it, in GUI pixels; the viewer translates its window events.

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Move([f32; 2]),
    /// The left button.
    Press([f32; 2]),
    Release([f32; 2]),
    /// Wheel notches, up positive.
    Wheel(f32),
    Key(Key, Mods),
    /// Characters typed; a space comes both as this and as [`Key::Space`].
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Tab,
    Enter,
    Space,
    Escape,
    Left,
    Right,
    Up,
    Down,
    Backspace,
    Delete,
    Home,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
}
