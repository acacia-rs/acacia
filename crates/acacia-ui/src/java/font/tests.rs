use std::io::Write;
use std::path::PathBuf;

use super::*;
use crate::java::mth::JavaRandom;

/// A namespace directory with these files.
fn root(name: &str, files: &[(&str, Vec<u8>)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!("acacia-font-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (path, bytes) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    root
}

fn png(width: u32, height: u32, lit: impl Fn(u32, u32) -> bool) -> Vec<u8> {
    let image = image::RgbaImage::from_fn(width, height, |x, y| image::Rgba(if lit(x, y) { [255; 4] } else { [0; 4] }));
    let mut out = std::io::Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn hex_zip(lines: &str) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("font.hex", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(lines.as_bytes()).unwrap();
    zip.finish().unwrap().into_inner()
}

fn fonts(name: &str, files: &[(&str, Vec<u8>)], options: FontOptions) -> Fonts {
    let dir = root(name, files);
    let fonts = Fonts::load(&Resources::dir("minecraft", &dir), options);
    let _ = std::fs::remove_dir_all(&dir);
    fonts
}

const DEFAULT_JSON: &str = r#"{"providers":[
  {"type":"reference","id":"minecraft:include/space"},
  {"type":"bitmap","file":"minecraft:font/a.png","ascent":7,"chars":["ab"],"filter":{"uniform":false}},
  {"type":"unihex","hex_file":"minecraft:font/u.zip","size_overrides":[{"from":"一","to":"丁","left":0,"right":15}]}]}"#;

fn sample(options: FontOptions) -> Fonts {
    // `a` lit in columns 0-1 (advance 3), `b` in 0-5 (advance 7).
    let sheet = png(16, 8, |x, _| x < 2 || (8..14).contains(&x));
    let hex = format!("0061:{}\n4E00:{}7FFE{}\n", "18".repeat(16), "0000".repeat(4), "0000".repeat(11));
    fonts("sample", &[
        ("font/default.json", DEFAULT_JSON.into()),
        ("font/include/space.json", r#"{"providers":[{"type":"space","advances":{" ":4,"‌":0}}]}"#.as_bytes().to_vec()),
        ("font/alt.json", br#"{"providers":[{"type":"reference","id":"minecraft:nope"}]}"#.to_vec()),
        ("font/broken.json", b"{".to_vec()),
        ("textures/font/a.png", sheet),
        ("font/u.zip", hex_zip(&hex)),
    ], options)
}

fn styled(font: &str) -> Style {
    Style { font: Some(font.into()), ..Style::default() }
}

#[test]
fn providers_in_order_with_filters_and_missing_fonts() {
    let f = sample(FontOptions::default());
    let plain = Style::default();
    assert_eq!((f.advance('a', &plain), f.advance('b', &plain), f.advance(' ', &plain)), (3.0, 7.0, 4.0));
    assert_eq!(f.advance('\u{200c}', &plain), 0.0);
    // 一: overridden to 16 columns, advance 16 / 2 + 1, half-pixel bold.
    let yi = f.glyph(0x4E00, &plain);
    assert_eq!((yi.advance, yi.bold_offset, yi.shadow_offset), (9.0, 0.5, 0.5));
    assert_eq!(yi.bitmap.as_ref().map(|b| (b.right, b.down)), Some((8.0, 8.0)));
    // Not anywhere: the missing glyph; a font with a dangling reference or no providers too.
    assert_eq!(f.glyph('z' as u32, &plain).bitmap.map(|b| b.texture), Some(GlyphTexture::Missing));
    assert_eq!(f.advance('a', &styled("minecraft:alt")), 6.0);
    assert_eq!(f.advance('a', &styled("minecraft:broken")), 6.0);
    assert!(f.ids().any(|id| id == "minecraft:broken") && !f.ids().any(|id| id == "minecraft:alt"));
    assert_eq!(f.width(&Text::legacy("§lab ")), 17);
    // Force Unicode Font drops the sheet: Unifont's 8-bit `a` (columns 3-4) takes over.
    let u = sample(FontOptions { uniform: true, japanese: false });
    assert_eq!(u.advance('a', &plain), 2.0);
}

#[test]
fn obfuscated_glyphs_come_from_the_same_width() {
    let f = sample(FontOptions::default());
    let by = f.set(None).by_width();
    assert_eq!(by[&3], vec![0x61]);
    assert_eq!(by[&4], vec![0x20]);
    let mut random = JavaRandom::new(1);
    let style = Style { obfuscated: Some(true), ..Style::default() };
    let p = f.prepare(&Text::styled("aq", &style), Pen::gui(0.0, 0.0, 0xFFFF_FFFF, false), &mut random);
    // `a` stays `a` (the only glyph 3 wide); `q` is missing and nothing is 6 wide.
    assert_eq!(p.quads[0].texture, GlyphTexture::Sheet("minecraft:textures/font/a.png".into()));
    assert_eq!(p.quads[1].texture, GlyphTexture::Missing);
}

#[test]
fn bold_italic_shadowed_underlined_quads() {
    let f = sample(FontOptions::default());
    let style = Style { bold: Some(true), italic: Some(true), underlined: Some(true), color: Some(0xFF5555), ..Style::default() };
    let p = f.prepare(&Text::styled("a", &style), Pen::gui(10.0, 20.0, 0x80FF_FFFF, true), &mut JavaRandom::new(0));
    let colors: Vec<u32> = p.quads.iter().map(|q| q.color).collect();
    let shadow = 0x803F_1515;
    assert_eq!(colors, vec![shadow, shadow, 0x80FF_5555, 0x80FF_5555, shadow, 0x80FF_5555]);
    let q = &p.quads[2];
    close(&q.rect, &[9.9, 19.9, 18.1, 28.1]);
    assert_eq!(q.skew, [1.0, -1.0]);
    close(&q.uv, &[0.01, 0.01, 7.99, 7.99]);
    close(&p.quads[1].rect[..1], &[11.9]);
    // The underline starts a pixel early (position 0) and runs the bold advance.
    assert_eq!(p.quads[5].rect, [9.0, 28.0, 14.0, 29.0]);
    assert!(p.quads[5].effect);
    close(&[p.quads[5].z], &[0.04]);
    close(&p.bounds.unwrap(), &[8.9, 19.9, 20.1, 30.0]);
}

fn close(a: &[f32], b: &[f32]) {
    assert!(a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5), "{a:?} != {b:?}");
}
