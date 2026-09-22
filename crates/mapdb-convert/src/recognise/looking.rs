//! Arms for crossings that **look at the room** to decide what to do: which
//! exits it lists, what it shows, whether it is the room that was wanted.
//! Mazes that are walked at random until they let go are here too, since what
//! ends them is the look of the room.

use cena_map::{Action, Cond, Crossing, Step};

use super::moves::quoted_list;
use super::{always, holes, is_plain_argument, is_word, quoted};

pub(super) fn crossing(script: &str, to: u32) -> Option<Crossing> {
    maze(script)
        .or_else(|| swim_up(script))
        .or_else(|| wander(script))
        .or_else(|| round_while(script))
        .or_else(|| detour(script, to))
        .or_else(|| unless_it_shows(script))
        .or_else(|| until_it_shows(script))
        .or_else(|| move_then_wait(script))
        .map(Crossing::Steps)
}

fn when(action: Action, cond: Cond) -> Step {
    Step {
        action,
        when: Some(cond),
    }
}

fn not(cond: Cond) -> Cond {
    Cond::Not(Box::new(cond))
}

fn exit(name: &str) -> Option<Cond> {
    is_word(name).then(|| Cond::Exit(name.to_owned()))
}

fn sees(name: &str) -> Option<Cond> {
    (is_plain_argument(name) && !name.contains(['/', '|', '.', '*', '?', '[', '(']))
        .then(|| Cond::Sees(name.to_owned()))
}

fn plain(command: &str) -> Option<String> {
    is_plain_argument(command).then(|| command.to_owned())
}

/// `move 'a'; fput 'b'; waitrt?` as the commands it sends. Upstream's pauses
/// for roundtime are the walker's to take, so they are read and dropped.
pub(super) fn commands(body: &str) -> Option<Vec<String>> {
    let mut found = Vec::new();
    for statement in body.split([';', '\n']).map(str::trim) {
        if matches!(statement, "" | "waitrt?" | "sleep 1") {
            continue;
        }
        let argument = statement
            .strip_prefix("move")
            .or_else(|| statement.strip_prefix("fput"))?
            .trim_start();
        let argument = argument
            .strip_prefix('(')
            .and_then(|inner| inner.strip_suffix(')'))
            .unwrap_or(argument);
        let [command] = quoted_list(argument)?[..] else {
            return None;
        };
        found.push(command.to_owned());
    }
    (!found.is_empty()).then_some(found)
}

/// One command is a move; several are a run of them under one guard.
fn run(mut list: Vec<String>) -> Action {
    if list.len() == 1 {
        Action::Move(list.remove(0))
    } else {
        Action::Moves(list)
    }
}

/// `['northwest','southwest'][rand(2)]`: the list, which the count must fit.
fn one_of(list: &str, count: &str) -> Option<Vec<String>> {
    let list: Vec<String> = quoted_list(list)?.into_iter().map(str::to_owned).collect();
    (list.len() > 1 && count.parse() == Ok(list.len())).then_some(list)
}

/// The look-alike rooms of a maze, every one listing `ne, se, sw, nw`: turn at
/// random until a room lists something else. 12 exits in four spellings.
fn maze(script: &str) -> Option<Vec<Step>> {
    const WHILE: &str = ";e while checkpaths == ['ne', 'se', 'sw', 'nw']; move [";
    const MODIFIER: &str = " while checkpaths == [ 'ne', 'se', 'sw', 'nw' ]";
    let look_alike = || Cond::ExitsAre(["ne", "se", "sw", "nw"].map(str::to_owned).to_vec());
    let (list, count, tail) =
        if let Some(found) = holes(script, &[WHILE, "][rand(", ")]; end;", ""]) {
            let [list, count, tail] = found[..] else {
                return None;
            };
            (list, count, tail)
        } else {
            let found = holes(script, &[";e move [", "][rand(", ")]", ""])?;
            let [list, count, tail] = found[..] else {
                return None;
            };
            (list, count, tail.strip_prefix(MODIFIER)?)
        };
    let mut steps = vec![always(Action::MoveAnyWhile(
        one_of(list, count)?,
        look_alike(),
    ))];
    if let Some(found) = holes(tail, &[" move '", "' while checkpaths.include?('", "')"]) {
        let [command, listed] = found[..] else {
            return None;
        };
        steps.push(always(Action::MoveWhile(plain(command)?, exit(listed)?)));
    } else if !tail.is_empty() {
        for clause in tail.strip_prefix("; ")?.split("; ") {
            let found = holes(clause, &["move '", "' if checkpaths.include?('", "')"])?;
            let [command, listed] = found[..] else {
                return None;
            };
            steps.push(when(Action::Move(plain(command)?), exit(listed)?));
        }
    }
    Some(steps)
}

/// Swimming one of two ways at random until the way up shows. 2 exits.
fn swim_up(script: &str) -> Option<Vec<Step>> {
    let found = holes(
        script,
        &[
            ";e until checkpaths.include?('",
            "'); fput 'swim ' + [",
            "][rand(",
            ")]; waitrt?; end",
        ],
    )?;
    let [listed, list, count] = found[..] else {
        return None;
    };
    let ways = one_of(list, count)?
        .into_iter()
        .map(|way| format!("swim {way}"))
        .collect();
    Some(vec![always(Action::MoveAnyWhile(ways, not(exit(listed)?)))])
}

/// Any obvious exit at random, until the room looks right. 5 exits. Upstream's
/// `walk` avoids the way it came and `checkpaths[rand]` does not; one step
/// serves both, and is seeded either way.
fn wander(script: &str) -> Option<Vec<Step>> {
    const ANY: &str = "move checkpaths[rand(checkpaths.length)]";
    if let Some(found) = holes(script, &[";e ", " until checkpaths.include?('", "')"]) {
        let [ANY, listed] = found[..] else {
            return None;
        };
        return Some(vec![always(Action::WanderWhile(not(exit(listed)?)))]);
    }
    if let Some(found) = holes(script, &[";e ", " while checkpaths.length > ", ""]) {
        let [first, count] = found[..] else {
            return None;
        };
        let first = first.strip_suffix(ANY)?;
        return Some(vec![
            always(run(commands(first)?)),
            always(Action::WanderWhile(Cond::ExitsOver(count.parse().ok()?))),
        ]);
    }
    let found = holes(
        script,
        &[";e walk until checkloot.include?('", "'); move '", "'"],
    )
    .or_else(|| {
        holes(
            script,
            &[
                ";e walk until GameObj.room_desc.find { |obj| obj.noun == '",
                "' }; move '",
                "'",
            ],
        )
    })?;
    let [thing, go] = found[..] else {
        return None;
    };
    Some(vec![
        always(Action::WanderWhile(not(sees(thing)?))),
        always(Action::Move(plain(go)?)),
    ])
}

/// The same few commands, round and round, while an exit is still listed:
/// `search`, `go fissure`. 4 exits.
fn round_while(script: &str) -> Option<Vec<Step>> {
    let found = holes(script, &[";e while checkpaths.include?('", "'); ", " end"])?;
    let [listed, body] = found[..] else {
        return None;
    };
    Some(vec![always(Action::RoundWhile(
        commands(body)?,
        exit(listed)?,
    ))])
}

/// A move that may land in the wrong place, and the moves that put it right.
/// 11 exits. Each guard is asked once, where the first moves ended, which is
/// why the rest are one step (`Action::Moves`).
fn detour(script: &str, to: u32) -> Option<Vec<Step>> {
    if let Some(found) = holes(
        script,
        &[";e ", "unless Room.current.id == ", " then ", "end"],
    )
    .or_else(|| holes(script, &[";e ", "unless Room.current.id == ", "; ", "end"]))
    {
        let [first, room, rest] = found[..] else {
            return None;
        };
        (room.parse() == Ok(to)).then_some(())?;
        return Some(vec![
            always(run(commands(first)?)),
            when(run(commands(rest)?), not(Cond::At(to))),
        ]);
    }
    if let Some(found) = holes(
        script,
        &[";e ", "unless checkpaths.include?('", "'); ", "end"],
    ) {
        let [first, listed, rest] = found[..] else {
            return None;
        };
        return Some(vec![
            always(run(commands(first)?)),
            when(run(commands(rest)?), not(exit(listed)?)),
        ]);
    }
    let found = holes(script, &[";e ", "if checkpaths.include?('", "'); ", "end"])?;
    let [first, listed, rest] = found[..] else {
        return None;
    };
    let mut steps = vec![always(run(commands(first)?))];
    let Some((then, otherwise)) = rest.split_once("elsif !checkpaths.include?('") else {
        steps.push(when(run(commands(rest)?), exit(listed)?));
        return Some(steps);
    };
    // `if … elsif …` is asked once upstream, and here the second question
    // comes after the first answer's moves. Those moves end at the
    // destination, so "not there yet" keeps the second from following them.
    let (other, last) = otherwise.split_once("'); ")?;
    steps.push(when(run(commands(then)?), exit(listed)?));
    steps.push(when(
        run(commands(last)?),
        Cond::All(vec![
            not(Cond::At(to)),
            not(exit(listed)?),
            not(exit(other)?),
        ]),
    ));
    Some(steps)
}

/// What opens the way, done only when the way is not already showing, then
/// the way itself. 6 exits.
fn unless_it_shows(script: &str) -> Option<Vec<Step>> {
    const SHOWS: &str = "GameObj.loot.any?{|i| i.name =~ /";
    if let Some(found) = holes(
        script,
        &[";e unless checkloot.include?", "end", "move '", "'"],
    ) {
        let [body, between, go] = found[..] else {
            return None;
        };
        matches!(between, "; " | "\n").then_some(())?;
        let (thing, body) = body.split_once([';', '\n'])?;
        let thing = thing.trim().trim_matches(['(', ')']);
        let [thing] = quoted_list(thing)?[..] else {
            return None;
        };
        return Some(vec![
            when(Action::Round(commands(body)?), not(sees(thing)?)),
            always(Action::Move(plain(go)?)),
        ]);
    }
    let found = holes(
        script,
        &[";e if !", "/};", "end;sleep 0.2 until ", "/};move('", "')"],
    )
    .or_else(|| {
        holes(
            script,
            &[";e if !", "/};", "sleep 0.2 until ", "/};end;move('", "')"],
        )
    })?;
    let [thing, body, again, go] = found[..] else {
        return None;
    };
    let thing = thing.strip_prefix(SHOWS)?;
    (again.strip_prefix(SHOWS) == Some(thing)).then_some(())?;
    Some(vec![
        when(Action::Round(commands(body)?), not(sees(thing)?)),
        always(Action::WaitUntil(sees(thing)?)),
        always(Action::Move(plain(go)?)),
    ])
}

/// `search` until the way shows, or wait for it to drift into reach. 3 exits.
fn until_it_shows(script: &str) -> Option<Vec<Step>> {
    if let Some(found) = holes(
        script,
        &[
            ";e fput 'search' until GameObj.loot.find{|x| x.name == '",
            "'}; fput '",
            "'",
        ],
    )
    .or_else(|| {
        holes(
            script,
            &[
                ";e until GameObj.loot.find {|o| o.noun=='",
                "'};fput 'search';waitrt?;end;move '",
                "';",
            ],
        )
    }) {
        let [thing, go] = found[..] else {
            return None;
        };
        return Some(vec![
            always(Action::PutWhile("search".to_owned(), not(sees(thing)?))),
            always(Action::Move(plain(go)?)),
        ]);
    }
    let found = quoted(
        script,
        &[
            ";e wait_until{GameObj.loot.find{|item| item.noun == '",
            "'}};fput '",
            "'",
        ],
    )?;
    let [thing, go] = found[..] else {
        return None;
    };
    Some(vec![
        always(Action::WaitUntil(sees(thing)?)),
        always(Action::Move(plain(go)?)),
    ])
}

/// A move, then a wait where it lands: for an exit to appear, or for the
/// stun of a jump to pass. 2 exits.
fn move_then_wait(script: &str) -> Option<Vec<Step>> {
    if let Some(found) = holes(
        script,
        &[";e move '", "'; wait_until { checkpaths.include?('", "') }"],
    ) {
        let [go, listed] = found[..] else {
            return None;
        };
        return Some(vec![
            always(Action::Move(plain(go)?)),
            always(Action::WaitUntil(exit(listed)?)),
        ]);
    }
    let [go] = holes(script, &[";e move '", "'; wait_while{checkstunned}"])?[..] else {
        return None;
    };
    Some(vec![
        always(Action::Move(plain(go)?)),
        always(Action::WaitUntil(Cond::Otherwise(Box::new(Cond::Flag(
            "stunned".to_owned(),
        ))))),
    ])
}

#[cfg(test)]
mod tests {
    use cena_map::moves_whatever_is_known;

    use super::*;

    fn steps(script: &str, to: u32) -> Vec<Step> {
        match crossing(script, to) {
            Some(Crossing::Steps(steps)) => {
                assert!(moves_whatever_is_known(&steps), "{script}");
                steps
            }
            _ => Vec::new(),
        }
    }

    fn actions(script: &str, to: u32) -> Vec<Action> {
        steps(script, to).into_iter().map(|s| s.action).collect()
    }

    fn lost() -> Cond {
        Cond::ExitsAre(vec!["ne".into(), "se".into(), "sw".into(), "nw".into()])
    }

    #[test]
    fn a_maze_is_turned_through_at_random_in_both_spellings() {
        let script = ";e while checkpaths == ['ne', 'se', 'sw', 'nw']; move \
                      ['northwest','southwest'][rand(2)]; end; move 'west' while \
                      checkpaths.include?('w')";
        assert_eq!(
            actions(script, 1),
            [
                Action::MoveAnyWhile(vec!["northwest".into(), "southwest".into()], lost()),
                Action::MoveWhile("west".into(), Cond::Exit("w".into())),
            ]
        );
        assert_eq!(actions(&script.replace("rand(2)", "rand(3)"), 1), []);

        let script = ";e move ['northwest','southwest'][rand(2)] while checkpaths == [ 'ne', \
                      'se', 'sw', 'nw' ]; move 'northwest' if checkpaths.include?('nw'); move \
                      'west' if checkpaths.include?('w')";
        let found = steps(script, 1);
        assert_eq!(found.len(), 3);
        assert_eq!(found[2].when, Some(Cond::Exit("w".into())));
        let bare = ";e move ['northeast','southeast'][rand(2)] while checkpaths == [ 'ne', 'se', \
                    'sw', 'nw' ]";
        assert_eq!(steps(bare, 1).len(), 1);
    }

    #[test]
    fn wandering_ends_when_the_room_looks_right() {
        let script = ";e move checkpaths[rand(checkpaths.length)] until checkpaths.include?('out')";
        assert_eq!(
            actions(script, 1),
            [Action::WanderWhile(not(Cond::Exit("out".into())))]
        );
        let script = ";e move 'southeast'; move 'northeast'; move \
                      checkpaths[rand(checkpaths.length)] while checkpaths.length > 2";
        assert_eq!(
            actions(script, 1),
            [
                Action::Moves(vec!["southeast".into(), "northeast".into()]),
                Action::WanderWhile(Cond::ExitsOver(2)),
            ]
        );
        let script = ";e walk until checkloot.include?('path'); move 'go path'";
        assert_eq!(
            actions(script, 1)[0],
            Action::WanderWhile(not(Cond::Sees("path".into())))
        );
    }

    #[test]
    fn a_round_of_commands_repeats_while_the_exit_is_listed() {
        let script = ";e while checkpaths.include?('nw'); fput 'search'; sleep 1; waitrt?; move \
                      'go crevice'; end";
        assert_eq!(
            actions(script, 1),
            [Action::RoundWhile(
                vec!["search".into(), "go crevice".into()],
                Cond::Exit("nw".into())
            )]
        );
        // A statement that is not a command is not read past.
        assert_eq!(actions(&script.replace("sleep 1", "exit"), 1), []);
    }

    #[test]
    fn a_detour_is_one_guarded_step() {
        let script = ";e move 'north'; unless Room.current.id == 5869 then move 'south'; move \
                      'south'; move 'south'; end";
        let found = steps(script, 5869);
        assert_eq!(found[0].action, Action::Move("north".into()));
        assert_eq!(
            found[1].action,
            Action::Moves(vec!["south".into(), "south".into(), "south".into()])
        );
        assert_eq!(found[1].when, Some(not(Cond::At(5869))));
        assert_eq!(steps(script, 5870), [], "it names another room");
    }

    #[test]
    fn the_second_answer_does_not_follow_the_first_ones_moves() {
        let script = ";e move 'down'; move 'out'; if checkpaths.include?('ne'); move \
                      'northeast'; move 'east'; elsif !checkpaths.include?('e'); move 'west'; end";
        let found = steps(script, 11675);
        assert_eq!(found.len(), 3);
        assert_eq!(found[2].action, Action::Move("west".into()));
        let Some(Cond::All(parts)) = &found[2].when else {
            panic!("an all");
        };
        assert_eq!(parts[0], not(Cond::At(11675)));
    }

    #[test]
    fn what_opens_the_way_is_skipped_when_it_shows() {
        let script = ";e unless checkloot.include? 'stairs'; fput 'turn stalactite'; fput 'turn \
                      stalagmite'; end; move 'go stair'";
        let found = steps(script, 1);
        assert_eq!(
            found[0].action,
            Action::Round(vec!["turn stalactite".into(), "turn stalagmite".into()])
        );
        assert_eq!(found[0].when, Some(not(Cond::Sees("stairs".into()))));

        let script = ";e if !GameObj.loot.any?{|i| i.name =~ /opening/};fput 'pull lever';sleep \
                      0.2 until GameObj.loot.any?{|i| i.name =~ /opening/};end;move('go opening')";
        assert_eq!(
            actions(script, 1),
            [
                Action::Round(vec!["pull lever".into()]),
                Action::WaitUntil(Cond::Sees("opening".into())),
                Action::Move("go opening".into()),
            ]
        );
        // Looking for one thing and waiting for another is not this shape.
        assert_eq!(actions(&script.replacen("opening", "door", 1), 1), []);
    }

    #[test]
    fn a_search_goes_on_while_the_way_is_not_to_be_seen() {
        let script = ";e until GameObj.loot.find {|o| o.noun=='gap'};fput \
                      'search';waitrt?;end;move 'go gap';";
        assert_eq!(
            actions(script, 1),
            [
                Action::PutWhile("search".into(), not(Cond::Sees("gap".into()))),
                Action::Move("go gap".into()),
            ]
        );
        let script = ";e wait_until{GameObj.loot.find{|item| item.noun == \"island\"}};fput \"go \
                      island\"";
        assert_eq!(
            actions(script, 1)[0],
            Action::WaitUntil(Cond::Sees("island".into()))
        );
    }
}
