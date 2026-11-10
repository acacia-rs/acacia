//! The committed generated files match the schema (the drift check; regenerating: see the crate docs).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::{env, fs};

use acacia_proto::packets::ItemRegistry;
use acacia_proto::Packet;

fn files(dir: &Path) -> BTreeMap<PathBuf, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.insert(path.strip_prefix(dir).unwrap().to_path_buf(), fs::read_to_string(&path).unwrap());
            }
        }
    }
    out
}

/// The `minecraft:` names, without the namespace, sorted and deduplicated.
fn vanilla<'a>(names: impl Iterator<Item = &'a str>) -> Vec<String> {
    let set: BTreeSet<_> = names.filter_map(|n| n.strip_prefix("minecraft:")).map(str::to_owned).collect();
    set.into_iter().collect()
}

fn block_names() -> Vec<String> {
    let registry = acacia_world::BlockRegistry::vanilla();
    vanilla((0..registry.len() as u32).filter_map(|id| registry.get(id)).map(|s| s.name))
}

/// From the `ItemRegistry` packet of the bundled BDS session.
fn item_names() -> Vec<String> {
    let sent = acacia_testserver::Script::bds_spawn().steps.into_iter().filter_map(|step| match step {
        acacia_testserver::Step::Send { packet, .. } => acacia_proto::RawPacket::parse(packet).ok(),
        acacia_testserver::Step::Wait(_) => None,
    });
    let registry: ItemRegistry = sent.filter(|p| p.id == ItemRegistry::ID).find_map(|p| p.decode().ok()).expect("the script's ItemRegistry");
    vanilla(registry.itemstates.iter().map(|i| i.name.as_str()))
}

#[test]
fn generated_files_are_up_to_date() {
    let committed = Path::new(env!("CARGO_MANIFEST_DIR")).join("generated");
    let fresh = env::temp_dir().join(format!("acacia-api-generated-{}", std::process::id()));
    let _ = fs::remove_dir_all(&fresh);
    fs::create_dir_all(&fresh).unwrap();
    acacia_api::export::generate(&fresh).unwrap();
    fs::write(fresh.join("names.ts"), acacia_api::export::names_ts(&block_names(), &item_names())).unwrap();
    let want = files(&fresh);
    if env::var_os("ACACIA_API_REGEN").is_some() {
        let _ = fs::remove_dir_all(&committed);
        for (path, text) in &want {
            let to = committed.join(path);
            fs::create_dir_all(to.parent().unwrap()).unwrap();
            fs::write(to, text).unwrap();
        }
    }
    let have = if committed.exists() { files(&committed) } else { BTreeMap::new() };
    let _ = fs::remove_dir_all(&fresh);
    let stale: BTreeSet<_> = want.keys().chain(have.keys()).filter(|p| want.get(*p) != have.get(*p)).collect();
    assert!(stale.is_empty(), "generated files drifted: {stale:?}; rerun with ACACIA_API_REGEN=1 and commit them");
}
