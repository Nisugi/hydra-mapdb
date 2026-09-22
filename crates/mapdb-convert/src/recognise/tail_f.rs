//! Arms for slice f of the long tail, round two
//! (`research/mapdb-inventory/tail/slice_f.tsv`).
//!
//! Same rules as every arm (`super`): the upstream script verbatim, with
//! holes; an exact match or none. This file's tests live at its foot.
//!
//! Most of what is here is "if that did not work, do this", said the way
//! `super::reactions` says it: an `Action::TryMove`, then steps that ask
//! `Cond::StillHere`.

use cena_map::{Action, Cond, Crossing, Step};

use super::{always, holes, is_plain_argument, is_word, quoted};

/// The steps for a crossing script in this slice, if an arm here knows it.
pub(super) fn crossing(script: &str, from: u32, to: u32) -> Option<Crossing> {
    let _ = to;
    twice_then_replan(script)
        .or_else(|| raft_on_a_geyser(script, from))
        .or_else(|| spoken_arch(script))
        .or_else(|| locked_door(script))
        .or_else(|| stone_door(script))
        .or_else(|| platinum(script))
        .or_else(|| row_if_seated(script, from))
        .or_else(|| ferry(script, from))
        .or_else(|| move_then_while(script))
}

fn plain(command: &str) -> Option<String> {
    is_plain_argument(command).then(|| command.to_owned())
}

fn when(cond: Cond, action: Action) -> Step {
    Step {
        action,
        when: Some(cond),
    }
}

fn if_still_here(action: Action) -> Step {
    when(Cond::StillHere, action)
}

fn put(command: &str) -> Step {
    always(Action::Put(command.to_owned()))
}

/// `fput 'go path';fput 'go path';$go2_restart = true` -- 2 exits, two
/// spellings. The same command twice with nobody checking either, landing
/// wherever it lands: two tries, then find out.
fn twice_then_replan(script: &str) -> Option<Crossing> {
    let found = holes(script, &[";e fput '", "';fput '", "';$go2_restart = true"])
        .or_else(|| holes(script, &[";e fput '", "';fput '", "';$go2_restart=true"]))?;
    let [first, second] = found[..] else {
        return None;
    };
    (first == second).then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::TryMove(plain(first)?)),
        always(Action::TryMove(plain(second)?)),
        always(Action::Replan),
    ]))
}

/// A raft set on a geyser -- 1 exit. Upstream waits for the room to change
/// *after* `go raft`, so boarding is not the crossing: the geyser is.
fn raft_on_a_geyser(script: &str, from: u32) -> Option<Crossing> {
    let [room] = holes(
        script,
        &[
            ";e fput \"get pile\";fput \"push raft\";fput \"go raft\";echo \"Waiting possibly \
             up to 5 minutes or more for the geyser to shoot...\";line = get until \
             Room.current.id != ",
            "",
        ],
    )?[..] else {
        return None;
    };
    (room.parse() == Ok(from)).then_some(())?;
    Some(Crossing::Steps(vec![
        put("get pile"),
        put("push raft"),
        put("go raft"),
        always(Action::AwaitArrival),
    ]))
}

/// An arch that opens to a phrase -- 1 exit. The phrase carries a quote of
/// its own, so there is no hole: the whole script is the template.
fn spoken_arch(script: &str) -> Option<Crossing> {
    (script == ";e fput \"say lalk gno'ap renqua to!'\";move ('go arch');").then(|| {
        Crossing::Steps(vec![
            put("say lalk gno'ap renqua to!'"),
            always(Action::Move("go arch".to_owned())),
        ])
    })
}

/// A door that may be locked -- 3 exits, one script each. Upstream opens it
/// and reads the answer; here the answer is whether `go door` worked. Hands
/// are emptied only on the locked side, and taking nothing back is no harm.
fn locked_door(script: &str) -> Option<Crossing> {
    const TURNED: [&str; 2] = [
        ";e fput 'open door';while line = get;if ['You open a large stone door.', 'That is \
         already open.'].include?(line);fput 'go door';break;elsif line == 'It appears to be \
         locked.';empty_hand;fput 'turn lock';fput 'open door';fput 'go door';fill_hand;break;\
         end;end;",
        ";e fput 'open door';while line = get;if line == \"It's locked, Blazyn!\";empty_hand;\
         fput 'turn lock';fput 'open door';fput 'go door';fill_hand;break;else;fput 'go door';\
         break;end;end",
    ];
    const PICKED: &str = ";e fput 'open door';while line = get;if ['You open the nearly \
         invisible stone door.', 'That is already open.'].include?(line);fput 'go door';break;\
         elsif line == 'It appears to be locked.';empty_hands;fput 'get lockpick';fput 'pick \
         door';fput 'stow lockpick';fill_hands;fput 'open door';fput 'go door';break;end;end";
    let go = || Action::TryMove("go door".to_owned());
    let fix = |command: &str| if_still_here(Action::Put(command.to_owned()));
    let mut steps = vec![put("open door"), always(go())];
    if TURNED.contains(&script) {
        steps.extend([
            if_still_here(Action::EmptyHands),
            fix("turn lock"),
            fix("open door"),
            if_still_here(Action::Move("go door".to_owned())),
            always(Action::FillHands),
        ]);
    } else if script == PICKED {
        steps.extend([
            if_still_here(Action::EmptyHands),
            fix("get lockpick"),
            fix("pick door"),
            fix("stow lockpick"),
            // Still in the room, so this takes back only what was put away.
            if_still_here(Action::FillHands),
            fix("open door"),
            if_still_here(Action::Move("go door".to_owned())),
        ]);
    } else {
        return None;
    }
    Some(Crossing::Steps(steps))
}

/// `if GameObj.loot.find{.. "door"}; go door; else push stone; ..` -- 1 exit.
/// A door that is not there cannot be gone through, so trying it asks the
/// same question. Pushing the stone knocks the walker down; `Move` stands.
fn stone_door(script: &str) -> Option<Crossing> {
    let [noun, first, fix, again] = holes(
        script,
        &[
            ";e if GameObj.loot.find{|item| item.noun == \"",
            "\"};fput \"",
            "\";else;fput \"",
            "\";fput \"stand\" until standing?;fput \"",
            "\";end",
        ],
    )?[..] else {
        return None;
    };
    (is_word(noun) && first == again).then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::TryMove(plain(first)?)),
        if_still_here(Action::Put(plain(fix)?)),
        if_still_here(Action::Move(plain(again)?)),
    ]))
}

/// `if $platinum; move 'go furrier'; else; move 'go warehouse'; end` -- 2
/// exits: the same building wears a different front on the Platinum
/// instance. A walker whose instance is unknown takes the Prime door.
fn platinum(script: &str) -> Option<Crossing> {
    let [there, elsewhere] = quoted(
        script,
        &[";e if $platinum; move '", "'; else; move '", "'; end"],
    )?[..] else {
        return None;
    };
    let platinum = Cond::Flag("platinum".to_owned());
    Some(Crossing::Steps(vec![
        when(platinum.clone(), Action::Move(plain(there)?)),
        when(
            Cond::Otherwise(Box::new(platinum)),
            Action::Move(plain(elsewhere)?),
        ),
    ]))
}

/// `if checksitting; while Room.current.id == N; fput('row shore'); ..` -- 3
/// exits: seated in a boat, row until ashore; otherwise climb or swim.
fn row_if_seated(script: &str, from: u32) -> Option<Crossing> {
    let [room, row, walk] = holes(
        script,
        &[
            ";e if checksitting;while Room.current.id == ",
            ";fput('",
            "');waitrt?;end;else;move('",
            "');end;fill_hand;",
        ],
    )?[..] else {
        return None;
    };
    (room.parse() == Ok(from)).then_some(())?;
    let seated = Cond::Posture("sitting".to_owned());
    Some(Crossing::Steps(vec![
        when(seated.clone(), Action::KeepMoving(plain(row)?)),
        when(
            Cond::Otherwise(Box::new(seated)),
            Action::Move(plain(walk)?),
        ),
        always(Action::FillHands),
    ]))
}

/// A ferry -- 2 exits. Board if it is in; if not, wait for the gangplank and
/// board. Then the same line again is the far shore, and `out`.
fn ferry(script: &str, from: u32) -> Option<Crossing> {
    let [board, room, _echo, docks, again, arrives, leave] = holes(
        script,
        &[
            ";e move '",
            "'\nif Room.current.id == ",
            "\necho '",
            "'\nwaitfor '",
            "'\nmove '",
            "'\nend\nwaitfor '",
            "'\nmove '",
            "'",
        ],
    )?[..] else {
        return None;
    };
    (room.parse() == Ok(from) && board == again && docks == arrives).then_some(())?;
    let line = plain(docks)?;
    Some(Crossing::Steps(vec![
        always(Action::TryMove(plain(board)?)),
        if_still_here(Action::Await(line.clone())),
        if_still_here(Action::Move(plain(again)?)),
        always(Action::Await(line)),
        always(Action::Move(plain(leave)?)),
    ]))
}

/// `move 'northeast'; move 'east' while checkpaths.include?('e')` -- 2 exits.
fn move_then_while(script: &str) -> Option<Crossing> {
    let [first, command, short] = quoted(
        script,
        &[
            ";e move '",
            "'; move '",
            "' while checkpaths.include?('",
            "')",
        ],
    )?[..] else {
        return None;
    };
    is_word(short).then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::Move(plain(first)?)),
        always(Action::MoveWhile(
            plain(command)?,
            Cond::Exit(short.to_owned()),
        )),
    ]))
}

#[cfg(test)]
mod tests {
    use cena_map::{Walker, moves_whatever_is_known};

    use super::*;

    fn steps(script: &str, from: u32) -> Vec<Step> {
        match crossing(script, from, 0) {
            Some(Crossing::Steps(steps)) => {
                assert!(moves_whatever_is_known(&steps), "{script}");
                steps
            }
            _ => Vec::new(),
        }
    }

    fn actions(script: &str, from: u32) -> Vec<Action> {
        steps(script, from).into_iter().map(|s| s.action).collect()
    }

    /// What is done by a walker who has, or has not, been moved yet.
    fn fires(steps: &[Step], walker: &Walker) -> Vec<Action> {
        steps
            .iter()
            .filter(|step| step.when.as_ref().is_none_or(|when| when.holds(walker)))
            .map(|step| step.action.clone())
            .collect()
    }

    fn still_here(answer: bool) -> Walker {
        Walker {
            still_here: Some(answer),
            ..Walker::default()
        }
    }

    fn put(command: &str) -> Action {
        Action::Put(command.into())
    }

    #[test]
    fn the_same_command_twice_then_a_replan() {
        let go = Action::TryMove("go vortex".into());
        assert_eq!(
            actions(";e fput 'go vortex';fput 'go vortex';$go2_restart=true", 1),
            [go.clone(), go, Action::Replan]
        );
        assert_eq!(
            actions(";e fput 'go path';fput 'go path';$go2_restart = true", 1).len(),
            3
        );
        assert_eq!(
            actions(";e fput 'go path';fput 'go vortex';$go2_restart = true", 1),
            []
        );
    }

    #[test]
    fn a_raft_waits_for_its_geyser() {
        let script = ";e fput \"get pile\";fput \"push raft\";fput \"go raft\";echo \"Waiting \
                      possibly up to 5 minutes or more for the geyser to shoot...\";line = get \
                      until Room.current.id != 24238";
        assert_eq!(
            actions(script, 24238),
            [
                put("get pile"),
                put("push raft"),
                put("go raft"),
                Action::AwaitArrival
            ]
        );
        assert_eq!(actions(script, 24239), []);
    }

    #[test]
    fn an_arch_is_spoken_to() {
        let script = ";e fput \"say lalk gno'ap renqua to!'\";move ('go arch');";
        assert_eq!(
            actions(script, 1),
            [
                put("say lalk gno'ap renqua to!'"),
                Action::Move("go arch".into())
            ]
        );
        assert_eq!(actions(&script.replace("arch", "door"), 1), []);
    }

    #[test]
    fn a_locked_door_is_unlocked_only_when_it_stops_the_walker() {
        let script = ";e fput 'open door';while line = get;if line == \"It's locked, \
                      Blazyn!\";empty_hand;fput 'turn lock';fput 'open door';fput 'go \
                      door';fill_hand;break;else;fput 'go door';break;end;end";
        let steps = steps(script, 1);
        let go = Action::TryMove("go door".into());
        assert_eq!(
            fires(&steps, &still_here(false)),
            [put("open door"), go.clone(), Action::FillHands]
        );
        assert_eq!(
            fires(&steps, &still_here(true)),
            [
                put("open door"),
                go,
                Action::EmptyHands,
                put("turn lock"),
                put("open door"),
                Action::Move("go door".into()),
                Action::FillHands
            ]
        );
        assert_eq!(actions(&script.replace("Blazyn", "Someone"), 1), []);
    }

    #[test]
    fn a_picked_door_takes_its_hands_back_before_going() {
        let script = ";e fput 'open door';while line = get;if ['You open the nearly invisible \
                      stone door.', 'That is already open.'].include?(line);fput 'go \
                      door';break;elsif line == 'It appears to be locked.';empty_hands;fput \
                      'get lockpick';fput 'pick door';fput 'stow lockpick';fill_hands;fput \
                      'open door';fput 'go door';break;end;end";
        let found = fires(&steps(script, 1), &still_here(true));
        assert_eq!(found.len(), 9);
        assert_eq!(found[6], Action::FillHands);
        assert_eq!(found[8], Action::Move("go door".into()));
    }

    #[test]
    fn a_stone_is_pushed_when_there_is_no_door() {
        let script = ";e if GameObj.loot.find{|item| item.noun == \"door\"};fput \"go \
                      door\";else;fput \"push stone\";fput \"stand\" until standing?;fput \"go \
                      door\";end";
        assert_eq!(
            actions(script, 1),
            [
                Action::TryMove("go door".into()),
                put("push stone"),
                Action::Move("go door".into())
            ]
        );
        assert_eq!(actions(&script.replacen("go door", "go gate", 1), 1), []);
    }

    #[test]
    fn platinum_has_its_own_door_and_the_unknown_take_primes() {
        let steps = steps(
            ";e if $platinum; move 'go furrier'; else; move 'go warehouse'; end",
            1,
        );
        let on = |platinum| Walker {
            flags: [("platinum".to_owned(), platinum)].into(),
            ..Walker::default()
        };
        assert_eq!(
            fires(&steps, &on(true)),
            [Action::Move("go furrier".into())]
        );
        let prime = [Action::Move("go warehouse".into())];
        assert_eq!(fires(&steps, &on(false)), prime);
        assert_eq!(fires(&steps, &Walker::default()), prime);
        assert_eq!(actions(";e if $platinum; move 'go furrier'; end", 1), []);
    }

    #[test]
    fn a_seated_walker_rows_and_any_other_climbs() {
        let script = ";e if checksitting;while Room.current.id == 18823;fput('row \
                      shore');waitrt?;end;else;move('climb shore');end;fill_hand;";
        let steps = steps(script, 18823);
        let seated = Walker {
            posture: Some("sitting".into()),
            ..Walker::default()
        };
        assert_eq!(
            fires(&steps, &seated),
            [Action::KeepMoving("row shore".into()), Action::FillHands]
        );
        assert_eq!(
            fires(&steps, &Walker::default()),
            [Action::Move("climb shore".into()), Action::FillHands]
        );
        assert_eq!(actions(script, 18822), []);
    }

    #[test]
    fn a_ferry_is_boarded_now_or_when_it_docks() {
        let line = "An elven crewmember scrambles back onto the boat and lowers the gangplank.";
        let script = format!(
            ";e move 'go gangplank'\nif Room.current.id == 10117\necho 'Waiting for ferry... \
             '\nwaitfor '{line}'\nmove 'go gangplank'\nend\nwaitfor '{line}'\nmove 'out'"
        );
        let steps = steps(&script, 10117);
        assert_eq!(
            fires(&steps, &still_here(false)),
            [
                Action::TryMove("go gangplank".into()),
                Action::Await(line.into()),
                Action::Move("out".into())
            ]
        );
        assert_eq!(fires(&steps, &still_here(true)).len(), 5);
        assert_eq!(actions(&script, 10119), []);
    }

    #[test]
    fn a_move_and_then_the_same_way_while_it_is_there() {
        assert_eq!(
            actions(
                ";e move 'northeast'; move 'east' while checkpaths.include?('e')",
                1
            ),
            [
                Action::Move("northeast".into()),
                Action::MoveWhile("east".into(), Cond::Exit("e".into()))
            ]
        );
        assert_eq!(
            actions(
                ";e move 'northeast'; move 'east' until checkpaths.include?('e')",
                1
            ),
            []
        );
    }
}
