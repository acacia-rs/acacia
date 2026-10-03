use std::fmt;

use serde::{Deserialize, Serialize};

/// Chosen by the caller, so ids stay unique across every node of a deployment.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BotId(pub String);

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Login {
    Offline { name: String },
    /// Resolved by the node through its token cache, keyed by this account id.
    Online { account: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    /// `host:port`.
    Server { address: String },
    /// Needs an online login that is a member of the realm.
    Realm { id: i64 },
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Server { address } => f.write_str(address),
            Target::Realm { id } => write!(f, "realm:{id}"),
        }
    }
}

/// Everything needed to run one bot on any node: plain data, so a coordinator can send it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BotSpec<S> {
    pub id: BotId,
    pub login: Login,
    pub target: Target,
    /// `host:port[:user:pass]`, used for both the game connection and sign-in.
    #[serde(default)]
    pub proxy: Option<String>,
    pub state: S,
}
