//! `BlockBehaviour.getEntityInsideCollisionShape` and the touch test `checkInsideBlocks` makes with it and with the
//! fluid's box: a block whose inside shape the move doesn't touch is neither run nor marked visited.

use acacia_registry::prop::{Attached, LevelCauldron};
use acacia_registry::{Block, BlockState, CollisionBox};

use crate::aabb::Aabb;
use crate::collide::{Blocks, Context};
use crate::fluid::{fluid_of, height};

/// `Block.column(size, min_y, max_y)` in pixels.
fn column(size: f64, min_y: f64, max_y: f64) -> CollisionBox {
    let (low, high) = ((8.0 - size / 2.0) / 16.0, (8.0 + size / 2.0) / 16.0);
    [low, min_y / 16.0, low, high, max_y / 16.0, high]
}

/// The inside shape's boxes; `None` is `Shapes.block()`, which counts as touched without a test.
fn inside_shape(state: BlockState, context: &Context, y: i32) -> Option<Vec<CollisionBox>> {
    let with_contents = |top: f64| {
        let mut boxes = state.java_collision().unwrap_or_default().to_vec();
        boxes.push(column(12.0, 4.0, top));
        Some(boxes)
    };
    match state.block() {
        Block::WaterCauldron | Block::PowderSnowCauldron => with_contents(6.0 + 3.0 * f64::from(state.get(LevelCauldron)?)),
        Block::LavaCauldron => with_contents(15.0),
        Block::Tripwire if state.get(Attached) == Some(true) => Some(vec![column(16.0, 1.0, 2.5)]),
        Block::Tripwire => Some(vec![column(16.0, 0.0, 8.0)]),
        Block::EndPortal => Some(vec![column(16.0, 6.0, 12.0)]),
        // Walking on it (snow boots) is the full block again; falling (> 2.5) is its lower 0.9.
        Block::PowderSnow => match context.powder_snow(y) {
            [] | [[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]] => None,
            boxes => Some(boxes.to_vec()),
        },
        _ => None,
    }
}

fn placed(b: &CollisionBox, pos: [i32; 3]) -> Aabb {
    let at = pos.map(f64::from);
    Aabb { min: [b[0] + at[0], b[1] + at[1], b[2] + at[2]], max: [b[3] + at[0], b[4] + at[1], b[5] + at[2]] }
}

/// What a body starting at `start` and moving by `travel` touches of the non-air block at `pos`: its inside shape,
/// then its fluid.
pub(crate) fn touched(world: &impl Blocks, state: BlockState, pos: [i32; 3], context: &Context, start: &Aabb, travel: [f64; 3]) -> (bool, bool) {
    let block = match inside_shape(state, context, pos[1]) {
        None => true,
        Some(boxes) => start.collided_along_vector(travel, boxes.iter().map(|b| placed(b, pos))),
    };
    let fluid = fluid_of(state).is_some_and(|f| {
        let top = f64::from(height(world, pos, f));
        start.collided_along_vector(travel, [placed(&[0.0, 0.0, 0.0, 1.0, top, 1.0], pos)])
    });
    (block, fluid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_one_cauldron_holds_water_to_nine_pixels() {
        assert_eq!(column(12.0, 4.0, 9.0), [0.125, 0.25, 0.125, 0.875, 0.5625, 0.875]);
    }

    #[test]
    fn a_body_above_the_water_line_misses_the_cauldron_but_not_its_rim() {
        let water = Block::WaterCauldron.default_state();
        let boxes = inside_shape(water, &Context::default(), 0).expect("own shape");
        let at = |y: f64| Aabb::of_entity([0.5, y, 0.5], 0.6, 1.8);
        assert!(!at(0.6).collided_along_vector([0.0; 3], boxes.iter().map(|b| placed(b, [0; 3]))), "level 1 stops at 0.5625");
        assert!(at(0.5).collided_along_vector([0.0; 3], boxes.iter().map(|b| placed(b, [0; 3]))));
    }
}
