//! What the BDS oracle cannot be asked: queries, context variables, arrays, `->`, `this`, randomness.

use acacia_molang::{Arrays, Compiler, Context, Engine, Env, ErrorKind, Host, Query, Scratch, Structs, Symbol, Value, Variables};

struct Owner;

impl Host for Owner {}

struct Game {
    variant: Query,
    property: Query,
    bone_origin: Query,
    nearby: Query,
    owning_entity: Context,
    warm: Symbol,
    xy: [Symbol; 2],
    owner: Variables,
    random: f32,
}

impl Host for Game {
    fn query(&self, query: Query, args: &[Value], structs: &mut Structs<'_>) -> Value {
        match query {
            q if q == self.variant => Value::Num(4.0),
            q if q == self.property => Value::Str(self.warm),
            q if q == self.bone_origin => structs.make(&[(self.xy[0], Value::Num(args.len() as f32)), (self.xy[1], Value::Num(7.0))]),
            q if q == self.nearby => structs.entities(&[9, 4, 9]),
            _ => Value::ZERO,
        }
    }

    fn context(&self, context: Context) -> Option<Value> {
        (context == self.owning_entity).then_some(Value::Entity(9))
    }

    fn random(&self) -> f32 {
        self.random
    }

    fn entity(&self, entity: u32) -> Option<(&dyn Host, &Variables)> {
        (entity == 9).then_some((&Owner, &self.owner))
    }
}

struct World {
    compiler: Compiler,
    game: Game,
    variables: Variables,
    scratch: Scratch,
}

impl World {
    fn new() -> World {
        let mut compiler = Compiler::new();
        let mut owner = Variables::new();
        owner.set(compiler.variable("speed"), 3.0);
        let game = Game {
            variant: compiler.query("variant"),
            property: compiler.query("property"),
            bone_origin: compiler.query("bone_origin"),
            nearby: compiler.query("get_nearby_entities"),
            owning_entity: compiler.context("owning_entity"),
            warm: compiler.symbol("warm"),
            xy: [compiler.symbol("x"), compiler.symbol("y")],
            owner,
            random: 0.5,
        };
        World { compiler, game, variables: Variables::new(), scratch: Scratch::new() }
    }

    fn run_with(&mut self, source: &str, arrays: &Arrays, this: f32) -> Value {
        let program = self.compiler.compile_with(source, arrays).unwrap_or_else(|error| panic!("{source}: {error}"));
        program.eval(&mut Env { host: &self.game, variables: &mut self.variables, scratch: &mut self.scratch, this })
    }

    fn run(&mut self, source: &str) -> Value {
        self.run_with(source, &Arrays::new(), 0.0)
    }

    fn num(&mut self, source: &str) -> f32 {
        self.run(source).num()
    }
}

#[test]
fn queries_answer_numbers_and_strings() {
    let mut world = World::new();
    assert_eq!(world.num("Query.Variant + 1"), 5.0);
    assert_eq!(world.num("q.property('minecraft:climate_variant') == 'warm' ? 1 : 2"), 1.0);
    assert_eq!(world.num("q.property('minecraft:climate_variant') == 'Warm' ? 1 : 2"), 2.0);
    assert_eq!(world.num("q.never_heard_of(1, 2) + 1"), 1.0);
}

#[test]
fn arrays_index_their_elements() {
    let mut world = World::new();
    let arrays: Arrays = [("skins".to_owned(), vec!["Texture.white".to_owned(), "texture.black".to_owned(), "q.variant".to_owned()])].into();
    // Variant 4 of three wraps to the second.
    let Value::Str(picked) = world.run_with("Array.skins[query.variant]", &arrays, 0.0) else { panic!("not a resource") };
    assert_eq!(world.compiler.text(picked), "texture.black");
    assert_eq!(world.run_with("array.skins[2] * 2", &arrays, 0.0), Value::Num(8.0));
    assert_eq!(world.run_with("array.skins[-1] == texture.white", &arrays, 0.0), Value::Num(1.0));
    let missing = world.compiler.compile("array.skins[0]").unwrap_err();
    assert_eq!(missing.kind, ErrorKind::UnknownArray("skins".to_owned()));
}

#[test]
fn arrow_reads_the_other_entity() {
    let mut world = World::new();
    assert_eq!(world.num("v.speed = 100; return c.owning_entity->v.speed * 2;"), 6.0);
    assert_eq!(world.num("-c.owning_entity->v.speed"), -3.0);
    assert_eq!(world.num("c.owning_entity->v.unset ?? 5"), 5.0);
    assert_eq!(world.num("c.nobody->v.speed"), 0.0);
    assert_eq!(world.num("c.nobody ?? 8"), 8.0);
}

#[test]
fn queries_return_structs() {
    let mut world = World::new();
    assert_eq!(world.num("v.origin = q.bone_origin('leg', 2); return v.origin.x * 10 + v.origin.y;"), 27.0);
    // The copy stays after the evaluation that made the struct.
    assert_eq!(world.num("v.origin.y"), 7.0);
    let origin = world.compiler.variable("origin");
    let held = world.variables.get(origin).unwrap();
    assert_eq!(world.variables.member(held, world.game.xy[0]), Some(Value::Num(2.0)));

    let returned = world.run("q.bone_origin");
    assert_eq!(world.scratch.member(returned, world.game.xy[1]), Some(Value::Num(7.0)));
}

#[test]
fn for_each_walks_an_entity_array() {
    let mut world = World::new();
    // Entity 9 has speed 3; entity 4 does not exist.
    let sum = "v.sum = 0; for_each(t.other, q.get_nearby_entities(4, 'minecraft:pig'), { v.sum = v.sum + t.other->v.speed; }); return v.sum;";
    assert_eq!(world.num(sum), 6.0);
    let stop = "v.seen = 0; for_each(v.other, q.get_nearby_entities(4), { v.seen = v.seen + 1; (v.seen == 2) ? break; }); return v.seen;";
    assert_eq!(world.num(stop), 2.0);
    // An array kept in a variable is still there for a later program.
    world.run("v.found = q.get_nearby_entities(4);");
    assert_eq!(world.num("v.n = 0; for_each(t.e, v.found, { v.n = v.n + 1; v.found = 0; }); return v.n;"), 1.0);
    assert_eq!(world.num("v.n = 0; for_each(t.e, 5, { v.n = v.n + 1; }); return v.n;"), 0.0);
    assert_eq!(world.compiler.compile("for_each(q.x, v.found, { v.n = 1; });").unwrap_err().kind, ErrorKind::NotAssignable);
}

#[test]
fn old_packs_follow_old_rules() {
    let mut world = World::new();
    world.variables.set(world.compiler.variable("d"), -2.0);
    assert_eq!(world.num("1 ? 0 : 1 ? 3 : 4"), 0.0);
    assert_eq!(world.num("6 / v.d"), -3.0);
    world.compiler.engine = Engine(1, 13, 0);
    assert_eq!(world.num("1 ? 0 : 1 ? 3 : 4"), 4.0);
    assert_eq!(world.num("1 || 0 && 0"), 0.0);
    assert_eq!(world.num("6 / v.d"), 3.0);
}

#[test]
fn hosts_set_variables_and_members() {
    let mut world = World::new();
    let shulker = world.compiler.variable("Shulker");
    world.variables.set_member(shulker, &world.game.xy, 4.0);
    assert_eq!(world.num("v.shulker.x.y + 1"), 5.0);
    world.variables.set(shulker, 2.0);
    assert_eq!(world.num("v.shulker"), 2.0);
    world.variables.clear();
    assert_eq!(world.num("v.shulker ?? -1"), -1.0);
}

#[test]
fn this_and_randomness_come_from_the_caller() {
    let mut world = World::new();
    assert_eq!(world.run_with("this * 2", &Arrays::new(), 3.0), Value::Num(6.0));
    assert_eq!(world.num("math.random(2, 4)"), 3.0);
    assert_eq!(world.num("math.die_roll(2, 1, 3)"), 4.0);
    world.game.random = 0.999;
    assert_eq!(world.num("math.random_integer(1, 6)"), 6.0);
    world.game.random = 0.0;
    assert_eq!(world.num("math.random_integer(1, 6)"), 1.0);
}

#[test]
fn a_script_split_over_lines_is_one_program() {
    let mut world = World::new();
    let script = ["v.equipped = q.variant > 3;", "(v.equipped) ? {", "  t.raise = q.variant * 2;", "  v.raise = t.raise + 1;", "};"].join(" ");
    world.run(&script);
    assert_eq!(world.num("v.raise"), 9.0);
}

#[test]
fn constants_are_recognised() {
    let mut compiler = Compiler::new();
    assert_eq!(compiler.compile("-20.0").unwrap().as_constant(), Some(-20.0));
    assert_eq!(compiler.compile("q.life_time").unwrap().as_constant(), None);
    assert_eq!(Compiler::constant(1.5).as_constant(), Some(1.5));
}
