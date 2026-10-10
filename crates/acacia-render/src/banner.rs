//! Banners: what a block entity says of one, and the texture both games compose for it. See
//! README "Banners".

use std::path::Path;

use image::RgbaImage;

use crate::assets::image_file;
use crate::entity::DYES;

const DIR: &str = "textures/entity/banner";
/// Java's `BannerRenderer.MAX_PATTERNS`.
const MAX_PATTERNS: usize = 16;
/// The flag's unfolded box in a 64×64 texture: 20×40×1 at 0, 0.
const FLAG: [u32; 2] = [42, 41];
const WHITE: u8 = 0;
const SHIELD_DIR: &str = "textures/entity/shield_patterns";
/// The shield plate's unfolded box in its 64×64 texture: 12×22×1 at 0, 0.
const SHIELD_PLATE: [u32; 2] = [26, 23];

/// Pattern codes of the block entity (`Pattern`) to the pattern's name in both games' files.
const CODES: [(&str, &str); 42] = [
    ("bl", "square_bottom_left"), ("br", "square_bottom_right"), ("tl", "square_top_left"), ("tr", "square_top_right"),
    ("bs", "stripe_bottom"), ("ts", "stripe_top"), ("ls", "stripe_left"), ("rs", "stripe_right"),
    ("cs", "stripe_center"), ("ms", "stripe_middle"), ("drs", "stripe_downright"), ("dls", "stripe_downleft"),
    ("ss", "small_stripes"), ("cr", "cross"), ("sc", "straight_cross"), ("bt", "triangle_bottom"),
    ("tt", "triangle_top"), ("bts", "triangles_bottom"), ("tts", "triangles_top"), ("ld", "diagonal_left"),
    ("rd", "diagonal_up_right"), ("lud", "diagonal_up_left"), ("rud", "diagonal_right"), ("mc", "circle"),
    ("mr", "rhombus"), ("vh", "half_vertical"), ("hh", "half_horizontal"), ("vhr", "half_vertical_right"),
    ("hhb", "half_horizontal_bottom"), ("bo", "border"), ("cbo", "curly_border"), ("gra", "gradient"),
    ("gru", "gradient_up"), ("bri", "bricks"), ("glb", "globe"), ("cre", "creeper"),
    ("sku", "skull"), ("flo", "flower"), ("moj", "mojang"), ("pig", "piglin"),
    ("flw", "flow"), ("gus", "guster"),
];

/// Java's ominous banner (`Raid.getOminousBannerInstance`), on white. Bedrock draws one image.
const OMINOUS: [(&str, u8); 8] = [
    ("rhombus", 9), ("stripe_bottom", 8), ("stripe_center", 7), ("border", 8),
    ("stripe_middle", 15), ("half_horizontal", 8), ("circle", 8), ("border", 15),
];

/// One banner's look. Dyes index [`DYES`], white first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Banner {
    pub base: u8,
    /// Pattern name (`stripe_top`) and dye, the lowest first.
    pub layers: Vec<(&'static str, u8)>,
    /// The illagers' banner (`Type` 1), whatever the rest says.
    pub ominous: bool,
}

impl Banner {
    /// From the block entity's `Base`, `Patterns` (`Pattern` code and `Color`) and `Type`. Bedrock
    /// counts banner dyes from black; patterns with unknown codes are left out.
    pub fn from_bedrock<'a>(base: i32, patterns: impl IntoIterator<Item = (&'a str, i32)>, kind: i32) -> Banner {
        let dye = |legacy: i32| 15 - legacy.clamp(0, 15) as u8;
        let layer = |(code, colour): (&str, i32)| Some((CODES.iter().find(|(c, _)| *c == code)?.1, dye(colour)));
        Banner { base: dye(base), layers: patterns.into_iter().filter_map(layer).collect(), ominous: kind == 1 }
    }

    /// The cloth's dye and the patterns laid on it.
    fn cloth(&self) -> (u8, &[(&'static str, u8)]) {
        if self.ominous { (WHITE, &OMINOUS[..]) } else { (self.base, &self.layers[..]) }
    }
}

/// The banner model's texture: the pack's `banner_base` with the cloth dyed and each pattern laid
/// over it in its dye. `None` without a base image; patterns without an image are skipped.
pub fn compose(root: &Path, banner: &Banner) -> Option<RgbaImage> {
    let open = |name: &str| Some(image::open(image_file(root, &format!("{DIR}/{name}"))?).ok()?.into_rgba8());
    // Only Java's files have the cloth as a pattern of its own.
    let cloth = open("base");
    if let Some(ominous) = open("banner_illager").filter(|_| banner.ominous && cloth.is_none()) {
        return Some(ominous);
    }
    let mut out = open("banner_base")?;
    let (base, layers) = banner.cloth();
    match cloth {
        Some(cloth) => lay(&mut out, &cloth, base),
        None => dye_flag(&mut out, base),
    }
    for &(name, dye) in layers.iter().take(MAX_PATTERNS) {
        if let Some(pattern) = open(name).or_else(|| open(&format!("banner_{}", bedrock_file(name)))) {
            lay(&mut out, &pattern, dye);
        }
    }
    Some(out)
}

/// A shield's texture carrying `banner`: both games keep the patterns redrawn for the plate
/// (`shield_patterns`, under Java's names), the cloth among them, and lay them on the plate only.
pub fn onto_shield(root: &Path, mut sheet: RgbaImage, banner: &Banner) -> RgbaImage {
    let open = |name: &str| Some(image::open(image_file(root, &format!("{SHIELD_DIR}/{name}"))?).ok()?.into_rgba8());
    let scale = sheet.width() / 64;
    let mut plate = image::imageops::crop_imm(&sheet, 0, 0, SHIELD_PLATE[0] * scale, SHIELD_PLATE[1] * scale).to_image();
    let (base, layers) = banner.cloth();
    for (name, dye) in std::iter::once(("base", base)).chain(layers.iter().copied().take(MAX_PATTERNS)) {
        if let Some(pattern) = open(name) {
            let pattern = image::imageops::resize(&pattern, sheet.width(), sheet.height(), image::imageops::FilterType::Nearest);
            let over = image::imageops::crop_imm(&pattern, 0, 0, plate.width(), plate.height()).to_image();
            lay(&mut plate, &over, dye);
        }
    }
    image::imageops::replace(&mut sheet, &plate, 0, 0);
    sheet
}

/// Bedrock's file for a pattern: the four diagonals hold the image Java keeps under the other
/// name of the same side (its `banner_diagonal_left` is Java's `diagonal_up_left`).
fn bedrock_file(name: &str) -> &str {
    match name {
        "diagonal_left" => "diagonal_up_left",
        "diagonal_up_left" => "diagonal_left",
        "diagonal_right" => "diagonal_up_right",
        "diagonal_up_right" => "diagonal_right",
        other => other,
    }
}

fn rgb(dye: u8) -> [u32; 3] {
    let colour = DYES[usize::from(dye) % DYES.len()];
    [16, 8, 0].map(|shift| (colour >> shift) & 255)
}

/// Blends `pattern` times the dye over `out` by the pattern's alpha, where `out` is opaque.
fn lay(out: &mut RgbaImage, pattern: &RgbaImage, dye: u8) {
    let scaled;
    let pattern = if pattern.dimensions() == out.dimensions() {
        pattern
    } else {
        scaled = image::imageops::resize(pattern, out.width(), out.height(), image::imageops::FilterType::Nearest);
        &scaled
    };
    let dye = rgb(dye);
    for (under, over) in out.pixels_mut().zip(pattern.pixels()).filter(|(u, o)| u.0[3] > 0 && o.0[3] > 0) {
        let alpha = u32::from(over.0[3]);
        for channel in 0..3 {
            let colour = u32::from(over.0[channel]) * dye[channel] / 255;
            under.0[channel] = ((colour * alpha + u32::from(under.0[channel]) * (255 - alpha) + 127) / 255) as u8;
        }
    }
}

/// Bedrock's base image holds the cloth itself: its texels times the dye.
fn dye_flag(out: &mut RgbaImage, dye: u8) {
    let (dye, scale) = (rgb(dye), out.width() / 64);
    for (_, _, texel) in out.enumerate_pixels_mut().filter(|(x, y, _)| *x < FLAG[0] * scale && *y < FLAG[1] * scale) {
        for channel in 0..3 {
            texel.0[channel] = (u32::from(texel.0[channel]) * dye[channel] / 255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use image::Rgba;

    use super::*;

    /// A pack directory holding `files`: name and a 64×64 image of one colour in the flag's box.
    fn pack(name: &str, files: &[(&str, [u8; 4])]) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("acacia-banner-{name}-{}", std::process::id()));
        std::fs::create_dir_all(root.join(DIR)).unwrap();
        for (file, colour) in files {
            let image = RgbaImage::from_fn(64, 64, |x, y| if x < FLAG[0] && y < FLAG[1] { Rgba(*colour) } else { Rgba([0; 4]) });
            image.save(root.join(DIR).join(file)).unwrap();
        }
        root
    }

    #[test]
    fn block_entity_codes_and_legacy_dyes() {
        let banner = Banner::from_bedrock(1, [("bo", 15), ("nope", 3), ("rud", 0), ("flw", 11)], 0);
        assert_eq!(banner, Banner { base: 14, layers: vec![("border", 0), ("diagonal_right", 15), ("flow", 4)], ominous: false });
        assert!(Banner::from_bedrock(0, [], 1).ominous);
        let mut names: Vec<_> = CODES.iter().flat_map(|(code, name)| [*code, *name]).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), CODES.len() * 2, "codes and names are distinct");
    }

    #[test]
    fn bedrock_dyes_the_cloth_and_lays_patterns_by_alpha() {
        let half = [255, 255, 255, 128];
        let root = pack("bedrock", &[("banner_base.png", [200, 200, 200, 255]), ("banner_border.png", [255; 4]), ("banner_circle.png", half), ("banner_illager.png", [1, 2, 3, 255]), ("banner_diagonal_up_left.png", [255; 4])]);
        let red = Banner { base: 14, ..Default::default() };
        let plain = compose(&root, &red).unwrap();
        // 0xB02E26 times 200/255.
        assert_eq!((plain.get_pixel(5, 5).0, plain.get_pixel(50, 5).0), ([138, 36, 29, 255], [0; 4]));
        let layered = compose(&root, &Banner { layers: vec![("border", 15), ("circle", 0), ("globe", 3)], ..red.clone() }).unwrap();
        // Black (0x1D1D21) fully over the red, then white (0xF9FFFE) at 128/255; no globe image.
        assert_eq!(layered.get_pixel(5, 5).0, [139, 142, 144, 255]);
        // Bedrock keeps Java's `diagonal_left` image as `banner_diagonal_up_left`.
        let diagonals = |name| compose(&root, &Banner { layers: vec![(name, 15)], ..red.clone() }).unwrap().get_pixel(5, 5).0;
        assert_eq!((diagonals("diagonal_left"), diagonals("diagonal_up_left")), ([0x1D, 0x1D, 0x21, 255], [138, 36, 29, 255]));
        assert_eq!(compose(&root, &Banner { ominous: true, ..red }).unwrap().get_pixel(5, 5).0, [1, 2, 3, 255]);
        assert_eq!(compose(&pack("empty", &[]), &Banner::default()), None);
    }

    #[test]
    fn a_shield_takes_the_cloth_and_patterns_on_its_plate_only() {
        let root = pack("shield", &[]);
        std::fs::create_dir_all(root.join(SHIELD_DIR)).unwrap();
        for (file, colour) in [("base.png", [255; 4]), ("border.png", [255, 255, 255, 128])] {
            RgbaImage::from_pixel(64, 64, Rgba(colour)).save(root.join(SHIELD_DIR).join(file)).unwrap();
        }
        let wood = RgbaImage::from_pixel(64, 64, Rgba([100, 100, 100, 255]));
        let out = onto_shield(&root, wood, &Banner { base: 14, layers: vec![("border", 15), ("globe", 3)], ominous: false });
        // Red (0xB02E26), then black (0x1D1D21) at 128/255; the handle's texels keep the wood.
        assert_eq!((out.get_pixel(5, 5).0, out.get_pixel(25, 22).0), ([102, 37, 35, 255], [102, 37, 35, 255]));
        assert_eq!((out.get_pixel(26, 5).0, out.get_pixel(5, 23).0), ([100, 100, 100, 255], [100, 100, 100, 255]));
    }

    #[test]
    fn java_lays_the_cloth_as_a_pattern_and_builds_the_ominous_banner() {
        let root = pack("java", &[("banner_base.png", [240, 240, 240, 255]), ("base.png", [128, 128, 128, 255]), ("border.png", [255; 4]), ("banner_illager.png", [1, 2, 3, 255])]);
        let blue = compose(&root, &Banner { base: 11, ..Default::default() }).unwrap();
        // 0x3C44AA times 128/255, replacing the base image.
        assert_eq!(blue.get_pixel(5, 5).0, [30, 34, 85, 255]);
        // The ominous banner is built from patterns and ends in a black border, on any base.
        let ominous = compose(&root, &Banner { base: 11, ominous: true, ..Default::default() }).unwrap();
        assert_eq!(ominous.get_pixel(5, 5).0, [0x1D, 0x1D, 0x21, 255]);
    }
}
