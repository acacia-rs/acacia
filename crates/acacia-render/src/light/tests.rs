use acacia_world::{BlockIds, BlockRegistry, ChunkView, World};

use super::column::{Lighter, PropsTable};
use super::data::LightData;

/// Overworld columns: stone at y 0..16, air above.
struct Fixture {
    view: ChunkView,
    table: PropsTable,
    data: LightData,
}

fn id(name: &str, props: &str) -> u32 {
    BlockRegistry::vanilla().find(name, props).unwrap_or_else(|| panic!("{name}"))
}

/// Version-9 section of one block, `id` zigzag-varint encoded.
fn uniform_section(section_y: i8, id: u32) -> Vec<u8> {
    let mut out = vec![9, 1, section_y as u8, 1];
    let mut v = id << 1;
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
    out
}

impl Fixture {
    fn new() -> Self {
        let world = World::new(BlockRegistry::vanilla_arc(), 0, BlockIds::Runtime);
        let table = PropsTable::new(world.registry());
        Fixture { view: ChunkView::new(world), table, data: LightData::default() }
    }

    fn load(&mut self, cx: i32, cz: i32) {
        let mut payload = uniform_section(0, id("minecraft:stone", ""));
        payload.extend(uniform_section(1, BlockRegistry::vanilla().air_id()));
        self.view.insert_level_chunk(cx, cz, 2, &payload).unwrap();
        self.relight(cx, cz);
    }

    fn relight(&mut self, cx: i32, cz: i32) {
        let world = self.view.world().clone();
        let lighter = Lighter { world: &world, table: &self.table, dim: world.dimension() };
        assert!(lighter.relight_column(&mut self.data, cx, cz));
    }

    /// Sets a block and updates the light like the light thread does for a block change.
    fn place(&mut self, x: i32, y: i32, z: i32, block: u32) {
        assert!(self.view.set_block(x, y, z, 0, block));
        let world = self.view.world().clone();
        let lighter = Lighter { world: &world, table: &self.table, dim: world.dimension() };
        lighter.update_block(&mut self.data, x, y, z);
    }

    fn sky(&self, x: i32, y: i32, z: i32) -> u8 {
        self.data.light(x, y, z).expect("loaded") & 15
    }

    fn block(&self, x: i32, y: i32, z: i32) -> u8 {
        self.data.light(x, y, z).expect("loaded") >> 4
    }
}

#[test]
fn open_sky_reaches_the_floor_and_not_into_stone() {
    let mut f = Fixture::new();
    f.load(0, 0);
    assert_eq!((f.sky(5, 300, 5), f.sky(5, 16, 5), f.sky(5, 15, 5)), (15, 15, 0));
    assert_eq!(f.block(5, 16, 5), 0);
    assert!(f.data.columns.contains(&(0, 0)));
}

#[test]
fn torch_light_falls_off_by_one_per_block_and_goes_away() {
    let mut f = Fixture::new();
    f.load(0, 0);
    let torch = id("minecraft:torch", "torch_facing_direction=top");
    f.place(8, 16, 8, torch);
    assert_eq!([f.block(8, 16, 8), f.block(9, 16, 8), f.block(8, 17, 10), f.block(8, 16, 1)], [14, 13, 11, 7]);
    assert_eq!(f.block(8, 15, 8), 0, "stone");
    f.place(8, 16, 8, BlockRegistry::vanilla().air_id());
    assert_eq!([f.block(8, 16, 8), f.block(9, 16, 8), f.block(8, 16, 1)], [0, 0, 0]);
}

#[test]
fn leaves_dim_sky_light_below_them() {
    let mut f = Fixture::new();
    f.load(0, 0);
    f.place(3, 30, 3, id("minecraft:oak_leaves", "persistent_bit=1,update_bit=0"));
    assert_eq!((f.sky(3, 30, 3), f.sky(3, 29, 3)), (13, 14), "below leaves, side light beats the filtered 12");
}

#[test]
fn a_block_over_open_sky_casts_a_shadow_filled_from_the_sides() {
    let mut f = Fixture::new();
    f.load(0, 0);
    f.place(5, 40, 5, id("minecraft:stone", ""));
    assert_eq!((f.sky(5, 41, 5), f.sky(5, 40, 5), f.sky(5, 39, 5), f.sky(5, 16, 5)), (15, 0, 14, 14));
    f.place(5, 40, 5, BlockRegistry::vanilla().air_id());
    assert_eq!(f.sky(5, 16, 5), 15);
}

#[test]
fn a_neighbour_column_lights_under_a_roof_through_the_shared_face() {
    let mut f = Fixture::new();
    f.load(0, 0);
    let stone = id("minecraft:stone", "");
    for x in 0..16 {
        for z in 0..16 {
            assert!(f.view.set_block(x, 20, z, 0, stone));
        }
    }
    f.relight(0, 0);
    assert_eq!(f.sky(8, 17, 8), 0, "roofed with no loaded neighbours");
    f.load(1, 0);
    assert_eq!((f.sky(16, 17, 8), f.sky(15, 17, 8), f.sky(12, 17, 8)), (15, 14, 11));
    assert_eq!(f.sky(0, 17, 8), 0, "the far side stays dark");
}
