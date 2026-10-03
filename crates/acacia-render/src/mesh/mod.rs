//! Section meshing: a [`Volume`] (section plus border) becomes packed quads for the solid and
//! translucent passes.

mod greedy;
pub mod quad;
mod shapes;
pub mod volume;

pub use quad::Quad;
pub use volume::Volume;

use crate::blocks::BlockTable;

#[derive(Debug, Default)]
pub struct SectionMesh {
    pub solid: Vec<Quad>,
    pub translucent: Vec<Quad>,
}

impl SectionMesh {
    pub fn is_empty(&self) -> bool {
        self.solid.is_empty() && self.translucent.is_empty()
    }
}

pub fn mesh_section(volume: &Volume, table: &BlockTable) -> SectionMesh {
    let mut out = SectionMesh::default();
    greedy::cubes(volume, table, &mut out);
    shapes::others(volume, table, &mut out);
    out
}
