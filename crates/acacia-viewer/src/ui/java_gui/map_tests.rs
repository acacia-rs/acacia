use super::*;

const RED: Rgba<u8> = Rgba([255, 0, 0, 255]);

/// An 8 × 8 marker image, clear but for its top left texel.
fn corner() -> RgbaImage {
    let mut image = RgbaImage::new(8, 8);
    image.put_pixel(0, 0, RED);
    image
}

fn map(rotation: u8) -> MapImage {
    let marker = MapMarker { offset: [0, 0], rotation, kind: Kind::MarkerWhite };
    MapImage { rgba: [10, 20, 30, 255].repeat(MAP_SIZE * MAP_SIZE), markers: vec![marker] }
}

#[test]
fn bedrock_markers_show_java_sprites() {
    assert_eq!(sprite(Kind::MarkerWhite), Some("player"));
    assert_eq!(sprite(Kind::MarkerTeal2), Some("player"));
    assert_eq!(sprite(Kind::MarkerGreen), Some("frame"));
    assert_eq!(sprite(Kind::SmallSquareWhite), Some("player_off_limits"));
    assert_eq!(sprite(Kind::VillageSnowy2), Some("snowy_village"));
    assert_eq!(sprite(Kind::WitchHut), Some("swamp_hut"));
    assert_eq!(sprite(Kind::MarkerSign), None);
    assert_eq!(sprite(Kind::NoDraw), None);
}

#[test]
fn a_marker_lands_where_map_poses_it() {
    // Unturned, the image's top left texel (V flipped) lands left of and below the middle.
    let out = compose(&map(0), |_| Some(corner()));
    assert_eq!(out.dimensions(), (512, 512));
    assert_eq!(*out.get_pixel(240, 272), RED);
    assert_eq!(*out.get_pixel(240, 240), Rgba([10, 20, 30, 255]));
    // A quarter turn clockwise carries it up and left.
    let out = compose(&map(4), |_| Some(corner()));
    assert_eq!(*out.get_pixel(240, 240), RED);
    assert_eq!(*out.get_pixel(240, 272), Rgba([10, 20, 30, 255]));
}

#[test]
fn the_atlas_holds_the_newest_picture() {
    let mut atlas = Atlas::new();
    let mut shown = MapPicture::default();
    let root = Path::new("no pack here");
    shown.show(&mut atlas, root, &Arc::new(map(0)));
    let sprite = shown.sprite(&atlas).expect("shown");
    let mut changed = map(0);
    changed.rgba[..4].copy_from_slice(&[1, 2, 3, 255]);
    shown.show(&mut atlas, root, &Arc::new(changed));
    assert_eq!(shown.sprite(&atlas), Some(sprite));
    assert_eq!(atlas.image().get_pixel(sprite.x, sprite.y).0, [1, 2, 3, 255]);
}
