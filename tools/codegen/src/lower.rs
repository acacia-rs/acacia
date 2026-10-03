//! AST → IR: resolves named types, hoists inline containers/enums into named items, binds field references.

use std::collections::HashMap;

use crate::ir::*;
use crate::names;
use crate::schema::{Count, Field, Schema, Ty, TypeDef};

pub struct Lowered {
    pub items: Vec<Item>,
}

pub(crate) struct Scope {
    /// (ProtoDef field name, local variable, type)
    pub(crate) fields: Vec<(String, String, IrTy)>,
}

pub(crate) struct Lower<'a> {
    pub(crate) schema: &'a Schema,
    pub(crate) items: Vec<Item>,
    taken: HashMap<Module, Vec<String>>,
    memo: HashMap<String, IrTy>,
    pub(crate) scopes: Vec<Scope>,
    module: Module,
    root: usize,
    packet_ids: HashMap<String, (u32, String)>,
}

pub fn lower(schema: &Schema, packet_ids: HashMap<String, (u32, String)>) -> Lowered {
    let mut l = Lower {
        schema,
        items: Vec::new(),
        taken: HashMap::new(),
        memo: HashMap::new(),
        scopes: Vec::new(),
        module: Module::Types,
        root: 0,
        packet_ids,
    };
    // Reserve every top-level name first so hoisted inline items never steal one.
    for (name, def) in &schema.types {
        if let TypeDef::Def(_) = def {
            let (module, rust) = l.top_level_name(name);
            l.taken.entry(module).or_default().push(rust);
        }
    }
    for (i, (name, def)) in schema.types.iter().enumerate() {
        if matches!(def, TypeDef::Def(_)) && name != "mcpe_packet" {
            l.root = i;
            l.named(name);
        }
    }
    Lowered { items: l.items }
}

impl<'a> Lower<'a> {
    fn top_level_name(&self, name: &str) -> (Module, String) {
        match name.strip_prefix("packet_") {
            Some(p) => (Module::Packets, names::pascal(p)),
            None => (Module::Types, names::pascal(name)),
        }
    }

    pub(crate) fn new_item(&mut self, base: &str, kind: ItemKind) -> ItemId {
        let name = names::dedupe(base.to_owned(), self.taken.entry(self.module).or_default());
        self.push_item(name, kind)
    }

    fn push_item(&mut self, name: String, kind: ItemKind) -> ItemId {
        self.items.push(Item {
            name,
            module: self.module,
            kind,
            root: self.root,
            packet: None,
            standalone: false,
        });
        self.items.len() - 1
    }

    /// Lowers a reference to a type from the `types` section, once.
    fn named(&mut self, name: &str) -> IrTy {
        if let Some(t) = self.memo.get(name) {
            return t.clone();
        }
        let Some(TypeDef::Def(def)) = self.schema.get(name) else {
            panic!("unknown or unhandled native type `{name}`");
        };
        let def = def.clone();
        let (module, rust) = self.top_level_name(name);
        let saved = (std::mem::take(&mut self.scopes), self.module, self.root);
        self.module = module;
        self.root = self
            .schema
            .types
            .iter()
            .position(|(n, _)| n == name)
            .expect("known type");
        let ty = match &def {
            Ty::Container(fields) => {
                let id = self.push_item(rust.clone(), ItemKind::Pending);
                self.items[id].packet = name
                    .strip_prefix("packet_")
                    .and_then(|p| self.packet_ids.get(p).cloned());
                self.items[id].standalone = true;
                self.memo.insert(name.to_owned(), IrTy::Item(id));
                self.container_into(id, fields);
                IrTy::Item(id)
            }
            Ty::Mapper { .. } | Ty::Bitflags { .. } | Ty::Bitfield(_) => {
                let id = self.push_item(rust.clone(), ItemKind::Pending);
                self.memo.insert(name.to_owned(), IrTy::Item(id));
                let IrTy::Item(inner) = self.ty(&def, &rust) else {
                    unreachable!()
                };
                // `ty` hoisted a second item under a deduped name; move its body into the reserved one.
                self.items[id].kind =
                    std::mem::replace(&mut self.items[inner].kind, ItemKind::Pending);
                self.items.remove(inner);
                IrTy::Item(id)
            }
            Ty::Ref(_) | Ty::PString { .. } | Ty::Buffer { .. } => self.ty(&def, &rust),
            _ => {
                let id = self.push_item(rust.clone(), ItemKind::Pending);
                let target = self.ty(&def, &rust);
                self.items[id].kind = ItemKind::Alias {
                    target: target.clone(),
                };
                IrTy::Alias {
                    item: id,
                    target: Box::new(target),
                }
            }
        };
        (self.scopes, self.module, self.root) = saved;
        self.memo.insert(name.to_owned(), ty.clone());
        ty
    }

    pub(crate) fn prim(&mut self, t: &Ty) -> Prim {
        match self.ty(t, "Count") {
            IrTy::Prim(p) => p,
            other => panic!("expected a primitive, got {other:?}"),
        }
    }

    /// Lowers a type expression; `hint` names any item hoisted out of it.
    pub(crate) fn ty(&mut self, t: &Ty, hint: &str) -> IrTy {
        match t {
            Ty::Ref(n) => match n.as_str() {
                "void" => IrTy::Void,
                "uuid" => IrTy::Uuid,
                "restBuffer" => IrTy::Rest,
                "nbt" => IrTy::Nbt { le: false },
                "lnbt" => IrTy::Nbt { le: true },
                "nbtLoop" => IrTy::NbtLoop,
                n => match Prim::from_name(n) {
                    Some(p) => IrTy::Prim(p),
                    None => self.named(n),
                },
            },
            Ty::Container(fields) => {
                let id = self.new_item(hint, ItemKind::Pending);
                self.container_into(id, fields);
                IrTy::Inline(id)
            }
            Ty::Array { count, elem } => {
                let elem = Box::new(self.ty(elem, &format!("{hint}Item")));
                match count {
                    Count::Type(c) => IrTy::Array {
                        count: IrCount::Prim(self.prim(c)),
                        elem,
                    },
                    Count::Fixed(n) => IrTy::Fixed { n: *n, elem },
                    Count::Field(f) => IrTy::Array {
                        count: IrCount::Var(self.resolve(f).0),
                        elem,
                    },
                }
            }
            Ty::Option(inner) => IrTy::Option(Box::new(self.ty(inner, hint))),
            Ty::Switch {
                compare_to,
                cases,
                default,
            } => self.switch(compare_to, cases, default, hint),
            Ty::Mapper { repr, mappings } => {
                let repr = self.prim(repr);
                let mut taken = vec!["Unknown".to_owned()];
                let variants = mappings
                    .iter()
                    .map(|(k, v)| {
                        // `Unknown(i64)` is the open-enum fallback; a mapping literally named "unknown" yields.
                        let base = match names::pascal(v) {
                            p if p == "Unknown" => "UnknownValue".to_owned(),
                            p => p,
                        };
                        (*k, v.clone(), names::dedupe(base, &mut taken))
                    })
                    .collect();
                IrTy::Item(self.new_item(hint, ItemKind::Mapper { repr, variants }))
            }
            Ty::Bitflags { repr, flags } => {
                let repr = self.prim(repr);
                let mut taken = Vec::new();
                let flags = flags
                    .iter()
                    .map(|(n, v)| (names::dedupe(names::upper(n), &mut taken), *v))
                    .collect();
                IrTy::Item(self.new_item(hint, ItemKind::Bitflags { repr, flags }))
            }
            Ty::Bitfield(fs) => {
                let mut taken = Vec::new();
                let fields = fs
                    .iter()
                    .map(|f| {
                        (
                            names::dedupe(names::field(&f.name), &mut taken),
                            f.size,
                            f.signed,
                        )
                    })
                    .collect();
                IrTy::Item(self.new_item(hint, ItemKind::Bitfield { fields }))
            }
            Ty::Encapsulated { len, inner } => IrTy::Encapsulated {
                len: self.prim(len),
                inner: Box::new(self.ty(inner, hint)),
            },
            Ty::PString { count, latin1 } => IrTy::Str {
                count: self.prim(count),
                latin1: *latin1,
            },
            Ty::Buffer { count } => IrTy::Buf {
                count: self.prim(count),
            },
            Ty::MaybeIncompleteArray { count, elem } => IrTy::MaybeIncomplete {
                count: self.prim(count),
                elem: Box::new(self.ty(elem, &format!("{hint}Item"))),
            },
            Ty::OptionalOnRemaining(inner) => IrTy::OnRemaining(Box::new(self.ty(inner, hint))),
        }
    }

    /// Lowers container fields into item `id`, in a fresh scope nested in the current one.
    fn container_into(&mut self, id: ItemId, fields: &[Field]) {
        let depth = self.scopes.len();
        let prefix = if depth == 0 {
            "f".to_owned()
        } else {
            format!("f{depth}")
        };
        self.scopes.push(Scope { fields: Vec::new() });
        let item_name = self.items[id].name.clone();
        let mut taken = Vec::new();
        let mut out = Vec::new();
        for f in fields {
            let proto = f.name.clone().unwrap_or_else(|| "content".to_owned());
            let name = names::dedupe(names::field(&proto), &mut taken);
            let var = format!("{prefix}_{}", name.trim_start_matches("r#"));
            let ty = self.ty(&f.ty, &format!("{item_name}{}", names::pascal(&proto)));
            self.scopes
                .last_mut()
                .unwrap()
                .fields
                .push((proto, var.clone(), ty.clone()));
            out.push(IrField { name, var, ty });
        }
        self.scopes.pop();
        self.items[id].kind = ItemKind::Struct { fields: out };
    }
}
