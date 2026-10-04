use std::path::Path;

use crate::Error;

/// Side of every texture-array layer.
pub const TEXTURE_SIZE: u32 = 16;
pub const TEXEL_BYTES: usize = (TEXTURE_SIZE * TEXTURE_SIZE * 4) as usize;

/// A 16×16 RGBA8 texture.
#[derive(Clone)]
pub struct Texture {
    pub rgba: Box<[u8; TEXEL_BYTES]>,
}

/// How a texture's alpha channel is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Alpha {
    Opaque,
    /// Only fully transparent or fully opaque texels.
    Cutout,
    Blended,
}

fn open(path: &Path) -> Result<image::RgbaImage, Error> {
    Ok(image::open(path).map_err(|source| Error::Image { path: path.display().to_string(), source })?.into_rgba8())
}

impl Texture {
    /// Loads and normalizes to 16×16: flipbook strips keep their first frame (its top-left tile
    /// when `quad`), other sizes are nearest-sampled.
    pub fn load(path: &Path, quad: bool) -> Result<Texture, Error> {
        let img = open(path)?;
        Ok(Texture::tile(&img, 0, quad))
    }

    /// Every frame of a flipbook strip, top to bottom, each normalized as [`Texture::load`] does.
    pub fn load_frames(path: &Path, quad: bool) -> Result<Vec<Texture>, Error> {
        let img = open(path)?;
        let (w, h) = img.dimensions();
        Ok((0..(h / w.min(h)).max(1)).map(|i| Texture::tile(&img, i, quad)).collect())
    }

    fn tile(img: &image::RgbaImage, index: u32, quad: bool) -> Texture {
        let (w, h) = img.dimensions();
        let side = w.min(h);
        let frame = side / if quad { 2 } else { 1 };
        let mut rgba = Box::new([0; TEXEL_BYTES]);
        for y in 0..TEXTURE_SIZE {
            for x in 0..TEXTURE_SIZE {
                let p = img.get_pixel(x * frame / TEXTURE_SIZE, index * side + y * frame / TEXTURE_SIZE);
                let i = ((y * TEXTURE_SIZE + x) * 4) as usize;
                rgba[i..i + 4].copy_from_slice(&p.0);
            }
        }
        Texture { rgba }
    }

    /// Magenta/black checkerboard for textures the pack lacks.
    pub fn missing() -> Texture {
        let mut rgba = Box::new([0; TEXEL_BYTES]);
        for (i, px) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (x, y) = (i as u32 % TEXTURE_SIZE, i as u32 / TEXTURE_SIZE);
            let magenta = (x / 8 + y / 8) % 2 == 0;
            px.copy_from_slice(if magenta { &[248, 0, 248, 255] } else { &[0, 0, 0, 255] });
        }
        Texture { rgba }
    }

    pub fn alpha(&self) -> Alpha {
        self.rgba.as_chunks::<4>().0.iter().map(|p| match p[3] {
            255 => Alpha::Opaque,
            0 => Alpha::Cutout,
            _ => Alpha::Blended,
        }).max().unwrap_or(Alpha::Opaque)
    }

    /// Mip chain below this level (8×8 down to 1×1), box-filtered. Cutout texels average only
    /// over opaque neighbours so edges don't darken.
    pub fn mips(&self) -> Vec<Vec<u8>> {
        let mut levels = Vec::new();
        let (mut prev, mut size) = (self.rgba.to_vec(), TEXTURE_SIZE as usize);
        while size > 1 {
            let half = size / 2;
            let mut next = vec![0u8; half * half * 4];
            for y in 0..half {
                for x in 0..half {
                    let texels = [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| ((y * 2 + dy) * size + x * 2 + dx) * 4);
                    let opaque: Vec<usize> = texels.iter().copied().filter(|&i| prev[i + 3] > 0).collect();
                    let src = if opaque.is_empty() { &texels[..] } else { &opaque[..] };
                    let o = (y * half + x) * 4;
                    for c in 0..3 {
                        next[o + c] = (src.iter().map(|&i| prev[i + c] as u32).sum::<u32>() / src.len() as u32) as u8;
                    }
                    next[o + 3] = (texels.iter().map(|&i| prev[i + 3] as u32).sum::<u32>() / 4) as u8;
                }
            }
            levels.push(next.clone());
            prev = next;
            size = half;
        }
        levels
    }
}
