//! MoLang ingredients: an expression over the item offered, such as
//! `query.any_tag('minecraft:planks', 'minecraft:logs')`. Tags come from `tags.rs`; every other
//! query answers 0.

use acacia_molang::{Compiler, Env, Host, Query, Scratch, Structs, Value, Variables};

use super::tags;

struct Offered<'a> {
    name: &'a str,
    compiler: &'a Compiler,
    any_tag: Query,
    all_tags: Query,
}

impl Host for Offered<'_> {
    fn query(&self, query: Query, args: &[Value], _structs: &mut Structs<'_>) -> Value {
        let tagged = |arg: &Value| matches!(arg, Value::Str(tag) if tags::has_tag(self.name, self.compiler.text(*tag)));
        if query == self.any_tag {
            args.iter().any(tagged).into()
        } else if query == self.all_tags {
            (!args.is_empty() && args.iter().all(tagged)).into()
        } else {
            Value::ZERO
        }
    }
}

/// Whether the item named `name` satisfies `expression`. One that does not compile accepts nothing.
// Compiled per call: these ingredients are rare, and compiling takes a few microseconds.
pub(super) fn accepts(expression: &str, name: &str) -> bool {
    let mut compiler = Compiler::new();
    let Ok(program) = compiler.compile(expression) else { return false };
    let (any_tag, all_tags) = (compiler.query("any_tag"), compiler.query("all_tags"));
    let host = Offered { name, compiler: &compiler, any_tag, all_tags };
    program.eval(&mut Env { host: &host, variables: &mut Variables::new(), scratch: &mut Scratch::new(), this: 0.0 }).truthy()
}

#[cfg(test)]
mod tests {
    use super::accepts;

    #[test]
    fn expressions_over_tags() {
        let either = "query.any_tag('minecraft:planks', 'minecraft:logs')";
        assert!(accepts(either, "minecraft:oak_planks") && accepts(either, "minecraft:birch_log") && !accepts(either, "minecraft:stone"));
        let both = "q.all_tags('minecraft:logs', 'minecraft:logs_that_burn')";
        assert!(accepts(both, "minecraft:oak_log") && !accepts(both, "minecraft:crimson_stem"));
        let burning_only = "q.any_tag('minecraft:logs') && !q.any_tag('minecraft:logs_that_burn')";
        assert!(accepts(burning_only, "minecraft:warped_stem") && !accepts(burning_only, "minecraft:oak_log"));
        assert!(!accepts("query.is_item_name_any('slot.weapon.mainhand', 0, 'minecraft:stick')", "minecraft:stick"));
        assert!(!accepts("query.any_tag(", "minecraft:oak_planks"));
    }
}
