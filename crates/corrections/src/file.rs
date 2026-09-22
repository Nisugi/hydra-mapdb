//! The corrections file, as it arrives.
//!
//! Parsing only: this module decides what a submission *says*, never whether
//! it may be accepted. That is [`crate::validate`]'s job, and keeping the two
//! apart is what lets a refusal name a room rather than a byte offset.
//!
//! The format is specified in `hydra-mapper`'s `docs/corrections-format.md`.
//! Versions 1 and 2 are both read; the differences are handled where they
//! arise and noted there.

use std::collections::BTreeMap;
use std::fmt;

use cena_map::{Dirto, Placement, Sheet, Uid};
use serde::{Deserialize, Deserializer, Serialize};

/// The highest format version this build understands.
///
/// A file claiming a higher number is **refused**, not read for the fields it
/// happens to recognise: a later version may change what an existing field
/// means, so reading it selectively would apply a correction that was never
/// submitted (`corrections-format.md`, "`version`").
pub const SUPPORTED: u64 = 2;

/// Why a submission could not be read at all.
///
/// Distinct from a [`crate::Refusal`]: this is a file that is not a
/// corrections file, where a refusal is a corrections file that may not be
/// accepted. The submitter needs to hear about them differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Not JSON, or not an object.
    Malformed(String),
    /// A format version later than [`SUPPORTED`].
    UnsupportedVersion { found: u64, supported: u64 },
    /// A room key that is not an integer. Keys are strings because JSON has
    /// no integer keys, but every one must parse as an `i64`.
    BadRoomKey { field: &'static str, key: String },
    /// A `dirto` value outside the ten bearings and `cross-group`.
    BadBearing { from: Uid, to: Uid, value: String },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Malformed(why) => write!(f, "not a corrections file: {why}"),
            ParseError::UnsupportedVersion { found, supported } => write!(
                f,
                "this is a version {found} corrections file; this build reads up to version \
                 {supported}. A newer version may change what an existing field means, so it is \
                 refused rather than read in part."
            ),
            ParseError::BadRoomKey { field, key } => {
                write!(f, "`{field}` has a key that is not a room number: {key:?}")
            }
            ParseError::BadBearing { from, to, value } => write!(
                f,
                "`dirto` {} -> {} is {value:?}, which is not one of the ten bearings or \
                 `cross-group`",
                from.0, to.0
            ),
        }
    }
}

impl std::error::Error for ParseError {}

/// The validation-only half of [`Dirto`].
///
/// `cena_map` owns the type -- it is what a room file and the binary carry,
/// and one definition is the point of depending on that crate rather than
/// restating it. What lives here is only what *checking a submission* needs
/// and a client never does: reading upstream's spellings, and knowing which
/// bearing must appear on the other room.
pub trait Bearing: Sized {
    /// The bearing that must appear on the other room, for this one to be
    /// consistent.
    #[must_use]
    fn opposite(self) -> Self;

    /// Read a value, treating upstream's `none` and `skip` as absent.
    ///
    /// `None` is "not a bearing at all" and `Some(None)` is "explicitly no
    /// correction" -- the caller refuses the first and drops the second.
    fn read(value: &str) -> Option<Option<Self>>;
}

impl Bearing for Dirto {
    /// `cross-group` is its own opposite: un-welding an edge un-welds it from
    /// both ends.
    fn opposite(self) -> Dirto {
        match self {
            Dirto::North => Dirto::South,
            Dirto::South => Dirto::North,
            Dirto::East => Dirto::West,
            Dirto::West => Dirto::East,
            Dirto::Northeast => Dirto::Southwest,
            Dirto::Southwest => Dirto::Northeast,
            Dirto::Southeast => Dirto::Northwest,
            Dirto::Northwest => Dirto::Southeast,
            Dirto::Up => Dirto::Down,
            Dirto::Down => Dirto::Up,
            Dirto::CrossGroup => Dirto::CrossGroup,
        }
    }

    fn read(value: &str) -> Option<Option<Dirto>> {
        Some(match value {
            "north" => Some(Dirto::North),
            "northeast" => Some(Dirto::Northeast),
            "east" => Some(Dirto::East),
            "southeast" => Some(Dirto::Southeast),
            "south" => Some(Dirto::South),
            "southwest" => Some(Dirto::Southwest),
            "west" => Some(Dirto::West),
            "northwest" => Some(Dirto::Northwest),
            "up" => Some(Dirto::Up),
            "down" => Some(Dirto::Down),
            "cross-group" => Some(Dirto::CrossGroup),
            // Identical to no correction at all, so it is not carried as one.
            "none" | "skip" => None,
            _ => return None,
        })
    }
}

/// The September 2026 pilot wrote `maps` as `slug -> name`, before `area`
/// existed. A bare string reads as that name with no area
/// (`corrections-format.md`, "Reading older files").
impl<'de> Deserialize<'de> for SheetEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Either {
            Name(String),
            Sheet { name: String, area: Option<String> },
        }
        Ok(SheetEntry(match Either::deserialize(deserializer)? {
            Either::Name(name) => Sheet { name, area: None },
            Either::Sheet { name, area } => Sheet { name, area },
        }))
    }
}

/// A [`Sheet`] in either the current or the pilot spelling.
#[derive(Debug)]
pub struct SheetEntry(pub Sheet);

/// A parsed corrections file.
///
/// Every collection is a `BTreeMap` so that folding, reporting and the JSON
/// written back out are all in a stable order -- a correction set whose diff
/// reshuffles on every build is not reviewable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Correction {
    /// The format version this file declared. A file with no `version` field
    /// is version 1: exports written before the field existed, the September
    /// 2026 pilot among them, are version 1 in every respect but saying so.
    pub version: u64,
    /// What wrote the file. Provenance for a file found on its own later;
    /// **not** a compatibility signal.
    pub generator: String,
    /// The map file the corrections were made against. Provenance only, but
    /// it is what lets a reviewer tell which build this was reasoned about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_map: Option<String>,

    /// Direction corrections: room uid -> destination uid -> bearing.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub dirto: BTreeMap<Uid, BTreeMap<Uid, Dirto>>,
    /// Which grid a room is laid out on, when not its area's own sheet.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub map_membership: BTreeMap<Uid, String>,
    /// What the plates are.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub maps: BTreeMap<String, Sheet>,
    /// Which place a room belongs to, as opposed to which grid it is drawn
    /// on. A plate is a grid, not a place.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub area: BTreeMap<Uid, String>,
    /// The drags.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub placement: BTreeMap<Uid, Placement>,
}

/// The raw shape, before room keys are parsed out of strings.
#[derive(Deserialize)]
struct Raw {
    version: Option<u64>,
    #[serde(default)]
    generator: String,
    #[serde(default)]
    source_map: Option<String>,
    #[serde(default)]
    dirto: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    map_membership: BTreeMap<String, String>,
    #[serde(default)]
    maps: BTreeMap<String, SheetEntry>,
    #[serde(default)]
    area: BTreeMap<String, String>,
    #[serde(default)]
    placement: BTreeMap<String, Placement>,
    /// Review evidence, ignored here.
    ///
    /// Pictures are not corrections: nothing in them is merged into a map, a
    /// submission is complete without them, and a validator "should ignore
    /// this field when deciding what to merge". They are read off the raw
    /// JSON by the binary that writes them out for review, never carried in
    /// a [`Correction`].
    #[serde(default)]
    #[allow(dead_code)]
    pictures: BTreeMap<String, String>,
}

/// Parse a submission.
///
/// # Errors
///
/// [`ParseError`] when the text is not a corrections file, declares a version
/// this build does not know, or holds a key or bearing that cannot be read.
pub fn parse(text: &str) -> Result<Correction, ParseError> {
    let raw: Raw =
        serde_json::from_str(text).map_err(|error| ParseError::Malformed(error.to_string()))?;

    // A file with no `version` is version 1, and one from the future is
    // refused before any field is looked at -- reading a v3 file's `dirto`
    // would be assuming v3 did not change what `dirto` means.
    let version = raw.version.unwrap_or(1);
    if version > SUPPORTED {
        return Err(ParseError::UnsupportedVersion {
            found: version,
            supported: SUPPORTED,
        });
    }

    let mut dirto: BTreeMap<Uid, BTreeMap<Uid, Dirto>> = BTreeMap::new();
    for (from, destinations) in raw.dirto {
        let from = room_key("dirto", &from)?;
        for (to, value) in destinations {
            let to = room_key("dirto", &to)?;
            let bearing =
                <Dirto as Bearing>::read(&value).ok_or_else(|| ParseError::BadBearing {
                    from,
                    to,
                    value: value.clone(),
                })?;
            // `none`/`skip` parsed as "no correction": drop it rather than
            // record an entry that means nothing.
            if let Some(bearing) = bearing {
                dirto.entry(from).or_default().insert(to, bearing);
            }
        }
    }

    Ok(Correction {
        version,
        generator: raw.generator,
        source_map: raw.source_map,
        dirto,
        map_membership: keyed_by_room("map_membership", raw.map_membership)?,
        maps: raw.maps.into_iter().map(|(k, v)| (k, v.0)).collect(),
        area: keyed_by_room("area", raw.area)?,
        placement: keyed_by_room("placement", raw.placement)?,
    })
}

/// Pull the `pictures` field out of a submission, without validating it.
///
/// Separate from [`parse`] because pictures are evidence rather than
/// correction: they travel inside the file only because the submission form
/// takes one attachment, and they are dropped once a submission is accepted.
///
/// # Errors
///
/// [`ParseError::Malformed`] when the text is not JSON.
pub fn pictures(text: &str) -> Result<BTreeMap<String, String>, ParseError> {
    let raw: Raw =
        serde_json::from_str(text).map_err(|error| ParseError::Malformed(error.to_string()))?;
    Ok(raw.pictures)
}

/// JSON has no integer keys, so every room key arrives as a string and must
/// parse as an `i64` -- signed and wide, because the game sends negative
/// numbers for generated areas.
fn room_key(field: &'static str, key: &str) -> Result<Uid, ParseError> {
    key.parse::<i64>()
        .map(Uid)
        .map_err(|_| ParseError::BadRoomKey {
            field,
            key: key.to_owned(),
        })
}

fn keyed_by_room<T>(
    field: &'static str,
    raw: BTreeMap<String, T>,
) -> Result<BTreeMap<Uid, T>, ParseError> {
    raw.into_iter()
        .map(|(key, value)| Ok((room_key(field, &key)?, value)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_with_no_version_is_version_1() {
        let parsed = parse(r#"{"generator":"hydra-mapper 0.1.0"}"#).unwrap();
        assert_eq!(parsed.version, 1);
    }

    #[test]
    fn a_later_version_is_refused_rather_than_read_in_part() {
        let error = parse(r#"{"version":3,"dirto":{"1":{"2":"east"}}}"#).unwrap_err();
        assert_eq!(
            error,
            ParseError::UnsupportedVersion {
                found: 3,
                supported: 2
            }
        );
    }

    /// Upstream's "fall through to the command text" is what no correction
    /// already means, so it is dropped rather than stored.
    #[test]
    fn none_and_skip_are_dropped_not_refused() {
        let parsed = parse(r#"{"version":2,"dirto":{"1":{"2":"none","3":"skip"}}}"#).unwrap();
        assert!(parsed.dirto.is_empty());
    }

    #[test]
    fn an_unknown_bearing_is_refused() {
        let error = parse(r#"{"version":2,"dirto":{"1":{"2":"widdershins"}}}"#).unwrap_err();
        assert!(matches!(error, ParseError::BadBearing { .. }));
    }

    #[test]
    fn a_room_key_must_be_an_integer() {
        let error = parse(r#"{"version":2,"area":{"the barn":"icemule"}}"#).unwrap_err();
        assert_eq!(
            error,
            ParseError::BadRoomKey {
                field: "area",
                key: "the barn".to_owned()
            }
        );
    }

    /// `plan/21` §3d: the game sends negative numbers for generated areas.
    #[test]
    fn a_room_key_may_be_negative() {
        let parsed = parse(r#"{"version":2,"area":{"-9054":"the rift"}}"#).unwrap();
        assert_eq!(
            parsed.area.get(&Uid(-9054)).map(String::as_str),
            Some("the rift")
        );
    }

    /// The September 2026 pilot wrote `maps` as `slug -> name`.
    #[test]
    fn a_pilot_maps_entry_reads_as_a_sheet_with_no_area() {
        let parsed = parse(r#"{"maps":{"landing.well":"The Town Well"}}"#).unwrap();
        assert_eq!(
            parsed.maps.get("landing.well"),
            Some(&Sheet {
                name: "The Town Well".to_owned(),
                area: None
            })
        );
    }

    #[test]
    fn the_current_maps_form_carries_an_area() {
        let parsed =
            parse(r#"{"maps":{"landing.well":{"name":"The Town Well","area":"wehnimers"}}}"#)
                .unwrap();
        assert_eq!(
            parsed.maps["landing.well"].area.as_deref(),
            Some("wehnimers")
        );
    }

    #[test]
    fn cross_group_is_its_own_opposite() {
        assert_eq!(Dirto::CrossGroup.opposite(), Dirto::CrossGroup);
        assert_eq!(Dirto::Northeast.opposite(), Dirto::Southwest);
    }

    #[test]
    fn pictures_are_read_separately_and_not_carried_as_correction() {
        let text = r#"{"version":2,"pictures":{"a.b":"<svg/>"},"area":{"1":"x"}}"#;
        assert_eq!(pictures(text).unwrap()["a.b"], "<svg/>");
        let parsed = parse(text).unwrap();
        assert_eq!(parsed.area.len(), 1);
    }
}
