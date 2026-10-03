//! Switch lowering and `compareTo` / `count` field resolution.

use crate::ir::*;
use crate::lower::Lower;
use crate::names;
use crate::schema::Ty;

impl Lower<'_> {
    /// Resolves a field reference (`name` or `../name`) to its local variable and type.
    /// Lookup walks outward like JS closures, which is how bedrock-protocol's compiled code resolves it.
    pub(crate) fn resolve(&self, path: &str) -> (String, IrTy) {
        let mut skip = 0;
        let mut name = path;
        while let Some(rest) = name.strip_prefix("../") {
            skip += 1;
            name = rest;
        }
        let scopes = &self.scopes[..self.scopes.len().saturating_sub(skip)];
        for scope in scopes.iter().rev() {
            if let Some((_, var, ty)) = scope.fields.iter().rev().find(|(p, _, _)| p == name) {
                return (var.clone(), ty.clone());
            }
        }
        panic!("cannot resolve field reference `{path}`");
    }

    fn disc(&self, compare_to: &str) -> Disc {
        let (var, mut ty) = self.resolve(compare_to);
        let mut optional = false;
        let kind = loop {
            ty = match ty {
                IrTy::Alias { target, .. } => *target,
                IrTy::Option(inner) if !optional => {
                    optional = true;
                    *inner
                }
                IrTy::Item(id) if matches!(self.items[id].kind, ItemKind::Mapper { .. }) => {
                    break DiscKind::Enum(id);
                }
                IrTy::Prim(Prim::Bool) => break DiscKind::Bool,
                IrTy::Prim(p) if p.is_int() => break DiscKind::Int(p),
                IrTy::Str { .. } => break DiscKind::Str,
                other => panic!("unsupported switch discriminant `{compare_to}`: {other:?}"),
            };
        };
        Disc {
            var,
            kind,
            optional,
        }
    }

    /// Whether `key` can ever match this discriminant; unmatched keys are dropped like JS would never select them.
    fn key_valid(&self, disc: &Disc, key: &str) -> bool {
        match &disc.kind {
            DiscKind::Enum(id) => self.items[*id].mapper_variant(key).is_some(),
            DiscKind::Bool => key == "true" || key == "false",
            DiscKind::Int(_) => key.parse::<i64>().is_ok() || key == "/ShieldItemID",
            DiscKind::Str => true,
        }
    }

    pub(crate) fn switch(
        &mut self,
        compare_to: &str,
        cases: &[(String, Ty)],
        default: &Ty,
        hint: &str,
    ) -> IrTy {
        let disc = self.disc(compare_to);
        let cases: Vec<&(String, Ty)> = cases
            .iter()
            .filter(|(k, _)| {
                let ok = self.key_valid(&disc, k);
                if !ok {
                    eprintln!(
                        "warning: {hint}: switch key `{k}` can never match `{compare_to}`, dropped"
                    );
                }
                ok
            })
            .collect();
        let is_void = |t: &Ty| matches!(t, Ty::Ref(n) if n == "void");
        let non_void: Vec<_> = cases.iter().filter(|(_, t)| !is_void(t)).collect();

        if is_void(default) && non_void.len() == 1 {
            let (key, t) = non_void[0];
            let inner = Box::new(self.ty(t, hint));
            return IrTy::SwitchOpt {
                disc,
                keys: vec![key.clone()],
                negate: false,
                inner,
            };
        }
        if !is_void(default) && non_void.is_empty() && !cases.is_empty() {
            let keys = cases.iter().map(|(k, _)| k.clone()).collect();
            let inner = Box::new(self.ty(default, hint));
            return IrTy::SwitchOpt {
                disc,
                keys,
                negate: true,
                inner,
            };
        }

        let id = self.new_item(hint, ItemKind::Pending);
        let enum_name = self.items[id].name.clone();
        let mut taken = Vec::new();
        let mut variants = Vec::new();
        for (key, t) in &cases {
            let base = match &disc.kind {
                DiscKind::Enum(e) => self.items[*e].mapper_variant(key).unwrap().to_owned(),
                _ => names::pascal(key),
            };
            let name = names::dedupe(base, &mut taken);
            let ty = (!is_void(t)).then(|| self.ty(t, &format!("{enum_name}{name}")));
            variants.push(Variant {
                name,
                key: Some(key.clone()),
                ty,
            });
        }
        let name = names::dedupe("Default".to_owned(), &mut taken);
        let ty = (!is_void(default)).then(|| self.ty(default, &format!("{enum_name}{name}")));
        variants.push(Variant {
            name,
            key: None,
            ty,
        });
        self.items[id].kind = ItemKind::Switch { variants };
        IrTy::Switch { disc, item: id }
    }
}
