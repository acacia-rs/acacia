use super::*;
use crate::column::JavaColumn;
use crate::mapping::{AIR, java};
use crate::section::{VOLUME, index};

fn st(name: &str, p: &[(&str, &str)]) -> JavaState {
    java().state(&format!("minecraft:{name}"), p).unwrap()
}

fn column(blocks: &[([usize; 3], JavaState)]) -> JavaColumn {
    let mut sections = vec![Blocks::Uniform(AIR); 24];
    let mut full = [AIR; VOLUME];
    for &([x, y, z], s) in blocks {
        full[index(x, y, z)] = s;
    }
    sections[4] = Blocks::from_full(&full);
    JavaColumn {
        x: 0,
        z: 0,
        min_section: -4,
        sections,
        biomes: vec![[0; 64]; 24],
    }
}

fn fixed(blocks: &[([usize; 3], JavaState)]) -> Vec<([usize; 3], JavaState)> {
    let props = PropsTable::new();
    let c = column(blocks);
    let mut cols = [None; 9];
    cols[4] = Some(&c);
    let mut e = Fixups::new().edits(&Neighbourhood { cols }, &props);
    for (p, _) in &mut e {
        p[1] -= 64;
    }
    e.sort();
    e
}

fn props_of(s: JavaState) -> String {
    let mut p: Vec<String> = java()
        .properties(s)
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    p.sort();
    p.join(",")
}

#[test]
fn snow_on_grass() {
    let e = fixed(&[
        ([1, 0, 1], st("grass_block", &[])),
        ([1, 1, 1], st("snow", &[])),
    ]);
    assert_eq!(
        e,
        vec![([1, 0, 1], st("grass_block", &[("snowy", "true")]))]
    );
}

#[test]
fn kelp_top_stays_kelp_and_the_rest_is_plant() {
    let kelp = st("kelp", &[]);
    let e = fixed(&[([2, 0, 2], kelp), ([2, 1, 2], kelp), ([2, 2, 2], kelp)]);
    let plant = st("kelp_plant", &[]);
    assert_eq!(e, vec![([2, 0, 2], plant), ([2, 1, 2], plant)]);
}

#[test]
fn redstone_line_and_lone_cross() {
    let wire = st("redstone_wire", &[]);
    let e = fixed(&[([5, 0, 5], wire), ([6, 0, 5], wire), ([10, 0, 10], wire)]);
    let shapes: Vec<String> = e
        .iter()
        .map(|(p, s)| format!("{p:?} {}", props_of(*s)))
        .collect();
    assert!(
        shapes[0].contains("[5, 0, 5]")
            && shapes[0].contains("east=side,north=none,power=0,south=none,west=side"),
        "{shapes:?}"
    );
    assert!(
        shapes[2].contains("[10, 0, 10]")
            && shapes[2].contains("east=side,north=side,power=0,south=side,west=side"),
        "{shapes:?}"
    );
}

#[test]
fn mushroom_faces_inside_a_cap_are_hidden() {
    let cap = st("red_mushroom_block", &[]);
    let e = fixed(&[([3, 0, 3], cap), ([4, 0, 3], cap)]);
    assert_eq!(java().property(e[0].1, "east"), Some("false"));
    assert_eq!(java().property(e[0].1, "west"), Some("true"));
    assert_eq!(java().property(e[1].1, "west"), Some("false"));
}
