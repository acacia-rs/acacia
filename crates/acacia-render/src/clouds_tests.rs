use super::*;

fn map(width: u32, height: u32, cells: &[(u32, u32)]) -> CloudMap {
    let mut image = RgbaImage::new(width, height);
    for &(x, z) in cells {
        image.put_pixel(x, z, image::Rgba([255; 4]));
    }
    CloudMap::new(&image)
}

fn frame_at(cell: [i32; 2], pos: RelativePos) -> CloudFrame {
    CloudFrame { cell, offset: Vec3::ZERO, pos }
}

#[test]
fn faint_texels_are_empty_and_neighbours_wrap_by_the_height() {
    let mut image = RgbaImage::new(8, 4);
    image.put_pixel(1, 1, image::Rgba([255, 255, 255, 9]));
    image.put_pixel(2, 1, image::Rgba([255, 255, 255, 10]));
    // East of x 3 is x (3 + 1) mod 4 = 0, not 4.
    image.put_pixel(3, 2, image::Rgba([255; 4]));
    image.put_pixel(4, 2, image::Rgba([255; 4]));
    let map = CloudMap::new(&image);
    assert_eq!((map.cell(1, 1), map.cell(2, 1)), (0, CLOUD | NORTH_EMPTY | EAST_EMPTY | SOUTH_EMPTY | WEST_EMPTY));
    assert_ne!(map.cell(3, 2) & EAST_EMPTY, 0, "x 4's cloud is not seen");
}

#[test]
fn a_cell_shows_only_the_sides_facing_the_camera() {
    let m = map(16, 16, &[(5, 0)]);
    let faces = build_mesh(&m, &frame_at([0, 0], RelativePos::Below), true, 8);
    // Five cells east, below the layer: its bottom and west side.
    assert_eq!(faces.iter().map(|f| f.dir).collect::<Vec<_>>(), [Dir::Down, Dir::West]);
    let above = build_mesh(&m, &frame_at([0, 0], RelativePos::Above), true, 8);
    assert_eq!(above.iter().map(|f| f.dir).collect::<Vec<_>>(), [Dir::Up, Dir::West]);
    let inside = build_mesh(&m, &frame_at([0, 0], RelativePos::Inside), true, 8);
    assert_eq!(inside.len(), 3);
}

#[test]
fn the_cells_around_the_camera_add_inside_faces() {
    let m = map(16, 16, &[(0, 0)]);
    let faces = build_mesh(&m, &frame_at([0, 0], RelativePos::Inside), true, 4);
    // Up, down, then all six seen from within; no outer sides at the centre.
    assert_eq!(faces.len(), 8);
    assert_eq!(faces.iter().filter(|f| f.inside).map(|f| f.dir).collect::<Vec<_>>(), DIRS);
}

#[test]
fn rings_run_outward_from_the_camera_inside_the_radius() {
    let all: Vec<(u32, u32)> = (0..64).flat_map(|z| (0..64).map(move |x| (x, z))).collect();
    let m = map(64, 64, &all);
    let faces = build_mesh(&m, &frame_at([32, 32], RelativePos::Below), false, 5);
    let cells: Vec<(i32, i32)> = faces.iter().map(|f| (f.x, f.z)).collect();
    assert_eq!(&cells[..5], [(0, 0), (-1, 0), (0, -1), (0, 1), (1, 0)]);
    assert!(cells.iter().all(|&(x, z)| x * x + z * z <= 25));
    assert_eq!(cells.len(), (-5..=5).flat_map(|x: i32| (-5..=5).map(move |z: i32| x * x + z * z)).filter(|&d| d <= 25).count());
    assert!(faces.iter().all(|f| f.dir == Dir::Down && f.top_color && !f.inside));
}

#[test]
fn the_layer_scrolls_east_with_game_time() {
    let m = map(256, 256, &[]);
    let camera = DVec3::new(100.0, 150.0, -40.0);
    let f = frame(&m, 192.0, camera, 1000, 0.5);
    assert_eq!(f.pos, RelativePos::Below);
    let x = 100.0 + f64::from(1000.5f32 * 0.030_000_001);
    let z = (-40.0 + f64::from(3.96f32)).rem_euclid(256.0 * 12.0);
    assert_eq!(f.cell, [(x / 12.0).floor() as i32, (z / 12.0).floor() as i32]);
    assert_eq!(f.offset, Vec3::new(-((x - f64::from(f.cell[0]) * 12.0) as f32), 42.0, -((z - f64::from(f.cell[1]) * 12.0) as f32)));
    // The scroll wraps after the map's width × 400 ticks.
    assert_eq!(frame(&m, 192.0, camera, 1000 + 256 * 400, 0.5), f);
    assert_eq!((frame(&m, 192.0, DVec3::new(0.0, 194.0, 0.0), 0, 0.0).pos, frame(&m, 192.0, DVec3::new(0.0, 197.0, 0.0), 0, 0.0).pos), (RelativePos::Inside, RelativePos::Above));
    assert_eq!((radius_cells(12), radius_cells(128)), (16, 171));
}

#[test]
fn faces_are_the_shaders_quads() {
    let (corners, shade) = face_quad(&Face { x: -1, z: 2, dir: Dir::Up, inside: false, top_color: false });
    assert_eq!(corners, [[-12.0, 4.0, 24.0], [-12.0, 4.0, 36.0], [0.0, 4.0, 36.0], [0.0, 4.0, 24.0]].map(Vec3::from));
    assert_eq!(shade, 1.0);
    let (inside, _) = face_quad(&Face { x: -1, z: 2, dir: Dir::Up, inside: true, top_color: false });
    assert_eq!(inside, [corners[3], corners[2], corners[1], corners[0]]);
    assert_eq!(face_quad(&Face { x: 0, z: 0, dir: Dir::Down, inside: false, top_color: true }).1, 1.0);
    assert_eq!(face_quad(&Face { x: 0, z: 0, dir: Dir::East, inside: false, top_color: false }).1, 0.9);
}
