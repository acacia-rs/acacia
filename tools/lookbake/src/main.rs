//! Bakes a look pack for the renderer (docs/java-look.md).
//! `cargo run --release -p lookbake -- bedrock [pack dir] [out dir]`
//! - pack dir: `$ACACIA_ASSETS`, else assets/vanilla (tools/fetch-vanilla-pack.sh).
//! - out dir: `$ACACIA_LOOKS`, else assets/looks, then `bedrock`.

use std::path::PathBuf;

use acacia_render::assets::Pack;
use acacia_render::{Look, LookPack};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("bedrock") {
        return Err("usage: lookbake bedrock [pack dir] [out dir]".into());
    }
    let assets = args.next().map_or_else(Pack::default_dir, PathBuf::from);
    let out = args.next().map_or_else(|| LookPack::default_dir("bedrock"), PathBuf::from);
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
