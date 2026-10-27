//! Bakes look packs for the renderer (docs/java-look.md). Directories default to: the Bedrock
//! pack `$ACACIA_ASSETS`, else assets/vanilla (tools/fetch-vanilla-pack.sh); Java assets
//! `$ACACIA_JAVA_ASSETS`, else assets/java; looks `$ACACIA_LOOKS`, else assets/looks.
//! - `lookbake bedrock [pack dir] [out dir]`: the Bedrock look.
//! - `lookbake java [pack dir] [java dir] [out dir]`: the Java look (java.rs), fetching what it needs.
//! - `lookbake fetch-java [java dir]`: downloads the pinned Java client jar, unpacks its assets
//!   (jar.rs) and downloads the Bedrock-to-Java block table (mapping.rs).
//! - `lookbake java-report [java dir]`: the same, then how many Bedrock block states reach a Java model.
//! - `lookbake java-exported [java dir] [out dir]`: only what the Java exporters write: entity
//!   models and item models with their textures (exporter.rs, entities.rs, items.rs; needs a JDK 25).
//! - `lookbake java-check <quads.json.gz> [java dir]`: our bake of every Java block state against a
//!   dump of the game's own (check.rs); tools/parity's harness `bake` makes the dump. Exits nonzero
//!   on any divergence.
//! - `lookbake java-environment [java dir] [out dir]`: only the Java look's `environment.json`
//!   (environment.rs); tools/parity's harnesses `sky` and `fog` read it.
//! - `lookbake java-light-check <facts.json> [pack dir] [java dir]`: the Java look's light facts
//!   (light.rs) against the game's; tools/parity's harness `light_facts` makes the dump.
//! - `lookbake java-world-light <scenes.jsonl> <quads.jsonl> [pack dir] [java dir]`: the Java
//!   look's vertex light on real chunks against the game's (check/world_light.rs; harness `world_light`).

mod banners;
mod biomes;
mod block;
mod blockstate;
mod check;
mod download;
mod entities;
mod environment;
mod exporter;
mod fluid;
mod items;
mod jar;
mod java;
mod light;
mod mapping;
mod model;
mod occlusion;
mod offset;
mod particles;
mod report;
mod shade;
mod sounds;
mod textures;
mod tint;
mod trims;
mod ui;

use std::path::PathBuf;

use acacia_render::assets::Pack;
use acacia_render::{Look, LookPack};

use download::Error;

const USAGE: &str = "usage: lookbake bedrock [pack dir] [out dir] | java [pack dir] [java dir] [out dir] | fetch-java [java dir] | java-exported [java dir] [out dir] | java-report [java dir] | java-check <quads.json.gz> [java dir] | java-light-check <facts.json> [pack dir] [java dir] | java-environment [java dir] [out dir] | java-world-light <scenes.jsonl> <quads.jsonl> [pack dir] [java dir]";

fn main() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);
    let command = args.next();
    let mut dir = |default: fn() -> PathBuf| args.next().map_or_else(default, PathBuf::from);
    match command.as_deref() {
        Some("bedrock") => {
            let (pack, out) = (Pack::load(&dir(Pack::default_dir))?, dir(|| LookPack::default_dir("bedrock")));
            let (look, report) = LookPack::bake_bedrock(&pack, Look::BEDROCK);
            look.save(&out)?;
            println!("{}; {} blocks without textures, {} images missing", summary(&look, &out), report.blocks_without_textures.len(), report.missing_images.len());
        }
        Some("java") => {
            let (pack, java, out) = (Pack::load(&dir(Pack::default_dir))?, dir(jar::default_dir), dir(|| LookPack::default_dir("java")));
            let (jar, mapping) = (jar::fetch(&java)?, mapping::Mapping::fetch(&java)?);
            let (look, report) = bake_java(&pack, &java, &jar, &mapping)?;
            look.save(&out)?;
            biomes::write(&biomes::java(&jar.biomes, &biomes::Colormaps::load(&jar.assets)?)?, &out)?;
            ui::copy(&jar.assets, &out)?;
            println!("  Java fonts from the asset index: {} files", ui::fetch_fonts(&out)?);
            environment::copy(&jar.assets, &out)?;
            environment::write(&jar.data, &out)?;
            particles::copy(&jar.assets, &out)?;
            banners::copy(&jar.assets, &out)?;
            exported(&java, &jar, &out)?;
            println!("  Java sounds: {} files", sounds::fetch(&out)?);
            println!("{}", summary(&look, &out));
            println!("  Java: {} states as cubes, {} as models, {} liquids; kept from Bedrock: {:?}", report.cubes, report.models, report.liquids, report.kept);
            println!("  textures missing: {:?}; models invalid: {:?}", report.missing_textures, report.invalid_models);
        }
        Some("java-exported") => {
            let (java, out) = (dir(jar::default_dir), dir(|| LookPack::default_dir("java")));
            if !exported(&java, &jar::fetch(&java)?, &out)? {
                return Err("no JDK 25 (JAVA_HOME or PATH)".into());
            }
        }
        Some(command @ ("fetch-java" | "java-report")) => {
            let java = dir(jar::default_dir);
            let (jar, mapping) = (jar::fetch(&java)?, mapping::Mapping::fetch(&java)?);
            println!("Java {} assets: {}", jar::VERSION, jar.assets.display());
            if command == "java-report" {
                report::print(&jar.assets, &mapping)?;
            }
        }
        Some("java-check") => {
            let truth = Some(dir(PathBuf::new)).filter(|p| !p.as_os_str().is_empty()).ok_or(USAGE)?;
            let java = dir(jar::default_dir);
            let (jar, mapping) = (jar::fetch(&java)?, mapping::Mapping::fetch(&java)?);
            let outcome = check::print(&jar.assets, &biomes::java(&jar.biomes, &biomes::Colormaps::load(&jar.assets)?)?, &mapping, &truth)?;
            outcome.print_for_parity();
            if !outcome.divergences.is_empty() {
                return Err(format!("{} divergences from the game", outcome.divergences.len()).into());
            }
        }
        Some("java-environment") => {
            let (java, out) = (dir(jar::default_dir), dir(|| LookPack::default_dir("java")));
            let jar = jar::fetch(&java)?;
            std::fs::create_dir_all(&out)?;
            environment::write(&jar.data, &out)?;
        }
        Some("java-light-check") => {
            let truth = Some(dir(PathBuf::new)).filter(|p| !p.as_os_str().is_empty()).ok_or(USAGE)?;
            let (pack, java) = (Pack::load(&dir(Pack::default_dir))?, dir(jar::default_dir));
            let (jar, mapping) = (jar::fetch(&java)?, mapping::Mapping::fetch(&java)?);
            let (look, _) = bake_java(&pack, &java, &jar, &mapping)?;
            let outcome = check::light::print(&look, &mapping, &truth)?;
            outcome.print_for_parity();
        }
        Some("java-world-light") => {
            let (scenes, quads) = (dir(PathBuf::new), dir(PathBuf::new));
            let (pack, java) = (Pack::load(&dir(Pack::default_dir))?, dir(jar::default_dir));
            let (jar, mapping) = (jar::fetch(&java)?, mapping::Mapping::fetch(&java)?);
            let (look, _) = bake_java(&pack, &java, &jar, &mapping)?;
            check::world_light::print(&look, &scenes, &quads)?.print_for_parity();
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

fn bake_java(pack: &Pack, java: &std::path::Path, jar: &jar::Jar, mapping: &mapping::Mapping) -> Result<(LookPack, java::Report), Error> {
    let shapes = occlusion::Shapes::export(java)?;
    if shapes.is_none() {
        println!("  Java face culling: no JDK 25 (JAVA_HOME or PATH), the renderer's own kept");
    }
    Ok(java::bake_look(pack, &jar.assets, mapping, &fluid::Tags::load(&jar.block_tags)?, shapes.as_ref()))
}

/// The exporters' entity and item models into the look; false (and the Bedrock ones kept) without a JDK 25.
fn exported(java: &std::path::Path, jar: &jar::Jar, out: &std::path::Path) -> Result<bool, Error> {
    let Some(exporter) = exporter::Exporter::prepare(java)? else {
        println!("  Java entity and item models: no JDK 25 (JAVA_HOME or PATH), Bedrock's kept");
        return Ok(false);
    };
    println!("  Java entity models: {} layers", entities::bake(&exporter, jar, out)?);
    println!("  Java armour trims: {} images", trims::bake(&jar.assets, out)?);
    let items = items::bake(&exporter, java, &jar.assets, out)?;
    println!("  Java item models: {} items, {} images, {} Bedrock names", items.items, items.images, items.names);
    Ok(true)
}

fn summary(look: &LookPack, out: &std::path::Path) -> String {
    let (states, blocks) = look.counts();
    format!("{}: {states} states, {blocks} distinct blocks, {} textures ({} animated)", out.display(), look.atlas.layers.len(), look.atlas.animations.len())
}
