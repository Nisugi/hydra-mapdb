//! Arms whose crossing depends on what the game does next: a move that may
//! not work, a room that decides the way on, a walker that is carried.
//!
//! The steps here lean on two questions asked *in the room*
//! (`cena_map::step`): `Cond::StillHere`, after an `Action::TryMove`, and
//! `Cond::Exit`.

use cena_map::{Action, Cond, Crossing, Step};

use super::moves::quoted_list;
use super::{always, holes, is_plain_argument, is_word, quoted};

pub(super) fn crossing(script: &str, from: u32) -> Option<Crossing> {
    behind_a_locker(script)
        .or_else(|| wait_for_it_to_open(script))
        .or_else(|| by_the_exits(script))
        .or_else(|| carried(script, from))
        .or_else(|| after_any_line(script))
}

/// What is left to do when the first try did not move the walker.
fn if_still_here(action: Action) -> Step {
    Step {
        action,
        when: Some(Cond::StillHere),
    }
}

fn plain(command: &str) -> Option<String> {
    is_plain_argument(command).then(|| command.to_owned())
}

/// A locker alcove: the curtain will not open while the locker is. Try it;
/// if still here, close the locker and go. 30 exits, three spellings, two of
/// which end in a replan.
fn behind_a_locker(script: &str) -> Option<Crossing> {
    const FORMS: [([&str; 4], bool); 3] = [
        (
            [
                ";e room = Room.current.id;fput '",
                "'; if ( room == Room.current.id ); fput '",
                "';move '",
                "'; end",
            ],
            false,
        ),
        (
            [
                ";e room = Room.current.id;fput '",
                "'; if ( room == Room.current.id ); fput '",
                "';move '",
                "'; end; $go2_restart = true",
            ],
            true,
        ),
        (
            [
                ";e room = Room.current.id;fput '",
                "'; if ( room == Room.current.id ); fput '",
                "';move '",
                "'; end ; $go2_restart = true",
            ],
            true,
        ),
    ];
    let (found, replan) = FORMS
        .iter()
        .find_map(|(form, replan)| Some((quoted(script, form)?, *replan)))?;
    let [first, fix, again] = found[..] else {
        return None;
    };
    let mut steps = vec![
        always(Action::TryMove(plain(first)?)),
        if_still_here(Action::Put(plain(fix)?)),
        if_still_here(Action::Move(plain(again)?)),
    ];
    if replan {
        steps.push(always(Action::Replan));
    }
    Some(Crossing::Steps(steps))
}

/// `unless move 'go door'; …; waitfor '…'; move 'go door'; end`: try the door;
/// if it is shut, do what opens it, wait for it, and go. 13 exits, four
/// spellings. The `echo`s are dropped.
fn wait_for_it_to_open(script: &str) -> Option<Crossing> {
    // (template, commands sent before the wait, commands sent after it)
    let waiting = quoted(
        script,
        &[
            ";e unless (move '",
            "'); echo '",
            "'; waitfor '",
            "'; move '",
            "'; end",
        ],
    )
    .or_else(|| {
        quoted(
            script,
            &[
                ";e unless move '",
                "'; echo '",
                "'; waitfor '",
                "'; move '",
                "'; end",
            ],
        )
    });
    if let Some(found) = waiting {
        let [first, _echo, line, again] = found[..] else {
            return None;
        };
        (first == again && !line.is_empty() && !line.contains('\'')).then_some(())?;
        return Some(Crossing::Steps(vec![
            always(Action::TryMove(plain(first)?)),
            if_still_here(Action::Await(line.to_owned())),
            if_still_here(Action::Move(plain(again)?)),
        ]));
    }
    if let Some(found) = quoted(
        script,
        &[
            ";e unless move '",
            "'; fput 'kneel'; fput '",
            "'; waitfor '",
            "'; fput 'stand'; move '",
            "'; end",
        ],
    ) {
        let [first, touch, line, again] = found[..] else {
            return None;
        };
        (first == again && !line.contains('\'')).then_some(())?;
        return Some(Crossing::Steps(vec![
            always(Action::TryMove(plain(first)?)),
            if_still_here(Action::Put("kneel".to_owned())),
            if_still_here(Action::Put(plain(touch)?)),
            if_still_here(Action::Await(line.to_owned())),
            if_still_here(Action::Put("stand".to_owned())),
            if_still_here(Action::Move(plain(again)?)),
        ]));
    }
    let found = quoted(
        script,
        &[
            ";e unless move '",
            "'; dothistimeout '",
            "', 3, /^Suddenly, the stone doors swing silently open\\.$/; move '",
            "'; end",
        ],
    )?;
    let [first, lever, again] = found[..] else {
        return None;
    };
    (first == again).then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::TryMove(plain(first)?)),
        if_still_here(Action::Put(plain(lever)?)),
        if_still_here(Action::Move(plain(again)?)),
    ]))
}

/// Rooms whose obvious exits say which way to go. 22 exits.
fn by_the_exits(script: &str) -> Option<Crossing> {
    let exit = |short: &str| is_word(short).then(|| Cond::Exit(short.to_owned()));
    if let Some(found) = quoted(
        script,
        &[";e move '", "' while checkpaths.include?('", "')"],
    ) {
        let [command, short] = found[..] else {
            return None;
        };
        let action = Action::MoveWhile(plain(command)?, exit(short)?);
        return Some(Crossing::Steps(vec![always(action)]));
    }
    for (form, wanted) in [
        (
            [";e move '", "'; move '", "' if checkpaths.include?('", "')"],
            true,
        ),
        (
            [
                ";e move '",
                "'; move '",
                "' unless checkpaths.include?('",
                "')",
            ],
            false,
        ),
    ] {
        if let Some(found) = quoted(script, &form) {
            let [first, second, short] = found[..] else {
                return None;
            };
            let there = exit(short)?;
            // Asked in the room the first move reached, where the exits are
            // known. `Not`, so that if they somehow are not the second move is
            // left out: that only lands the walker short, and it replans,
            // where a move wrongly made walks it somewhere else.
            let when = if wanted {
                there
            } else {
                Cond::Not(Box::new(there))
            };
            return Some(Crossing::Steps(vec![
                always(Action::Move(plain(first)?)),
                Step {
                    action: Action::Move(plain(second)?),
                    when: Some(when),
                },
            ]));
        }
    }
    let found = quoted(script, &[";e move (XMLData.room_exits - [ '", "' ]).first"])?;
    let [came_by] = found[..] else {
        return None;
    };
    is_word(came_by).then_some(())?;
    Some(Crossing::Steps(vec![always(Action::MoveByAnyExitBut(
        came_by.to_owned(),
    ))]))
}

/// Nothing to send: the walker is carried, and the crossing is over when the
/// room changes. 9 exits. The room named must be the one being left.
fn carried(script: &str, from: u32) -> Option<Crossing> {
    const FORMS: [([&str; 2], bool); 3] = [
        ([";e wait_until{Map.current.id != ", "}"], false),
        ([";e wait_while { Room.current.id == ", " }"], false),
        (
            [";e wait_while{Map.current.id == ", "};$go2_restart=true;"],
            true,
        ),
    ];
    let (found, replan) = FORMS
        .iter()
        .find_map(|(form, replan)| Some((holes(script, form)?, *replan)))?;
    let [room] = found[..] else {
        return None;
    };
    (room.parse() == Ok(from)).then_some(())?;
    let mut steps = vec![always(Action::AwaitArrival)];
    if replan {
        steps.push(always(Action::Replan));
    }
    Some(Crossing::Steps(steps))
}

/// Wait for any one of several lines. 9 exits: a swinging disk, and the
/// Hinterwilds caravans, whose three halts are the same event in three
/// wordings.
fn after_any_line(script: &str) -> Option<Crossing> {
    const MEMORY: &str = "UserVars.mapdb_hinterwilds_location = ";
    let lines = |list: &str| -> Option<Vec<String>> {
        let lines = quoted_list(list)?;
        (lines.len() > 1).then(|| lines.into_iter().map(str::to_owned).collect())
    };
    if let Some(found) = holes(script, &[";e waitfor ", "\nmove '", "'"]) {
        let [list, command] = found[..] else {
            return None;
        };
        return Some(Crossing::Steps(vec![
            always(Action::AwaitAny(lines(list)?)),
            always(Action::Move(plain(command)?)),
        ]));
    }
    let body = script.strip_prefix(";e ")?;
    // The caravan into the Hinterwilds remembers which end it left from; the
    // one out forgets. Written once arrived, whichever side upstream put it.
    let (body, remember) = match body.strip_prefix(MEMORY) {
        Some(rest) => {
            let (end, rest) = rest.split_once(';')?;
            let end = end.strip_prefix('\'')?.strip_suffix('\'')?;
            (rest, Some(is_word(end).then_some(end)?))
        }
        None => (body, None),
    };
    let (body, forget) = match body.strip_suffix("UserVars.mapdb_hinterwilds_location = nil;") {
        Some(rest) => (rest, true),
        None => (body, false),
    };
    let [orders, halts] = holes(body, &["multifput ", ";waitfor ", ";"])?[..] else {
        return None;
    };
    let mut steps: Vec<Step> = quoted_list(orders)?
        .into_iter()
        .map(|order| always(Action::Put(order.to_owned())))
        .collect();
    if steps.is_empty() || (forget && remember.is_some()) {
        return None;
    }
    steps.push(always(Action::AwaitAny(lines(halts)?)));
    let memory = "hinterwilds_location".to_owned();
    if let Some(end) = remember {
        steps.push(always(Action::Remember(memory, end.to_owned())));
    } else if forget {
        steps.push(always(Action::Forget(memory)));
    }
    Some(Crossing::Steps(steps))
}

#[cfg(test)]
mod tests {
    use cena_map::{Walker, moves_whatever_is_known};

    use super::*;

    fn steps(script: &str, from: u32) -> Vec<Step> {
        match crossing(script, from) {
            Some(Crossing::Steps(steps)) => {
                assert!(moves_whatever_is_known(&steps), "{script}");
                steps
            }
            _ => Vec::new(),
        }
    }

    fn actions(script: &str, from: u32) -> Vec<Action> {
        steps(script, from)
            .into_iter()
            .map(|step| step.action)
            .collect()
    }

    /// The curtain opens at once, or the locker is shut first: either way the
    /// walker gets through, and only the second way does any more.
    #[test]
    fn a_locker_alcove_tries_then_fixes_then_goes() {
        let script = ";e room = Room.current.id;fput 'go curtain'; if ( room == Room.current.id \
                      ); fput 'close locker';move 'go curtain'; end; $go2_restart = true";
        let steps = steps(script, 2485);
        let fires = |still_here| {
            let walker = Walker {
                still_here: Some(still_here),
                ..Walker::default()
            };
            steps
                .iter()
                .filter(|step| step.when.as_ref().is_none_or(|when| when.holds(&walker)))
                .map(|step| step.action.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            fires(false),
            [Action::TryMove("go curtain".into()), Action::Replan]
        );
        assert_eq!(
            fires(true),
            [
                Action::TryMove("go curtain".into()),
                Action::Put("close locker".into()),
                Action::Move("go curtain".into()),
                Action::Replan
            ]
        );
    }

    #[test]
    fn a_door_on_its_own_schedule_is_tried_then_waited_for() {
        let script = ";e unless (move 'go door'); echo 'Waiting for the door to open... '; \
                      waitfor 'You hear a soft click from the door and it suddenly flies open.'; \
                      move 'go door'; end";
        assert_eq!(
            actions(script, 1),
            [
                Action::TryMove("go door".into()),
                Action::Await(
                    "You hear a soft click from the door and it suddenly flies open.".into()
                ),
                Action::Move("go door".into()),
            ]
        );
        // Trying one door and then going through another is not this shape.
        assert_eq!(actions(&script.replacen("go door", "go gate", 1), 1), []);
    }

    #[test]
    fn the_rooms_exits_choose_the_way() {
        assert_eq!(
            actions(";e move 'northeast' while checkpaths.include?('nw')", 1),
            [Action::MoveWhile(
                "northeast".into(),
                Cond::Exit("nw".into())
            )]
        );
        let unless = steps(
            ";e move 'northwest'; move 'southwest' unless checkpaths.include?('ne')",
            1,
        );
        let with = |exits: &[&str]| Walker {
            exits: Some(exits.iter().map(|exit| (*exit).to_owned()).collect()),
            ..Walker::default()
        };
        let second = unless[1].when.as_ref().unwrap();
        assert!(second.holds(&with(&["s", "sw"])));
        assert!(!second.holds(&with(&["ne", "sw"])));
        assert_eq!(
            actions(";e move (XMLData.room_exits - [ 'southeast' ]).first", 1),
            [Action::MoveByAnyExitBut("southeast".into())]
        );
    }

    #[test]
    fn a_walker_being_carried_sends_nothing() {
        assert_eq!(
            actions(";e wait_until{Map.current.id != 30812}", 30812),
            [Action::AwaitArrival]
        );
        assert_eq!(actions(";e wait_until{Map.current.id != 30812}", 1), []);
        assert_eq!(
            actions(
                ";e wait_while{Map.current.id == 17665};$go2_restart=true;",
                17665
            ),
            [Action::AwaitArrival, Action::Replan]
        );
    }

    #[test]
    fn a_caravan_halts_in_any_of_three_wordings() {
        let halts = "waitfor 'The wagon comes to a halt','The wagon draws to an abrupt \
                     halt','the caravan comes to a stop';";
        let out = format!(
            ";e multifput 'inquire','order 1','order confirm';{halts}\
             UserVars.mapdb_hinterwilds_location = nil;"
        );
        let found = actions(&out, 1);
        assert_eq!(found.len(), 5);
        assert!(matches!(&found[3], Action::AwaitAny(lines) if lines.len() == 3));
        assert_eq!(found[4], Action::Forget("hinterwilds_location".into()));

        let into = format!(
            ";e UserVars.mapdb_hinterwilds_location = 'IM';multifput 'inquire','order 3',\
             'order confirm';{halts}"
        );
        assert_eq!(
            actions(&into, 1).last(),
            Some(&Action::Remember(
                "hinterwilds_location".into(),
                "IM".into()
            ))
        );
    }
}
