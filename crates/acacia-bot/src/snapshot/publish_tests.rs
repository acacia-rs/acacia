use std::sync::Arc;

use super::*;
use crate::snapshot::slots::tests::{add, remove, walk};
use crate::snapshot::EntityInfo;

fn info(p: &Publisher, id: u64) -> EntityInfo {
    p.reader.entities().entities.iter().find(|e| e.runtime_id == id).cloned().expect("listed")
}

#[test]
fn a_move_patches_only_the_stamped_entity_and_unlisted_packets_publish_nothing() {
    let (mut es, mut p) = (Entities::default(), Publisher::default());
    add(&mut es, 3, 0.0);
    add(&mut es, 4, 1.0);
    p.publish_entities(&mut es);
    let first = p.reader.entities();
    let (kept, walker) = (info(&p, 4), info(&p, 3));

    walk(&mut es, 3, 5.0);
    p.publish_entities(&mut es);
    let second = p.reader.entities();
    assert_eq!(second.version, first.version + 1);
    assert_eq!(info(&p, 4), kept, "the unchanged entity keeps its row");
    assert!(Arc::ptr_eq(&info(&p, 4).kind, &kept.kind) && Arc::ptr_eq(&info(&p, 3).kind, &walker.kind), "and the strings");
    assert_eq!(info(&p, 3).position[0], 5.0, "the moved one updates");
    assert_eq!(first.entities.iter().find(|e| e.runtime_id == 3).unwrap().position[0], 0.0, "a held list is not touched");
    let delta = p.reader.entity_delta(Some(first.version));
    assert_eq!(delta.moved.iter().map(|&i| delta.list.entities[i].runtime_id).collect::<Vec<_>>(), [3]);
    assert!(delta.added.is_empty() && delta.removed.is_empty());

    drop((first, second));
    walk(&mut es, 99, 1.0);
    walk(&mut es, 3, 5.0);
    p.publish_entities(&mut es);
    assert_eq!(p.reader.entities().version, 2, "an untracked move or a repeated one lists nothing new");

    walk(&mut es, 4, 2.0);
    p.publish_entities(&mut es);
    assert_eq!((p.reader.entities().version, info(&p, 4).position[0]), (3, 2.0), "patched in place");

    remove(&mut es, 3);
    walk(&mut es, 4, 3.0);
    p.publish_entities(&mut es);
    let delta = p.reader.entity_delta(Some(3));
    assert_eq!(delta.removed.iter().map(|(_, e)| e.runtime_id).collect::<Vec<_>>(), [3], "a removal rebuilds");
    assert_eq!(delta.moved.iter().map(|&i| delta.list.entities[i].position[0]).collect::<Vec<_>>(), [3.0]);
}
