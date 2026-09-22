//! Reading a conversion directory into a [`Map`], and writing it as one file.

use std::fmt;
use std::fs;
use std::path::Path;

use cena_map::{Map, Room, RoomId, binary, files};

/// Why a directory could not be combined. Carries the path, because "invalid
/// JSON" among tens of thousands of files is not a message anyone can act on.
#[derive(Debug)]
pub struct CombineError(String);

impl fmt::Display for CombineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CombineError {}

fn failed(path: &Path, why: impl fmt::Display) -> CombineError {
    CombineError(format!("{}: {why}", path.display()))
}

/// Read every room the index names.
///
/// Only the index is trusted to say what is current (`cena_map::files`): a room
/// file it does not name is ignored, and one it names that is missing, invalid,
/// or holds a different id is an error -- a map silently missing a room is
/// worse than no map.
///
/// # Errors
///
/// [`CombineError`], naming the file.
pub fn combine(dir: &Path) -> Result<Map, CombineError> {
    let index_path = dir.join(files::INDEX);
    let index = fs::read_to_string(&index_path).map_err(|error| failed(&index_path, error))?;
    let ids: Vec<RoomId> =
        serde_json::from_str(&index).map_err(|error| failed(&index_path, error))?;

    let mut rooms = Vec::with_capacity(ids.len());
    for id in ids {
        let path = dir.join(files::room_file(id));
        let text = fs::read_to_string(&path).map_err(|error| failed(&path, error))?;
        let room: Room = serde_json::from_str(&text).map_err(|error| failed(&path, error))?;
        if room.id != id {
            return Err(failed(
                &path,
                format!("holds room {}, not {}", room.id.0, id.0),
            ));
        }
        rooms.push(room);
    }
    Map::from_rooms(rooms).map_err(|error| failed(&index_path, error))
}

/// What was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Written {
    pub rooms: usize,
    pub bytes: usize,
}

/// Encode a map and write it, **then read it back and compare**. A map file
/// that does not decode to what was encoded must never reach a client, and the
/// moment it is written is the last moment anything can refuse it.
///
/// # Errors
///
/// [`CombineError`] when the map does not encode, the file does not write, or
/// what was written does not decode to the same map.
pub fn write(map: &Map, out: &Path) -> Result<Written, CombineError> {
    let bytes = binary::encode(map).map_err(|error| failed(out, error))?;
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|error| failed(parent, error))?;
    }
    fs::write(out, &bytes).map_err(|error| failed(out, error))?;

    let read_back = fs::read(out).map_err(|error| failed(out, error))?;
    let decoded = binary::decode(&read_back).map_err(|error| failed(out, error))?;
    if &decoded != map {
        return Err(failed(
            out,
            "the written file does not decode to the map that was encoded",
        ));
    }
    Ok(Written {
        rooms: map.len(),
        bytes: bytes.len(),
    })
}
