//! Rust type names and decode expressions for IR types.

use crate::ir::*;

/// Emission context: which module the code lands in decides how item paths are spelled.
pub struct Ctx<'a> {
    pub items: &'a [Item],
    pub module: Module,
}

impl Ctx<'_> {
    pub fn path(&self, id: ItemId) -> String {
        let item = &self.items[id];
        match (item.module, self.module) {
            (Module::Types, Module::Packets) => format!("types::{}", item.name),
            (Module::Packets, Module::Types) => format!("crate::packets::{}", item.name),
            _ => item.name.clone(),
        }
    }

    pub fn rust_ty(&self, t: &IrTy) -> String {
        match t {
            IrTy::Prim(p) => p.rust().to_owned(),
            IrTy::Void => "()".to_owned(),
            IrTy::Str { .. } => "String".to_owned(),
            IrTy::Buf { .. } | IrTy::Rest => "Bytes".to_owned(),
            IrTy::Uuid => "crate::manual::Uuid".to_owned(),
            IrTy::Nbt { le: true } => "crate::nbt::Nbt".to_owned(),
            IrTy::Nbt { le: false } => "crate::nbt::Raw<crate::nbt::Network>".to_owned(),
            IrTy::NbtLoop => "Vec<crate::nbt::Nbt>".to_owned(),
            IrTy::Item(id)
            | IrTy::Inline(id)
            | IrTy::Switch { item: id, .. }
            | IrTy::Alias { item: id, .. } => self.path(*id),
            IrTy::Array { elem, .. } | IrTy::MaybeIncomplete { elem, .. } => {
                format!("Vec<{}>", self.rust_ty(elem))
            }
            IrTy::Fixed { n, elem } => format!("[{}; {n}]", self.rust_ty(elem)),
            IrTy::Option(i)
            | IrTy::SwitchOpt { inner: i, .. }
            | IrTy::Encapsulated { inner: i, .. }
            | IrTy::OnRemaining(i) => {
                format!("Option<{}>", self.rust_ty(i))
            }
        }
    }

    pub fn count_read(p: Prim) -> String {
        format!("to_len(read_{}(r)?)?", p.codec())
    }

    /// A boolean expression testing the discriminant against one switch key.
    pub fn key_test(&self, disc: &Disc, key: &str) -> String {
        let rhs = match &disc.kind {
            DiscKind::Enum(id) => {
                let variant = self.items[*id].mapper_variant(key).expect("validated key");
                format!("{}::{variant}", self.path(*id))
            }
            DiscKind::Int(p) if key == "/ShieldItemID" => {
                format!("(crate::manual::shield_item_id() as {})", p.rust())
            }
            DiscKind::Bool | DiscKind::Int(_) => key.to_owned(),
            DiscKind::Str => format!("{key:?}"),
        };
        match (&disc.kind, disc.optional) {
            (DiscKind::Str, true) => format!("{}.as_deref() == Some({rhs})", disc.var),
            (_, true) => format!("{} == Some({rhs})", disc.var),
            _ => format!("{} == {rhs}", disc.var),
        }
    }

    /// An expression decoding `t` from `r: &mut &[u8]`, propagating errors with `?`.
    pub fn read(&self, t: &IrTy) -> String {
        match t {
            IrTy::Prim(Prim::ByteRot) => "(read_u8(r)? as f32 * (360.0 / 256.0))".to_owned(),
            IrTy::Prim(p) => format!("read_{}(r)?", p.codec()),
            IrTy::Void => "()".to_owned(),
            IrTy::Str { count, latin1 } => {
                let f = if *latin1 { "read_latin1" } else { "read_utf8" };
                format!("{{ let n = {}; {f}(r, n)? }}", Self::count_read(*count))
            }
            IrTy::Buf { count } => format!(
                "{{ let n = {}; read_bytes(r, n)? }}",
                Self::count_read(*count)
            ),
            IrTy::Rest => "read_rest(r)".to_owned(),
            IrTy::Uuid => "read_uuid(r)?".to_owned(),
            IrTy::Nbt { le: true } => {
                "crate::nbt::read::<crate::nbt::LittleEndian>(r)?".to_owned()
            }
            IrTy::Nbt { le: false } => "crate::nbt::read_raw::<crate::nbt::Network>(r)?".to_owned(),
            IrTy::NbtLoop => "crate::nbt::read_loop(r)?".to_owned(),
            IrTy::Item(id) => format!("{}::read(r)?", self.path(*id)),
            IrTy::Inline(id) => self.read_inline_struct(*id),
            IrTy::Alias { target, .. } => self.read(target),
            IrTy::Array { count, elem } => {
                let n = match count {
                    IrCount::Prim(p) => Self::count_read(*p),
                    IrCount::Var(v) => format!("to_len({v})?"),
                };
                format!(
                    "{{ let n = {n}; let mut v = Vec::with_capacity(cap(n, r)); for _ in 0..n {{ v.push({}); }} v }}",
                    self.read(elem)
                )
            }
            IrTy::Fixed { n, elem } => {
                let e = self.read(elem);
                format!("[{}]", vec![e; *n].join(", "))
            }
            IrTy::Option(inner) => format!(
                "if read_bool(r)? {{ Some({}) }} else {{ None }}",
                self.read(inner)
            ),
            IrTy::SwitchOpt {
                disc,
                keys,
                negate,
                inner,
            } => {
                let test = keys
                    .iter()
                    .map(|k| self.key_test(disc, k))
                    .collect::<Vec<_>>()
                    .join(" || ");
                let test = if *negate {
                    format!("!({test})")
                } else {
                    format!("({test})")
                };
                format!("if {test} {{ Some({}) }} else {{ None }}", self.read(inner))
            }
            IrTy::Switch { disc, item } => self.read_switch(disc, *item),
            IrTy::Encapsulated { len, inner } => format!(
                "{{ let n = {}; if n == 0 {{ None }} else {{ let mut sub = take(r, n)?; let r = &mut sub; let v = {}; \
                 if !r.is_empty() {{ crate::strict::note(crate::strict::Leniency::BlockRest(r.len())); }} Some(v) }} }}",
                Self::count_read(*len),
                self.read(inner)
            ),
            IrTy::MaybeIncomplete { count, elem } => format!(
                "{{ let n = {}; let mut v = Vec::new(); for _ in 0..n {{ if r.is_empty() {{ break; }} let save = *r; \
                 match (|r: &mut &[u8]| -> Result<_> {{ Ok({}) }})(r) {{ Ok(x) => v.push(x), \
                 Err(e) if matches!(e.root(), DecodeError::Eof {{ .. }}) => {{ *r = save; break; }} Err(e) => return Err(e), }} }} \
                 if v.len() < n {{ crate::strict::note(crate::strict::Leniency::TruncatedList); }} v }}",
                Self::count_read(*count),
                self.read(elem)
            ),
            IrTy::OnRemaining(inner) => format!(
                "if r.is_empty() {{ None }} else {{ Some({}) }}",
                self.read(inner)
            ),
        }
    }

    fn read_inline_struct(&self, id: ItemId) -> String {
        let ItemKind::Struct { fields } = &self.items[id].kind else {
            panic!("inline non-struct")
        };
        let mut s = String::from("{ ");
        for f in fields {
            s += &format!("let {} = {}; ", f.var, self.read(&f.ty));
        }
        let inits: Vec<String> = fields
            .iter()
            .map(|f| format!("{}: {}", f.name, f.var))
            .collect();
        s += &format!("{} {{ {} }} }}", self.path(id), inits.join(", "));
        s
    }

    fn read_switch(&self, disc: &Disc, id: ItemId) -> String {
        let ItemKind::Switch { variants } = &self.items[id].kind else {
            panic!("switch item")
        };
        let path = self.path(id);
        let build = |v: &Variant| match &v.ty {
            Some(t) => format!("{path}::{}({})", v.name, self.read(t)),
            None => format!("{path}::{}", v.name),
        };
        let mut s = String::new();
        for v in variants {
            match &v.key {
                Some(k) => s += &format!("if {} {{ {} }} else ", self.key_test(disc, k), build(v)),
                None => s += &format!("{{ {} }}", build(v)),
            }
        }
        s
    }
}
