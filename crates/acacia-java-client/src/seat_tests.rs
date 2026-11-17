use super::*;

const HEIGHT: f32 = 2.375;

#[test]
fn a_standing_camel_seats_at_its_anchor() {
    assert_eq!(camel_anchor(HEIGHT, false, 1.0, true, 100, 1000), f64::from(HEIGHT) - 0.375);
    assert_eq!(camel_anchor(HEIGHT, false, 1.0, false, 100, 1000), f64::from(HEIGHT) - 0.375);
}

#[test]
fn a_sitting_camel_lowers_the_seat_to_its_bottom() {
    let sitting = HEIGHT - 1.43;
    let sat = camel_anchor(sitting, false, 1.0, true, -100, 1000);
    assert_eq!(sat, f64::from(sitting) - 0.375 + f64::from(1.43_f32 - (1.43_f32 - 0.2)));
    // Just sat down: the anchor starts a full sitting drop up and passes the flex point at tick 28.
    let start = camel_anchor(sitting, false, 1.0, true, -1000, 1000);
    assert!((start - (f64::from(HEIGHT) - 0.375)).abs() < 1.0E-6, "{start}");
    let flex = camel_anchor(sitting, false, 1.0, true, -1000, 1028);
    assert!(start > flex && flex > sat, "{start} {flex} {sat}");
}

#[test]
fn the_back_seat_sinks_later() {
    let sitting = HEIGHT - 1.43;
    let front = camel_anchor(sitting, false, 1.0, true, -1000, 1028);
    let back = camel_anchor(sitting, false, 1.0, false, -1000, 1028);
    assert!(back > front, "flex points: back {back}, front {front}");
}

#[test]
fn standing_up_climbs_back_to_the_standing_anchor() {
    let rising = camel_anchor(HEIGHT, false, 1.0, true, 1000, 1000);
    let low = f64::from(HEIGHT) - 0.375 + f64::from(0.2_f32 - 1.43);
    assert!((rising - low).abs() < 1.0E-6, "{rising} vs {low}");
    assert_eq!(camel_anchor(HEIGHT, false, 1.0, true, 1000, 1052), f64::from(HEIGHT) - 0.375);
    assert_eq!(camel_anchor(HEIGHT, false, 1.0, false, 1000, 1052), f64::from(HEIGHT) - 0.375);
}

#[test]
fn boat_seats_split_fore_and_aft_for_two() {
    let (alone, pair) = (Place::DRIVER, |index| Place { index, count: 2 });
    assert_eq!(boat_seat_z("oak_boat", alone), 0.0);
    assert_eq!(boat_seat_z("oak_chest_boat", alone), 0.15);
    assert_eq!(boat_seat_z("oak_boat", pair(0)), 0.2);
    assert_eq!(boat_seat_z("bamboo_raft", pair(1)), -0.6);
}
