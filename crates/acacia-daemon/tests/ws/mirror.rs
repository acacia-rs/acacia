//! Entity deltas in `mirror` records (contract: the crate docs, Entity deltas).

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use acacia_testserver::{FakeServer, Script};
use serde_json::{json, Value};

use crate::common::{daemon, Client};

/// A client's copy of one bot's entities, built from its deltas: id and feet x per slot.
#[derive(Default)]
struct Copy {
    by_slot: BTreeMap<u64, (String, f64)>,
    applied: usize,
    deltas: Vec<Value>,
}

impl Copy {
    fn apply(&mut self, d: &Value) {
        let rows = |key: &str| d[key].as_array().cloned().unwrap_or_default();
        let slot = |row: &Value| row[0].as_u64().unwrap();
        if d["full"] == true {
            let kept: Vec<_> = rows("added").iter().map(|m| (slot(m), m[1].clone())).collect();
            self.by_slot.retain(|s, (id, _)| kept.contains(&(*s, json!(id))));
        }
        for r in rows("removed") {
            assert!(self.by_slot.remove(&slot(&r)).is_some(), "removed a held entity: {d}");
        }
        for m in rows("added") {
            self.by_slot.insert(slot(&m), (m[1].as_str().unwrap().to_owned(), f64::NAN));
        }
        for r in rows("numbers") {
            self.by_slot.get_mut(&slot(&r)).expect("numbers for a held entity").1 = r[1].as_f64().unwrap();
        }
        self.deltas.push(d.clone());
    }

    /// Applies the bot's deltas among `c`'s records, reading more until `want` holds.
    async fn until(&mut self, c: &mut Client, bot: &str, what: &str, want: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            while let Some(r) = c.seen.get(self.applied).cloned() {
                self.applied += 1;
                if r["type"] == "mirror" && r["bot"] == bot && r.get("entities").is_some() {
                    self.apply(&r["entities"]);
                }
            }
            if want(self) {
                return;
            }
            c.read_frame(what, deadline).await;
        }
    }

    fn x(&self, id: &str) -> Option<f64> {
        self.by_slot.values().find(|(i, _)| i == id).map(|(_, x)| *x)
    }

    fn ids(&self) -> Vec<(u64, String)> {
        self.by_slot.iter().map(|(s, (id, _))| (*s, id.clone())).collect()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn entity_deltas_converge_and_a_late_client_starts_full() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let d = daemon(None).await;
    let mut first = Client::greeted(d.local_addr(), json!({})).await;
    let bot = first.create(&server, "Mirrored", json!({ "entities": true, "persist": true })).await;
    let mut copy = Copy::default();
    server.add_entity("minecraft:zombie", 900, 9000, [1.5, 70.0, 0.5]);
    server.add_entity("minecraft:pig", 901, 9001, [3.5, 70.0, 0.5]);
    copy.until(&mut first, &bot, "both entities", |c| c.x("900").is_some() && c.x("901").is_some()).await;
    assert_eq!((copy.x("900"), copy.x("901")), (Some(1.5), Some(3.5)));

    let held = copy.deltas.len();
    server.move_entity(900, [5.5, 70.0, 0.5]);
    copy.until(&mut first, &bot, "the zombie moved", |c| c.x("900") == Some(5.5)).await;
    let moves = &copy.deltas[held..];
    assert!(moves.iter().all(|d| d.get("full").is_none() && d.get("added").is_none()), "a move sends numbers only: {moves:?}");
    assert!(moves.iter().all(|d| d["numbers"].as_array().is_some_and(|rows| rows.len() <= 2)), "only what moved: {moves:?}");

    server.remove_entity(9001);
    copy.until(&mut first, &bot, "the pig gone", |c| c.x("901").is_none()).await;
    assert_eq!(copy.x("900"), Some(5.5), "the zombie kept its slot and numbers");
    assert_eq!(copy.deltas.iter().filter(|d| d["full"] == true).count(), 1, "one full delta, the first");

    let mut late = Client::greeted(d.local_addr(), json!({})).await;
    late.ok("attach", json!({ "bot": bot })).await;
    let mut late_copy = Copy::default();
    late_copy.until(&mut late, &bot, "the late client's copy", |c| !c.deltas.is_empty()).await;
    assert_eq!(late_copy.deltas[0]["full"], true, "a new mirror starts full");
    assert_eq!((late_copy.ids(), late_copy.x("900")), (copy.ids(), Some(5.5)), "both copies agree");
    d.shutdown().await;
}
