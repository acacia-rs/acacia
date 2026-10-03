//! Embeds every `assets/skins/*.json` (see src/login/skin.rs) as `$OUT_DIR/skins.rs`.

use std::path::Path;
use std::{env, fs};

fn main() {
    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets/skins");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<_> = fs::read_dir(&dir)
        .map(|entries| entries.filter_map(|e| Some(e.ok()?.path())).collect())
        .unwrap_or_default();
    files.retain(|p| p.extension().is_some_and(|e| e == "json"));
    files.sort();
    let items: String = files.iter().map(|p| format!("include_str!({:?}),", p.display().to_string())).collect();
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("skins.rs"), format!("&[{items}]")).unwrap();
}
