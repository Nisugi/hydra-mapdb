//! The upstream map file, typed exactly as loosely as it really is.
//!
//! MEASURED against `map-1789942730.json` (`plan/21` §1): every field but `id`,
//! `wayto` and `timeto` is absent from some room; `location` is a string on
//! 35,230 rooms and the boolean `false` on 661; a `timeto` value is a float on
//! 82,945 edges, an integer on 108 and a Ruby string on 1,873.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

/// One room as upstream stores it.
#[derive(Debug, Deserialize)]
pub struct UpstreamRoom {
    pub id: u32,
    #[serde(default)]
    pub title: Option<Vec<String>>,
    #[serde(default)]
    pub description: Option<Vec<String>>,
    #[serde(default)]
    pub paths: Option<Vec<String>>,
    #[serde(default)]
    pub location: Option<UpstreamLocation>,
    #[serde(default)]
    pub climate: Option<String>,
    #[serde(default)]
    pub terrain: Option<String>,
    /// Destination id (as a string) to a command, or to `;e <ruby>`.
    pub wayto: BTreeMap<String, String>,
    /// Destination id (as a string) to a cost in seconds, or to `;e <ruby>`.
    pub timeto: BTreeMap<String, Option<UpstreamCost>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub uid: Option<Vec<i64>>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub image_coords: Option<Vec<i64>>,
    #[serde(default)]
    pub check_location: Option<bool>,
    #[serde(default)]
    pub unique_loot: Option<Vec<String>>,
    /// Any field this converter does not know. Collected rather than rejected:
    /// a new upstream field should be *reported*, not stop the map updating.
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

/// `location` is a name, or `false` where the `location` verb failed when the
/// room was mapped.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum UpstreamLocation {
    Named(String),
    Flag(bool),
}

impl UpstreamRoom {
    /// What this room's own cost scripts may ask about it.
    #[must_use]
    pub fn room_facts(&self) -> crate::recognise::RoomFacts<'_> {
        crate::recognise::RoomFacts {
            climate: self.climate.as_deref(),
            title: self
                .title
                .as_ref()
                .and_then(|titles| titles.first())
                .map(String::as_str),
            location: match &self.location {
                Some(UpstreamLocation::Named(name)) => Some(name),
                _ => None,
            },
        }
    }
}

/// A `timeto` value.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum UpstreamCost {
    Seconds(f64),
    Script(String),
}

/// What marks a `wayto` or `timeto` string as Ruby rather than data. Lich tests
/// for `';e '` and takes `value[3..]`
/// (`reference/lich-5/lib/common/map/map_base.rb:376-383`).
pub const SCRIPT_PREFIX: &str = ";e";

/// Whether an upstream string is a script.
#[must_use]
pub fn is_script(value: &str) -> bool {
    value.starts_with(SCRIPT_PREFIX)
}
