//! Reading the repository: the current map's uids, and the accepted set.
//!
//! The one place this crate touches a filesystem. Everything decidable is
//! decided in [`crate::validate`] and [`crate::fold`] against plain values,
//! so this module only has to find them.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cena_map::{Room, RoomId, files};

use crate::file::{Correction, ParseError, parse};
use crate::validate::CurrentMap;

/// Where accepted corrections live, relative to the repository root.
pub const DIR: &str = "corrections";

/// Every uid the current conversion knows, read from the room files.
///
/// Only `index.json` is trusted to say what is current, exactly as the
/// combiner does: a room file the index does not name is stale, and a
/// correction validated against one would be accepted for a room that is not
/// in the map.
///
/// # Errors
///
/// Any filesystem error, or a room file that is not valid JSON.
pub fn current_map(dir: &Path) -> io::Result<CurrentMap> {
    let index_path = dir.join(files::INDEX);
    let index = fs::read_to_string(&index_path)?;
    let ids: Vec<RoomId> = serde_json::from_str(&index).map_err(io::Error::other)?;

    let mut map = CurrentMap::default();
    for id in ids {
        let path = dir.join(files::room_file(id));
        let text = fs::read_to_string(&path)
            .map_err(|error| io::Error::other(format!("{}: {error}", path.display())))?;
        let room: Room = serde_json::from_str(&text)
            .map_err(|error| io::Error::other(format!("{}: {error}", path.display())))?;
        map.uids.extend(room.uid);
    }

    // Provenance for the staleness warning: what upstream build `rooms/` was
    // converted from. Absent is not an error -- it only means no warning can
    // be given.
    map.source_map = fs::read_to_string(dir.join("upstream_updated_at"))
        .ok()
        .map(|text| text.trim().to_owned());

    Ok(map)
}

/// Every accepted correction, in name order.
///
/// Names begin with the date the submission was accepted, so name order is
/// the order they landed -- which is the order [`crate::fold`] resolves an
/// override in.
///
/// # Errors
///
/// Any filesystem error, or a correction file that no longer parses.
pub fn accepted(root: &Path) -> io::Result<Vec<(String, Correction)>> {
    let dir = root.join(DIR);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let text = fs::read_to_string(&path)?;
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let correction = parse(&text).map_err(|error: ParseError| {
                io::Error::other(format!("{}: {error}", path.display()))
            })?;
            Ok((name, correction))
        })
        .collect()
}

/// Write a correction where the repository keeps them, pretty-printed with a
/// trailing newline so a diff of it is readable.
///
/// # Errors
///
/// Any filesystem error, or a correction that does not serialise.
pub fn write(root: &Path, name: &str, correction: &Correction) -> io::Result<PathBuf> {
    let dir = root.join(DIR);
    fs::create_dir_all(&dir)?;
    let path = dir.join(name);
    let mut text = serde_json::to_string_pretty(correction).map_err(io::Error::other)?;
    text.push('\n');
    fs::write(&path, text)?;
    Ok(path)
}

/// Write the pictures out as loose SVG files, for a reviewer to look at.
///
/// These are **evidence, not correction**: they are written on the review
/// branch so GitHub renders them in the pull request, and the merge does not
/// carry them into `main`. Nothing downstream reads them.
///
/// # Errors
///
/// Any filesystem error.
pub fn write_pictures(dir: &Path, pictures: &BTreeMap<String, String>) -> io::Result<Vec<PathBuf>> {
    if pictures.is_empty() {
        return Ok(Vec::new());
    }
    fs::create_dir_all(dir)?;
    pictures
        .iter()
        .map(|(slug, svg)| {
            // A slug is dotted (`icemule.south.barn.interiors`), which is
            // already a safe file name, but a submitted file is not trusted
            // to hold one: anything that could climb out of the directory is
            // replaced rather than refused, since pictures never gate a
            // submission.
            let safe: String = slug
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let path = dir.join(format!("{safe}.svg"));
            fs::write(&path, svg)?;
            Ok(path)
        })
        .collect()
}
