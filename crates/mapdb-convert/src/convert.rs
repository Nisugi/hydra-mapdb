//! One upstream room in, one [`cena_map::Room`] out.
//!
//! `plan/21` §5 step 2: **plain edges only.** A plain command and a numeric
//! cost pass through; every `;e` script becomes `Unported` with the id of its
//! shape. Nothing here interprets Ruby, and nothing is dropped: whatever cannot
//! be carried across is returned as a [`Problem`] for the report.

use cena_map::{Cost, Crossing, Exit, ExitKind, Image, Room, RoomId, Uid};

use crate::shape::shape_id;
use crate::upstream::{UpstreamCost, UpstreamLocation, UpstreamRoom, is_script};

/// Something in an upstream room that could not be carried across as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The room it was found in.
    pub room: u32,
    /// What was wrong, in words a map author can act on.
    pub what: String,
}

/// A converted room, and what could not be converted in it.
#[derive(Debug)]
pub struct Converted {
    pub room: Room,
    pub problems: Vec<Problem>,
}

const META_PREFIX: &str = "meta:";
/// Forage sightings are most of upstream's tag volume (44,781 entries) and are
/// lookup data, not routing data. They are left out of `meta` here; where they
/// go instead is `plan/21` §6.
const FORAGE_META: &str = "forage-sensed";

/// An exit's cost, or the string upstream had there that is not a script.
fn cost_of<'u>(
    upstream: Option<&'u Option<UpstreamCost>>,
    room: &crate::recognise::RoomFacts<'_>,
) -> Result<Option<Cost>, &'u str> {
    match upstream {
        Some(Some(UpstreamCost::Seconds(seconds))) => Ok(Some(Cost::Fixed(*seconds))),
        Some(Some(UpstreamCost::Script(script))) if is_script(script) => Ok(Some(
            crate::recognise::cost(script, room).unwrap_or_else(|| Cost::Unported {
                unported: shape_id(script),
            }),
        )),
        Some(Some(UpstreamCost::Script(other))) => Err(other),
        Some(None) | None => Ok(None),
    }
}

/// Convert one room.
#[must_use]
pub fn convert_room(upstream: UpstreamRoom) -> Converted {
    let id = upstream.id;
    let mut problems = Vec::new();
    let mut problem = |what: String| problems.push(Problem { room: id, what });

    for field in upstream.unknown.keys() {
        problem(format!("unknown upstream field `{field}`"));
    }

    // Exits first, while the room is still whole: their costs may ask about
    // it (`RoomFacts`), and what follows takes it apart.
    let facts = upstream.room_facts();
    let mut exits = Vec::with_capacity(upstream.wayto.len());
    for (destination, command) in &upstream.wayto {
        let Ok(to) = destination.parse::<u32>() else {
            problem(format!("wayto key `{destination}` is not a room id"));
            continue;
        };
        let cost = match cost_of(upstream.timeto.get(destination), &facts) {
            Ok(cost) => cost,
            Err(other) => {
                problem(format!(
                    "timeto for {to} is a string but not a script: {other:?}"
                ));
                None
            }
        };
        let (kind, crossing) = if is_script(command) {
            let crossing = crate::recognise::crossing(command, id, to)
                .unwrap_or_else(|| Crossing::Unported(shape_id(command)));
            (ExitKind::Scripted, crossing)
        } else {
            (kind_of(command), Crossing::Command(command.clone()))
        };
        let cost = crate::recognise::priced_for_crossing(&crossing, cost);
        let cost = crate::go2::as_go2_prices_it(id, to, cost);
        exits.push(Exit {
            to: RoomId(to),
            kind,
            crossing,
            cost,
            // Upstream has no layout data. A corrected bearing arrives later,
            // from `corrections/`, applied over this conversion.
            dirto: None,
        });
    }
    for destination in upstream.timeto.keys() {
        if !upstream.wayto.contains_key(destination) {
            problem(format!("timeto for `{destination}` has no matching wayto"));
        }
    }
    exits.sort_by_key(|exit| exit.to);

    let (location, location_unknowable) = match upstream.location {
        Some(UpstreamLocation::Named(name)) => (Some(name), false),
        Some(UpstreamLocation::Flag(false)) => (None, true),
        Some(UpstreamLocation::Flag(true)) => {
            problem("`location: true` has no meaning upstream defines".to_owned());
            (None, false)
        }
        None => (None, false),
    };

    let image = match (upstream.image, upstream.image_coords) {
        (Some(file), Some(coords)) => {
            let rect = rect(&coords);
            if rect.is_none() {
                problem(format!("image_coords is not four pixel values: {coords:?}"));
            }
            rect.map(|rect| Image { file, rect })
        }
        (None, None) => None,
        // The two fields do not always travel together upstream: 21,838 rooms
        // have coords and 21,711 an image (`plan/21` §1). Half a position
        // places nothing, so neither half is kept, and the report says so.
        (Some(_), None) => {
            problem("image with no image_coords".to_owned());
            None
        }
        (None, Some(_)) => {
            problem("image_coords with no image".to_owned());
            None
        }
    };

    let (tags, meta) = split_tags(upstream.tags.unwrap_or_default());

    let room = Room {
        id: RoomId(id),
        uid: upstream
            .uid
            .unwrap_or_default()
            .into_iter()
            .map(Uid)
            .collect(),
        title: upstream.title.unwrap_or_default(),
        description: upstream.description.unwrap_or_default(),
        paths: upstream.paths.unwrap_or_default(),
        location,
        location_unknowable,
        check_location: upstream.check_location.unwrap_or(false),
        unique_loot: upstream.unique_loot.unwrap_or_default(),
        climate: upstream.climate,
        terrain: upstream.terrain,
        tags,
        meta,
        image,
        exits,
        // Layout is not upstream's to say: `map`, `area` and `placement` are
        // filled in from `corrections/` after this conversion runs.
        map: None,
        area: None,
        placement: None,
    };
    Converted { room, problems }
}

fn rect(coords: &[i64]) -> Option<[i32; 4]> {
    let [left, top, right, bottom] = coords else {
        return None;
    };
    Some([
        i32::try_from(*left).ok()?,
        i32::try_from(*top).ok()?,
        i32::try_from(*right).ok()?,
        i32::try_from(*bottom).ok()?,
    ])
}

/// Plain tags, and `meta:` tags with the prefix removed and forage sightings
/// left out. Both keep upstream's order.
fn split_tags(all: Vec<String>) -> (Vec<String>, Vec<String>) {
    let mut tags = Vec::new();
    let mut meta = Vec::new();
    for tag in all {
        match tag.strip_prefix(META_PREFIX) {
            Some(rest) if rest.starts_with(FORAGE_META) => {}
            Some(rest) => meta.push(rest.to_owned()),
            None => tags.push(tag),
        }
    }
    (tags, meta)
}

/// What kind of exit a plain command is. Upstream spells directions out in
/// full; the short forms are accepted because a hand-written override will use
/// them.
#[must_use]
pub fn kind_of(command: &str) -> ExitKind {
    let mut words = command.split_whitespace();
    let verb = words.next().unwrap_or_default();
    let has_object = words.next().is_some();
    match verb {
        "north" | "northeast" | "east" | "southeast" | "south" | "southwest" | "west"
        | "northwest" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "nw"
            if !has_object =>
        {
            ExitKind::Cardinal
        }
        "up" | "down" | "u" | "d" if !has_object => ExitKind::Vertical,
        "out" if !has_object => ExitKind::Out,
        "go" => ExitKind::Go,
        "climb" => ExitKind::Climb,
        _ => ExitKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_come_from_the_command() {
        assert_eq!(kind_of("north"), ExitKind::Cardinal);
        assert_eq!(kind_of("sw"), ExitKind::Cardinal);
        assert_eq!(kind_of("up"), ExitKind::Vertical);
        assert_eq!(kind_of("out"), ExitKind::Out);
        assert_eq!(kind_of("go oak door"), ExitKind::Go);
        assert_eq!(kind_of("climb rope"), ExitKind::Climb);
        assert_eq!(kind_of("swim north"), ExitKind::Other);
        assert_eq!(kind_of("urchin guide bank"), ExitKind::Other);
        // `north gate` is a command with an object, not the direction.
        assert_eq!(kind_of("north gate"), ExitKind::Other);
    }

    #[test]
    fn forage_sightings_are_left_out_and_the_prefix_is_removed() {
        let (tags, meta) = split_tags(vec![
            "bank".into(),
            "meta:forage-sensed".into(),
            "meta:forage-sensed:day:2026-09".into(),
            "meta:nomagic".into(),
            "murdroot".into(),
        ]);
        assert_eq!(tags, ["bank", "murdroot"]);
        assert_eq!(meta, ["nomagic"]);
    }
}
