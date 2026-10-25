//! `OptionInstance`: an option's caption, value set and value label, and the widget
//! `createButton` makes for it — a `CycleButton` naming the value, or an
//! `OptionInstanceSliderButton` whose message is the label of the value the slider lands on.

use super::defs::def;
use crate::java::text::Text;
use crate::java::widget::button::{CycleButton, Display};
use crate::java::widget::slider::Slider;
use crate::java::widget::{Ctx, Widget};

/// An option's value: a boolean, an integer, a double or an enum constant's (or a string's) name.
#[derive(Clone, Debug, PartialEq)]
pub enum V {
    Bool(bool),
    Int(i32),
    Num(f64),
    Name(String),
}

impl V {
    pub fn b(&self) -> bool {
        matches!(self, V::Bool(true))
    }
    pub fn i(&self) -> i32 {
        match self {
            V::Int(v) => *v,
            V::Num(v) => *v as i32,
            _ => 0,
        }
    }
    pub fn f(&self) -> f64 {
        match self {
            V::Int(v) => f64::from(*v),
            V::Num(v) => *v,
            _ => 0.0,
        }
    }
    pub fn s(&self) -> &str {
        match self {
            V::Name(s) => s,
            _ => "",
        }
    }
}

/// `CaptionBasedToString`.
#[derive(Clone, Copy)]
pub enum Label {
    F(fn(&Ctx, Text, &V) -> Text),
    /// `BOOLEAN_TO_STRING`: on or off.
    OnOff,
    /// An enum's own caption: the key by constant name.
    Keys(&'static [(&'static str, &'static str)]),
}

impl Label {
    pub fn apply(&self, ctx: &Ctx, caption: Text, v: &V) -> Text {
        match self {
            Label::F(f) => f(ctx, caption, v),
            Label::OnOff => ctx.tr(if v.b() { "options.on" } else { "options.off" }),
            Label::Keys(keys) => ctx.tr(keys.iter().find(|k| k.0 == v.s()).map_or("", |k| k.1)),
        }
    }
}

/// The `ValueSet`: cycled through by a button, or slid over.
#[derive(Clone, Copy)]
pub enum Set {
    Cycle,
    /// `IntRange` (and `ClampingLazyMaxIntRange` as a slider).
    Int { min: i32, max: i32 },
    /// `IntRange.xmap(to, from)`.
    Mapped { min: i32, max: i32, to: fn(i32) -> f64, from: fn(f64) -> i32 },
    /// `UnitDouble`.
    Unit,
    /// `UnitDouble.xmap(to, from)`.
    UnitMapped { to: fn(f64) -> f64, from: fn(f64) -> f64 },
    /// `SliderableEnum` over these constants.
    Enum(&'static [&'static str]),
}

pub struct Def {
    pub caption: &'static str,
    pub set: Set,
    pub default: V,
    pub label: Label,
}

fn map(v: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    to_min + (v - from_min) / (from_max - from_min) * (to_max - to_min)
}

/// `slider >= 1` becomes `0.99999F`, so the top of the track lands on the last value.
fn below_one(s: f64) -> f64 {
    if s >= 1.0 { f64::from(0.99999_f32) } else { s }
}

fn int_to_slider(v: i32, min: i32, max: i32) -> f64 {
    if v == min {
        0.0
    } else if v == max {
        1.0
    } else {
        map(f64::from(v) + 0.5, f64::from(min), f64::from(max) + 1.0, 0.0, 1.0)
    }
}

fn int_from_slider(s: f64, min: i32, max: i32) -> i32 {
    map(below_one(s), 0.0, 1.0, f64::from(min), f64::from(max) + 1.0).floor() as i32
}

impl Set {
    /// `toSliderValue`.
    pub fn to_slider(&self, v: &V) -> f64 {
        match *self {
            Set::Cycle => 0.0,
            Set::Int { min, max } => int_to_slider(v.i(), min, max),
            Set::Mapped { min, max, from, .. } => int_to_slider(from(v.f()), min, max),
            Set::Unit => v.f(),
            Set::UnitMapped { from, .. } => from(v.f()),
            Set::Enum(names) => {
                let i = names.iter().position(|n| *n == v.s()).unwrap_or(0);
                if i == 0 {
                    0.0
                } else if i == names.len() - 1 {
                    1.0
                } else {
                    map(i as f64, 0.0, (names.len() - 1) as f64, 0.0, 1.0)
                }
            }
        }
    }

    /// `fromSliderValue`.
    pub fn from_slider(&self, s: f64) -> V {
        match *self {
            Set::Cycle => V::Num(s),
            Set::Int { min, max } => V::Int(int_from_slider(s, min, max)),
            Set::Mapped { min, max, to, .. } => V::Num(to(int_from_slider(s, min, max))),
            Set::Unit => V::Num(s),
            Set::UnitMapped { to, .. } => V::Num(to(s)),
            Set::Enum(names) => {
                let i = map(below_one(s), 0.0, 1.0, 0.0, names.len() as f64).floor() as i32;
                V::Name(names[i.clamp(0, names.len() as i32 - 1) as usize].to_owned())
            }
        }
    }
}

/// The options' values by id (`Options`' getter name; `sound.<source>` for the volumes).
pub type Values<'a> = &'a dyn Fn(&str) -> Option<V>;

/// The option's value, or its default.
pub fn value(values: Values, id: &str) -> V {
    values(id).unwrap_or_else(|| def(id).default)
}

/// `OptionInstance.createButton(options, 0, 0, width)`.
pub fn create_button(ctx: &Ctx, values: Values, id: &str, width: i32) -> Box<dyn Widget> {
    let d = def(id);
    let v = value(values, id);
    let caption = ctx.tr(d.caption);
    let mut w: Box<dyn Widget> = match d.set {
        Set::Cycle => {
            let shown = d.label.apply(ctx, caption.clone(), &v);
            Box::new(CycleButton::new(ctx.lang, 0, 0, width, 20, caption, shown, Display::NameAndValue))
        }
        set => {
            let s = set.to_slider(&v);
            Box::new(Slider::new(0, 0, width, 20, d.label.apply(ctx, caption, &set.from_slider(s)), s))
        }
    };
    w.base_mut().id = id.to_owned().into();
    w
}
