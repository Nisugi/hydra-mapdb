//! The converter over a cut of the real upstream map.
//!
//! `fixtures/mapdb_cut.json` is 24 rooms cut from `map-1789942730.json` by
//! `research/mapdb-inventory/cut_fixture.py`, each chosen for what it
//! exercises; their exits are trimmed to one another so nothing dangles.

use cena_map::{Action, Cond, Cost, Crossing, ExitKind, Room, RoomId, Uid, Walker};
use cena_mapdb_convert::run::{Conversion, convert};
use cena_mapdb_convert::{output, run};

const CUT: &str = include_str!("fixtures/mapdb_cut.json");

/// Helpers return `Result`/`Option` and the tests unwrap: clippy's test
/// exemption covers `#[test]` functions, not the helpers beside them.
fn converted() -> Result<Conversion, serde_json::Error> {
    convert(CUT)
}

fn room(conversion: &Conversion, id: u32) -> Option<&Room> {
    conversion.rooms.iter().find(|room| room.id == RoomId(id))
}

#[test]
fn the_cut_converts_cleanly() {
    let conversion = converted().unwrap();
    assert_eq!(conversion.rooms.len(), 24);
    assert_eq!(conversion.report.dangling, 0);
    assert!(
        conversion.report.problems.is_empty(),
        "{:#?}",
        conversion.report.problems
    );
    assert!(
        conversion
            .rooms
            .windows(2)
            .all(|pair| pair[0].id < pair[1].id),
        "rooms are ordered"
    );
}

/// The inn tables: upstream's most repeated hand-written script, 478 exits.
/// Two table names, one arm -- which is the whole premise of porting by shape.
#[test]
fn scripted_crossings_that_differ_only_in_a_parameter_share_an_arm() {
    let conversion = converted().unwrap();
    let atrium = room(&conversion, 0).unwrap();
    let tables: Vec<_> = atrium
        .exits
        .iter()
        .filter_map(|exit| match &exit.crossing {
            Crossing::Steps(steps) => Some((exit, steps)),
            _ => None,
        })
        .collect();
    assert_eq!(tables.len(), 2);
    for (exit, steps) in &tables {
        let [step] = &steps[..] else {
            panic!("one move: {steps:?}");
        };
        let Action::Move(command) = &step.action else {
            panic!("a move: {step:?}");
        };
        assert!(
            command.starts_with("go ") && command.ends_with(" table"),
            "{command}"
        );
        assert_eq!(exit.kind, ExitKind::Scripted);
        assert!(exit.is_routable(), "ported, at a plain cost: an exit again");
    }
}

/// The first ported script (`plan/21` §4.8): upstream's Ruby in, guarded steps
/// out, and the exit is an exit again.
#[test]
fn the_icy_path_is_ported_to_a_guarded_pause_and_a_move() {
    let conversion = converted().unwrap();
    let exit = room(&conversion, 2497)
        .unwrap()
        .exits
        .iter()
        .find(|exit| exit.to == RoomId(2496))
        .unwrap();
    let Crossing::Steps(steps) = &exit.crossing else {
        panic!("not ported: {:?}", exit.crossing);
    };
    assert!(
        exit.is_routable(),
        "a ported crossing with a plain cost routes"
    );
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0].action, Action::Cast("Sigil of Resolve".into()));
    assert_eq!(steps[1].action, Action::Pause(4200));
    assert_eq!(steps[2].action, Action::Move("west".into()));
    assert_eq!(steps[2].when, None);
    // This one, two inn tables, and the two `;e true` exits into the urchin hub.
    assert_eq!(conversion.report.ported_crossings, 5);

    // The guard says what the Ruby said.
    let slippery = steps[1].when.as_ref().unwrap();
    let walker = |ice: &str, load: u32, survival: u32, haste: bool| Walker {
        settings: [("ice_mode".to_owned(), ice.to_owned())].into(),
        encumbrance: Some(load),
        skills: Some([("survival".to_owned(), survival)].into()),
        active_spells: Some(haste.then(|| "Haste".to_owned()).into_iter().collect()),
        ..Walker::default()
    };
    assert!(
        slippery.holds(&walker("wait", 0, 300, true)),
        "wait: always"
    );
    assert!(
        !slippery.holds(&walker("run", 99, 0, false)),
        "run: never wait"
    );
    assert!(slippery.holds(&walker("auto", 51, 300, true)), "heavy");
    assert!(slippery.holds(&walker("auto", 10, 49, false)), "unskilled");
    assert!(
        !slippery.holds(&walker("auto", 10, 49, true)),
        "unskilled but hasted"
    );
    assert!(!slippery.holds(&walker("auto", 10, 50, false)), "skilled");

    // The author's rule: cast Resolve when it is known, affordable and not up.
    let can_cast = steps[0].when.as_ref().unwrap();
    let sigil = || Some(["Sigil of Resolve".to_owned()].into());
    let member = Walker {
        settings: [("ice_mode".to_owned(), "auto".to_owned())].into(),
        active_spells: Some([].into()),
        known_spells: sigil(),
        affordable_spells: sigil(),
        ..Walker::default()
    };
    assert!(can_cast.holds(&member));
    let broke = Walker {
        affordable_spells: Some([].into()),
        ..member.clone()
    };
    assert!(!can_cast.holds(&broke));
    let already_up = Walker {
        active_spells: sigil(),
        ..member.clone()
    };
    assert!(!can_cast.holds(&already_up));
    let running = Walker {
        settings: [("ice_mode".to_owned(), "run".to_owned())].into(),
        ..member.clone()
    };
    assert!(!can_cast.holds(&running), "run means run: no sigil either");
    assert!(!can_cast.holds(&Walker::default()), "nobody has looked: no");

    assert!(
        matches!(slippery, Cond::Any(_)),
        "and it is data, not code: {slippery:?}"
    );
}

/// Urchins (`plan/21` §2b): a `;e true` crossing into a virtual hub, gated by a
/// scripted cost; and plain commands out of it, gated by another.
#[test]
fn the_urchin_hub_is_data_on_both_sides() {
    let conversion = converted().unwrap();
    let into_hub = room(&conversion, 7)
        .unwrap()
        .exits
        .iter()
        .find(|exit| exit.to == RoomId(30714))
        .expect("BriarStone Court enters the hub");
    assert_eq!(
        into_hub.crossing,
        Crossing::PassThrough(cena_map::Pass),
        "nothing is sent: the hub exists only in the map"
    );
    // The way in is priced for a walker whose guides are paid up and who can
    // be seen and is on foot -- and for nobody Hydra has not looked at.
    let cost = into_hub.cost.as_ref().unwrap();
    let flags = |hidden: bool| Walker {
        settings: [("use_urchins".to_owned(), "true".to_owned())].into(),
        flags: [
            ("urchin_access".to_owned(), true),
            ("hidden".to_owned(), hidden),
            ("invisible".to_owned(), false),
            ("mounted".to_owned(), false),
        ]
        .into(),
        ..Walker::default()
    };
    assert_eq!(cost.price(&flags(false)), Some(0.1));
    assert_eq!(
        cost.price(&flags(true)),
        None,
        "the urchins will not guide someone hiding"
    );
    assert_eq!(cost.price(&Walker::default()), None);

    let hub = room(&conversion, 30714).unwrap();
    assert!(hub.uid.is_empty(), "a virtual room has no game room number");
    for exit in &hub.exits {
        let Crossing::Command(command) = &exit.crossing else {
            panic!("the hub's exits are plain commands");
        };
        assert!(command.starts_with("urchin guide "), "{command}");
        assert_eq!(exit.kind, ExitKind::Other);
        assert!(
            exit.is_routable(),
            "upstream's `only while go2 runs` is always true for a planned walk"
        );
    }
}

#[test]
fn identification_fields_survive() {
    let conversion = converted().unwrap();
    assert_eq!(
        room(&conversion, 4136).unwrap().uid.first(),
        Some(&Uid(-9054)),
        "negative uids are real"
    );
    assert!(
        room(&conversion, 18011).unwrap().uid.len() > 1,
        "an instanced room has several uids"
    );

    let unknowable = room(&conversion, 2640).unwrap();
    assert!(unknowable.location_unknowable && unknowable.location.is_none());
    let named = room(&conversion, 0).unwrap();
    assert_eq!(named.location.as_deref(), Some("the Moonglae Inn"));
    assert!(!named.location_unknowable);

    assert!(room(&conversion, 2927).unwrap().check_location);
    assert!(!room(&conversion, 682).unwrap().unique_loot.is_empty());
    assert!(room(&conversion, 87).unwrap().image.is_some());
}

#[test]
fn plain_exits_are_routable_and_typed() {
    let conversion = converted().unwrap();
    let exits = &room(&conversion, 87).unwrap().exits;
    assert!(exits.iter().all(cena_map::Exit::is_routable));
    assert!(exits.iter().any(|exit| exit.kind == ExitKind::Vertical));
    assert_eq!(conversion.report.exits, 32);
    assert_eq!(
        conversion.report.routable
            + conversion.report.unported_crossings
            + conversion
                .rooms
                .iter()
                .flat_map(|room| &room.exits)
                .filter(|exit| {
                    exit.crossing.is_crossable() && !matches!(exit.cost, Some(Cost::Fixed(_)))
                })
                .count(),
        conversion.report.exits,
        "every exit is routable, an unported crossing, or crossable without a constant cost",
    );
}

/// Forage sightings are most of upstream's tag volume and are not routing
/// data; the `meta:` prefix is structure, not content.
#[test]
fn tags_are_split() {
    let conversion = converted().unwrap();
    for room in &conversion.rooms {
        assert!(
            room.tags.iter().all(|tag| !tag.starts_with("meta:")),
            "{:?}",
            room.tags
        );
        assert!(
            room.meta
                .iter()
                .all(|meta| !meta.starts_with("forage-sensed")),
            "{:?}",
            room.meta
        );
    }
}

/// `output.rs`'s two promises: the same input writes the same bytes, and a
/// second run touches nothing.
#[test]
fn writing_is_deterministic_and_quiet() {
    let out = std::env::temp_dir().join(format!("cena-mapdb-convert-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);

    let first = output::write(&out, &converted().unwrap()).unwrap();
    assert_eq!((first.created, first.updated, first.unchanged), (24, 0, 0));
    let second = output::write(&out, &run::convert(CUT).unwrap()).unwrap();
    assert_eq!(
        (second.created, second.updated, second.unchanged),
        (0, 0, 24)
    );

    // What was written reads back as the same room.
    let conversion = converted().unwrap();
    for room in &conversion.rooms {
        let text = std::fs::read_to_string(output::room_path(&out, room.id)).unwrap();
        assert!(text.ends_with("}\n"));
        assert_eq!(&serde_json::from_str::<Room>(&text).unwrap(), room);
    }
    let index: Vec<RoomId> =
        serde_json::from_str(&std::fs::read_to_string(out.join("index.json")).unwrap()).unwrap();
    assert_eq!(index.len(), 24);
    assert!(out.join("report").join("unported_crossings.tsv").is_file());

    std::fs::remove_dir_all(&out).unwrap();
}

#[test]
fn a_file_that_is_not_a_map_is_an_error_not_an_empty_map() {
    assert!(convert("{}").is_err());
    assert!(convert("not json").is_err());
    assert_eq!(convert("[]").unwrap().rooms.len(), 0);
}
