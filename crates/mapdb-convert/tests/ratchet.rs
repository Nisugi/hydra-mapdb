//! The porting ratchet: over the real upstream map, the number of unported
//! edges may only fall.
//!
//! `plan/21` §5 step 2. This test **is the worklist and the update path**. Drop
//! in a newer upstream map and run it: if upstream grew a shape nobody has
//! ported, the count rises, the test fails, and the residue it prints names the
//! shape. Port a shape, the count falls, and the test fails until the baseline
//! is turned down to match -- so the gain is recorded and cannot be given back.
//!
//! # The second number: rooms a walker can reach
//!
//! Unported *exits* is the wrong thing to watch alone. MEASURED (`plan/21` §5
//! step 5): plain exits are 90% of the map and reach a quarter of it, because
//! a few scripted exits are the bridges. So `reachable` pins how many rooms a
//! walk from Wehnimer's Town Square reaches knowing nothing about the walker,
//! and it may only **rise**.
//!
//! Env-gated, like `cena-protocol`'s corpus replay: the upstream map is 43 MB
//! and lives under the gitignored `reference/`.
//!
//! ```text
//! $env:CENA_MAPDB = "E:\Cena\reference\mapdb\map-1789942730.json"
//! cargo test -p cena-mapdb-convert --test ratchet -- --nocapture
//! ```

use std::path::PathBuf;

use cena_map::{Map, RoomId, Target, Walker, as_converted, priced_for};
use cena_mapdb_convert::report::{Report, ShapeRow};
use cena_mapdb_convert::run::convert;

/// The env var naming the upstream map file.
const MAPDB_VAR: &str = "CENA_MAPDB";
const BASELINE: &str = include_str!("unported.baseline");
/// How many shapes the residue listing shows.
const RESIDUE_ROWS: usize = 25;

/// The upstream file, or `None` when the tier is not enabled.
///
/// Takes the value rather than reading the environment so both branches are
/// testable: `set_var` is `unsafe` in this edition and `unsafe_code` is denied.
fn mapdb_of(raw: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let raw = raw?;
    if raw.to_string_lossy().trim().is_empty() {
        return None;
    }
    let path = PathBuf::from(raw);
    assert!(
        path.is_file(),
        "{MAPDB_VAR} is set to {} but that is not a file. An enabled gate that cannot find \
         its input must fail loudly -- silently skipping is how a ratchet stops turning.",
        path.display()
    );
    Some(path)
}

/// `name count` lines; `#` starts a comment.
fn baseline(name: &str) -> Option<usize> {
    BASELINE
        .lines()
        .filter_map(|line| line.split('#').next())
        .filter_map(|line| line.trim().split_once(' '))
        .find(|(key, _)| *key == name)
        .and_then(|(_, value)| value.trim().parse().ok())
}

fn residue(title: &str, rows: &[&ShapeRow]) {
    let total: usize = rows.iter().map(|row| row.edges).sum();
    println!("\n{title}: {total} edges in {} shapes", rows.len());
    for row in rows.iter().take(RESIDUE_ROWS) {
        let shape: String = row.shape.chars().take(150).collect();
        println!("{:>6}  {}  {:<14} {shape}", row.edges, row.id.0, row.sample);
    }
}

fn check(what: &str, now: usize, pinned: usize) {
    assert!(
        now <= pinned,
        "{what}: {now} unported, baseline {pinned}. Upstream has gained scripted edges that \
         nothing here ports. The residue above names their shapes; port them, or -- if the new \
         map is simply bigger -- raise the baseline in the same change that says why."
    );
    assert!(
        now >= pinned,
        "{what}: {now} unported, baseline {pinned}. That is progress -- turn \
         `tests/unported.baseline` down to {now} so it cannot be given back."
    );
}

/// Town Square Central, Wehnimer's Landing.
const START: RoomId = RoomId(228);

/// A walker with the paid services switched on and nothing in the way of
/// them: urchin guides and portmasters, seen and on foot. **No profession**,
/// so every guild-only exit stays shut -- this is "what money opens", and it
/// is the second reachability figure because the first counts no gate at all.
fn equipped() -> Walker {
    let on = |name: &str| (name.to_owned(), "true".to_owned());
    let flag = |name: &str, value: bool| (name.to_owned(), value);
    Walker {
        settings: [on("use_urchins"), on("use_portmasters")].into(),
        flags: [
            flag("urchin_access", true),
            flag("hidden", false),
            flag("invisible", false),
            flag("mounted", false),
        ]
        .into(),
        ..Walker::default()
    }
}

fn count(map: &Map, routes: &cena_map::Routes<'_>) -> usize {
    map.rooms()
        .iter()
        .filter(|room| routes.seconds_to(room.id).is_some())
        .count()
}

fn rises(what: &str, now: usize, pinned: usize) {
    assert!(
        now >= pinned,
        "{what}: {now} rooms, baseline {pinned}. The map got SMALLER for a walker: an arm \
         stopped matching, or upstream cut a bridge. The residue above names what is unported."
    );
    assert!(
        now <= pinned,
        "{what}: {now} rooms, baseline {pinned}. That is progress -- turn \
         `tests/unported.baseline` up to {now} so it cannot be given back."
    );
}

#[test]
fn unported_edges_only_fall() {
    let Some(path) = mapdb_of(std::env::var_os(MAPDB_VAR).as_deref()) else {
        eprintln!("{MAPDB_VAR} not set; skipping the porting ratchet");
        return;
    };
    let text = std::fs::read_to_string(&path).unwrap();
    let conversion = convert(&text).unwrap();
    let report: &Report = &conversion.report;

    print!("{}", report.summary());
    residue("UNPORTED CROSSINGS", &report.crossing_shapes());
    residue("UNPORTED COSTS", &report.cost_shapes());
    if !report.problems.is_empty() {
        println!("\nPROBLEMS: {}", report.problems.len());
        for problem in report.problems.iter().take(RESIDUE_ROWS) {
            println!("  room {}: {}", problem.room, problem.what);
        }
    }

    check(
        "crossings",
        report.unported_crossings,
        baseline("crossings").unwrap(),
    );
    check("costs", report.unported_costs, baseline("costs").unwrap());

    // `cena_map::step`: a guard may change how an exit is crossed, never
    // whether. Anything that decides *whether* belongs in the cost.
    let stranding: Vec<(u32, u32)> = conversion
        .rooms
        .iter()
        .flat_map(|room| room.exits.iter().map(move |exit| (room.id.0, exit)))
        .filter_map(|(from, exit)| match &exit.crossing {
            cena_map::Crossing::Steps(steps) if !cena_map::moves_whatever_is_known(steps) => {
                Some((from, exit.to.0))
            }
            _ => None,
        })
        .collect();
    assert!(
        stranding.is_empty(),
        "ported crossings whose guards can leave a walker with no move: {:?}",
        &stranding[..stranding.len().min(10)]
    );

    let map = Map::from_rooms(conversion.rooms).unwrap();
    let plain = count(&map, &map.routes(START, Target::Everything, as_converted));
    let walker = equipped();
    let paid = count(
        &map,
        &map.routes(START, Target::Everything, priced_for(&walker)),
    );
    println!();
    println!(
        "REACHABLE from room {} of {} rooms: {plain} knowing nothing, {paid} with paid services on",
        START.0,
        map.len()
    );
    rises("reachable", plain, baseline("reachable").unwrap());
    rises(
        "reachable_equipped",
        paid,
        baseline("reachable_equipped").unwrap(),
    );
}

#[test]
fn the_gate_skips_when_unset_and_is_loud_when_wrong() {
    assert_eq!(mapdb_of(None), None);
    assert_eq!(mapdb_of(Some(std::ffi::OsStr::new("  "))), None);
    let missing =
        std::panic::catch_unwind(|| mapdb_of(Some(std::ffi::OsStr::new("no/such/map.json"))));
    assert!(
        missing.is_err(),
        "a set-but-wrong path must not skip quietly"
    );
}

#[test]
fn the_baseline_parses() {
    // Both reached zero on 2026-09-21: every upstream script is ported. What
    // is asked of them now is only that they are there to be read.
    assert!(baseline("crossings").is_some());
    assert!(baseline("costs").is_some());
    assert!(baseline("reachable").unwrap() > 0);
    assert!(baseline("reachable_equipped").unwrap() > 0);
}
