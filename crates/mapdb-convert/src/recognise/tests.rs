//! The arms, one shape at a time.

use cena_map::{Action, Cond, Cost, Crossing, Landmark, Opening, RoomId, Routine};

use super::moves::{cast_clause, quoted_list};
use super::{RoomFacts, cost, crossing, holes, is_plain_argument};

#[test]
fn holes_are_exact_at_both_ends() {
    assert_eq!(holes("move 'west'", &["move '", "'"]), Some(vec!["west"]));
    // The last part anchors at the end, so a trailing statement lands IN
    // the hole -- where `is_plain_argument` refuses it.
    let smuggled = holes("move 'west'; fput 'x'", &["move '", "'"]).unwrap();
    assert_eq!(smuggled, ["west'; fput 'x"]);
    assert!(!is_plain_argument(smuggled[0]));
    assert_eq!(holes("xmove 'west'", &["move '", "'"]), None);
    assert_eq!(holes("a=1;b=2;", &["a=", ";b=", ";"]), Some(vec!["1", "2"]));
    assert_eq!(holes("anything", &[]), None);
}

#[test]
fn an_argument_cannot_smuggle_a_second_statement() {
    assert!(is_plain_argument("go bridge"));
    assert!(!is_plain_argument("west'; fput 'quit"));
    assert!(!is_plain_argument(""));
}

fn steps(script: &str, from: u32) -> Vec<Action> {
    match crossing(script, from, 0) {
        Some(Crossing::Steps(steps)) => steps.into_iter().map(|step| step.action).collect(),
        _ => Vec::new(),
    }
}

#[test]
fn a_scripted_plain_move_is_a_move_however_it_is_quoted_or_ended() {
    let go = vec![Action::Move("go thinness".into())];
    assert_eq!(steps(";e move 'go thinness'", 1), go);
    assert_eq!(steps(";e move \"go thinness\"; waitrt?", 1), go);
    assert_eq!(steps(";e move 'go thinness';waitrt", 1), go);
    // Anything after it that is not a known ending is not this shape.
    assert_eq!(steps(";e move 'go thinness'; fput 'quit'", 1), vec![]);
}

#[test]
fn a_put_then_a_move() {
    assert_eq!(
        steps(";e fput 'open gate'\nmove 'go gate'", 1),
        vec![
            Action::Put("open gate".into()),
            Action::Move("go gate".into())
        ]
    );
}

/// The transport asks once and goes on the second asking; what is
/// remembered is the room left, whichever way upstream wrote it.
#[test]
fn an_event_transport_remembers_the_room_it_left() {
    let expected = vec![
        Action::Put("event transport duskruin".into()),
        Action::Move("event transport duskruin".into()),
        Action::Remember("duskruin_origin".into(), "7".into()),
    ];
    let literal = ";e 2.times{fput \"event transport duskruin\"};\
                   UserVars.mapdb_duskruin_origin = 7;";
    let current = ";e 2.times{fput \"event transport duskruin\"};\
                   UserVars.mapdb_duskruin_origin = Map.current.id;";
    assert_eq!(steps(literal, 7), expected);
    assert_eq!(steps(current, 7), expected);
    assert_eq!(steps(literal, 8), vec![], "it names another room");
}

fn gate(script: &str) -> Option<(Cond, f64)> {
    match cost(script, &RoomFacts::default()) {
        Some(Cost::Gated {
            when,
            then,
            otherwise: None,
        }) => Some((when, then)),
        _ => None,
    }
}

#[test]
fn a_profession_gate_is_one_gate_however_upstream_wrote_it() {
    for script in [
        ";e Stats.prof == 'Bard' ? 0.2 : nil",
        ";e ((!defined?(Stats.prof) or Stats.prof == 'Bard') ? 0.2 : nil);",
        ";e if Stats.prof == \"Bard\"; 0.2; else; nil; end",
    ] {
        assert_eq!(
            gate(script),
            Some((Cond::Profession("Bard".into()), 0.2)),
            "{script}"
        );
    }
    assert_eq!(gate(";e Stats.prof == 'Bard' ? -1 : nil"), None);
}

#[test]
fn the_way_back_is_gated_on_the_memory_the_way_in_wrote() {
    assert_eq!(
        gate(
            ";e (!UserVars.mapdb_duskruin_origin.nil? and \
             UserVars.mapdb_duskruin_origin == 7) ? 0.2 : nil;"
        ),
        Some((Cond::Remembered("duskruin_origin".into(), "7".into()), 0.2))
    );
    assert_eq!(
        gate(
            ";e (!UserVars.mapdb_duskruin_origin.nil? and \
             UserVars.mapdb_talondown_origin == 7) ? 0.2 : nil;"
        ),
        None,
        "two different variables is not this shape"
    );
    assert_eq!(
        gate(";e (UserVars.mapdb_fwi_return_room == 3668 ? 5 : nil);"),
        Some((
            Cond::Remembered("fwi_return_room".into(), "3668".into()),
            5.0
        ))
    );
}

#[test]
fn settings_and_months() {
    assert_eq!(
        gate(";e UserVars.mapdb_use_portmasters == true ? 1200 : nil"),
        Some((
            Cond::Setting("use_portmasters".into(), "true".into()),
            1200.0
        ))
    );
    assert_eq!(
        gate(";e Time.now.month == 10 ? 0.2 : nil"),
        Some((Cond::Month(10), 0.2))
    );
    assert_eq!(gate(";e Time.now.month == 13 ? 0.2 : nil"), None);
}

#[test]
fn the_confluence_is_a_routine_whose_goal_is_the_exit() {
    let inside = ";e $mapdb_confluence_target = 23290; Room[23282].wayto['23282'].call";
    let out = ";e $mapdb_confluence_target = 'tranquility'; Room[23282].wayto['23282'].call";
    assert_eq!(
        crossing(inside, 23282, 23290),
        Some(Crossing::Routine(Routine::Confluence { leave: false }))
    );
    assert_eq!(
        crossing(out, 23282, 188),
        Some(Crossing::Routine(Routine::Confluence { leave: true }))
    );
    assert_eq!(
        crossing(inside, 23282, 23291),
        None,
        "a goal that is not this exit"
    );
}

#[test]
fn the_minotaur_maze_is_a_routine_only_with_upstreams_exact_search() {
    let search = include_str!("../upstream_scripts/minotaur_maze.rb");
    let script = format!(";e target_room_id = 6192; maze_rooms = [6191, 6254, 6192]; {search}");
    assert_eq!(
        crossing(&script, 6191, 6192),
        Some(Crossing::Routine(Routine::MinotaurMaze {
            rooms: vec![RoomId(6191), RoomId(6254), RoomId(6192)]
        }))
    );
    assert_eq!(
        crossing(&script, 6191, 6254),
        None,
        "the goal is not this exit"
    );
    let edited = script.replace("sleep 0.1", "sleep 0.2");
    assert_eq!(
        crossing(&edited, 6191, 6192),
        None,
        "upstream changed the search"
    );
}

#[test]
fn a_portmaster_asks_twice_and_waits_out_the_voyage() {
    let script = ";e multifput 'ask portmaster about travel 4','ask portmaster about travel 4';\
                  waitfor 'A crew member escorts you off the ship.'";
    assert_eq!(
        steps(script, 250),
        vec![
            Action::Put("ask portmaster about travel 4".into()),
            Action::Put("ask portmaster about travel 4".into()),
            Action::Await("A crew member escorts you off the ship.".into()),
        ]
    );
    let mismatched = script.replacen("travel 4", "travel 5", 1);
    assert_eq!(steps(&mismatched, 250), vec![], "two different asks");
}

#[test]
fn the_arctic_waters_walk_if_they_can_and_swim_if_they_cannot() {
    let script = format!(
        ";e {}{}fput (Spell[112].active? ? 'go north' : 'swim north')",
        cast_clause("resolve", 9704),
        cast_clause("waterwalking", 112)
    );
    let Some(Crossing::Steps(steps)) = crossing(&script, 1, 2) else {
        panic!("not recognised");
    };
    let actions: Vec<_> = steps.iter().map(|step| step.action.clone()).collect();
    assert_eq!(
        actions,
        vec![
            Action::Cast("Sigil of Resolve".into()),
            Action::Cast("Water Walking".into()),
            Action::Move("go north".into()),
            Action::Move("swim north".into()),
        ]
    );
    // Exactly one of the two moves happens, whatever the walker can cast.
    let up = cena_map::Walker {
        active_spells: Some(["Water Walking".to_owned()].into()),
        ..cena_map::Walker::default()
    };
    let down = cena_map::Walker {
        active_spells: Some([].into()),
        ..cena_map::Walker::default()
    };
    let moves = |walker| {
        steps[2..]
            .iter()
            .filter(|step| step.when.as_ref().is_some_and(|when| when.holds(walker)))
            .count()
    };
    assert_eq!((moves(&up), moves(&down)), (1, 1));
}

#[test]
fn hands_are_emptied_for_the_move_and_refilled_only_where_upstream_does() {
    assert_eq!(
        steps(
            ";e empty_hands\nmove \"climb well\"\nwaitrt?\nfill_hands",
            1
        ),
        vec![
            Action::EmptyHands,
            Action::Move("climb well".into()),
            Action::FillHands
        ]
    );
    assert_eq!(
        steps(";e empty_hands; move 'climb slope'", 1),
        vec![Action::EmptyHands, Action::Move("climb slope".into())]
    );
}

#[test]
fn several_commands_and_the_last_one_moves() {
    assert_eq!(
        steps(";e multifput 'unlatch door', 'open door', 'go door'", 1),
        vec![
            Action::Put("unlatch door".into()),
            Action::Put("open door".into()),
            Action::Move("go door".into()),
        ]
    );
    assert_eq!(
        steps(";e multifput \"search\",\"go opening\"; waitrt?;", 1),
        vec![
            Action::Put("search".into()),
            Action::Move("go opening".into())
        ]
    );
    assert_eq!(
        steps(";e multifput 'go door'", 1),
        vec![],
        "one command is not this shape"
    );
    assert_eq!(quoted_list("'a', 'b' extra"), None);
    assert_eq!(quoted_list("'a'; system('x')"), None);
}

#[test]
fn the_way_out_of_an_event_forgets_the_way_in() {
    assert_eq!(
        steps(
            ";e move('go wagon');UserVars.mapdb_duskruin_origin = nil;",
            1
        ),
        vec![
            Action::Move("go wagon".into()),
            Action::Forget("duskruin_origin".into())
        ]
    );
}

#[test]
fn an_inn_table_is_one_move_with_the_apostrophe_intact() {
    let script = ";e table = \"Cat's Paw\"; fput \"go #{table} table\" if \
        dothistimeout(\"go #{table} table\", 25, /You (?:and your group )?head over to|\
        waves.*you.*(?:invites|inviting) you(?: and your group)? to (?:join|come sit at)/) \
        =~ /inviting you|invites you/";
    assert_eq!(
        steps(script, 0),
        vec![Action::Move("go Cat's Paw table".into())]
    );
}

const ROUND: &str = "; move dirs[index]; index += 1; index = 0 if index >= dirs.length; end; ";
const LOST: &str =
    "else; echo 'error: mini-script expected a different room'; end; $go2_restart = true";

fn rift(seen: &str, through: &str) -> String {
    format!(
        ";e start_room = [ 12095, nil, 12097 ]; dirs = [ 'southwest', 'west', 'east', 'north' ]; \
         if index = start_room.index(Room.current.id); until {seen}{ROUND}{through}{LOST}"
    )
}

/// A patrol's arguments: starts, dirs, landmarks, after.
type Patrol = (Vec<Option<RoomId>>, Vec<String>, Vec<Landmark>, Vec<String>);

fn patrol_of(script: &str) -> Option<Patrol> {
    match crossing(script, 1, 2)? {
        Crossing::Routine(Routine::Patrol {
            starts,
            dirs,
            landmarks,
            after,
        }) => Some((starts, dirs, landmarks, after)),
        _ => None,
    }
}

/// The two tables need not be the same length, and a `nil` keeps its place.
#[test]
fn a_patrol_keeps_upstreams_tables_as_they_are() {
    let script = rift(
        "checkloot.include?('thread')",
        "move 'climb thread'; waitrt?; fput 'stand'; ",
    );
    let (starts, dirs, landmarks, after) = patrol_of(&script).unwrap();
    assert_eq!(starts, [Some(RoomId(12095)), None, Some(RoomId(12097))]);
    assert_eq!(dirs, ["southwest", "west", "east", "north"]);
    assert_eq!(
        landmarks,
        [Landmark {
            noun: "thread".into(),
            enter: "climb thread".into(),
            open: None
        }]
    );
    assert_eq!(after, ["stand"]);
}

#[test]
fn a_patrol_may_look_for_either_of_two_ways_out() {
    for end in ["end;; ", "end; "] {
        let script = rift(
            "checkloot.include?('door') or checkloot.include?('mirror')",
            &format!(
                "if checkloot.include?('door'); move 'go door'; \
                 elsif checkloot.include?('mirror'); move 'go mirror'; {end}"
            ),
        );
        let (_, _, landmarks, after) = patrol_of(&script).unwrap();
        let ways: Vec<_> = landmarks
            .iter()
            .map(|way| (way.noun.as_str(), way.enter.as_str()))
            .collect();
        assert_eq!(ways, [("door", "go door"), ("mirror", "go mirror")]);
        assert!(after.is_empty());
    }
    // The nouns looked for and the nouns entered must be the same two.
    let crossed = rift(
        "checkloot.include?('door') or checkloot.include?('mirror')",
        "if checkloot.include?('door'); move 'go door'; \
         elsif checkloot.include?('maw'); move 'go maw'; end; ",
    );
    assert_eq!(patrol_of(&crossed), None);
}

#[test]
fn a_fissure_is_worked_open_first() {
    let script = rift(
        "checkloot.include?('fissure')",
        "5.times { waitrt?; fput 'stand' unless standing?; waitrt?; result = dothistimeout \
         'push fissure', 3, /^Grasping the distorted edges|^A wide fissure cannot be opened any \
         farther\\.|^As you move to touch a sealed fissure|^What were you referring to\\?/; \
         waitrt?; fput 'stand' unless standing?; waitrt?; break if result =~ /^A wide fissure \
         cannot be opened any farther\\./ }; move 'go fissure'; ",
    );
    let (_, _, landmarks, _) = patrol_of(&script).unwrap();
    assert_eq!(
        landmarks[0].open,
        Some(Opening {
            command: "push fissure".into(),
            until: "A wide fissure cannot be opened any farther.".into(),
            tries: 5
        })
    );
    assert_eq!(landmarks[0].enter, "go fissure");
}

#[test]
fn a_pedal_boat_keeps_pedalling_until_it_is_somewhere_else() {
    for spacing in ["; ", ";"] {
        let script = format!(
            ";e direction=\"west\";start=Room.current.id{spacing}dothistimeout \
             \"pedal #{{direction}}\", 2, /pedal/ while Room.current.id == start"
        );
        assert_eq!(
            steps(&script, 1),
            vec![Action::KeepMoving("pedal west".into())]
        );
    }
    assert_eq!(
        steps(";e direction=\"west\";start=Room.current.id", 1),
        vec![]
    );
}

#[test]
fn signposts_keep_the_table_and_take_the_exit_as_the_goal() {
    let script = ";e empty_hand if [ 12662, 20786 ].include?(Room.current.id); swim_dir = \
        { 20786 => 'down', 12662 => 'whirlpool' }; while Room.current.id != 12677; if \
        swim_dir[Room.current.id]; put \"swim #{swim_dir[Room.current.id]}\"; else; echo \
        \"Oh crap.. I'm lost..\"; put \"swim #{checkpaths[rand(checkpaths.length)]}\"; end; \
        sleep 1; waitrt?; end; fill_hand";
    assert_eq!(
        crossing(script, 12662, 12677),
        Some(Crossing::Routine(Routine::Signposts {
            verb: "swim".into(),
            dirs: vec![
                (RoomId(20786), "down".into()),
                (RoomId(12662), "whirlpool".into())
            ],
            hands_free_in: vec![RoomId(12662), RoomId(20786)],
        }))
    );
    assert_eq!(
        crossing(script, 12662, 12678),
        None,
        "the goal is not this exit"
    );
}

#[test]
fn a_command_sent_until_the_walker_is_there_names_the_exits_own_room() {
    let there = |script: &str, to| match crossing(script, 1, to) {
        Some(Crossing::Steps(steps)) => steps.into_iter().map(|step| step.action).collect(),
        _ => Vec::new(),
    };
    let forest = ";e 50.times { move 'go forest'; break if Room.current.id == 13183 }";
    assert_eq!(
        there(forest, 13183),
        vec![Action::MoveUntilThere("go forest".into())]
    );
    assert_eq!(there(forest, 13184), vec![], "it waits for another room");
    assert_eq!(
        there(
            ";e begin\nfput 'swim north'\nwaitrt?\nend until Room.current.id == 10815",
            10815
        ),
        vec![Action::MoveUntilThere("swim north".into())]
    );
}

#[test]
fn seeking_takes_the_exit_as_its_destination_and_knows_the_red_forests_sides() {
    let script =
        |to: u32| format!(";e $mapdb_seeking_destination = {to};Map[3600].wayto['3600'].call;");
    assert_eq!(
        crossing(&script(12603), 1983, 12603),
        Some(Crossing::Routine(Routine::Seeking { remember: None }))
    );
    assert_eq!(crossing(&script(12603), 1983, 12604), None);
    let side = |from| match crossing(&script(24715), from, 24715) {
        Some(Crossing::Routine(Routine::Seeking { remember })) => remember,
        _ => None,
    };
    assert_eq!(side(3600), Some(("redforest_location".into(), "WL".into())));
    assert_eq!(
        side(10125),
        Some(("redforest_location".into(), "EN".into()))
    );
    assert_eq!(side(1), None);
}

#[test]
fn the_trinket_is_the_script_or_a_call_to_it() {
    let trinket = Some(Crossing::Routine(Routine::Trinket));
    assert_eq!(
        crossing(";e Map[7].wayto['3668'].call;", 284, 3668),
        trinket
    );
    assert_eq!(
        crossing(";e Map[7].wayto['3668'].call", 3669, 35593),
        trinket
    );
    assert_eq!(
        crossing(include_str!("../upstream_scripts/fwi_trinket.rb"), 7, 3668),
        trinket
    );
    assert_eq!(crossing(";e Map[8].wayto['3668'].call;", 284, 3668), None);
}

/// Upstream's script refuses a walker with no password halfway through; here
/// that is part of the price, so the walker is never sent to the door.
#[test]
fn a_guild_door_is_priced_only_when_the_profile_has_the_password() {
    let door = crossing(
        ";e Map[12421].wayto['14089'].call; # rogue guild proc",
        15694,
        17964,
    )
    .unwrap();
    assert_eq!(door, Crossing::Routine(Routine::GuildPassword));
    let rogues_only = cost(
        ";e if Stats.prof == 'Rogue'; 1.6; else; nil; end",
        &RoomFacts::default(),
    );
    let priced = super::priced_for_crossing(&door, rogues_only.clone()).unwrap();
    let rogue = |password: &str| cena_map::Walker {
        profession: Some("Rogue".into()),
        settings: [("rogue_password".to_owned(), password.to_owned())].into(),
        ..cena_map::Walker::default()
    };
    assert_eq!(priced.price(&rogue("kick, slap")), Some(1.6));
    assert_eq!(priced.price(&rogue("")), None, "a rogue with no password");
    // Any other crossing leaves its cost alone.
    let plain = Crossing::Command("north".into());
    assert_eq!(
        super::priced_for_crossing(&plain, rogues_only.clone()),
        rogues_only
    );
}
