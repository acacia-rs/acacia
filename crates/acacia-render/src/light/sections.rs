//! One byte per block for every section of the loaded columns, uniform sections stored as one value.

use acacia_world::SECTION_VOLUME;
use rustc_hash::FxHashMap;

use crate::workers::SectionKey;

enum Section {
    Uniform(u8),
    Cells(Box<[u8; SECTION_VOLUME]>),
}

/// Section-local index in the world's XZY order.
#[inline]
pub fn index(x: i32, y: i32, z: i32) -> usize {
    (((x & 15) << 8) | ((z & 15) << 4) | (y & 15)) as usize
}

#[derive(Default)]
pub struct Sections {
    map: FxHashMap<SectionKey, Section>,
}

impl Sections {
    /// `None` outside loaded columns and the world's height.
    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        match self.map.get(&(x >> 4, y >> 4, z >> 4))? {
            Section::Uniform(v) => Some(*v),
            Section::Cells(c) => Some(c[index(x, y, z)]),
        }
    }

    /// Ignored outside loaded sections.
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, z: i32, value: u8) {
        let Some(s) = self.map.get_mut(&(x >> 4, y >> 4, z >> 4)) else { return };
        match s {
            Section::Uniform(v) if *v == value => {}
            Section::Uniform(v) => {
                let mut cells = Box::new([*v; SECTION_VOLUME]);
                cells[index(x, y, z)] = value;
                *s = Section::Cells(cells);
            }
            Section::Cells(c) => c[index(x, y, z)] = value,
        }
    }

    pub fn insert(&mut self, key: SectionKey, cells: &[u8; SECTION_VOLUME]) {
        let uniform = cells.iter().all(|&v| v == cells[0]);
        self.map.insert(key, if uniform { Section::Uniform(cells[0]) } else { Section::Cells(Box::new(*cells)) });
    }

    pub fn fill(&mut self, key: SectionKey, value: u8) {
        self.map.insert(key, Section::Uniform(value));
    }

    pub fn remove(&mut self, key: SectionKey) {
        self.map.remove(&key);
    }

    /// Calls `f(local index, value)` for every nonzero cell of a section.
    pub fn for_each_nonzero(&self, key: SectionKey, mut f: impl FnMut(usize, u8)) {
        match self.map.get(&key) {
            Some(Section::Uniform(0)) | None => {}
            Some(Section::Uniform(v)) => (0..SECTION_VOLUME).for_each(|i| f(i, *v)),
            Some(Section::Cells(c)) => c.iter().enumerate().filter(|(_, v)| **v != 0).for_each(|(i, v)| f(i, *v)),
        }
    }
}
