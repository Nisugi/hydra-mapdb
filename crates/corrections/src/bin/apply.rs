//! `cena-corrections-apply <repo-root> <rooms-dir>`
//!
//! Fold every accepted correction and write the result into the room files,
//! so the combiner's build carries them.
//!
//! This runs **in the build, not at merge**, and the distinction is the whole
//! design. `corrections/` is the reviewed source; `rooms/` is regenerated
//! from upstream by `convert.yml` and anything written into it is lost on the
//! next refresh. Re-applying here means an upstream update cannot silently
//! drop a correction somebody submitted, because the corrections are applied
//! again after every conversion.
//!
//! Exit codes: `0` applied, `1` a clash between two accepted files, `2` the
//! tool was called wrongly.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use cena_corrections::disk;
use cena_corrections::fold::{Folded, fold};
use cena_map::{Room, RoomId, Uid, files};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [root, rooms] = args.as_slice() else {
        eprintln!("usage: cena-corrections-apply <repo-root> <rooms-dir>");
        return ExitCode::from(2);
    };
    match run(Path::new(root), Path::new(rooms)) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("cena-corrections-apply: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(root: &Path, rooms: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let files_in = disk::accepted(root)?;
    if files_in.is_empty() {
        println!("no corrections to apply");
        return Ok(true);
    }
    let names: Vec<&str> = files_in.iter().map(|(name, _)| name.as_str()).collect();
    println!(
        "folding {} correction file(s): {}",
        names.len(),
        names.join(", ")
    );

    let out = fold(&files_in);
    for (what, count) in out.folded.counts() {
        println!("  {count} {what}");
    }

    // A clash means two accepted files correct the same thing differently.
    // Validation refuses that before acceptance, so reaching here means two
    // submissions were merged too close together to have been compared --
    // the build stops rather than picking one by file order.
    if !out.clashes.is_empty() {
        eprintln!("\n{} correction(s) conflict:", out.clashes.len());
        for clash in &out.clashes {
            eprintln!("  {clash}");
        }
        eprintln!(
            "\nThe later file would win by name order, which is not a decision a build should \
             make. Revert one of the two, or supersede both with a file that says what is right."
        );
        return Ok(false);
    }

    let applied = apply(rooms, &out.folded)?;
    println!("\napplied to {applied} room file(s)");
    Ok(true)
}

/// Write the folded corrections into the room files they name.
///
/// Corrections are uid-keyed and rooms are id-keyed, so this builds the
/// uid -> id index first. A uid naming several rooms -- an instanced or
/// morphing room (`plan/21` §3d) -- has the correction applied to every room
/// carrying it: the submitter corrected a game room number, and every node
/// that number can mean is what they meant.
fn apply(rooms: &Path, folded: &Folded) -> Result<usize, Box<dyn std::error::Error>> {
    let index_path = rooms.join(files::INDEX);
    let ids: Vec<RoomId> = serde_json::from_str(&std::fs::read_to_string(&index_path)?)?;

    // Read every room once, keeping only what the corrections touch.
    let dirto = folded.dirto();
    let membership = folded.map_membership();
    let area = folded.area();
    let placement = folded.placement();

    let mut wanted: BTreeMap<Uid, Vec<RoomId>> = BTreeMap::new();
    let mut rooms_by_id: BTreeMap<RoomId, Room> = BTreeMap::new();
    for id in ids {
        let path = rooms.join(files::room_file(id));
        let room: Room = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        let touched = room.uid.iter().any(|uid| {
            dirto.contains_key(uid)
                || membership.contains_key(uid)
                || area.contains_key(uid)
                || placement.contains_key(uid)
        });
        for uid in &room.uid {
            wanted.entry(*uid).or_default().push(id);
        }
        if touched {
            rooms_by_id.insert(id, room);
        }
    }

    let mut changed = 0;
    let ids: Vec<RoomId> = rooms_by_id.keys().copied().collect();
    for id in ids {
        let Some(room) = rooms_by_id.get(&id) else {
            continue;
        };
        let mut room = room.clone();
        let before = room.clone();

        for uid in &before.uid {
            if let Some(slug) = membership.get(uid) {
                room.map = Some((*slug).to_owned());
            }
            if let Some(name) = area.get(uid) {
                room.area = Some((*name).to_owned());
            }
            if let Some(spot) = placement.get(uid) {
                room.placement = Some(*spot);
            }
            // `dirto` is per edge: the correction names a destination uid,
            // and the exit to set it on is the one leading to a room that
            // carries that uid.
            if let Some(destinations) = dirto.get(uid) {
                for (to, bearing) in destinations {
                    let targets = wanted.get(to).map(Vec::as_slice).unwrap_or_default();
                    for exit in &mut room.exits {
                        if targets.contains(&exit.to) {
                            exit.dirto = Some(*bearing);
                        }
                    }
                }
            }
        }

        if room != before {
            let path = rooms.join(files::room_file(id));
            let mut text = serde_json::to_string_pretty(&room)?;
            text.push('\n');
            std::fs::write(&path, text)?;
            changed += 1;
        }
    }

    Ok(changed)
}
