//! The whole offline pipeline: upstream JSON → converter → per-room files →
//! combiner → one binary → the map a client would load.
//!
//! The converter is a dev-dependency for exactly this test. "The combiner reads
//! what the converter writes" is the contract between the two tools, and a test
//! that hand-wrote its own room files would hold neither side to it.

use std::path::PathBuf;

use cena_map::{Crossing, RoomId, Uid, binary, files};
use cena_map_combine::combine::{combine, write};
use cena_mapdb_convert::{output, run};

const CUT: &str = include_str!("../../mapdb-convert/tests/fixtures/mapdb_cut.json");

/// A scratch directory unique to this test process and this test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-map-combine-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn the_converters_output_combines_into_the_map_it_described() {
    let dir = scratch("e2e");
    let conversion = run::convert(CUT).unwrap();
    output::write(&dir, &conversion).unwrap();

    let map = combine(&dir).unwrap();
    assert_eq!(
        map.rooms(),
        conversion.rooms.as_slice(),
        "nothing lost between the two tools"
    );

    let out = dir.join("hydra.map");
    let written = write(&map, &out).unwrap();
    assert_eq!(written.rooms, 24);

    // What a client does: read the file, decode it, ask it questions.
    let loaded = binary::decode(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(loaded, map);
    assert_eq!(
        loaded.ids_for_uid(Uid(-9054)),
        [RoomId(4136)],
        "found by the game's number"
    );
    assert!(loaded.ids_for_uid(Uid(0)).is_empty());
    let hub = loaded.room(RoomId(30714)).unwrap();
    assert!(
        hub.exits
            .iter()
            .all(|exit| matches!(exit.crossing, Crossing::Command(_)))
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// Only the index says which room files are current.
#[test]
fn a_stale_room_file_the_index_does_not_name_is_ignored() {
    let dir = scratch("stale");
    output::write(&dir, &run::convert(CUT).unwrap()).unwrap();
    let stale = dir.join(files::room_file(RoomId(99_999)));
    std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
    std::fs::write(&stale, r#"{"id":99999}"#).unwrap();

    let map = combine(&dir).unwrap();
    assert_eq!(map.len(), 24);
    assert!(map.room(RoomId(99_999)).is_none());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A map silently missing a room is worse than no map.
#[test]
fn a_room_the_index_names_must_be_there_and_be_itself() {
    let dir = scratch("missing");
    output::write(&dir, &run::convert(CUT).unwrap()).unwrap();
    let path = dir.join(files::room_file(RoomId(7)));

    std::fs::write(&path, r#"{"id":8}"#).unwrap();
    let wrong = combine(&dir).unwrap_err().to_string();
    assert!(wrong.contains("holds room 8, not 7"), "{wrong}");

    std::fs::write(&path, "{").unwrap();
    assert!(combine(&dir).unwrap_err().to_string().contains("7.json"));

    std::fs::remove_file(&path).unwrap();
    assert!(combine(&dir).unwrap_err().to_string().contains("7.json"));

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_directory_with_no_index_is_not_a_conversion() {
    let dir = scratch("empty");
    std::fs::create_dir_all(&dir).unwrap();
    assert!(
        combine(&dir)
            .unwrap_err()
            .to_string()
            .contains("index.json")
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
