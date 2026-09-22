//! Arms for slice e of the long tail, round two
//! (`research/mapdb-inventory/tail/slice_e.tsv`).
//!
//! Same rules as every arm (`super`): the upstream script verbatim, with
//! holes; an exact match or none. This file's tests live at its foot.

use cena_map::{Action, Cond, Crossing, Step};

use super::moves::cast_if_able;
use super::{always, holes, is_plain_argument, is_word, quoted};

/// The steps for a crossing script in this slice, if an arm here knows it.
pub(super) fn crossing(script: &str, from: u32, to: u32) -> Option<Crossing> {
    silverwood(script)
        .or_else(|| fog_in(script))
        .or_else(|| until_the_room_changes(script, from))
        .or_else(|| nudge_then_keep_moving(script, from, to))
        .or_else(|| kneel_first(script))
        .or_else(|| torch(script))
        .or_else(|| search_between_tries(script))
        .or_else(|| jump_down(script, to))
        .or_else(|| small_ones(script))
        .or_else(|| water_tunnel(script))
        .or_else(|| sphere(script))
        .or_else(|| depressions(script))
}

fn plain(command: &str) -> Option<String> {
    is_plain_argument(command).then(|| command.to_owned())
}

fn put(command: &str) -> Step {
    always(Action::Put(command.to_owned()))
}

fn wait(line: &str) -> Step {
    always(Action::Await(line.to_owned()))
}

/// What is left to do when nothing so far has moved the walker.
fn if_still_here(action: Action) -> Step {
    Step {
        action,
        when: Some(Cond::StillHere),
    }
}

/// Into Silverwood Manor by a town's door, remembering which town: the manor's
/// four doors out are priced by it. 4 exits. Written once through the door
/// (`Action::Remember`), where upstream writes it first.
fn silverwood(script: &str) -> Option<Crossing> {
    let [town, command] = holes(script, &[";e $SILVERWOOD_TOWN=:", ";move '", "'"])?[..] else {
        return None;
    };
    is_word(town).then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::Move(plain(command)?)),
        always(Action::Remember(
            "silverwood_town".to_owned(),
            town.to_owned(),
        )),
    ]))
}

/// Into the Red Forest through its fog, remembering which side: 2 exits. The
/// fog itself is `super::tail_c`'s, with another line for having arrived.
fn fog_in(script: &str) -> Option<Crossing> {
    const TURNED: &str = "You attempt to navigate your way through the fog, but get turned \
        around and come right back out where you started!";
    const ARRIVED: &str = "Obvious paths: northeast, southeast";
    let rest = script.strip_prefix(";e UserVars.mapdb_redforest_location = '")?;
    let (side, rest) = rest.split_once("';")?;
    let fog = format!(
        "result = nil;until result =~ /{ARRIVED}/;fput \"stand\" until standing?;\
         result = dothistimeout \"go fog\", 5, /{TURNED}|{ARRIVED}/;\
         if result =~ /{TURNED}/;sleep 0.5;waitrt?;end;end"
    );
    (is_word(side) && rest == fog).then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::KeepMoving("go fog".to_owned())),
        always(Action::Remember(
            "redforest_location".to_owned(),
            side.to_owned(),
        )),
    ]))
}

/// One command until the walker is somewhere else (`Action::KeepMoving`), in
/// the spellings `super::tail_b` and `super::tail_c` do not have. 5 exits.
fn until_the_room_changes(script: &str, from: u32) -> Option<Crossing> {
    const UP: &str = "You carefully make your way up the dilapidated ladder.";
    let keep = |command: &str| Some(always(Action::KeepMoving(plain(command)?)));
    if let Some(found) = quoted(script, &[";e fput '", "' until Room.current.id != ", ""]) {
        let [command, room] = found[..] else {
            return None;
        };
        (room.parse() == Ok(from)).then_some(())?;
        return Some(Crossing::Steps(vec![keep(command)?]));
    }
    // An underwater opening, hands free. 2 exits.
    if let Some(found) = holes(
        script,
        &[
            ";e empty_hands; while Room.current == Room[",
            "]; put '",
            "'; sleep 1; waitrt?; end; fill_hands",
        ],
    ) {
        let [room, command] = found[..] else {
            return None;
        };
        (room.parse() == Ok(from)).then_some(())?;
        return Some(Crossing::Steps(vec![
            always(Action::EmptyHands),
            keep(command)?,
            always(Action::FillHands),
        ]));
    }
    // A ladder whose rungs break: climb, stand up, climb, until the line that
    // says the walker is up. 1 exit. Standing is the walker's own business.
    let ladder = format!(
        ";e begin\nclear\nfput 'climb ladder'\nclimb_result = waitfor \"As you try to climb \
         the ladder, a rung breaks under your weight\", \"{UP}\"\nfput 'stand' unless \
         climb_result == \"{UP}\"\nend until climb_result == \"{UP}\""
    );
    if script == ladder {
        return Some(Crossing::Steps(vec![keep("climb ladder")?]));
    }
    // A fissure climbed until the answer is no longer roundtime. 1 exit.
    let [command] = holes(
        script,
        &[
            ";e empty_hands\nbegin\nresult = dothistimeout '",
            "', 2, /Round time|Roundtime|find yourself|Crawlway/\nwaitrt?\nend until \
             result.to_s !~ /round/i\nfill_hands",
        ],
    )?[..] else {
        return None;
    };
    Some(Crossing::Steps(vec![
        always(Action::EmptyHands),
        keep(command)?,
        always(Action::FillHands),
    ]))
}

/// `fput 'north'; move 'north' while Room.current.id == 2925`: a first send
/// that may be all it takes, then a command until the room changes. 3 exits.
/// The one that may land elsewhere replans, as upstream does.
fn nudge_then_keep_moving(script: &str, from: u32, to: u32) -> Option<Crossing> {
    let (first, again, replan) = if let Some(found) = quoted(
        script,
        &[";e fput '", "'; move '", "' while Room.current.id == ", ""],
    ) {
        let [first, again, room] = found[..] else {
            return None;
        };
        (room.parse() == Ok(from)).then_some(())?;
        (first, again, false)
    } else {
        let found = quoted(
            script,
            &[
                ";e fput '",
                "'; while Room.current.id == ",
                "; fput '",
                "'; waitrt; end; if Room.current.id != ",
                "; $go2_restart = true; end;",
            ],
        )?;
        let [first, left, again, reached] = found[..] else {
            return None;
        };
        (left.parse() == Ok(from) && reached.parse() == Ok(to)).then_some(())?;
        (first, again, true)
    };
    let mut steps = vec![
        always(Action::TryMove(plain(first)?)),
        if_still_here(Action::KeepMoving(plain(again)?)),
    ];
    if replan {
        steps.push(always(Action::Replan));
    }
    Some(Crossing::Steps(steps))
}

/// `fput 'kneel' until kneeling?`, then through. 2 exits; one crawls by
/// `fput` and stands after.
fn kneel_first(script: &str) -> Option<Crossing> {
    if let Some(found) = holes(script, &[";e fput 'kneel' until kneeling?; move '", "'"]) {
        let [command] = found[..] else {
            return None;
        };
        return Some(Crossing::Steps(vec![
            put("kneel"),
            always(Action::Move(plain(command)?)),
        ]));
    }
    let [command] = holes(
        script,
        &[
            ";e fput 'kneel' until kneeling?;fput '",
            "'\nwaitrt?\nfput 'stand' until standing?",
        ],
    )?[..] else {
        return None;
    };
    Some(Crossing::Steps(vec![
        put("kneel"),
        always(Action::Move(plain(command)?)),
        put("stand"),
    ]))
}

/// A stairway a torch reveals: upstream turns the torch unless the stairway
/// is showing. Here the stairway is tried, and the torch turned only if that
/// went nowhere -- the same sends in both cases. 1 exit.
fn torch(script: &str) -> Option<Crossing> {
    let found = quoted(
        script,
        &[
            ";e fput '",
            "' unless checkloot.include?('",
            "'); move '",
            "'",
        ],
    )?;
    let [reveal, noun, command] = found[..] else {
        return None;
    };
    (is_word(noun) && command.strip_prefix("go ") == Some(noun)).then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::TryMove(plain(command)?)),
        if_still_here(Action::Put(plain(reveal)?)),
        if_still_here(Action::Move(plain(command)?)),
    ]))
}

/// Try the way; if it is not there, search and try again, a fixed number of
/// times. Unrolled, because steps do not hold steps. 2 exits.
fn search_between_tries(script: &str) -> Option<Crossing> {
    const CELERITY: &str = "Celerity"; // 506
    if let Some(found) = holes(
        script,
        &[
            ";e attempts = 0;moved = false;loop do;waitrt?;moved = move('",
            "');break if moved;fput 'search';attempts += 1;break if attempts >= 5;end;moved",
        ],
    ) {
        let [command] = found[..] else {
            return None;
        };
        let command = plain(command)?;
        let mut steps = vec![always(Action::TryMove(command.clone()))];
        for _ in 0..3 {
            steps.push(if_still_here(Action::Put("search".to_owned())));
            steps.push(if_still_here(Action::TryMove(command.clone())));
        }
        steps.push(if_still_here(Action::Put("search".to_owned())));
        // The fifth try is upstream's last: a `Move`, so failing is reported.
        steps.push(if_still_here(Action::Move(command)));
        return Some(Crossing::Steps(steps));
    }
    // An icy ledge: Celerity if able, search, try; eight times.
    let before = format!(
        ";e 8.times {{ {}dothistimeout 'search', 3, /You make a careful search of the area and \
         discover a narrow icy ledge!|You don't find anything of interest here./; break if \
         move '",
        super::moves::cast_clause("celerity", 506)
    );
    let [command] = holes(script, &[&before, "' }"])?[..] else {
        return None;
    };
    let command = plain(command)?;
    let cast = cast_if_able(CELERITY);
    let mut steps = vec![
        cast.clone(),
        put("search"),
        always(Action::TryMove(command.clone())),
    ];
    for round in 1..8 {
        steps.push(Step {
            action: cast.action.clone(),
            when: Some(Cond::All(vec![Cond::StillHere, cast.when.clone()?])),
        });
        steps.push(if_still_here(Action::Put("search".to_owned())));
        steps.push(if_still_here(if round == 7 {
            Action::Move(command.clone())
        } else {
            Action::TryMove(command.clone())
        }));
    }
    Some(Crossing::Steps(steps))
}

/// Jump, land, get up, and head west until at the destination. 1 exit.
fn jump_down(script: &str, to: u32) -> Option<Crossing> {
    const LANDED: &str = "You feel the presence of cold hard stone underneath you.";
    let before = format!(
        ";e fput 'jump'\nwaitfor '{LANDED}'\nsleep 0.1\nfput 'stand' unless standing?\nmove '"
    );
    let [command, room] = holes(script, &[&before, "' until Room.current.id == ", ""])?[..] else {
        return None;
    };
    (room.parse() == Ok(to)).then_some(())?;
    Some(Crossing::Steps(vec![
        put("jump"),
        wait(LANDED),
        always(Action::Pause(100)),
        put("stand"),
        always(Action::MoveUntilThere(plain(command)?)),
    ]))
}

/// Three one-off straight lines. 3 exits.
fn small_ones(script: &str) -> Option<Crossing> {
    // A lock turned with a free hand. `empty_hand` is `EmptyHands`, as in
    // `super::tail_a`; the `echo` warns a Lich user that the door locks.
    if script
        == ";e empty_hand;fput 'turn lock';fill_hand;fput 'open door';fput 'go door';echo '** \
            WARNING: The door closes behind you!  Get out soon before it locks! **'"
    {
        return Some(Crossing::Steps(vec![
            always(Action::EmptyHands),
            put("turn lock"),
            always(Action::FillHands),
            put("open door"),
            always(Action::Move("go door".to_owned())),
        ]));
    }
    // A river that will not take the unseen, swum hands free.
    if let Some(found) = holes(
        script,
        &[
            ";e empty_hands\nfput 'unhide' if checkspell 'invisibility'\nmove '",
            "'",
        ],
    ) {
        let [command] = found[..] else {
            return None;
        };
        return Some(Crossing::Steps(vec![
            always(Action::EmptyHands),
            Step {
                action: Action::Put("unhide".to_owned()),
                when: Some(Cond::SpellActive("Invisibility".to_owned())), // 916
            },
            always(Action::Move(plain(command)?)),
        ]));
    }
    // A slippery column: stand, climb, three tries. That is what a `Move` is.
    let slippery = ";e 3.times do;  waitrt?;  fput 'stand' unless standing?;  waitrt?;  \
        climb_result = dothistimeout \"climb stone\", 3, /^(?:You start to climb the column of \
        stone, but it proves so slippery that you slide back down and end up on the ground in \
        a heap\\.|You dance lightly up the column of stone and into the area beyond\\.)$/;  \
        break if climb_result =~ /^You dance/;end";
    (script == slippery)
        .then(|| Crossing::Steps(vec![always(Action::Move("climb stone".to_owned()))]))
}

/// The water tunnel: lie down and lean left at the right moments. 2 exits;
/// one starts outside, stows what is held and goes in first. The `_respond`s
/// and the stopwatch talk to a Lich user.
fn water_tunnel(script: &str) -> Option<Crossing> {
    // (the line waited for, whether to lean left once it comes)
    const BENDS: [(&str, bool); 4] = [
        ("one on the left and one on the right", true),
        ("branches off to the left just ahead", true),
        ("Another tunnel branches off to the left just ahead", false),
        (
            "Suddenly the tunnel turns into a nearly vertical drop",
            true,
        ),
    ];
    const OUT: &str = "Obvious paths: southwest";
    let said =
        |text: &str| format!("_respond \"#{{monsterbold_start}}{text}#{{monsterbold_end}}\";\n");
    let mut ride = "fput 'lay';\n".to_owned();
    let mut steps = vec![put("lay")];
    for (line, lean) in BENDS {
        ride += &said(&format!("Waiting for '{line}'."));
        ride = format!("{ride}waitfor \"{line}\";\n");
        steps.push(wait(line));
        if lean {
            ride += "fput \"lean left\";\n";
            steps.push(put("lean left"));
        }
    }
    ride += &said("Waiting to exit the tunnels.");
    ride = format!("{ride}waitfor \"{OUT}\";\n");
    ride += &said("water tunnel time: #{Time.now.to_i - start_time} seconds.");
    steps.push(wait(OUT));
    steps.push(always(Action::FillHands));
    let inside = format!(";e ;\nstart_time = Time.now.to_i;\n{ride}fill_hands;\n");
    let outside = format!(
        ";e ;\nstart_time = Time.now.to_i;\nrefill_hands=false;(refill_hands = \
         true;empty_hands;) if GameObj.right_hand.id or GameObj.left_hand.id;\nfput 'go \
         opening';\n{ride}fill_hands if refill_hands;\n"
    );
    if script == outside {
        steps.splice(
            0..0,
            [
                always(Action::EmptyHands),
                always(Action::Move("go opening".to_owned())),
            ],
        );
    } else if script != inside {
        return None;
    }
    Some(Crossing::Steps(steps))
}

/// The sphere: step in, then walk one way through the fog until it lets go,
/// and get up. 3 exits. Inside there is no room to speak of, so the direction
/// is sent until the room changes.
fn sphere(script: &str) -> Option<Crossing> {
    const TORN: &str = "You feel every shred of yourself torn to tiny pieces and \
        reformed...|You can do nothing... you can feel nothing... you are nothing...";
    let after = format!(
        "', 2, /You are surrounded by an ethereal fog.|{TORN}/)\n  break if result =~ \
         /{TORN}/\n  break unless XMLData.room_description.empty?\nend\nsleep(1) while \
         XMLData.room_description.empty?\nfput 'stand' unless standing? \n"
    );
    let [way] = holes(
        script,
        &[
            ";e fput 'go sphere'\nloop do\n  result = dothistimeout('",
            &after,
        ],
    )?[..] else {
        return None;
    };
    is_word(way).then_some(())?;
    Some(Crossing::Steps(vec![
        put("go sphere"),
        always(Action::KeepMoving(way.to_owned())),
        put("stand"),
    ]))
}

/// Four depressions pushed in turn, then stand. 1 exit. Which push drops the
/// walker is not said, so each is a `TryMove`: sent whether or not the last
/// one moved it, as upstream sends them.
fn depressions(script: &str) -> Option<Crossing> {
    const SHAPES: [&str; 4] = ["circular", "triangular", "square", "rectangular"];
    let upstream = ";e %w(circular triangular square rectangular).each{|d| dothistimeout \
        \"push #{d} depression\", 3, /\\*[A-Z]+\\*$/};fput \"stand\" until standing?";
    (script == upstream).then_some(())?;
    let mut steps: Vec<Step> = SHAPES
        .iter()
        .map(|shape| always(Action::TryMove(format!("push {shape} depression"))))
        .collect();
    steps.push(put("stand"));
    Some(Crossing::Steps(steps))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps(script: &str, from: u32, to: u32) -> Option<Vec<Step>> {
        match crossing(script, from, to)? {
            Crossing::Steps(steps) => {
                assert!(cena_map::moves_whatever_is_known(&steps), "{script}");
                Some(steps)
            }
            _ => None,
        }
    }

    fn actions(script: &str, from: u32, to: u32) -> Vec<Action> {
        steps(script, from, to)
            .unwrap_or_default()
            .into_iter()
            .map(|step| step.action)
            .collect()
    }

    fn mv(command: &str) -> Action {
        Action::Move(command.into())
    }

    fn pt(command: &str) -> Action {
        Action::Put(command.into())
    }

    fn keep(command: &str) -> Action {
        Action::KeepMoving(command.into())
    }

    #[test]
    fn a_manor_door_remembers_the_town() {
        assert_eq!(
            actions(";e $SILVERWOOD_TOWN=:imt;move 'go door'", 8696, 13532),
            [
                mv("go door"),
                Action::Remember("silverwood_town".into(), "imt".into())
            ]
        );
        assert_eq!(actions(";e $SILVERWOOD_TOWN=nil;move 'go door'", 1, 2), []);
        assert_eq!(actions(";e $SILVERWOOD_TOWN=:a;b;move 'go door'", 1, 2), []);
    }

    #[test]
    fn the_fog_in_remembers_the_side() {
        let script = ";e UserVars.mapdb_redforest_location = 'WL';result = nil;until result =~ \
            /Obvious paths: northeast, southeast/;fput \"stand\" until standing?;result = \
            dothistimeout \"go fog\", 5, /You attempt to navigate your way through the fog, but \
            get turned around and come right back out where you started!|Obvious paths: \
            northeast, southeast/;if result =~ /You attempt to navigate your way through the \
            fog, but get turned around and come right back out where you started!/;sleep \
            0.5;waitrt?;end;end";
        assert_eq!(
            actions(script, 7892, 24675),
            [
                keep("go fog"),
                Action::Remember("redforest_location".into(), "WL".into())
            ]
        );
        assert_eq!(actions(&script.replace("southeast", "south"), 1, 2), []);
    }

    #[test]
    fn one_command_until_somewhere_else() {
        let root = ";e fput \"climb root\" until Room.current.id != 24241";
        assert_eq!(actions(root, 24241, 20233), [keep("climb root")]);
        assert_eq!(actions(root, 1, 20233), [], "the room left, or nothing");
        let swim = ";e empty_hands; while Room.current == Room[6481]; put 'swim opening'; \
                    sleep 1; waitrt?; end; fill_hands";
        assert_eq!(
            actions(swim, 6481, 6484),
            [Action::EmptyHands, keep("swim opening"), Action::FillHands]
        );
        assert_eq!(actions(swim, 6484, 6481), []);
        let ladder = ";e begin\nclear\nfput 'climb ladder'\nclimb_result = waitfor \"As you try \
            to climb the ladder, a rung breaks under your weight\", \"You carefully make your \
            way up the dilapidated ladder.\"\nfput 'stand' unless climb_result == \"You \
            carefully make your way up the dilapidated ladder.\"\nend until climb_result == \
            \"You carefully make your way up the dilapidated ladder.\"";
        assert_eq!(actions(ladder, 7517, 8686), [keep("climb ladder")]);
        let fissure = ";e empty_hands\nbegin\nresult = dothistimeout 'climb fissure', 2, /Round \
            time|Roundtime|find yourself|Crawlway/\nwaitrt?\nend until result.to_s !~ \
            /round/i\nfill_hands";
        assert_eq!(
            actions(fissure, 2208, 2209),
            [Action::EmptyHands, keep("climb fissure"), Action::FillHands]
        );
        assert_eq!(actions(&fissure.replace(", 2,", ", 3,"), 2208, 2209), []);
    }

    #[test]
    fn a_first_send_and_then_until_it_works() {
        let found = steps(
            ";e fput 'north'; move 'north' while Room.current.id == 2925",
            2925,
            2924,
        )
        .unwrap();
        assert_eq!(found[0], always(Action::TryMove("north".into())));
        assert_eq!(found[1], if_still_here(keep("north")));
        let lands = ";e fput 'west'; while Room.current.id == 30115; fput 'north'; waitrt; end; \
                     if Room.current.id != 29867; $go2_restart = true; end;";
        assert_eq!(
            actions(lands, 30115, 29867),
            [
                Action::TryMove("west".into()),
                keep("north"),
                Action::Replan
            ]
        );
        assert_eq!(actions(lands, 30115, 1), [], "the destination, or nothing");
    }

    #[test]
    fn kneeling_first() {
        assert_eq!(
            actions(";e fput 'kneel' until kneeling?; move 'go opening'", 1, 2),
            [pt("kneel"), mv("go opening")]
        );
        assert_eq!(
            actions(
                ";e fput 'kneel' until kneeling?;fput 'crawl crack'\nwaitrt?\nfput 'stand' \
                 until standing?",
                1,
                2
            ),
            [pt("kneel"), mv("crawl crack"), pt("stand")]
        );
        assert_eq!(
            actions(";e fput 'kneel' until kneeling?; move 'go'; exit'", 1, 2),
            []
        );
    }

    #[test]
    fn a_torch_is_turned_only_if_the_stairway_is_not_there() {
        let script = ";e fput 'turn torch' unless checkloot.include?('stairway'); move 'go \
                      stairway'";
        let found = steps(script, 4154, 4168).unwrap();
        assert_eq!(
            found,
            [
                always(Action::TryMove("go stairway".into())),
                if_still_here(pt("turn torch")),
                if_still_here(mv("go stairway")),
            ]
        );
        assert_eq!(actions(&script.replace("go stairway", "go door"), 1, 2), []);
    }

    #[test]
    fn searching_between_tries_is_unrolled() {
        let path = ";e attempts = 0;moved = false;loop do;waitrt?;moved = move('go \
                    path');break if moved;fput 'search';attempts += 1;break if attempts >= \
                    5;end;moved";
        let found = actions(path, 1242, 1241);
        assert_eq!(found.len(), 9);
        assert_eq!(found[0], Action::TryMove("go path".into()));
        assert_eq!(found[8], mv("go path"));
        assert_eq!(found.iter().filter(|a| **a == pt("search")).count(), 4);
        assert_eq!(actions(&path.replace(">= 5", ">= 6"), 1, 2), []);

        let ledge = ";e 8.times { if celerity = Spell[506] and celerity.known? and \
            celerity.affordable? and not celerity.active?; celerity.cast; end; dothistimeout \
            'search', 3, /You make a careful search of the area and discover a narrow icy \
            ledge!|You don't find anything of interest here./; break if move 'go ledge' }";
        let found = steps(ledge, 2679, 2678).unwrap();
        assert_eq!(found.len(), 24);
        assert_eq!(found[2], always(Action::TryMove("go ledge".into())));
        assert_eq!(found[23], if_still_here(mv("go ledge")));
        assert!(matches!(&found[3].when, Some(Cond::All(parts)) if parts[0] == Cond::StillHere));
        assert_eq!(actions(&ledge.replace("8.times", "9.times"), 1, 2), []);
    }

    #[test]
    fn a_jump_and_the_walk_from_where_it_lands() {
        let script = ";e fput 'jump'\nwaitfor 'You feel the presence of cold hard stone \
            underneath you.'\nsleep 0.1\nfput 'stand' unless standing?\nmove 'west' until \
            Room.current.id == 12593";
        assert_eq!(
            actions(script, 12589, 12593),
            [
                pt("jump"),
                Action::Await("You feel the presence of cold hard stone underneath you.".into()),
                Action::Pause(100),
                pt("stand"),
                Action::MoveUntilThere("west".into()),
            ]
        );
        assert_eq!(actions(script, 12589, 1), []);
    }

    #[test]
    fn the_small_ones() {
        let lock = ";e empty_hand;fput 'turn lock';fill_hand;fput 'open door';fput 'go \
            door';echo '** WARNING: The door closes behind you!  Get out soon before it locks! \
            **'";
        assert_eq!(
            actions(lock, 20008, 20011),
            [
                Action::EmptyHands,
                pt("turn lock"),
                Action::FillHands,
                pt("open door"),
                mv("go door")
            ]
        );
        assert_eq!(actions(&lock.replace("turn lock", "pick lock"), 1, 2), []);
        let river = ";e empty_hands\nfput 'unhide' if checkspell 'invisibility'\nmove 'go river'";
        let found = steps(river, 11432, 11433).unwrap();
        assert_eq!(
            found[1].when,
            Some(Cond::SpellActive("Invisibility".into()))
        );
        assert_eq!(found[2], always(mv("go river")));
        let column = ";e 3.times do;  waitrt?;  fput 'stand' unless standing?;  waitrt?;  \
            climb_result = dothistimeout \"climb stone\", 3, /^(?:You start to climb the column \
            of stone, but it proves so slippery that you slide back down and end up on the \
            ground in a heap\\.|You dance lightly up the column of stone and into the area \
            beyond\\.)$/;  break if climb_result =~ /^You dance/;end";
        assert_eq!(actions(column, 7664, 7669), [mv("climb stone")]);
        assert_eq!(actions(&column.replace("3.times", "4.times"), 1, 2), []);
    }

    #[test]
    fn the_water_tunnel_from_inside_and_out() {
        let inside = ";e ;\nstart_time = Time.now.to_i;\nfput 'lay';\n_respond \
            \"#{monsterbold_start}Waiting for 'one on the left and one on the \
            right'.#{monsterbold_end}\";\nwaitfor \"one on the left and one on the \
            right\";\nfput \"lean left\";\n_respond \"#{monsterbold_start}Waiting for 'branches \
            off to the left just ahead'.#{monsterbold_end}\";\nwaitfor \"branches off to the \
            left just ahead\";\nfput \"lean left\";\n_respond \"#{monsterbold_start}Waiting for \
            'Another tunnel branches off to the left just ahead'.#{monsterbold_end}\";\nwaitfor \
            \"Another tunnel branches off to the left just ahead\";\n_respond \
            \"#{monsterbold_start}Waiting for 'Suddenly the tunnel turns into a nearly vertical \
            drop'.#{monsterbold_end}\";\nwaitfor \"Suddenly the tunnel turns into a nearly \
            vertical drop\";\nfput \"lean left\";\n_respond \"#{monsterbold_start}Waiting to \
            exit the tunnels.#{monsterbold_end}\";\nwaitfor \"Obvious paths: \
            southwest\";\n_respond \"#{monsterbold_start}water tunnel time: #{Time.now.to_i - \
            start_time} seconds.#{monsterbold_end}\";\nfill_hands;\n";
        let found = actions(inside, 18187, 451);
        assert_eq!(found.len(), 10);
        assert_eq!(found[0], pt("lay"));
        assert_eq!(
            found[5..7],
            [
                Action::Await("Another tunnel branches off to the left just ahead".into()),
                Action::Await("Suddenly the tunnel turns into a nearly vertical drop".into()),
            ]
        );
        assert_eq!(found[9], Action::FillHands);

        let outside = inside
            .replace(
                "fput 'lay'",
                "refill_hands=false;(refill_hands = true;empty_hands;) if \
                 GameObj.right_hand.id or GameObj.left_hand.id;\nfput 'go opening';\nfput 'lay'",
            )
            .replace("fill_hands;\n", "fill_hands if refill_hands;\n");
        let found = actions(&outside, 18186, 451);
        assert_eq!(found.len(), 12);
        assert_eq!(found[..2], [Action::EmptyHands, mv("go opening")]);
        assert_eq!(
            actions(&inside.replace("lean left", "lean right"), 1, 2),
            []
        );
    }

    #[test]
    fn the_sphere_and_the_depressions() {
        let sphere = ";e fput 'go sphere'\nloop do\n  result = dothistimeout('east', 2, /You are \
            surrounded by an ethereal fog.|You feel every shred of yourself torn to tiny pieces \
            and reformed...|You can do nothing... you can feel nothing... you are \
            nothing.../)\n  break if result =~ /You feel every shred of yourself torn to tiny \
            pieces and reformed...|You can do nothing... you can feel nothing... you are \
            nothing.../\n  break unless XMLData.room_description.empty?\nend\nsleep(1) while \
            XMLData.room_description.empty?\nfput 'stand' unless standing? \n";
        assert_eq!(
            actions(sphere, 2636, 12093),
            [pt("go sphere"), keep("east"), pt("stand")]
        );
        assert_eq!(actions(&sphere.replace("'east'", "'go east'"), 1, 2), []);

        let pushes = ";e %w(circular triangular square rectangular).each{|d| dothistimeout \
            \"push #{d} depression\", 3, /\\*[A-Z]+\\*$/};fput \"stand\" until standing?";
        let found = actions(pushes, 4537, 7389);
        assert_eq!(found.len(), 5);
        assert_eq!(
            found[3],
            Action::TryMove("push rectangular depression".into())
        );
        assert_eq!(actions(&pushes.replace("square ", ""), 1, 2), []);
    }
}
