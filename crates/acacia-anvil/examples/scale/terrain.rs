//! Synthetic Bedrock chunks: 16 templates (rough heights, ores, torches, cliffs between chunks, and per
//! template one of edge redstone, snow cover, a glowstone-lit cave at the chunk edge, a pond) placed by a
//! hash of the position. Revision `n` of a chunk adds `n` floating glowstone blocks (a player building).
use acacia_world::{BlockRegistry, Chunk, Dimension};

pub struct Terrain {
    templates: Vec<Chunk>,
    glowstone: u32,
}

pub fn mix(mut h: u64) -> u64 {
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^ (h >> 31)
}

fn key(cx: i32, cz: i32, n: u32) -> u64 {
    mix(((cx as u32 as u64) << 32 | cz as u32 as u64) ^ mix(n as u64 + 1))
}

impl Terrain {
    pub fn new(reg: &BlockRegistry) -> Self {
        let id = |n: &str| reg.states_of(n).next().unwrap_or_else(|| panic!("{n}")).0;
        let [stone, dirt, grass, ore, torch, glow, wire, snow, water, sand] = [
            "minecraft:stone",
            "minecraft:dirt",
            "minecraft:grass_block",
            "minecraft:iron_ore",
            "minecraft:torch",
            "minecraft:glowstone",
            "minecraft:redstone_wire",
            "minecraft:snow_layer",
            "minecraft:water",
            "minecraft:sand",
        ]
        .map(id);
        let templates = (0..16u32)
            .map(|t| {
                let mut c = Chunk::empty(0, 0, Dimension::overworld(reg.air_id()));
                c.set_air_from(0);
                let base = 52 + 5 * (t % 8) as i32;
                for x in 0..16 {
                    for z in 0..16 {
                        let r = key(x, z, t);
                        let top = base + (r % 6) as i32;
                        for y in -64..top {
                            let b = match key(x * 7 + y, z, t) % 60 {
                                0 => ore,
                                _ if y > top - 4 => dirt,
                                _ => stone,
                            };
                            c.set(x, y, z, 0, b);
                        }
                        c.set(x, top, z, 0, grass);
                        match t % 4 {
                            0 if x == 0 || x == 15 => c.set(x, top + 1, z, 0, wire),
                            1 => c.set(x, top + 1, z, 0, snow),
                            2 if x < 6 && (4..10).contains(&z) => {
                                for y in 10..16 {
                                    c.set(x, y, z, 0, if (x, y) == (0, 10) { glow } else { reg.air_id() });
                                }
                            }
                            3 if (5..11).contains(&x) && (5..11).contains(&z) => {
                                c.set(x, top, z, 0, sand);
                                c.set(x, top + 1, z, 0, water);
                            }
                            _ if r.is_multiple_of(97) => c.set(x, top + 1, z, 0, torch),
                            _ => {}
                        }
                    }
                }
                c
            })
            .collect();
        Self {
            templates,
            glowstone: glow,
        }
    }

    pub fn chunk(&self, cx: i32, cz: i32, rev: u32) -> Chunk {
        let mut c = self.templates[(key(cx, cz, 0) % 16) as usize].clone();
        (c.x, c.z) = (cx, cz);
        for n in 1..=rev {
            let h = key(cx, cz, n);
            let (lx, lz, y) = ((h & 15) as i32, ((h >> 4) & 15) as i32, 90 + ((h >> 8) % 20) as i32);
            c.set(cx * 16 + lx, y, cz * 16 + lz, 0, self.glowstone);
        }
        c
    }
}
