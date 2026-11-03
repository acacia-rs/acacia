//! A connection's copy of one bot's published state: what it was last sent, so only changes go out.

use acacia_api::BotId;
use acacia_host::Host;
use serde::Serialize;

#[derive(Default)]
pub(crate) struct Mirror {
    hot: String,
    cold: Option<u64>,
    /// The entity list version this connection holds: its delta cursor.
    entities: Option<u64>,
    items: bool,
    /// The last pass that looked (see `Router::pass`), so a batch looks once per bot.
    pub looked: u64,
}

impl Mirror {
    /// A `mirror` record with the parts that changed since the last one; `None` when nothing did.
    pub(crate) fn update(&mut self, host: &Host, bot: &BotId) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(hot) = host.hot(bot).map(|h| to_json(&h)).filter(|h| *h != self.hot) {
            self.hot = hot.clone();
            parts.push(("hot", hot));
        }
        if host.cold_version(bot).is_some_and(|v| Some(v) != self.cold)
            && let Some(cold) = host.cold(bot)
        {
            self.cold = Some(cold.version);
            parts.push(("cold", to_json(&cold)));
        }
        if host.entities_version(bot).is_some_and(|v| Some(v) != self.entities)
            && let Some(delta) = host.entity_delta(bot, self.entities)
        {
            self.entities = Some(delta.version());
            if !delta.is_empty() {
                parts.push(("entities", delta.to_json()));
            }
        }
        if !self.items
            && let Some(table) = host.item_table(bot).filter(|t| !t.items.is_empty())
        {
            self.items = true;
            parts.push(("items", to_json(&table)));
        }
        if parts.is_empty() {
            return None;
        }
        let mut record = format!(r#"{{"type":"mirror","bot":{}"#, to_json(bot));
        for (name, json) in parts {
            record.push_str(&format!(r#","{name}":{json}"#));
        }
        record.push('}');
        Some(record)
    }
}

fn to_json(value: &impl Serialize) -> String {
    serde_json::to_string(value).expect("snapshots serialize")
}
