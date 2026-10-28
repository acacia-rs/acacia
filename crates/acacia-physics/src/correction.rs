//! The server's collision box after a small correction (README "Deliberate deviations"): BDS never rebuilds its box, so
//! after one off by an ulp the corrected position alone does not say where its faces are.

use crate::aabb::Aabb;
use crate::math::Vec3;
use crate::state::{CollisionShape, PlayerState};

/// Corrections farther than this are real disagreements: the box is rebuilt from the position.
const SMALL: f32 = 1e-4;
/// Ulps each face may be off ours.
const REACH: i32 = 3;
/// Ticks of our own history searched for the faces we last shared with the server.
const WINDOW: usize = 20;

fn step(x: f32, n: i32) -> f32 {
    f32::from_bits((x.to_bits() as i32 + if x < 0.0 { -n } else { n }) as u32)
}

fn centre(lo: f32, hi: f32) -> f32 {
    (lo + hi) * 0.5
}

/// The faces nearest `(lo, hi)` that, moved by each tick's displacement, give every centre in turn.
fn fit(lo: f32, hi: f32, start: f32, moves: &[(f32, f32)]) -> Option<(f32, f32)> {
    let mut best: Option<(i32, f32, f32)> = None;
    for a in -REACH..=REACH {
        for b in -REACH..=REACH {
            let (mut l, mut h) = (step(lo, a), step(hi, b));
            if centre(l, h) != start || best.is_some_and(|(c, ..)| a.abs() + b.abs() >= c) {
                continue;
            }
            let ok = moves.iter().all(|&(m, c)| {
                (l, h) = (l + m, h + m);
                centre(l, h) == c
            });
            if ok {
                best = Some((a.abs() + b.abs(), step(lo, a), step(hi, b)));
            }
        }
    }
    best.map(|(_, l, h)| {
        let (mut l, mut h) = (l, h);
        for &(m, _) in moves {
            (l, h) = (l + m, h + m);
        }
        (l, h)
    })
}

/// The box `st` (our state on the corrected tick) should keep for a correction to `feet`. `earlier` holds our
/// states on the ticks before it, oldest first: the server's centre matched ours on each, so its faces are those
/// that, moved by the same displacements, land on our centres and then on `feet`.
pub(crate) fn server_box(st: &PlayerState, feet: Vec3, earlier: &[&PlayerState]) -> Option<Aabb> {
    let shape = st.shape.filter(|s| s.pos == st.pos)?;
    // Against a wall the faces are the wall's: the rebuild recovers that contact (`pose`).
    if (0..3).any(|i| (feet[i] - st.pos[i]).abs() > SMALL) || st.collide_x || st.collide_z || st.mov[0] == 0.0 || st.mov[2] == 0.0 {
        return None;
    }
    // The ticks back to the last one whose box we know, in the same pose.
    let mut chain: Vec<&PlayerState> =
        earlier.iter().rev().take(WINDOW).take_while(|e| e.shape.is_some_and(|s| s.pos == e.pos && s.dims == shape.dims)).copied().collect();
    chain.reverse();
    let mut bb = shape.bb;
    for i in [0, 2] {
        let mut found = None;
        for from in 0..chain.len() {
            let base = chain[from].shape?.bb;
            let mut moves: Vec<(f32, f32)> = chain[from + 1..].iter().map(|e| (e.mov[i], e.pos[i])).collect();
            moves.push((st.mov[i], feet[i]));
            found = fit(base.min[i], base.max[i], chain[from].pos[i], &moves);
            if found.is_some() {
                break;
            }
        }
        // Without history: the faces nearest ours, shifted, around the corrected centre.
        let shift = feet[i] - st.pos[i];
        let (lo, hi) = found.or_else(|| fit(bb.min[i] + shift, bb.max[i] + shift, feet[i], &[]))?;
        bb.min[i] = lo;
        bb.max[i] = hi;
    }
    if feet[1] != st.pos[1] {
        bb.min[1] = feet[1];
        bb.max[1] = feet[1] + shape.dims[1];
    }
    Some(bb)
}

impl PlayerState {
    /// [`PlayerState::apply_correction`] that keeps the server's box when the correction is an ulp-level one:
    /// `earlier` are our states on the ticks before the corrected one, oldest first (see `server_box`).
    pub fn apply_small_correction(&mut self, feet: Vec3, delta: Vec3, on_ground: bool, earlier: &[&PlayerState]) {
        let bb = server_box(self, feet, earlier);
        let dims = self.collision_dims();
        self.apply_correction(feet, delta, on_ground);
        if let Some(bb) = bb {
            self.shape = Some(CollisionShape { bb, pos: feet, dims });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// States moving by `v` on x and z a tick from `start`, each keeping its translated box.
    fn walk(start: f32, v: f32, ticks: usize) -> Vec<PlayerState> {
        let mut st = PlayerState::new([start, 0.0, 0.5]);
        st.remember_box(st.fresh_box());
        let mut out = vec![st.clone()];
        for _ in 0..ticks {
            let bb = st.bounding_box().translate([v, 0.0, v]);
            st.pos = [centre(bb.min[0], bb.max[0]), 0.0, centre(bb.min[2], bb.max[2])];
            st.mov = [v, 0.0, v];
            st.remember_box(bb);
            out.push(st.clone());
        }
        out
    }

    #[test]
    fn keeps_the_faces_that_explain_a_one_ulp_correction() {
        let states = walk(60.3, 0.2731, 12);
        let (last, earlier) = states.split_last().unwrap();
        let earlier: Vec<&PlayerState> = earlier.iter().collect();
        let mut feet = last.pos;
        feet[0] = step(feet[0], 1);
        let bb = server_box(last, feet, &earlier).expect("a box one ulp over");
        assert_eq!(centre(bb.min[0], bb.max[0]), feet[0]);
        let ours = last.bounding_box();
        assert!((bb.min[0] - ours.min[0]).abs() < 1e-5 && (bb.max[0] - ours.max[0]).abs() < 1e-5);
    }

    #[test]
    fn rebuilds_after_a_real_disagreement() {
        let states = walk(60.3, 0.2731, 3);
        let (last, earlier) = states.split_last().unwrap();
        let earlier: Vec<&PlayerState> = earlier.iter().collect();
        assert!(server_box(last, [last.pos[0] + 0.01, 0.0, last.pos[2]], &earlier).is_none());
        let mut against_wall = last.clone();
        against_wall.collide_x = true;
        assert!(server_box(&against_wall, last.pos, &earlier).is_none());
    }
}
