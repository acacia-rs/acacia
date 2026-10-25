//! Every Java block state's fluid facts (the oracle's `facts.jsonl`) against the `FluidRules` of the
//! Bedrock state mapped to it. Prints `CASES` and `DIVERGENCE` lines.

use std::io::{BufRead, BufReader};

use acacia_render::blocks::BlockTable;
use serde_json::Value;

use super::World;

const SIDES: [usize; 4] = [0, 1, 4, 5];

pub fn check(world: &World, table: &BlockTable, path: &str) {
    let mut cases = 0;
    for line in BufReader::new(std::fs::File::open(path).expect("facts")).lines() {
        let java: Value = serde_json::from_str(&line.expect("read")).expect("facts json");
        let key = java["state"].as_str().expect("state");
        // A state no Bedrock state maps to (an extended piston, a coloured bed) is no world's.
        let Some(id) = world.exact(key) else { continue };
        cases += 1;
        let block = table.get(id);
        let rules = &block.fluid_rules;
        let mut diverge = |field: String, expected: String, got: String| {
            if expected != got {
                println!("DIVERGENCE\t{key}\t{field}\t{expected}\t{got}");
            }
        };
        for face in 0..6 {
            let expected = java["cover"][face].as_f64().expect("cover");
            let got = f64::from(rules.occluding(face));
            if (expected - got).abs() > 1e-4 {
                diverge(format!("cover[{face}]"), format!("{expected}"), format!("{got}"));
            }
        }
        for face in SIDES {
            diverge(format!("sturdy[{face}]"), java["sturdy"][face].to_string(), rules.sturdy[face].to_string());
        }
        let flag = |name: &str| java[name].to_string();
        diverge("solid".into(), flag("solid"), rules.solid.to_string());
        diverge("blocks_motion".into(), flag("blocks_motion"), rules.blocks_motion.to_string());
        diverge("overlay".into(), flag("overlay"), rules.overlay.to_string());
        diverge("ice".into(), flag("ice"), rules.ice.to_string());
        diverge("solid_render".into(), flag("solid_render"), rules.solid_render().to_string());
    }
    println!("CASES\t{cases}");
}
