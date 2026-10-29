use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// An entity's runtime id (the per-connection one actions take), as a decimal string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(transparent)]
pub struct EntityId(#[serde(with = "dec_str")] #[ts(type = "string")] #[schemars(with = "String")] pub u64);

/// An entity's unique id (the one player lists and containers name), as a decimal string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(transparent)]
pub struct UniqueId(#[serde(with = "dec_str")] #[ts(type = "string")] #[schemars(with = "String")] pub i64);

/// A bot the host runs, named by the host when `createBot` is called.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS, JsonSchema)]
#[serde(transparent)]
pub struct BotId(pub String);

/// The caller's id for one call, echoed in its `done` record. A plain number: the caller picks it (below 2^53).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(transparent)]
pub struct CallId(#[ts(type = "number")] pub u64);

impl fmt::Display for BotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for BotId {
    fn from(id: &str) -> Self {
        Self(id.to_owned())
    }
}

/// 64-bit integers as decimal strings, which JSON numbers (doubles) cannot carry exactly.
mod dec_str {
    use std::fmt::Display;
    use std::str::FromStr;

    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<T: Display, S: Serializer>(value: &T, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(value)
    }

    pub fn deserialize<'de, T, D>(d: D) -> Result<T, D::Error>
    where
        T: FromStr<Err: Display>,
        D: Deserializer<'de>,
    {
        String::deserialize(d)?.parse().map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_ids_travel_as_exact_strings() {
        let id = EntityId(u64::MAX - 1);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"18446744073709551614\"");
        assert_eq!(serde_json::from_str::<EntityId>(&json).unwrap(), id);
        assert_eq!(serde_json::to_string(&UniqueId(-5)).unwrap(), "\"-5\"");
        assert!(serde_json::from_str::<EntityId>("12").is_err(), "numbers are refused");
    }
}
