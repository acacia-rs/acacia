//! Encode statements for IR types. `val` is always an expression of type `&T`.

use crate::emit_read::Ctx;
use crate::ir::*;

impl Ctx<'_> {
    fn write_len(p: Prim, len: &str) -> String {
        format!("write_{}(w, {len} as {});", p.codec(), p.rust())
    }

    /// Statements encoding `val` into `w`. `d` keeps nested binding names unique.
    pub fn write(&self, t: &IrTy, val: &str, d: usize) -> String {
        let x = format!("x{d}");
        // `recv` is usable as a method receiver, `deref` as a by-value Copy read.
        let (recv, deref) = match val.strip_prefix('&') {
            Some(place) => (place.to_owned(), place.to_owned()),
            None => (val.to_owned(), format!("*{val}")),
        };
        match t {
            IrTy::Prim(Prim::ByteRot) => {
                format!("write_u8(w, ({deref} / (360.0 / 256.0)) as i32 as u8);")
            }
            IrTy::Prim(p) => format!("write_{}(w, {deref});", p.codec()),
            IrTy::Void => String::new(),
            IrTy::Str { count, latin1 } => {
                let bytes = if *latin1 {
                    format!("&latin1_bytes({val})[..]")
                } else {
                    format!("{recv}.as_bytes()")
                };
                format!(
                    "{{ let b = {bytes}; {} write_slice(w, b); }}",
                    Self::write_len(*count, "b.len()")
                )
            }
            IrTy::Buf { count } => format!(
                "{} write_slice(w, &{recv}[..]);",
                Self::write_len(*count, &format!("{recv}.len()"))
            ),
            IrTy::Rest => format!("write_slice(w, &{recv}[..]);"),
            IrTy::Uuid => format!("write_uuid(w, {val});"),
            IrTy::Nbt { le } => {
                format!(
                    "crate::nbt::write::<crate::nbt::{}>(w, {val});",
                    if *le { "LittleEndian" } else { "Network" }
                )
            }
            IrTy::NbtLoop => format!("crate::nbt::write_loop(w, {val});"),
            IrTy::Item(_) => format!("{recv}.write(w);"),
            IrTy::Inline(id) => {
                let ItemKind::Struct { fields } = &self.items[*id].kind else {
                    panic!("inline non-struct")
                };
                let body: String = fields
                    .iter()
                    .map(|f| self.write(&f.ty, &format!("&{x}.{}", f.name), d + 1))
                    .collect();
                format!("{{ let {x} = {val}; {body} }}")
            }
            IrTy::Alias { target, .. } => self.write(target, val, d),
            IrTy::Array { count, elem } => {
                let len = match count {
                    IrCount::Prim(p) => Self::write_len(*p, &format!("{recv}.len()")),
                    IrCount::Var(_) => String::new(),
                };
                format!(
                    "{len} for {x} in {recv}.iter() {{ {} }}",
                    self.write(elem, &x, d + 1)
                )
            }
            IrTy::MaybeIncomplete { count, elem } => format!(
                "{} for {x} in {recv}.iter() {{ {} }}",
                Self::write_len(*count, &format!("{recv}.len()")),
                self.write(elem, &x, d + 1)
            ),
            IrTy::Fixed { elem, .. } => format!(
                "for {x} in {recv}.iter() {{ {} }}",
                self.write(elem, &x, d + 1)
            ),
            IrTy::Option(inner) => format!(
                "match {val} {{ Some({x}) => {{ write_bool(w, true); {} }} None => write_bool(w, false), }}",
                self.write(inner, &x, d + 1)
            ),
            IrTy::SwitchOpt { inner, .. } | IrTy::OnRemaining(inner) => {
                format!(
                    "if let Some({x}) = {val} {{ {} }}",
                    self.write(inner, &x, d + 1)
                )
            }
            IrTy::Encapsulated { len, inner } => format!(
                "match {val} {{ None => {{ {} }} Some({x}) => write_encapsulated(w, |w| {{ {} }}, |w, n| {{ {} }}), }}",
                Self::write_len(*len, "0"),
                self.write(inner, &x, d + 1),
                Self::write_len(*len, "n"),
            ),
            IrTy::Switch { item, .. } => {
                let ItemKind::Switch { variants } = &self.items[*item].kind else {
                    panic!("switch item")
                };
                let path = self.path(*item);
                let arms: String = variants
                    .iter()
                    .map(|v| match &v.ty {
                        Some(t) => format!(
                            "{path}::{}({x}) => {{ {} }}",
                            v.name,
                            self.write(t, &x, d + 1)
                        ),
                        None => format!("{path}::{} => {{}}", v.name),
                    })
                    .collect();
                format!("match {val} {{ {arms} }}")
            }
        }
    }
}
