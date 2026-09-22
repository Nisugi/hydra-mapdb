//! Arms for puzzles that turn out to need no routine: their state is the
//! room the walker is in, which a step's guard can ask about.

use cena_map::{Action, Cond, Crossing, Step};

use super::looking::commands;
use super::moves::quoted_list;
use super::{always, holes, is_plain_argument};

pub(super) fn crossing(script: &str, to: u32) -> Option<Crossing> {
    spike_trap(script)
        .or_else(|| roots(script, to))
        .or_else(|| cell_door(script))
        .map(Crossing::Steps)
}

fn when(action: Action, cond: Cond) -> Step {
    Step {
        action,
        when: Some(cond),
    }
}

/// Room 7046: work the spike until the floor gives way, be knocked down
/// somewhere else, stand. 5 exits. **Where the trap drops the walker is
/// random.** Upstream hard-codes a walk from each landing to this exit's
/// destination; finding out where it is and planning again does the same
/// with the map it already has. The walks are read, to know the shape, and
/// dropped.
fn spike_trap(script: &str) -> Option<Vec<Step>> {
    let [walks] = holes(
        script,
        &[
            ";e loop { fput 'search'; fput 'turn bent spike'; sleep 0.2; break unless checkpaths \
             == [ 'out' ] }; wait_while { standing? }; fput 'stand';",
            "",
        ],
    )?[..] else {
        return None;
    };
    for walk in walks.strip_suffix("; end")?.split("; end;") {
        let (room, moves) = walk
            .strip_prefix(" if Room.current == Room[")?
            .split_once("]; ")?;
        room.parse::<u32>().ok()?;
        commands(moves)?;
    }
    Some(vec![
        always(Action::RoundWhile(
            vec!["search".to_owned(), "turn bent spike".to_owned()],
            Cond::ExitsAre(vec!["out".to_owned()]),
        )),
        always(Action::WaitUntil(Cond::Not(Box::new(Cond::Posture(
            "standing".to_owned(),
        ))))),
        always(Action::Put("stand".to_owned())),
        always(Action::Replan),
    ])
}

/// Maaghara's roots: `go root` drops the walker onto the refuse heap, when
/// the root is awake. When it is not, each of five rooms has a walk to
/// another of them, to try again from. 5 exits, one script.
///
/// Upstream loops; here one try, one walk, and planning again -- which comes
/// back to this exit from wherever the walk ended.
fn roots(script: &str, to: u32) -> Option<Vec<Step>> {
    let found = holes(
        script,
        &[
            ";e next_exit = { ",
            " }; loop { if move '",
            "'; sleep(0.5) until XMLData.room_title == '[Maaghara Tower, Refuse Heap]'; fput \
             'stand' unless standing?; waitrt?; break; else; if dir_list = \
             next_exit[Room.current.id]; dir_list.each { |dir| move dir }; else; echo 'error: out \
             of cheese'; break; end; end }",
        ],
    )?;
    let [table, root] = found[..] else {
        return None;
    };
    let mut steps = vec![
        always(Action::TryMove(
            is_plain_argument(root).then(|| root.to_owned())?,
        )),
        // The fall takes a moment; the heap is where it ends.
        when(
            Action::WaitUntil(Cond::At(to)),
            Cond::Not(Box::new(Cond::StillHere)),
        ),
    ];
    for entry in table.strip_suffix(" ]")?.split(" ], ") {
        let (at, dirs) = entry.split_once(" => [ ")?;
        let dirs = quoted_list(dirs)?.into_iter().map(str::to_owned).collect();
        steps.push(when(Action::Moves(dirs), Cond::At(at.parse().ok()?)));
    }
    steps.push(always(Action::Replan));
    Some(steps)
}

/// The cell doors: through a ruined one, or batter it until it is. 5 exits,
/// four of them by calling room 18700's script. Upstream refuses a walker
/// with nothing in its right hand, and gives up on `ineffective`; here the
/// last move simply fails, as any blocked move does -- what is in a hand
/// when the walk is planned says little about when it gets there.
fn cell_door(script: &str) -> Option<Vec<Step>> {
    const SCRIPT: &str = include_str!("../upstream_scripts/cell_door.rb");
    (script == SCRIPT || script == ";e Map[18700].wayto['18250'].call;").then(|| {
        vec![
            always(Action::TryMove("go door".to_owned())),
            when(
                Action::PutUntil {
                    command: "batter door".to_owned(),
                    until: vec!["destroyed".to_owned(), "ineffective".to_owned()],
                    tries: None,
                },
                Cond::StillHere,
            ),
            when(Action::Move("go door".to_owned()), Cond::StillHere),
        ]
    })
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

    const TRAP: &str = ";e loop { fput 'search'; fput 'turn bent spike'; sleep 0.2; break unless \
                        checkpaths == [ 'out' ] }; wait_while { standing? }; fput 'stand'; if \
                        Room.current == Room[7144]; move 'go door'; move 'go hole'; end; if \
                        Room.current == Room[7083]; move 'go space'; end";

    #[test]
    fn the_trap_ends_by_finding_out_where_it_dropped_the_walker() {
        let found = steps(TRAP, 1);
        assert_eq!(found.len(), 4);
        assert_eq!(found[3].action, Action::Replan);
        // A landing that does something other than walk is not read past.
        assert_eq!(steps(&TRAP.replace("move 'go space'", "exit"), 1), []);
    }

    #[test]
    fn a_sleeping_root_means_a_walk_and_another_try() {
        let script = ";e next_exit = { 9823 => [ 'southeast', 'south' ], 9788 => [ 'southwest' ] \
                      }; loop { if move 'go root'; sleep(0.5) until XMLData.room_title == \
                      '[Maaghara Tower, Refuse Heap]'; fput 'stand' unless standing?; waitrt?; \
                      break; else; if dir_list = next_exit[Room.current.id]; dir_list.each { \
                      |dir| move dir }; else; echo 'error: out of cheese'; break; end; end }";
        let found = steps(script, 9734);
        assert_eq!(found.len(), 5);
        assert_eq!(found[2].when, Some(Cond::At(9823)));
        assert_eq!(
            found[3].action,
            Action::Moves(vec!["southwest".into()]),
            "one move or several, it is one step"
        );
    }

    #[test]
    fn the_cell_doors_are_one_script_called_from_four_places() {
        let called = steps(";e Map[18700].wayto['18250'].call;", 1);
        assert_eq!(called.len(), 3);
        assert_eq!(steps(";e Map[18700].wayto['18251'].call;", 1), []);
    }
}
