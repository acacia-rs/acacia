//! The server's side of the sim: each act judged and applied as BDS would, from where the simulated player stands.

use acacia_physics::{BlockPos, Vec3};
use acacia_world::BlockAccess;

use super::super::grid::Grid;
use super::super::interact_model;
use super::super::place_model::bds_state;
use super::super::place_twocell::{bds_pair, other_half};
use super::super::redstone_sim;
use super::{take, view, watched, World};
use crate::interact::{block_distance, Face, BLOCK_REACH};
use crate::movement::Movement;
use crate::pathfind::process::{Act, BlockView};
use crate::ActionError;

fn eye(m: &Movement) -> Vec3 {
    let f = m.position().expect("started");
    [f[0], f[1] + 1.62, f[2]]
}

fn in_body(m: &Movement, p: BlockPos) -> bool {
    body_at(m.position().expect("started"), p)
}

fn body_at(f: Vec3, p: BlockPos) -> bool {
    let o = |lo: f32, hi: f32, c: i32| lo < (c + 1) as f32 && hi > c as f32;
    o(f[0] - 0.3, f[0] + 0.3, p[0]) && o(f[1], f[1] + 1.8, p[1]) && o(f[2] - 0.3, f[2] + 0.3, p[2])
}

/// Does an act as the server would, from where the simulated player stands.
pub(super) fn act(w: &mut World, m: &Movement, act: &Act) -> Result<(), ActionError> {
    let r = act_inner(w, m, act);
    let cell = match act {
        Act::Place { against, face, .. } => face.adjacent(*against),
        Act::Break { pos } | Act::Fill { pos, .. } | Act::Use { pos, .. } | Act::Sign { pos, .. } => *pos,
        Act::Restock { station, .. } => *station,
        Act::UseOn { cell, .. } => *cell,
    };
    if watched(cell) || matches!(act, Act::Place { against, .. } if watched(*against)) {
        eprintln!("WATCH bot {} tick {} eye {:?} {act:?} -> {r:?}; now {:?}", w.me, w.now, eye(m), view(w.g, cell));
    }
    r
}

fn act_inner(w: &mut World, m: &Movement, act: &Act) -> Result<(), ActionError> {
    w.log.push(act.clone());
    let e = eye(m);
    let far = |p: BlockPos| block_distance(e, p) > BLOCK_REACH;
    let blocked = |w: &World, p: BlockPos| in_body(m, p) || w.others.iter().any(|o| body_at(*o, p));
    // The server judges by the true world, not what this bot sees.
    let solid = |w: &World, p: BlockPos| w.g.registry.get(w.g.view.block(p[0], p[1], p[2])).map(BlockView::of);
    match act {
        Act::Place { item, against, face, click } => {
            let pos = face.adjacent(*against);
            let cell = solid(w, pos).expect("loaded");
            let point = [against[0] as f32 + click[0], against[1] as f32 + click[1], against[2] as f32 + click[2]];
            let modeled = interact_model::place(item, *face, e, point, &cell);
            if far(*against) || solid(w, *against).is_none_or(|b| !b.solid()) || blocked(w, pos) || (cell.solid() && !cell.name.ends_with("_slab") && modeled.is_none()) {
                return Err(ActionError::Rejected(format!("place {item} at {pos:?} from {e:?}")));
            }
            if let Some(state) = redstone_sim::place(w.g, item, *face, pos) {
                let (name, props) = state.ok_or_else(|| ActionError::Rejected(format!("{item} refused at {pos:?}")))?;
                take(w, item)?;
                w.set(pos, &name, &props);
                redstone_sim::join(w.g, pos);
                return Ok(());
            }
            let g: &Grid = w.g;
            if let Some(pair) = bds_pair(item, *face, e, point, pos, &|p| g.registry.get(g.view.block(p[0], p[1], p[2]))) {
                let cells = pair.filter(|cs| cs.iter().all(|(p, ..)| !blocked(w, *p))).ok_or_else(|| ActionError::Rejected(format!("{item} refused at {pos:?}")))?;
                take(w, item)?;
                cells.iter().for_each(|(p, n, s)| w.set(*p, n, s));
                return Ok(());
            }
            let state = modeled.unwrap_or_else(|| bds_state(item, *face, *click, e, point, cell.name, pos));
            let (name, props) = state.ok_or_else(|| ActionError::Rejected("refused".into()))?;
            take(w, item)?;
            w.set(pos, &name, &props);
            Ok(())
        }
        Act::Break { pos } => {
            if far(*pos) {
                return Err(ActionError::NotPossible(format!("break {pos:?} from {e:?}")));
            }
            if let Some(o) = w.g.registry.get(w.g.view.block(pos[0], pos[1], pos[2])).and_then(|s| other_half(s, *pos)) {
                w.set(o, "air", "");
            }
            w.set(*pos, "air", "");
            Ok(())
        }
        Act::Fill { pos, blocks } => {
            let reach = Face::ALL.iter().any(|f| {
                let o = f.offset();
                let a = [pos[0] - o[0], pos[1] - o[1], pos[2] - o[2]];
                solid(w, a).is_some_and(|b| b.solid()) && !far(a)
            });
            let item = blocks.iter().find(|b| w.inv.get(*b).copied().unwrap_or(0) > 0).cloned();
            match item {
                Some(item) if reach && !blocked(w, *pos) => {
                    take(w, &item)?;
                    w.set(*pos, item.trim_start_matches("minecraft:"), "");
                    Ok(())
                }
                _ => Err(ActionError::NotPossible(format!("fill {pos:?} from {e:?}"))),
            }
        }
        Act::UseOn { item, against, face, .. } if !far(*against) => redstone_sim::use_on(w.g, &mut w.inv, item, *against, *face),
        Act::UseOn { .. } => Err(ActionError::NotPossible("use out of reach".into())),
        Act::Use { pos, item: Some(item), face } if item.ends_with("_hoe") && !far(*pos) => redstone_sim::use_on(w.g, &mut w.inv, item, *pos, *face),
        Act::Use { .. } | Act::Sign { .. } => interact_model::act(w, e, act),
        Act::Restock { station, items } => {
            if far(*station) {
                return Err(ActionError::NotPossible("station out of reach".into()));
            }
            let stock = w.stations.get_mut(station).ok_or_else(|| ActionError::NotPossible("no station".into()))?;
            for (item, want) in items {
                let have = stock.get(item).copied().unwrap_or(0);
                let moved = have.min(want.div_ceil(64) * 64);
                stock.insert(item.clone(), have - moved);
                *w.inv.entry(item.clone()).or_default() += moved;
            }
            Ok(())
        }
    }
}
