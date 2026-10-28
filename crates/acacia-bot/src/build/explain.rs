use acacia_physics::Vec3;

use super::builder::Builder;
use super::plan::{in_body, STANDS};
use super::reach::{click_for, stands, Keep, STAND_SLACK};
use super::rules;
use super::survey::Need;
use crate::pathfind::process::Look;

/// Why each open placement is not being done from `feet`.
pub(crate) fn explain(b: &Builder, look: &dyn Look, feet: Vec3) -> Vec<String> {
    let eye = [feet[0], feet[1] + crate::movement::EYE_HEIGHT, feet[2]];
    let needs = b.survey(look);
    b.cells
        .iter()
        .zip(&needs)
        .filter(|(_, n)| **n == Need::Place)
        .map(|(c, _)| {
            let now = click_for(c.pos, &c.target, eye, look, 0.0);
            let found = stands(c.pos, feet, look, &Keep { feet: &|_| false, pillar: &|_| false }, &|e| click_for(c.pos, &c.target, e, look, STAND_SLACK).is_some(), STANDS);
            let item = rules::item(&c.target).map(|i| look.count(&i));
            format!("{:?} {} [{}]: held {item:?} resting {} body {} now {now:?} stands {:?}", c.pos, c.target.name, c.target.props, b.resting(c.pos), in_body(feet, c.pos), found.iter().map(|s| s.feet).collect::<Vec<_>>())
        })
        .collect()
}
