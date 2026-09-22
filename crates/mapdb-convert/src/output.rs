//! Writing a conversion to disk.
//!
//! One room per file, so a weekly upstream update is a small, reviewable diff
//! (`plan/21` §3a; the idea is `Nisugi/cartographer`'s). Files are sharded by
//! thousand -- `rooms/036/36838.json` -- because 36,838 entries in one
//! directory is unkind to every tool that lists it.
//!
//! Two properties make the output diffable. It is **deterministic**: field
//! order is the struct's, exits are sorted, and the text is pretty-printed with
//! a trailing newline. And it is **quiet**: a file whose content did not change
//! is not rewritten, so its modification time still means something.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cena_map::{Room, RoomId};

use crate::report::Report;
use crate::run::Conversion;

/// What a write did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Written {
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
}

/// Where a room's file lives under `out`. The layout is `cena_map::files`'s,
/// shared with the combiner that reads it.
#[must_use]
pub fn room_path(out: &Path, id: RoomId) -> PathBuf {
    out.join(cena_map::files::room_file(id))
}

/// Write every room, the index, and the report.
///
/// `index.json` lists every room id in this conversion. It, not a directory
/// listing, is what says which room files are current: a room removed upstream
/// leaves a stale file behind, and nothing downstream may pick it up by
/// walking the tree.
///
/// # Errors
///
/// Any filesystem error, or a room that cannot be serialised.
pub fn write(out: &Path, conversion: &Conversion) -> io::Result<Written> {
    let mut written = Written::default();
    for room in &conversion.rooms {
        let path = room_path(out, room.id);
        match write_if_changed(&path, &pretty(room)?)? {
            Change::Created => written.created += 1,
            Change::Updated => written.updated += 1,
            Change::Unchanged => written.unchanged += 1,
        }
    }
    let ids: Vec<RoomId> = conversion.rooms.iter().map(|room| room.id).collect();
    let index = serde_json::to_string(&ids).map_err(io::Error::other)?;
    write_if_changed(&out.join(cena_map::files::INDEX), &format!("{index}\n"))?;
    write_report(out, &conversion.report)?;
    Ok(written)
}

fn write_report(out: &Path, report: &Report) -> io::Result<()> {
    let dir = out.join("report");
    write_if_changed(&dir.join("summary.txt"), &report.summary())?;
    write_if_changed(
        &dir.join("unported_crossings.tsv"),
        &Report::shapes_tsv(&report.crossing_shapes()),
    )?;
    write_if_changed(
        &dir.join("unported_costs.tsv"),
        &Report::shapes_tsv(&report.cost_shapes()),
    )?;
    let mut problems = String::from("room\tproblem\n");
    for problem in &report.problems {
        // Writing to a String cannot fail.
        let _ = writeln!(problems, "{}\t{}", problem.room, problem.what);
    }
    write_if_changed(&dir.join("problems.tsv"), &problems)?;
    Ok(())
}

fn pretty(room: &Room) -> io::Result<String> {
    let mut text = serde_json::to_string_pretty(room).map_err(io::Error::other)?;
    text.push('\n');
    Ok(text)
}

enum Change {
    Created,
    Updated,
    Unchanged,
}

fn write_if_changed(path: &Path, content: &str) -> io::Result<Change> {
    let change = match fs::read_to_string(path) {
        Ok(existing) if existing == content => return Ok(Change::Unchanged),
        Ok(_) => Change::Updated,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Change::Created,
        Err(error) => return Err(error),
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(change)
}
