//! Input as Java 26.3's widgets take it (`GuiEventListener`, `AbstractWidget`,
//! `ContainerEventHandler`): the keys, what a press fires, and the routing over a screen's children
//! flattened in their order (a list's rows in its place, clipped to it). Screens are rebuilt from
//! the client's state, so the focus and drag state lives in [`Interaction`], kept by their owner.

use std::borrow::Cow;

use super::Widget;

/// A key the GUI handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Backspace,
    Delete,
    Home,
    End,
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Tab,
    Escape,
    /// Enter or keypad Enter.
    Enter,
    Space,
    /// The `A` key (Ctrl+A selects all).
    A,
}

impl Key {
    /// `KeyEvent.isSelection`.
    pub fn is_selection(self) -> bool {
        matches!(self, Key::Enter | Key::Space)
    }
}

/// What pressing a widget does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// `Button.onPress`.
    Press,
    /// `CycleButton.onPress`: to the next value, or the previous with Shift.
    Cycle { back: bool },
    /// A slider moved to this value (0 to 1).
    Slide(f64),
}

/// A widget's action, by the widget's id.
#[derive(Clone, Debug, PartialEq)]
pub struct Fired {
    pub id: Cow<'static, str>,
    pub action: Action,
}

/// A screen's child, and the box a list clips it to.
pub struct Child<'a> {
    pub widget: &'a mut dyn Widget,
    pub clip: Option<[i32; 4]>,
}

impl Child<'_> {
    /// `getChildAt`'s test: inside the list holding it and `isMouseOver` (active and inside).
    fn over(&self, x: f64, y: f64) -> bool {
        let inside = |[x0, y0, x1, y1]: [i32; 4]| x >= f64::from(x0) && y >= f64::from(y0) && x < f64::from(x1) && y < f64::from(y1);
        let w = &*self.widget;
        let b = w.base();
        self.clip.is_none_or(inside) && b.is_active() && inside([b.x, b.y, b.x + w.width(), b.y + w.height()])
    }

    fn fired(&self, action: Option<Action>) -> Option<Fired> {
        action.map(|action| Fired { id: self.widget.base().id.clone(), action })
    }
}

/// `ContainerEventHandler`'s focused child and dragging flag (the screen's flattened children),
/// the focused slider's `canChangeValue`, and a list's scroll and scroll-bar drag.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Interaction {
    pub focused: Option<usize>,
    pub dragging: bool,
    pub can_change: bool,
    pub scroll: f64,
    pub scrolling: bool,
}

/// The focus a rebuilt screen had.
pub fn restore(children: &mut [Child], s: &Interaction) {
    if let Some(c) = s.focused.and_then(|i| children.get_mut(i)) {
        c.widget.base_mut().focused = true;
        c.widget.set_can_change(s.can_change);
    }
}

/// `setFocused`: the old child loses focus, the new one takes it (`pointer`: by the mouse or Tab).
pub fn focus(children: &mut [Child], s: &mut Interaction, i: Option<usize>, pointer: bool) {
    if let Some(old) = s.focused.and_then(|o| children.get_mut(o)) {
        old.widget.set_focused(false, pointer);
    }
    s.focused = i;
    s.can_change = false;
    if let Some(c) = i.and_then(|i| children.get_mut(i)) {
        c.widget.set_focused(true, pointer);
        s.can_change = c.widget.can_change();
    }
}

/// `ContainerEventHandler.mouseClicked` with the left button: the first child under the mouse is
/// clicked and focused.
pub fn mouse_clicked(children: &mut [Child], s: &mut Interaction, x: f64, y: f64, shift: bool) -> Option<Fired> {
    let i = children.iter().position(|c| c.over(x, y))?;
    let action = children[i].widget.on_click(x, shift);
    focus(children, s, Some(i), true);
    s.dragging = true;
    children[i].fired(action)
}

/// `mouseReleased`: the drag ends on the focused child.
pub fn mouse_released(children: &mut [Child], s: &mut Interaction) {
    if s.dragging {
        s.dragging = false;
        if let Some(c) = s.focused.and_then(|i| children.get_mut(i)) {
            c.widget.on_release();
        }
    }
}

/// `mouseDragged`: the focused child, while dragging.
pub fn mouse_dragged(children: &mut [Child], s: &Interaction, x: f64) -> Option<Fired> {
    let c = children.get_mut(s.focused.filter(|_| s.dragging)?)?;
    let action = c.widget.on_drag(x);
    c.fired(action)
}

/// `Screen.keyPressed` after Escape: the focused child's `keyPressed`, else Tab moves the focus.
/// Arrow-key focus navigation (`createArrowEvent`) is not ported.
pub fn key_pressed(children: &mut [Child], s: &mut Interaction, key: Key, shift: bool) -> Option<Fired> {
    if let Some(c) = s.focused.and_then(|i| children.get_mut(i))
        && let Some(action) = c.widget.key_pressed(key, shift)
    {
        s.can_change = c.widget.can_change();
        return c.fired(action);
    }
    if key == Key::Tab {
        let next = tab_target(children, s.focused, !shift);
        focus(children, s, next, true);
    }
    None
}

/// `nextFocusPath(TabNavigation)`: the next active child after the focused one, wrapping round
/// (`clearFocus` and again from the end).
fn tab_target(children: &[Child], from: Option<usize>, forward: bool) -> Option<usize> {
    let n = children.len();
    let active = |i: &usize| children[*i].widget.base().is_active();
    let order: Vec<usize> = match (from, forward) {
        (Some(f), true) => (f + 1..n).chain(0..n).collect(),
        (Some(f), false) => (0..f).rev().chain((0..n).rev()).collect(),
        (None, true) => (0..n).collect(),
        (None, false) => (0..n).rev().collect(),
    };
    order.into_iter().find(active)
}

/// What a widget's key handler did: `None` when the key is not its, else what it fires.
pub type KeyOutcome = Option<Option<Action>>;
