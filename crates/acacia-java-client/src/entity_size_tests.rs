use acacia_java_proto::entity_types::ENTITY_TYPES;

use super::*;

fn entity(name: &str) -> Entity {
    let kind = ENTITY_TYPES.iter().position(|&n| n == name).unwrap() as i32;
    Entity::new(0, kind, [0.0; 3], 0.0, 0.0)
}

#[test]
fn poses_and_babies_size_the_box() {
    let mut player = entity("minecraft:player");
    player.read_body_data(&[0, 0, 0, 6, 20, 5, 0xff]).unwrap();
    assert_eq!(player.dimensions(), Some((0.6, 1.5, EntityCollision::Pushable)));
    let mut pig = entity("minecraft:pig");
    pig.read_body_data(&[16, 8, 1, 0xff]).unwrap();
    assert_eq!(pig.dimensions(), Some((0.45, 0.45, EntityCollision::Pushable)));
    let boat = entity("minecraft:oak_boat");
    assert_eq!(boat.dimensions(), Some((1.375, 0.5625, EntityCollision::Solid)));
}

#[test]
fn the_scale_attribute_scales_all_but_fixed_sizes() {
    let mut zombie = entity("minecraft:zombie");
    zombie.shape.scale = 2.0;
    assert_eq!(zombie.dimensions().map(|d| (d.0, d.1)), Some((1.2, 3.9)));
    zombie.pose = POSE_SLEEPING;
    assert_eq!(zombie.dimensions().map(|d| (d.0, d.1)), Some((0.2, 0.2)));
    let mut dragon = entity("minecraft:ender_dragon");
    dragon.shape.scale = 2.0;
    assert_eq!(dragon.scale(), 1.0);
}

#[test]
fn type_data_sizes_slimes_and_pufferfish() {
    let mut slime = entity("minecraft:slime");
    assert_eq!(slime.dimensions().map(|d| d.0), Some(0.52));
    // Captured from Paper: health 16, mob flags (NoAI), size 4.
    slime.read_body_data(&[0x09, 0x03, 0x41, 0x80, 0x00, 0x00, 0x0f, 0x00, 0x01, 0x12, 0x01, 0x04, 0xff]).unwrap();
    assert_eq!(slime.dimensions().map(|d| d.0), Some(0.52 * 4.0));
    let mut puffer = entity("minecraft:pufferfish");
    assert_eq!(puffer.dimensions().map(|d| d.0), Some(0.7 * 0.5));
    puffer.read_body_data(&[16, 8, 0, 17, 1, 2, 0xff]).unwrap();
    assert_eq!(puffer.dimensions().map(|d| d.0), Some(0.7));
}

#[test]
fn a_shulker_grows_out_of_its_attach_face() {
    let mut shulker = entity("minecraft:shulker");
    assert_eq!(shulker.bounding_box(), Some(Aabb { min: [-0.5, 0.0, -0.5], max: [0.5, 1.0, 0.5] }));
    // Attached to the west face, opened fully: the lid grows east.
    shulker.read_body_data(&[16, 12, 4, 17, 0, 100, 0xff]).unwrap();
    for _ in 0..20 {
        assert!(shulker.shape.update_peek());
    }
    assert!(!shulker.shape.update_peek());
    assert_eq!(shulker.shape.peek.current, 1.0);
    let bb = shulker.bounding_box().unwrap();
    assert_eq!((bb.min[0], bb.max[1]), (-0.5, 1.0));
    assert!((bb.max[0] - 1.5).abs() < 1e-6, "{bb:?}");
}

#[test]
fn a_sulfur_cube_is_sized_like_a_slime() {
    let mut cube = entity("minecraft:sulfur_cube");
    let one = cube.dimensions().map(|d| d.0).unwrap();
    cube.read_body_data(&[0x12, 0x01, 0x03, 0xff]).unwrap();
    assert_eq!(cube.dimensions().map(|d| d.0), Some(one * 3.0));
}

#[test]
fn a_sitting_camel_has_a_standing_size_to_rise_to() {
    let mut camel = entity("minecraft:camel");
    camel.pose = POSE_SITTING;
    assert_eq!(camel.dimensions().map(|d| d.1), Some(2.375 - 1.43));
    assert_eq!(camel.dimensions_in(POSE_STANDING).map(|d| d.1), Some(2.375));
}
