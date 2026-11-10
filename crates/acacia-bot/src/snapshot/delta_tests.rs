use std::collections::BTreeMap;

use super::*;
use crate::snapshot::slots::tests::{add, remove, walk};
use crate::snapshot::slots::ListBuilder;
use crate::state::Entities;

#[derive(Default)]
struct Publisher {
    es: Entities,
    builder: ListBuilder,
    log: ChangeLog,
    list: Arc<EntityList>,
}

impl Publisher {
    fn publish(&mut self) {
        let (list, changes) = self.builder.build(&self.es, self.list.version + 1, &self.list);
        self.log.push(list.version, changes);
        self.list = Arc::new(list);
    }
}

/// A reader's copy: runtime id and x per slot.
#[derive(Default)]
struct Reader {
    cursor: Option<u64>,
    by_slot: BTreeMap<u32, (u64, f32)>,
}

impl Reader {
    fn read(&mut self, p: &Publisher) -> EntityDelta {
        let d = p.log.delta(&p.list, self.cursor);
        if d.full {
            self.by_slot.clear();
        }
        for (slot, e) in &d.removed {
            assert_eq!(self.by_slot.remove(slot).map(|(id, _)| id), Some(e.runtime_id), "removed what it held");
        }
        let row = |i: usize| (d.list.slots[i], (d.list.entities[i].runtime_id, d.list.entities[i].position[0]));
        for (slot, value) in d.added.iter().map(|&i| row(i)) {
            assert!(self.by_slot.insert(slot, value).is_none(), "added into a free slot");
        }
        for (slot, value) in d.moved.iter().map(|&i| row(i)) {
            assert_eq!(self.by_slot.insert(slot, value).map(|(id, _)| id), Some(value.0), "moved what it held");
        }
        self.cursor = Some(d.list.version);
        assert_eq!(self.by_slot, p.list.slots.iter().zip(&p.list.entities).map(|(&s, e)| (s, (e.runtime_id, e.position[0]))).collect());
        d
    }
}

fn ids(d: &EntityDelta, of: &[usize]) -> Vec<u64> {
    of.iter().map(|&i| d.list.entities[i].runtime_id).collect()
}

#[test]
fn changes_merge_removals_first_and_skip_what_came_and_went() {
    let (mut p, mut r) = (Publisher::default(), Reader::default());
    add(&mut p.es, 3, 0.0);
    add(&mut p.es, 4, 0.0);
    p.publish();
    assert!(r.read(&p).full, "no cursor yet");

    walk(&mut p.es, 3, 5.0);
    walk(&mut p.es, 4, 6.0);
    p.publish();
    remove(&mut p.es, 3);
    add(&mut p.es, 5, 1.0);
    p.publish();
    add(&mut p.es, 6, 0.0);
    p.publish();
    remove(&mut p.es, 6);
    p.publish();
    let d = r.read(&p);
    assert!(!d.full);
    assert_eq!(d.removed.iter().map(|(_, e)| (e.runtime_id, e.position[0])).collect::<Vec<_>>(), [(3, 5.0)], "as last published");
    assert_eq!((ids(&d, &d.added), ids(&d, &d.moved)), (vec![5], vec![4]), "6 came and went");
    assert!(r.read(&p).is_empty(), "nothing since");
}

#[test]
fn a_reader_behind_the_log_resyncs_and_others_keep_their_cursors() {
    let (mut p, mut live, mut late, mut stale) = (Publisher::default(), Reader::default(), Reader::default(), Reader::default());
    add(&mut p.es, 3, 0.0);
    p.publish();
    live.read(&p);
    stale.read(&p);
    for step in 1..=MAX_VERSIONS + 1 {
        walk(&mut p.es, 3, step as f32);
        if step == 10 {
            add(&mut p.es, 4, 0.0);
        }
        p.publish();
        assert!(!live.read(&p).full);
        if step == 20 {
            late.read(&p);
        }
    }
    let d = late.read(&p);
    assert!(!d.full, "within the log");
    assert_eq!(ids(&d, &d.moved), [3]);
    let d = stale.read(&p);
    assert!(d.full, "older than the log");
    assert_eq!(ids(&d, &d.added), [3, 4]);
}

#[test]
fn the_log_is_bounded_by_changes_too() {
    let mut p = Publisher::default();
    for id in 0..MAX_ITEMS as u64 {
        add(&mut p.es, 10 + id, 0.0);
    }
    p.publish();
    let mut r = Reader::default();
    r.read(&p);
    for id in 0..MAX_ITEMS as u64 {
        walk(&mut p.es, 10 + id, 1.0);
    }
    p.publish();
    assert!(!r.read(&p).full, "the newest publish stays, however large");
    walk(&mut p.es, 10, 2.0);
    p.publish();
    assert!(Reader { cursor: Some(1), ..Reader::default() }.read(&p).full, "the first publish was dropped");
}
