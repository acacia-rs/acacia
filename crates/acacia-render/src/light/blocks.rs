//! What the light engine reads of blocks: per runtime id, its props byte and shape, by the
//! renderer's own rules (Bedrock's light data) or by Java's (the look's [`crate::blocks::LightFacts`]).

use acacia_world::{BlockRegistry, SECTION_VOLUME, World};

use super::data::props;
use super::rules::Rules;
use crate::blocks::{BlockTable, FaceShapes};

/// A block to the engine: props byte (emission << 4 | filter, Java's dampening under Java's rules)
/// and shape ([`Rules::occludes`], 0 for none).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Cell {
    pub props: u8,
    pub shape: u8,
}

pub trait LightBlocks {
    /// A loaded column's sections bottom up, `None` for one of air only; `None` when the column
    /// isn't loaded.
    #[allow(clippy::type_complexity)]
    fn column(&self, cx: i32, cz: i32) -> Option<Vec<Option<Box<[Cell; SECTION_VOLUME]>>>>;
    /// `None` outside loaded columns.
    fn cell(&self, x: i32, y: i32, z: i32) -> Option<Cell>;
    fn air(&self) -> Cell;
}

/// Light cells per runtime id.
pub struct PropsTable {
    cells: Vec<Cell>,
    shapes: Vec<FaceShapes>,
    java: bool,
}

impl PropsTable {
    /// Bedrock's light data: unknown ids block all light.
    pub fn new(registry: &BlockRegistry) -> Self {
        let cells = (0..registry.len() as u32)
            .map(|id| Cell { props: registry.get(id).map_or(15, |s| props(s.light_emission, s.light_filter)), shape: 0 })
            .collect();
        PropsTable { cells, shapes: Vec::new(), java: false }
    }

    /// The table's own light facts under Java lighting, else [`PropsTable::new`].
    pub fn of_look(registry: &BlockRegistry, table: &BlockTable) -> Self {
        if table.lighting != crate::look::Lighting::Java {
            return PropsTable::new(registry);
        }
        let cell = |id: u32| {
            let f = table.get(id).light;
            Cell { props: props(f.emission, f.dampening), shape: u8::try_from(f.shape).expect("at most 255 light shapes") }
        };
        let cells = (0..registry.len() as u32).map(cell).collect();
        PropsTable { cells, shapes: table.light_shapes.to_vec(), java: true }
    }

    pub fn rules(&self) -> Rules<'_> {
        if self.java { Rules::Java(&self.shapes) } else { Rules::Bedrock }
    }

    /// Both layers: the brighter emission and the stronger filter (waterlogged blocks), the block's shape.
    #[inline]
    fn of(&self, block: u32, liquid: u32) -> Cell {
        let (a, b) = (self.get(block), self.get(liquid));
        Cell { props: (a.props & 0xF0).max(b.props & 0xF0) | (a.props & 15).max(b.props & 15), shape: a.shape }
    }

    #[inline]
    fn get(&self, id: u32) -> Cell {
        self.cells.get(id as usize).copied().unwrap_or(Cell { props: 15, shape: 0 })
    }
}

pub struct WorldBlocks<'a> {
    pub world: &'a World,
    pub table: &'a PropsTable,
}

impl LightBlocks for WorldBlocks<'_> {
    fn column(&self, cx: i32, cz: i32) -> Option<Vec<Option<Box<[Cell; SECTION_VOLUME]>>>> {
        let shared = self.world.get(cx, cz)?;
        let chunk = shared.read();
        let (mut blocks, mut liquid) = (Box::new([0; SECTION_VOLUME]), Box::new([0; SECTION_VOLUME]));
        let dim = self.world.dimension();
        let sections = (0..dim.height as usize / 16).map(|index| {
            chunk.copy_section(index, &mut blocks, &mut liquid).then(|| {
                let mut cells = Box::new([Cell::default(); SECTION_VOLUME]);
                (0..SECTION_VOLUME).for_each(|i| cells[i] = self.table.of(blocks[i], liquid[i]));
                cells
            })
        });
        Some(sections.collect())
    }

    fn cell(&self, x: i32, y: i32, z: i32) -> Option<Cell> {
        let shared = self.world.get(x >> 4, z >> 4)?;
        let chunk = shared.read();
        Some(self.table.of(chunk.block(x, y, z), chunk.liquid(x, y, z)))
    }

    fn air(&self) -> Cell {
        let air = self.world.dimension().air;
        self.table.of(air, air)
    }
}
