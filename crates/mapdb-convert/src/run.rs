//! The conversion as a pure function: upstream JSON text in, rooms and a report
//! out. File I/O lives in [`crate::output`] and `main.rs`, so tests can run the
//! whole pipeline on a string.

use std::collections::BTreeSet;

use cena_map::{Room, RoomId};

use crate::convert::{Problem, convert_room};
use crate::report::Report;
use crate::upstream::UpstreamRoom;

/// Everything one conversion produced.
#[derive(Debug)]
pub struct Conversion {
    /// Rooms, ordered by id.
    pub rooms: Vec<Room>,
    pub report: Report,
}

/// Convert an upstream map file's text.
///
/// # Errors
///
/// When the text is not a JSON list of rooms. A malformed *room* does not fail
/// the run -- it becomes problems in the report -- but a file that is not a map
/// at all has nothing to report on.
pub fn convert(upstream_json: &str) -> Result<Conversion, serde_json::Error> {
    let mut upstream: Vec<UpstreamRoom> = serde_json::from_str(upstream_json)?;
    let mut report = Report::default();
    // Before anything reads a cost: see `delegate`.
    report.delegations_resolved = crate::delegate::resolve(&mut upstream);
    let mut rooms = Vec::with_capacity(upstream.len());
    for room in upstream {
        report.see_scripts(&room);
        let converted = convert_room(room);
        report.problems.extend(converted.problems);
        rooms.push(converted.room);
    }
    rooms.sort_by_key(|room| room.id);

    let ids: BTreeSet<RoomId> = rooms.iter().map(|room| room.id).collect();
    if ids.len() != rooms.len() {
        let mut seen = BTreeSet::new();
        for room in &rooms {
            if !seen.insert(room.id) {
                report.problems.push(Problem {
                    room: room.id.0,
                    what: "room id appears more than once".to_owned(),
                });
            }
        }
    }
    for room in &rooms {
        report.see_room(room);
        for exit in &room.exits {
            if !ids.contains(&exit.to) {
                report.dangling += 1;
                report.problems.push(Problem {
                    room: room.id.0,
                    what: format!("exit to {}, which is not a room", exit.to.0),
                });
            }
        }
    }
    Ok(Conversion { rooms, report })
}
