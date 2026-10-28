//! `BlockModelLighter`'s per-face tables (`AdjacencyInfo`): the four cells around a face, the
//! face-shape factors that weigh them at each vertex of a partial face, and which vertex each
//! computed corner lands on. Copied from Horizon's `mesher/lighting.rs` as is.

use super::smooth::{DOWN, Dir, EAST, NORTH, SOUTH, UP, WEST};
// Indices into `Shape::bounds`: a direction's bound, or 6 + it for one minus that bound.
const FLIP_DOWN: u8 = 6;
const FLIP_UP: u8 = 7;
const FLIP_NORTH: u8 = 8;
const FLIP_SOUTH: u8 = 9;
const FLIP_WEST: u8 = 10;
const FLIP_EAST: u8 = 11;

pub(super) struct Adjacency {
    pub corners: [Dir; 4],
    pub weights: [[u8; 8]; 4],
    pub remap: [usize; 4],
}

pub(super) const ADJACENCY: [Adjacency; 6] = [
    Adjacency {
        corners: [WEST, EAST, NORTH, SOUTH],
        weights: [
            [FLIP_WEST, SOUTH, FLIP_WEST, FLIP_SOUTH, WEST, FLIP_SOUTH, WEST, SOUTH],
            [FLIP_WEST, NORTH, FLIP_WEST, FLIP_NORTH, WEST, FLIP_NORTH, WEST, NORTH],
            [FLIP_EAST, NORTH, FLIP_EAST, FLIP_NORTH, EAST, FLIP_NORTH, EAST, NORTH],
            [FLIP_EAST, SOUTH, FLIP_EAST, FLIP_SOUTH, EAST, FLIP_SOUTH, EAST, SOUTH],
        ],
        remap: [0, 1, 2, 3],
    },
    Adjacency {
        corners: [EAST, WEST, NORTH, SOUTH],
        weights: [
            [EAST, SOUTH, EAST, FLIP_SOUTH, FLIP_EAST, FLIP_SOUTH, FLIP_EAST, SOUTH],
            [EAST, NORTH, EAST, FLIP_NORTH, FLIP_EAST, FLIP_NORTH, FLIP_EAST, NORTH],
            [WEST, NORTH, WEST, FLIP_NORTH, FLIP_WEST, FLIP_NORTH, FLIP_WEST, NORTH],
            [WEST, SOUTH, WEST, FLIP_SOUTH, FLIP_WEST, FLIP_SOUTH, FLIP_WEST, SOUTH],
        ],
        remap: [2, 3, 0, 1],
    },
    Adjacency {
        corners: [UP, DOWN, EAST, WEST],
        weights: [
            [UP, FLIP_WEST, UP, WEST, FLIP_UP, WEST, FLIP_UP, FLIP_WEST],
            [UP, FLIP_EAST, UP, EAST, FLIP_UP, EAST, FLIP_UP, FLIP_EAST],
            [DOWN, FLIP_EAST, DOWN, EAST, FLIP_DOWN, EAST, FLIP_DOWN, FLIP_EAST],
            [DOWN, FLIP_WEST, DOWN, WEST, FLIP_DOWN, WEST, FLIP_DOWN, FLIP_WEST],
        ],
        remap: [3, 0, 1, 2],
    },
    Adjacency {
        corners: [WEST, EAST, DOWN, UP],
        weights: [
            [UP, FLIP_WEST, FLIP_UP, FLIP_WEST, FLIP_UP, WEST, UP, WEST],
            [DOWN, FLIP_WEST, FLIP_DOWN, FLIP_WEST, FLIP_DOWN, WEST, DOWN, WEST],
            [DOWN, FLIP_EAST, FLIP_DOWN, FLIP_EAST, FLIP_DOWN, EAST, DOWN, EAST],
            [UP, FLIP_EAST, FLIP_UP, FLIP_EAST, FLIP_UP, EAST, UP, EAST],
        ],
        remap: [0, 1, 2, 3],
    },
    Adjacency {
        corners: [UP, DOWN, NORTH, SOUTH],
        weights: [
            [UP, SOUTH, UP, FLIP_SOUTH, FLIP_UP, FLIP_SOUTH, FLIP_UP, SOUTH],
            [UP, NORTH, UP, FLIP_NORTH, FLIP_UP, FLIP_NORTH, FLIP_UP, NORTH],
            [DOWN, NORTH, DOWN, FLIP_NORTH, FLIP_DOWN, FLIP_NORTH, FLIP_DOWN, NORTH],
            [DOWN, SOUTH, DOWN, FLIP_SOUTH, FLIP_DOWN, FLIP_SOUTH, FLIP_DOWN, SOUTH],
        ],
        remap: [3, 0, 1, 2],
    },
    Adjacency {
        corners: [DOWN, UP, NORTH, SOUTH],
        weights: [
            [FLIP_DOWN, SOUTH, FLIP_DOWN, FLIP_SOUTH, DOWN, FLIP_SOUTH, DOWN, SOUTH],
            [FLIP_DOWN, NORTH, FLIP_DOWN, FLIP_NORTH, DOWN, FLIP_NORTH, DOWN, NORTH],
            [FLIP_UP, NORTH, FLIP_UP, FLIP_NORTH, UP, FLIP_NORTH, UP, NORTH],
            [FLIP_UP, SOUTH, FLIP_UP, FLIP_SOUTH, UP, FLIP_SOUTH, UP, SOUTH],
        ],
        remap: [1, 2, 3, 0],
    },
];
