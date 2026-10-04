//! Bakes look packs for the renderer (docs/java-look.md).
//! - `lookbake bedrock [pack dir] [out dir]`: the Bedrock look. Pack dir: `$ACACIA_ASSETS`, else
//!   assets/vanilla (tools/fetch-vanilla-pack.sh). Out dir: `$ACACIA_LOOKS`, else assets/looks, then `bedrock`.
//! - `lookbake fetch-java [dir]`: downloads the pinned Java client jar and unpacks its assets
//!   (jar.rs). Dir: `$ACACIA_JAVA_ASSETS`, else assets/java.

mod jar;

use std::path::PathBuf;

use acacia_render::assets::Pack;
use acacia_render::{Look, LookPack};

const USAGE: &str = "usage: lookbake bedrock [pack dir] [out dir] | lookbake fetch-java [dir]";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("bedrock") => bake_bedrock(args.next(), args.next()),
        Some("fetch-java") => {
            let assets = jar::fetch(&args.next().map_or_else(jar::default_dir, PathBuf::from))?;
            println!("Java {} assets: {}", jar::VERSION, assets.display());
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}

fn bake_bedrock(assets: Option<String>, out: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let assets = assets.map_or_else(Pack::default_dir, PathBuf::from);
    let out = out.map_or_else(|| LookPack::default_dir("bedrock"), PathBuf::from);
    let (pack, report) = LookPack::bake_bedrock(&Pack::load(&assets)?, Look::BEDROCK);
    pack.save(&out)?;
    let (states, blocks) = pack.counts();
    println!(
        "{}: {states} states, {blocks} distinct blocks, {} textures ({} animated); {} blocks without textures, {} images missing",
        out.display(),
        pack.atlas.layers.len(),
        pack.atlas.animations.len(),
        report.blocks_without_textures.len(),
        report.missing_images.len(),
    );
    Ok(())
}
