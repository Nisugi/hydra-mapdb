//! Arms for slice c of the long tail (`research/mapdb-inventory/tail/slice_c.tsv`).
//!
//! Same rules as every arm (`super`): the upstream script verbatim, with
//! holes; an exact match or none. This file's tests live at its foot.

use cena_map::{Action, Crossing, Step};

use super::moves::quoted_list;
use super::{always, holes, is_plain_argument, is_word, quoted};

/// The steps for a crossing script in this slice, if an arm here knows it.
pub(super) fn crossing(script: &str, from: u32, to: u32) -> Option<Crossing> {
    let _ = to;
    move_then_tail(script)
        .or_else(|| moves_in_a_row(script))
        .or_else(|| move_and_remember(script, from))
        .or_else(|| rickety_stairs(script))
        .or_else(|| listed_puts_then_move(script))
        .or_else(|| put_then_move(script))
        .or_else(|| await_then_move(script))
        .or_else(|| cab_ride(script))
        .or_else(|| kneel_through(script))
        .or_else(|| until_the_room_changes(script, from))
        .or_else(|| fog(script, from))
}

fn moved(command: &str) -> Option<Step> {
    is_plain_argument(command).then(|| always(Action::Move(command.to_owned())))
}

fn put(command: &str) -> Option<Step> {
    is_plain_argument(command).then(|| always(Action::Put(command.to_owned())))
}

/// The same command until the room changes: `Action::KeepMoving`.
fn keep_moving(command: &str) -> Option<Step> {
    is_plain_argument(command).then(|| always(Action::KeepMoving(command.to_owned())))
}

fn stand() -> Step {
    always(Action::Put("stand".to_owned()))
}

/// What follows a lone move.
#[derive(Clone, Copy)]
enum Tail {
    /// A leading `waitrt?`, which `Action::Move` already means. 9 exits.
    Nothing,
    /// `$go2_restart = true`: the exit may land somewhere else. 9 exits.
    Replan,
    /// `fill_hands`, after a climb that emptied them. 5 exits.
    Fill,
    /// `fput 'stand' unless standing?`: a crawl. Sent unguarded -- the game
    /// answers a walker already standing, and nothing else changes. 6 exits.
    Stand,
    /// `empty_hands` before and `fill_hands` after. 1 exit.
    HandsFree,
}

/// One move with something small around it.
fn move_then_tail(script: &str) -> Option<Crossing> {
    const FORMS: [([&str; 2], Tail); 12] = [
        ([";e move '", "'; waitrt?; fill_hands"], Tail::Fill),
        ([";e move '", "';fill_hand;"], Tail::Fill),
        ([";e move '", "';fill_hands"], Tail::Fill),
        ([";e move '", "';$go2_restart = true"], Tail::Replan),
        ([";e move '", "';$go2_restart = true;"], Tail::Replan),
        ([";e move '", "';$go2_restart=true"], Tail::Replan),
        ([";e move('", "');$go2_restart = true"], Tail::Replan),
        ([";e waitrt?; move '", "'"], Tail::Nothing),
        (
            [
                ";e move '",
                "'; waitrt?; fput 'stand' unless standing?; waitrt?",
            ],
            Tail::Stand,
        ),
        ([";e move '", "';fput 'stand' until standing?"], Tail::Stand),
        ([";e move('", "'); fput 'stand'"], Tail::Stand),
        (
            [";e waitrt?; empty_hands; move '", "'; fill_hands"],
            Tail::HandsFree,
        ),
    ];
    let (found, tail) = FORMS
        .iter()
        .find_map(|(form, tail)| Some((quoted(script, form)?, *tail)))?;
    let [command] = found[..] else {
        return None;
    };
    let mut steps = vec![moved(command)?];
    match tail {
        Tail::Nothing => {}
        Tail::Replan => steps.push(always(Action::Replan)),
        Tail::Fill => steps.push(always(Action::FillHands)),
        Tail::Stand => steps.push(stand()),
        Tail::HandsFree => {
            steps.insert(0, always(Action::EmptyHands));
            steps.push(always(Action::FillHands));
        }
    }
    Some(Crossing::Steps(steps))
}

/// `move 'southwest';move 'northeast'`: two moves, or four, through rooms the
/// map does not hold. 6 exits.
fn moves_in_a_row(script: &str) -> Option<Crossing> {
    let steps = script
        .strip_prefix(";e ")?
        .split(';')
        .map(|piece| {
            let [command] = quoted(piece, &["move '", "'"])?[..] else {
                return None;
            };
            moved(command)
        })
        .collect::<Option<Vec<Step>>>()?;
    matches!(steps.len(), 2 | 4).then_some(Crossing::Steps(steps))
}

/// Into an event ground by its door, remembering the room left. 10 exits.
fn move_and_remember(script: &str, from: u32) -> Option<Crossing> {
    let found = quoted(script, &[";e move('", "');UserVars.mapdb_", " = ", ";"])?;
    let [command, name, value] = found[..] else {
        return None;
    };
    (is_word(name) && value.parse() == Ok(from)).then_some(())?;
    Some(Crossing::Steps(vec![
        moved(command)?,
        always(Action::Remember(name.to_owned(), from.to_string())),
    ]))
}

/// Stairs that knock the walker down: stand, move, a beat, stand. 16 exits.
/// Standing *before* is `Action::Move`'s own; standing after is sent
/// unguarded, as in [`Tail::Stand`].
fn rickety_stairs(script: &str) -> Option<Crossing> {
    const STAND: &str =
        "8.times { if standing?; break; else; fput 'stand'; sleep 0.2; waitrt?; end }; ";
    let before = format!(";e waitrt?; {STAND}move '");
    let after = format!("'; sleep 0.2; waitrt?; {STAND}waitrt?");
    let [command] = holes(script, &[&before, &after])?[..] else {
        return None;
    };
    Some(Crossing::Steps(vec![
        moved(command)?,
        always(Action::Pause(200)),
        stand(),
    ]))
}

/// `multifput('search','push block');move 'go opening'`, and the two whose
/// last listed command is the one that moves. 3 exits.
fn listed_puts_then_move(script: &str) -> Option<Crossing> {
    let rest = script.strip_prefix(";e multifput")?;
    let (list, mover, stands) = if let Some(rest) = rest.strip_prefix('(') {
        match rest.split_once(");move '") {
            Some((list, mover)) => (list, Some(mover.strip_suffix('\'')?), false),
            None => (rest.strip_suffix(')')?, None, false),
        }
    } else {
        let list = rest.strip_prefix(' ')?;
        (list.strip_suffix("\nwaitrt?\nfput \"stand\"")?, None, true)
    };
    let mut commands = quoted_list(list)?;
    let mover = mover.or_else(|| commands.pop())?;
    (!commands.is_empty()).then_some(())?;
    let mut steps = commands
        .into_iter()
        .map(put)
        .collect::<Option<Vec<Step>>>()?;
    steps.push(moved(mover)?);
    if stands {
        steps.push(stand());
    }
    Some(Crossing::Steps(steps))
}

/// `fput 'search'` and then the way it uncovers. 3 exits. Two `put`s of the
/// *same* command are refused: which of them moves is not known.
fn put_then_move(script: &str) -> Option<Crossing> {
    let lone = quoted(script, &[";e put '", "'"]);
    if let Some(step) = lone.and_then(|found| moved(found.first()?)) {
        return Some(Crossing::Steps(vec![step]));
    }
    let (found, pause) =
        if let Some(found) = quoted(script, &[";e pause 0.2;fput '", "';waitrt;fput '", "'"]) {
            (found, true)
        } else {
            let found = quoted(script, &[";e waitrt?; fput '", "'; move '", "'"])
                .or_else(|| quoted(script, &[";e put '", "';put '", "'"]))?;
            (found, false)
        };
    let [first, then] = found[..] else {
        return None;
    };
    (first != then).then_some(())?;
    let mut steps = vec![put(first)?, moved(then)?];
    if pause {
        steps.insert(0, always(Action::Pause(200)));
    }
    Some(Crossing::Steps(steps))
}

/// `waitfor 'The boat arrives…'; move 'out'`, and one ride that ends by
/// itself. 7 exits.
fn await_then_move(script: &str) -> Option<Crossing> {
    let lone = quoted(script, &[";e waitfor '", "'"]);
    let lone = lone.and_then(|found| found.first().copied());
    if let Some(said) = lone.filter(|said| is_plain_argument(said)) {
        return Some(Crossing::Steps(vec![always(Action::Await(
            said.to_owned(),
        ))]));
    }
    let found = quoted(script, &[";e waitfor '", "'\nmove '", "'"])
        .or_else(|| quoted(script, &[";e waitfor '", "'; move '", "'"]))?;
    let [said, command] = found[..] else {
        return None;
    };
    is_plain_argument(said).then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::Await(said.to_owned())),
        moved(command)?,
    ]))
}

/// The cable cab: wait for the ledge, get out. 1 exit. The `_respond` talks
/// to a Lich user.
fn cab_ride(script: &str) -> Option<Crossing> {
    const LEDGE: &str = "ledge comes into view and the cab grinds to a clunky halt";
    const SCRIPT: &str = ";e \n  sleep(0.2);\n  _respond \"#{monsterbold_start}Waiting for \
        'ledge comes into view'.  This may take around four minutes.#{monsterbold_end}  \
        #{Time.now}\";\n  waitfor \"ledge comes into view and the cab grinds to a clunky halt\";\n  \
        move(\"out\");\n";
    (script == SCRIPT).then(|| {
        Crossing::Steps(vec![
            always(Action::Pause(200)),
            always(Action::Await(LEDGE.to_owned())),
            always(Action::Move("out".to_owned())),
        ])
    })
}

/// Search, kneel, through the hole, stand. 2 exits. `unless kneeling?` is
/// dropped as `unless standing?` is in [`Tail::Stand`].
fn kneel_through(script: &str) -> Option<Crossing> {
    let found = holes(
        script,
        &[
            ";e waitrt?; fput 'search'; waitrt?; fput 'kneel' unless kneeling?; move '",
            "'; fput 'stand'; waitrt?",
        ],
    )?;
    let [command] = found[..] else {
        return None;
    };
    Some(Crossing::Steps(vec![
        put("search")?,
        put("kneel")?,
        moved(command)?,
        stand(),
    ]))
}

/// `while Room.current.id == 13429; fput 'row south'; waitrt?; end`: the same
/// command until the room changes. 13 exits.
fn until_the_room_changes(script: &str, from: u32) -> Option<Crossing> {
    const ROWING: [[&str; 3]; 3] = [
        [
            ";e while Room.current.id == ",
            "; fput '",
            "'; waitrt; end;",
        ],
        [
            ";e while Room.current.id == ",
            "; fput '",
            "'; waitrt?; end",
        ],
        [";e while Room.current.id == ", ";fput '", "';waitrt?;end"],
    ];
    if let Some(found) = ROWING.iter().find_map(|form| quoted(script, form)) {
        let [room, command] = found[..] else {
            return None;
        };
        (room.parse() == Ok(from)).then_some(())?;
        return Some(Crossing::Steps(vec![keep_moving(command)?]));
    }
    if let Some(found) = quoted(
        script,
        &[
            ";e x=XMLData.room_count;fput '",
            "' until XMLData.room_count > x",
        ],
    ) {
        let [command] = found[..] else {
            return None;
        };
        return Some(Crossing::Steps(vec![keep_moving(command)?]));
    }
    // Two rooms on, by the same command.
    let found = quoted(
        script,
        &[
            ";e x=XMLData.room_count+2;fput '",
            "' until XMLData.room_count == x",
        ],
    )?;
    let [command] = found[..] else {
        return None;
    };
    Some(Crossing::Steps(vec![
        keep_moving(command)?,
        keep_moving(command)?,
    ]))
}

/// The Red Forest's fog turns the walker around; `go fog` until it does not.
/// 8 exits. Two also forget where in the forest the walker was.
fn fog(script: &str, from: u32) -> Option<Crossing> {
    const TURNED: &str = "You attempt to navigate your way through the fog, but get turned \
        around and come right back out where you started!";
    const ARRIVED: &str = "Obvious paths: northeast, south";
    const FORGET: &str = ";UserVars.mapdb_redforest_location = nil; $go2_restart=true";
    let go = Crossing::Steps(vec![keep_moving("go fog")?]);
    let out = format!(
        ";e result = nil;until result =~ /{ARRIVED}/;fput \"stand\" until standing?;\
         result = dothistimeout \"go fog\", 5, /{TURNED}|{ARRIVED}/;\
         if result =~ /{TURNED}/;sleep 0.5;waitrt?;end;end"
    );
    if script == out {
        return Some(go);
    }
    if script.strip_suffix(FORGET) == Some(&out) {
        return Some(Crossing::Steps(vec![
            keep_moving("go fog")?,
            always(Action::Forget("redforest_location".to_owned())),
            always(Action::Replan),
        ]));
    }
    let back = format!(
        ";e while Room.current.id == {from} do;fput \"stand\" until standing?;\
         success = /^Rows upon rows of oak trees/;fail = /^You attempt to navigate/;\
         result = dothistimeout \"go fog\", 5, Regexp.union(success, fail);\
         if result =~ fail;sleep 0.5;waitrt?;end;end"
    );
    (script == back).then_some(go)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps(script: &str, from: u32) -> Option<Vec<Step>> {
        match crossing(script, from, 0)? {
            Crossing::Steps(steps) => Some(steps),
            _ => None,
        }
    }

    fn keep(command: &str) -> Step {
        always(Action::KeepMoving(command.to_owned()))
    }

    fn mv(command: &str) -> Step {
        always(Action::Move(command.to_owned()))
    }

    fn pt(command: &str) -> Step {
        always(Action::Put(command.to_owned()))
    }

    fn check(script: &str, from: u32, expected: Vec<Step>) {
        assert!(cena_map::moves_whatever_is_known(&expected));
        assert_eq!(steps(script, from), Some(expected), "{script}");
    }

    #[test]
    fn a_move_and_what_trails_it() {
        check(
            ";e move 'jump'; waitrt?; fill_hands",
            0,
            vec![mv("jump"), always(Action::FillHands)],
        );
        check(
            ";e move 'go floating dock';fill_hand;",
            0,
            vec![mv("go floating dock"), always(Action::FillHands)],
        );
        check(
            ";e move \"go door\";$go2_restart=true",
            0,
            vec![mv("go door"), always(Action::Replan)],
        );
        check(
            ";e move('go ethereal portal');$go2_restart = true",
            0,
            vec![mv("go ethereal portal"), always(Action::Replan)],
        );
        check(";e waitrt?; move 'east'", 0, vec![mv("east")]);
        check(
            ";e move \"crawl hollow\";fput \"stand\" until standing?",
            0,
            vec![mv("crawl hollow"), stand()],
        );
        check(
            ";e move 'crawl north'; waitrt?; fput 'stand' unless standing?; waitrt?",
            0,
            vec![mv("crawl north"), stand()],
        );
        check(
            ";e move('go cave'); fput 'stand'",
            0,
            vec![mv("go cave"), stand()],
        );
        check(
            ";e waitrt?; empty_hands; move 'climb wall'; fill_hands",
            0,
            vec![
                always(Action::EmptyHands),
                mv("climb wall"),
                always(Action::FillHands),
            ],
        );
        // Another global is not go2's restart.
        assert_eq!(
            steps(
                ";e move 'go door';$go2_restart=true;$SILVERWOOD_TOWN=nil",
                0
            ),
            None
        );
        assert_eq!(
            steps(";e move 'jump pit'; wait_while{checkstunned}", 0),
            None
        );
    }

    #[test]
    fn moves_through_unmapped_rooms() {
        check(
            ";e move 'go door';move 'go passage'",
            0,
            vec![mv("go door"), mv("go passage")],
        );
        check(
            ";e move 'southeast';move 'southwest';move 'northeast';move 'southwest'",
            0,
            vec![
                mv("southeast"),
                mv("southwest"),
                mv("northeast"),
                mv("southwest"),
            ],
        );
        assert_eq!(steps(";e move 'a';move 'b';move 'c'", 0), None);
        assert_eq!(steps(";e move 'a';fput 'b'", 0), None);
    }

    #[test]
    fn a_door_into_an_event_ground() {
        let script = ";e move('go arch');UserVars.mapdb_talondown_origin = 682;";
        check(
            script,
            682,
            vec![
                mv("go arch"),
                always(Action::Remember(
                    "talondown_origin".to_owned(),
                    "682".to_owned(),
                )),
            ],
        );
        // Remembering a room other than the one left.
        assert_eq!(steps(script, 417), None);
    }

    #[test]
    fn stairs_that_knock_the_walker_down() {
        let script = ";e waitrt?; 8.times { if standing?; break; else; fput 'stand'; sleep 0.2; \
            waitrt?; end }; move 'up'; sleep 0.2; waitrt?; 8.times { if standing?; break; else; \
            fput 'stand'; sleep 0.2; waitrt?; end }; waitrt?";
        check(
            script,
            0,
            vec![mv("up"), always(Action::Pause(200)), stand()],
        );
        assert_eq!(steps(&script.replace("8.times", "9.times"), 0), None);
    }

    #[test]
    fn listed_commands_and_the_one_that_moves() {
        check(
            ";e multifput('search','push block');move 'go opening'",
            0,
            vec![pt("search"), pt("push block"), mv("go opening")],
        );
        check(
            ";e multifput('pull silver panel','push gold panel','pull silver panel')",
            0,
            vec![
                pt("pull silver panel"),
                pt("push gold panel"),
                mv("pull silver panel"),
            ],
        );
        check(
            ";e multifput 'push jewel carv', 'push skull carv', 'push griff carv', \
             'push crown carv', 'push trian carv'\nwaitrt?\nfput \"stand\"",
            0,
            vec![
                pt("push jewel carv"),
                pt("push skull carv"),
                pt("push griff carv"),
                pt("push crown carv"),
                mv("push trian carv"),
                stand(),
            ],
        );
        assert_eq!(steps(";e multifput('only one')", 0), None);
        assert_eq!(
            steps(
                ";e multifput('sit chair', 'pull splinter'); waitrt?; fput('stand'); waitrt?",
                0
            ),
            None
        );
    }

    #[test]
    fn a_search_and_the_way_it_uncovers() {
        check(";e put 'tap globe'", 0, vec![mv("tap globe")]);
        check(
            ";e put 'search';put 'go blood-soaked trail'",
            0,
            vec![pt("search"), mv("go blood-soaked trail")],
        );
        check(
            ";e pause 0.2;fput 'search';waitrt;fput 'go passage'",
            0,
            vec![always(Action::Pause(200)), pt("search"), mv("go passage")],
        );
        check(
            ";e waitrt?; fput 'search'; move 'go fissure'",
            0,
            vec![pt("search"), mv("go fissure")],
        );
        // The same command twice: which one moves is not known.
        assert_eq!(steps(";e put 'go field';put 'go field'", 0), None);
    }

    #[test]
    fn waiting_for_a_ride() {
        let boat = "The boat arrives at the dock with a soft thunk.";
        check(
            &format!(";e waitfor '{boat}'; move 'out'"),
            0,
            vec![always(Action::Await(boat.to_owned())), mv("out")],
        );
        check(
            ";e waitfor 'You swing near the western ledge'\nmove 'jump ledge'",
            0,
            vec![
                always(Action::Await("You swing near the western ledge".to_owned())),
                mv("jump ledge"),
            ],
        );
        check(
            ";e waitfor 'and pull you down even further into the murk'",
            0,
            vec![always(Action::Await(
                "and pull you down even further into the murk".to_owned(),
            ))],
        );
        // Either of two lines: `Await` names one.
        assert_eq!(steps(";e waitfor 'a', 'b'\nmove 'jump east disk'", 0), None);
    }

    #[test]
    fn the_cab() {
        let script = ";e \n  sleep(0.2);\n  _respond \"#{monsterbold_start}Waiting for 'ledge comes \
            into view'.  This may take around four minutes.#{monsterbold_end}  #{Time.now}\";\n  \
            waitfor \"ledge comes into view and the cab grinds to a clunky halt\";\n  move(\"out\");\n";
        let got = steps(script, 0);
        assert_eq!(got.as_ref().map(Vec::len), Some(3));
        assert_eq!(got.and_then(|steps| steps.last().cloned()), Some(mv("out")));
        assert_eq!(steps(&script.replace("out", "in"), 0), None);
    }

    #[test]
    fn kneeling_through_a_hole() {
        let script = ";e waitrt?; fput 'search'; waitrt?; fput 'kneel' unless kneeling?; \
            move 'go hole'; fput 'stand'; waitrt?";
        check(
            script,
            0,
            vec![pt("search"), pt("kneel"), mv("go hole"), stand()],
        );
        assert_eq!(steps(&script.replace("kneeling?", "sitting?"), 0), None);
    }

    #[test]
    fn the_same_command_until_the_room_changes() {
        check(
            ";e while Room.current.id == 13429; fput 'row south'; waitrt?; end",
            13429,
            vec![keep("row south")],
        );
        check(
            ";e while Room.current.id == 18843;fput 'row west';waitrt?;end",
            18843,
            vec![keep("row west")],
        );
        check(
            ";e x=XMLData.room_count;fput \"ne\" until XMLData.room_count > x",
            0,
            vec![keep("ne")],
        );
        check(
            ";e x=XMLData.room_count+2;fput \"n\" until XMLData.room_count == x",
            0,
            vec![keep("n"), keep("n")],
        );
        // Waiting on a room other than the one left.
        assert_eq!(
            steps(
                ";e while Room.current.id == 13429; fput 'row south'; waitrt?; end",
                1
            ),
            None
        );
    }

    #[test]
    fn the_fog() {
        let out = ";e result = nil;until result =~ /Obvious paths: northeast, south/;fput \"stand\" \
            until standing?;result = dothistimeout \"go fog\", 5, /You attempt to navigate your way \
            through the fog, but get turned around and come right back out where you started!|\
            Obvious paths: northeast, south/;if result =~ /You attempt to navigate your way through \
            the fog, but get turned around and come right back out where you started!/;sleep 0.5;\
            waitrt?;end;end";
        check(out, 0, vec![keep("go fog")]);
        check(
            &format!("{out};UserVars.mapdb_redforest_location = nil; $go2_restart=true"),
            0,
            vec![
                keep("go fog"),
                always(Action::Forget("redforest_location".to_owned())),
                always(Action::Replan),
            ],
        );
        let back = ";e while Room.current.id == 24730 do;fput \"stand\" until standing?;success = \
            /^Rows upon rows of oak trees/;fail = /^You attempt to navigate/;result = dothistimeout \
            \"go fog\", 5, Regexp.union(success, fail);if result =~ fail;sleep 0.5;waitrt?;end;end";
        check(back, 24730, vec![keep("go fog")]);
        assert_eq!(steps(back, 24731), None);
        assert_eq!(steps(&out.replace("go fog", "go mist"), 0), None);
    }
}
