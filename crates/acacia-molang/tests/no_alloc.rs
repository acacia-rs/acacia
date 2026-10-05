//! Evaluating must not allocate once the scratch space and the variables have grown to fit.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use acacia_molang::{Compiler, Env, Host, Query, Scratch, Structs, Symbol, Value, Variables};

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

struct Bones(Symbol);

impl Host for Bones {
    fn query(&self, _query: Query, args: &[Value], structs: &mut Structs<'_>) -> Value {
        match args {
            [Value::Str(_)] => structs.make(&[(self.0, Value::Num(2.0))]),
            _ => Value::Num(1.5),
        }
    }
}

#[test]
fn evaluation_does_not_allocate() {
    let mut compiler = Compiler::new();
    let sources = [
        "math.sin(q.life_time * 38.17) * 57.3 + this",
        "v.count = (v.count ?? 0) + 1; t.half = v.count / 2; v.half = t.half;",
        "v.origin = q.bone_origin('leg'); v.copy = v.origin; v.copy.y = v.origin.y + 1; v.origin = 0;",
        "v.i = 0; loop(8, { v.i = v.i + q.pick(v.i, 2, 'a'); (v.i > 6) ? break; });",
        "q.is_baby ? 'small' : 'big'",
    ];
    let programs = sources.map(|source| compiler.compile(source).unwrap());
    let host = Bones(compiler.symbol("y"));
    let mut variables = Variables::new();
    let mut scratch = Scratch::new();
    let mut run = |times: usize| {
        for _ in 0..times {
            for program in &programs {
                program.eval(&mut Env { host: &host, variables: &mut variables, scratch: &mut scratch, this: 1.0 });
            }
        }
    };
    run(4);
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    run(100);
    assert_eq!(ALLOCATIONS.load(Ordering::Relaxed) - before, 0);
}
