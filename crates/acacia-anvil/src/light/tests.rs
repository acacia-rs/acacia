use super::*;
use crate::column::JavaColumn;
use crate::mapping::{AIR, JavaState, java};
use crate::section::{VOLUME, index};

fn props() -> PropsTable {
    PropsTable::new()
}

fn state(name: &str, p: &[(&str, &str)]) -> JavaState {
    java().state(&format!("minecraft:{name}"), p).unwrap()
}

/// An overworld column: stone up to y = 0 (sections −4..−1 plus y 0), air above.
fn ground(x: i32, z: i32) -> JavaColumn {
    let stone = state("stone", &[]);
    let mut sections = vec![Blocks::Uniform(stone); 4];
    let mut s = [AIR; VOLUME];
    for z in 0..16 {
        for x in 0..16 {
            s[index(x, 0, z)] = stone;
        }
    }
    sections.push(Blocks::from_full(&s));
    sections.extend(std::iter::repeat_n(Blocks::Uniform(AIR), 19));
    JavaColumn {
        x,
        z,
        min_section: -4,
        sections,
        biomes: vec![[0; 64]; 24],
    }
}

fn set(col: &mut JavaColumn, x: usize, y: i32, z: usize, st: JavaState) {
    let si = ((y >> 4) - col.min_section) as usize;
    let mut full = [AIR; VOLUME];
    col.sections[si].copy_to(&mut full);
    full[index(x, (y & 15) as usize, z)] = st;
    col.sections[si] = Blocks::from_full(&full);
}

fn get(light: &[SectionLight], sky: bool, x: usize, y: i32, z: usize) -> u8 {
    let s = &light[((y >> 4) + 4) as usize];
    let arr = if sky { &s.sky } else { &s.block };
    let i = index(x, (y & 15) as usize, z);
    (arr[i >> 1] >> ((i & 1) * 4)) & 15
}

fn run(centre: &JavaColumn, ring: &[JavaColumn]) -> Vec<SectionLight> {
    let mut cols = [None; 9];
    cols[4] = Some(centre);
    for (i, c) in ring.iter().enumerate() {
        cols[if i < 4 { i } else { i + 1 }] = Some(c);
    }
    Lighter::default().light(&Neighbourhood { cols }, &props(), true)
}

fn ring() -> Vec<JavaColumn> {
    (0..8).map(|i| ground(i, 0)).collect()
}

#[test]
fn open_sky_reaches_the_floor_not_into_stone() {
    let l = run(&ground(0, 0), &ring());
    assert_eq!(get(&l, true, 5, 1, 5), 15);
    assert_eq!(get(&l, true, 5, 200, 5), 15);
    assert_eq!(get(&l, true, 5, 0, 5), 0);
    assert_eq!(get(&l, true, 5, -30, 5), 0);
}

#[test]
fn torch_falls_off_by_one() {
    let mut c = ground(0, 0);
    set(&mut c, 8, 1, 8, state("torch", &[]));
    let l = run(&c, &ring());
    assert_eq!(get(&l, false, 8, 1, 8), 14);
    assert_eq!(get(&l, false, 8, 1, 11), 11);
    assert_eq!(get(&l, false, 8, 5, 8), 10);
    assert_eq!(get(&l, false, 8, 0, 8), 0);
}

#[test]
fn roof_shadow_fills_from_the_side_and_slabs_block_downwards() {
    let mut c = ground(0, 0);
    set(&mut c, 8, 3, 8, state("stone", &[]));
    let slab = state("oak_slab", &[("type", "bottom")]);
    for x in 0..3 {
        for z in 0..3 {
            set(&mut c, x, 3, z, slab);
        }
    }
    let l = run(&c, &ring());
    assert_eq!(get(&l, true, 8, 2, 8), 14);
    assert_eq!(get(&l, true, 8, 3, 8), 0);
    assert_eq!(
        get(&l, true, 1, 3, 1),
        15,
        "a bottom slab is lit from above"
    );
    assert_eq!(
        get(&l, true, 1, 2, 1),
        13,
        "and casts a shadow like a full block"
    );
}

#[test]
fn water_dims_sky_light_per_block() {
    let mut c = ground(0, 0);
    let water = state("water", &[]);
    for y in 1..6 {
        set(&mut c, 4, y, 4, water);
    }
    let l = run(&c, &ring());
    assert_eq!(get(&l, true, 4, 5, 4), 14);
    assert_eq!(
        get(&l, true, 4, 1, 4),
        14,
        "the open sides relight the column"
    );
}

#[test]
fn neighbour_light_crosses_the_border() {
    let mut east = ground(1, 0);
    set(&mut east, 0, 1, 8, state("glowstone", &[]));
    let mut ring = ring();
    ring[4] = east;
    let l = run(&ground(0, 0), &ring);
    assert_eq!(get(&l, false, 15, 1, 8), 14);
    assert_eq!(get(&l, false, 12, 1, 8), 11);
}

#[test]
fn missing_neighbour_reads_opaque() {
    let mut cols = [None; 9];
    let c = ground(0, 0);
    cols[4] = Some(&c);
    let l = Lighter::default().light(&Neighbourhood { cols }, &props(), true);
    assert_eq!(get(&l, true, 0, 1, 0), 15);
}
