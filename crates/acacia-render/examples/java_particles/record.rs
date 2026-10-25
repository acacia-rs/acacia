//! The records both sides write, field for field: see ParticlesOracle.java.

use acacia_render::particles::java::Type;
use acacia_render::particles::java::probe::{self, Seeds, Snapshot, Tick};
use serde_json::{Value, json};

use crate::vec3;

/// `[[tick, type, x, y, z, xa, ya, za, seed], ...]` into one table per tick.
pub fn seeds(record: &Value) -> Vec<Seeds> {
    let mut out: Vec<Seeds> = Vec::new();
    for row in record["seeds"].as_array().expect("seeds") {
        let tick = row[0].as_u64().expect("tick") as usize;
        let ty = Type::from_name(row[1].as_str().expect("type"));
        let at = vec3(&json!([row[2], row[3], row[4]]));
        let velocity = vec3(&json!([row[5], row[6], row[7]]));
        if out.len() <= tick {
            out.resize_with(tick + 1, Seeds::default);
        }
        // Types not ported never come up here: nothing of ours makes them.
        if let Some(ty) = ty {
            out[tick].insert(probe::key(ty, at, velocity), row[8].as_i64().expect("seed"));
        }
    }
    out
}

pub fn case(name: &str, run: &[Tick]) -> Value {
    json!({ "case": name, "ticks": run.iter().map(tick).collect::<Vec<_>>() })
}

fn tick(t: &Tick) -> Value {
    let mut spawns: Vec<(&str, [f64; 6])> =
        t.spawns.iter().map(|(o, p, v)| (o.ty.name(), [p.x, p.y, p.z, v.x, v.y, v.z])).collect();
    let numbers = |a: &[f64; 6], b: &[f64; 6]| a.iter().zip(b).map(|(x, y)| x.total_cmp(y)).find(|o| o.is_ne()).unwrap_or(std::cmp::Ordering::Equal);
    spawns.sort_by(|a, b| a.0.cmp(b.0).then_with(|| numbers(&a.1, &b.1)));
    let spawns: Vec<Value> = spawns.iter().map(|(ty, c)| json!([ty, c[0], c[1], c[2], c[3], c[4], c[5]])).collect();
    json!({ "spawns": spawns, "particles": t.particles.iter().map(particle).collect::<Vec<_>>() })
}

fn particle(p: &Snapshot) -> Value {
    json!({
        "seed": p.seed,
        "type": p.ty,
        "pos": p.position.to_array(),
        "vel": p.velocity.to_array(),
        "age": p.age,
        "lifetime": p.lifetime,
        "sprite": p.sprite,
        "size": p.size,
        "rgba": p.rgba,
        "ground": p.on_ground,
    })
}
