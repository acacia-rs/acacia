//! Offline survey of real vanilla structures (research/schematic-trial.md): each file's cells by need on an empty
//! flat site, and every unsupported block with the rule that rejected it. Files are Mojang content and live
//! outside the repo (`tools/save-vanilla-structures.sh`).
//! `cargo test -p acacia-bot --lib vanilla_survey -- --ignored --nocapture` (`SCHEMATICS=<dir>`, default
//! `~/work/schematics/vanilla`).

use std::collections::BTreeMap;

use crate::build::rules::{self, redstone, twocell};
use crate::build::survey::{need, Need};
use crate::build::{Block, Structure};
use crate::pathfind::process::BlockView;

const AIR: BlockView = BlockView { name: "minecraft:air", props: "", air: true, liquid: false, full: false, interactive: false };

/// Why `rules::item` gives no item for `target`: the `DENIED` substring, or the check that failed.
fn reason(target: &Block) -> String {
    let n = target.name.trim_start_matches("minecraft:");
    if !rules::supported(&target.name) {
        return match rules::DENIED.iter().find(|d| n.contains(*d)) {
            Some(d) => format!("DENIED \"{d}\""),
            None => "redstone::denied".into(),
        };
    }
    let unknown: Vec<&str> = target.props.split(',').filter(|kv| !kv.is_empty() && !rules::keys_known(&Block::new(&target.name, kv))).collect();
    match () {
        _ if !unknown.is_empty() => format!("keys_known: {}", unknown.join(",")),
        _ if twocell::unplaceable(target) => "twocell::unplaceable".into(),
        _ if !redstone::placeable(target) => "redstone::placeable".into(),
        _ => "?".into(),
    }
}

fn survey(path: &std::path::Path) -> String {
    let s = Structure::load(&std::fs::read(path).expect("read")).expect("load");
    let (mut by_need, mut unsupported, mut placed) = (BTreeMap::<String, u32>::new(), BTreeMap::<(String, String), u32>::new(), BTreeMap::<String, u32>::new());
    let mut cells = 0;
    for (_, b) in s.cells() {
        cells += 1;
        let n = need(b, Some(AIR), false);
        *by_need.entry(format!("{n:?}")).or_default() += 1;
        match n {
            Need::Unsupported => *unsupported.entry((b.name.trim_start_matches("minecraft:").to_string(), reason(b))).or_default() += 1,
            Need::Place => *placed.entry(b.name.trim_start_matches("minecraft:").to_string()).or_default() += 1,
            _ => {}
        }
    }
    let solid = cells - by_need.get("Done").copied().unwrap_or(0);
    let place = by_need.get("Place").copied().unwrap_or(0);
    let mut out = format!(
        "== {} size {:?} cells {cells} non-air {solid} place {place} ({:.1}% supported) needs {by_need:?}\n",
        path.file_stem().unwrap().to_string_lossy(),
        s.size,
        100.0 * place as f32 / solid.max(1) as f32
    );
    for ((name, why), c) in &unsupported {
        out += &format!("  UNSUPPORTED {c:4} {name} [{why}]\n");
    }
    let mut placed: Vec<_> = placed.into_iter().collect();
    placed.sort_by(|a, b| b.1.cmp(&a.1));
    out += &format!("  place: {}\n", placed.iter().map(|(n, c)| format!("{n} {c}")).collect::<Vec<_>>().join(", "));
    out
}

#[test]
#[ignore = "needs the saved vanilla structures"]
fn vanilla_survey() {
    let dir = std::env::var("SCHEMATICS").unwrap_or_else(|_| format!("{}/work/schematics/vanilla", std::env::var("HOME").unwrap_or_default()));
    let mut files: Vec<_> = std::fs::read_dir(&dir).expect("schematics dir").filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "mcstructure")).collect();
    files.sort();
    assert!(!files.is_empty(), "no .mcstructure in {dir}");
    files.iter().for_each(|f| print!("{}", survey(f)));
}
