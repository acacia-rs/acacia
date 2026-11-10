//! The `api!` macro behind [`crate::methods`]: one table entry per method, expanded into the [`Method`] enum,
//! [`Reply`], [`Method::parse`] and the [`METHODS`] list the generators walk.
//!
//! [`Method`]: crate::Method
//! [`Reply`]: crate::Reply
//! [`Method::parse`]: crate::Method::parse
//! [`METHODS`]: crate::METHODS

use std::path::PathBuf;

use schemars::{generate::SchemaSettings, JsonSchema, Schema};
use serde::Serialize;
use ts_rs::TS;

/// How the host runs a method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Lane {
    /// Handled by the host itself, not by one bot.
    Host,
    /// Takes the bot: calls run one at a time, in order.
    Queued,
    /// Runs at once, also while a queued call runs.
    Immediate,
}

/// One method of the table.
#[derive(Debug)]
pub struct MethodInfo {
    pub name: &'static str,
    /// The doc comment: JSDoc and the MCP tool description.
    pub doc: &'static str,
    pub lane: Lane,
    /// `mcp` (offered as an MCP tool), `privileged` (MCP offers it only when the operator opts in), `experimental`.
    pub flags: &'static [&'static str],
    pub params: TypeInfo,
    pub result: TypeInfo,
}

/// What the generators need of a params or result type.
#[derive(Debug)]
pub struct TypeInfo {
    pub schema: fn(SchemaSettings) -> Schema,
    pub ts_name: fn(&ts_rs::Config) -> String,
    /// `None` for primitives (`()` is `null`), which need no import.
    pub ts_path: fn() -> Option<PathBuf>,
    pub ts_export: fn(&ts_rs::Config) -> Result<(), ts_rs::ExportError>,
}

impl TypeInfo {
    pub const fn of<T: JsonSchema + TS + 'static>() -> Self {
        Self { schema: schema::<T>, ts_name: T::name, ts_path: T::output_path, ts_export: export::<T> }
    }
}

fn schema<T: JsonSchema>(settings: SchemaSettings) -> Schema {
    settings.into_generator().into_root_schema_for::<T>()
}

fn export<T: TS + 'static>(cfg: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    if T::output_path().is_none() { Ok(()) } else { T::export_all(cfg) }
}

/// The bot a method's params address: every params type but a host method's has a `bot` field.
macro_rules! target {
    (Host, $p:ident) => {{
        let _ = $p;
        None
    }};
    ($lane:ident, $p:ident) => {
        Some(&$p.bot)
    };
}

macro_rules! api {
    ($( $(#[doc = $doc:literal])* $name:literal => $variant:ident($params:ty) -> $result:ty, $lane:ident [$($flag:ident),*]; )*) => {
        /// One call's method and params ([`crate::Command`] adds the call id).
        #[derive(Debug, Clone, PartialEq, serde::Serialize)]
        #[serde(tag = "method", content = "params")]
        pub enum Method {
            $( $(#[doc = $doc])* #[serde(rename = $name)] $variant($params), )*
        }

        /// A method's result, serialized bare (`()` is `null`).
        #[derive(Debug, Clone, PartialEq, serde::Serialize)]
        #[serde(untagged)]
        pub enum Reply {
            $( $variant($result), )*
        }

        impl Method {
            /// The params of method `name`, checked against its schema.
            pub fn parse(name: &str, params: serde_json::Value) -> Result<Self, $crate::ApiError> {
                match name {
                    $( $name => serde_json::from_value(params).map(Self::$variant).map_err(|e| {
                        $crate::ApiError::new($crate::ErrorCode::BadParams, format!("{}: {e}", $name))
                    }), )*
                    _ => Err($crate::ApiError::new($crate::ErrorCode::UnknownMethod, format!("no method {name:?}"))),
                }
            }

            pub fn name(&self) -> &'static str {
                match self { $( Self::$variant(_) => $name, )* }
            }

            pub fn lane(&self) -> $crate::Lane {
                match self { $( Self::$variant(_) => $crate::Lane::$lane, )* }
            }

            pub fn bot(&self) -> Option<&$crate::BotId> {
                match self { $( Self::$variant(p) => $crate::table::target!($lane, p), )* }
            }
        }

        /// Every method, in table order.
        pub static METHODS: &[$crate::MethodInfo] = &[ $(
            $crate::MethodInfo {
                name: $name,
                doc: concat!($($doc, "\n"),*),
                lane: $crate::Lane::$lane,
                flags: &[$(stringify!($flag)),*],
                params: $crate::table::TypeInfo::of::<$params>(),
                result: $crate::table::TypeInfo::of::<$result>(),
            },
        )* ];
    };
}

pub(crate) use {api, target};
