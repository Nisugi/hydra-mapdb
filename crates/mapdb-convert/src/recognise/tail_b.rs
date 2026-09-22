//! Arms for slice b of the long tail (`research/mapdb-inventory/tail/slice_b.tsv`).
//!
//! Same rules as every arm (`super`): the upstream script verbatim, with
//! holes; an exact match or none. This file's tests live at its foot.

use cena_map::{Action, Cond, Crossing, Step};

use super::moves::{cast_clause, cast_if_able};
use super::{always, is_plain_argument, quoted};

/// The steps for a crossing script in this slice, if an arm here knows it.
pub(super) fn crossing(script: &str, from: u32, to: u32) -> Option<Crossing> {
    walk_or_swim(script)
        .or_else(|| until_the_room_changes(script, from, to))
        .or_else(|| plain_statements(script))
}

const WATER_WALKING: &str = "Water Walking"; // 112

/// Walk across if Water Walking is up, swim otherwise: 143 exits in three
/// spellings. The third (2) empties the hands for the swim only.
///
/// The swim is always `swim` plus the walk's own direction; an arm that saw
/// anything else refuses.
fn walk_or_swim(script: &str) -> Option<Crossing> {
    const BARE: [[&str; 3]; 2] = [
        // 113
        [
            ";e if checkspell(112) then move '",
            "' else move '",
            "' end; waitrt?",
        ],
        // 28
        [
            ";e if Spell[112].active?; move '",
            "';else; move '",
            "';end",
        ],
    ];
    const HANDS_FREE: [&str; 3] = [
        ";e if checkspell(112); move '",
        "'; else; empty_hands; move '",
        "'; fill_hands; end; waitrt?",
    ];
    let (found, hands_free) = match BARE.iter().find_map(|form| quoted(script, form)) {
        Some(found) => (found, false),
        None => (quoted(script, &HANDS_FREE)?, true),
    };
    let [walk, swim] = found[..] else {
        return None;
    };
    (is_plain_argument(walk) && swim.strip_prefix("swim ") == Some(walk)).then_some(())?;
    let walking = || Cond::SpellActive(WATER_WALKING.to_owned());
    let swimming = |action| Step {
        action,
        when: Some(Cond::Otherwise(Box::new(walking()))),
    };
    let mut steps = vec![Step {
        action: Action::Move(walk.to_owned()),
        when: Some(walking()),
    }];
    if hands_free {
        steps.push(swimming(Action::EmptyHands));
    }
    steps.push(swimming(Action::Move(swim.to_owned())));
    if hands_free {
        steps.push(swimming(Action::FillHands));
    }
    Some(Crossing::Steps(steps))
}

/// One command, sent again until the walker is somewhere else: 8 exits.
/// `Action::KeepMoving`, not `Move`: a `Move` gives an exit up after a few
/// tries, and these are written to be tried until they work.
///
/// Each spelling names a room; it must be the one the exit leaves, or the one
/// it reaches, as that spelling means it. Five end `$go2_restart=true`: where
/// they land is not known, so they end in `Action::Replan`.
fn until_the_room_changes(script: &str, from: u32, to: u32) -> Option<Crossing> {
    let leaving = quoted(script, &[";e move '", "' while Room.current.id == ", ""]);
    let reaching = quoted(
        script,
        &[
            ";e loop{fput '",
            "';pause 0.2;if Room.current.id == ",
            ";break;end}",
        ],
    );
    let (command, replan) = if let Some(found) = leaving.or(reaching.clone()) {
        let [command, room] = found[..] else {
            return None;
        };
        let named = if reaching.is_some() { to } else { from };
        ((room.parse() == Ok(named)).then_some(command)?, false)
    } else {
        let found = quoted(
            script,
            &[
                ";e id=Room.current.id;move '",
                "' until Room.current.id != id;$go2_restart=true",
            ],
        )?;
        (*found.first()?, true)
    };
    is_plain_argument(command).then_some(())?;
    let mut steps = vec![always(Action::KeepMoving(command.to_owned()))];
    if replan {
        steps.push(always(Action::Replan));
    }
    Some(Crossing::Steps(steps))
}

/// One statement of a straight-line script, as upstream spells it.
enum Statement<'s> {
    Fput(&'s str),
    Move(&'s str),
    Step(Action),
    /// `waitrt?`: nothing for the walker to do, `Action::Move` waits already.
    Nothing,
}

/// The statement at the head of `text`, and what follows it.
fn statement(text: &str) -> Option<(Statement<'_>, &str)> {
    const OPENINGS: [(&str, bool, bool); 11] = [
        // (spelling, is a move, closes with a bracket)
        ("fput '", false, false),
        ("fput \"", false, false),
        ("fput\"", false, false),
        ("fput('", false, true),
        ("fput(\"", false, true),
        ("move '", true, false),
        ("move \"", true, false),
        ("move('", true, true),
        ("move(\"", true, true),
        ("move ('", true, true),
        ("move (\"", true, true),
    ];
    // Standing when already standing costs one refused command; asking first
    // would need a guard the map has no word for.
    const STAND: &str = "fput 'stand' unless standing?";
    const BARE: [(&str, Option<Action>); 8] = [
        ("waitrt?", None),
        ("waitrt", None),
        ("$go2_restart = true", Some(Action::Replan)),
        ("$go2_restart=true", Some(Action::Replan)),
        ("sleep 0.5", Some(Action::Pause(500))),
        ("sleep 1", Some(Action::Pause(1000))),
        ("empty_hands", Some(Action::EmptyHands)),
        ("fill_hands", Some(Action::FillHands)),
    ];
    if let Some(rest) = text.strip_prefix(STAND) {
        return Some((Statement::Step(Action::Put("stand".to_owned())), rest));
    }
    for (word, action) in BARE {
        if let Some(rest) = text.strip_prefix(word) {
            return Some((action.map_or(Statement::Nothing, Statement::Step), rest));
        }
    }
    let (opening, moves, bracket) = OPENINGS
        .iter()
        .find(|(opening, _, _)| text.starts_with(opening))?;
    let quote = opening.chars().last()?;
    let (command, rest) = text.strip_prefix(opening)?.split_once(quote)?;
    is_plain_argument(command).then_some(())?;
    let rest = if *bracket {
        rest.strip_prefix(')')?
    } else {
        rest
    };
    let found = if *moves {
        Statement::Move(command)
    } else {
        Statement::Fput(command)
    };
    Some((found, rest))
}

/// A straight line of `fput`, `move`, `sleep`, `waitrt?` and the hands, with
/// nothing decided along the way: some 85 exits in three dozen spellings
/// (`fput 'search';waitrt?;move ('go black arch')`), too many to list one
/// template each. Every statement is one of a fixed list, spelled exactly, and
/// separated by exactly `;`, `; ` or a newline.
///
/// A `move` is a move and an `fput` beside one is sent where the walker
/// stands. With no `move` at all, the **last** `fput` is what changes rooms
/// (`fput 'get rope';fput 'push rope'`) -- the exit leads somewhere, and
/// nothing else in the script could take the walker there.
///
/// May open with casting Celerity (506) or Sigil of Resolve (9704) if able.
fn plain_statements(script: &str) -> Option<Crossing> {
    let body = script.strip_prefix(";e ")?;
    let mut steps = Vec::new();
    let mut rest = body;
    for (variable, number, spell) in [
        ("celerity", 506, "Celerity"),
        ("resolve", 9704, "Sigil of Resolve"),
    ] {
        if let Some(after) = body.strip_prefix(cast_clause(variable, number).as_str()) {
            steps.push(cast_if_able(spell));
            rest = after;
        }
    }
    let mut statements = Vec::new();
    while !rest.is_empty() {
        let (found, after) = statement(rest)?;
        // `$go2_restart` only after the move whose landing it doubts.
        if rest.starts_with('$') && !matches!(statements.last(), Some(Statement::Move(_))) {
            return None;
        }
        statements.push(found);
        rest = ["; ", ";", "\n"]
            .iter()
            .find_map(|separator| after.strip_prefix(separator))
            .or_else(|| after.is_empty().then_some(after))?;
    }
    let has_move = statements
        .iter()
        .any(|found| matches!(found, Statement::Move(_)));
    let last_fput = statements
        .iter()
        .rposition(|found| matches!(found, Statement::Fput(_)));
    let lines = statements.len();
    let mut moved = false;
    for (index, found) in statements.into_iter().enumerate() {
        let action = match found {
            Statement::Move(command) => Action::Move(command.to_owned()),
            Statement::Fput(command) if !has_move && Some(index) == last_fput => {
                Action::Move(command.to_owned())
            }
            // After the move, only tidying up: anything else sent from the
            // far side is not a way of crossing.
            Statement::Fput(command) if moved && !["stand", "close door"].contains(&command) => {
                return None;
            }
            Statement::Fput(command) => Action::Put(command.to_owned()),
            Statement::Step(action) => action,
            Statement::Nothing => continue,
        };
        moved |= matches!(action, Action::Move(_));
        steps.push(always(action));
    }
    // `Replan` is the end of a crossing or it is not in it (`cena_map::step`).
    let replans = steps.iter().position(|step| step.action == Action::Replan);
    if replans.is_some_and(|at| at + 1 != steps.len()) {
        return None;
    }
    // A lone `move 'x'` belongs to `plain_move`; this arm is for lines.
    (moved && lines > 1).then_some(Crossing::Steps(steps))
}

#[cfg(test)]
mod tests {
    use cena_map::{Action, Cond, Crossing, Step, moves_whatever_is_known};

    use super::crossing;

    fn steps(script: &str, from: u32, to: u32) -> Vec<Step> {
        match crossing(script, from, to) {
            Some(Crossing::Steps(steps)) => {
                assert!(moves_whatever_is_known(&steps), "{script}");
                steps
            }
            _ => Vec::new(),
        }
    }

    fn actions(script: &str) -> Vec<Action> {
        steps(script, 1, 2)
            .into_iter()
            .map(|step| {
                assert_eq!(step.when, None);
                step.action
            })
            .collect()
    }

    fn put(command: &str) -> Action {
        Action::Put(command.into())
    }

    fn go(command: &str) -> Action {
        Action::Move(command.into())
    }

    #[test]
    fn water_is_walked_when_water_walking_is_up_and_swum_otherwise() {
        let up = Cond::SpellActive("Water Walking".into());
        let down = Cond::Otherwise(Box::new(up.clone()));
        let step = |action, when: &Cond| Step {
            action,
            when: Some(when.clone()),
        };
        let bare = vec![step(go("west"), &up), step(go("swim west"), &down)];
        assert_eq!(
            steps(
                ";e if checkspell(112) then move 'west' else move 'swim west' end; waitrt?",
                1,
                2
            ),
            bare
        );
        assert_eq!(
            steps(
                ";e if Spell[112].active?; move 'west';else; move 'swim west';end",
                1,
                2
            ),
            bare
        );
        assert_eq!(
            steps(
                ";e if checkspell(112); move 'north'; else; empty_hands; move 'swim north'; \
                 fill_hands; end; waitrt?",
                1,
                2
            ),
            vec![
                step(go("north"), &up),
                step(Action::EmptyHands, &down),
                step(go("swim north"), &down),
                step(Action::FillHands, &down),
            ]
        );
        // Another spell, and a swim that is not the walk's direction.
        for near in [
            ";e if checkspell(113) then move 'west' else move 'swim west' end; waitrt?",
            ";e if checkspell(112) then move 'west' else move 'swim east' end; waitrt?",
            ";e if checkspell(112) then move 'west' else move 'swim west' end; waitrt?; x",
        ] {
            assert_eq!(crossing(near, 1, 2), None, "{near}");
        }
    }

    #[test]
    fn a_command_repeated_until_the_room_changes_keeps_moving() {
        let always = |action| Step { action, when: None };
        let south = vec![always(Action::KeepMoving("south".into()))];
        let leaving = ";e move 'south' while Room.current.id == 3035";
        assert_eq!(steps(leaving, 3035, 3034), south);
        assert_eq!(crossing(leaving, 3036, 3034), None);
        let reaching = ";e loop{fput 'south';pause 0.2;if Room.current.id == 11069;break;end}";
        assert_eq!(steps(reaching, 11068, 11069), south);
        assert_eq!(crossing(reaching, 11069, 11068), None);
        let anywhere =
            ";e id=Room.current.id;move \"south\" until Room.current.id != id;$go2_restart=true";
        // Where it lands is not known, so it ends by planning again.
        assert_eq!(
            steps(anywhere, 1, 2),
            vec![
                always(Action::KeepMoving("south".into())),
                always(Action::Replan)
            ]
        );
        // `until` a named room may walk through others: not a retry.
        assert_eq!(
            crossing(";e move 'south' until Room.current.id == 5867", 5868, 5867),
            None
        );
    }

    #[test]
    fn a_line_of_fputs_and_moves_keeps_its_order() {
        assert_eq!(
            actions(";e fput 'search';move 'go path';move 'northeast';move 'northeast'"),
            [
                put("search"),
                go("go path"),
                go("northeast"),
                go("northeast")
            ]
        );
        assert_eq!(
            actions(";e fput 'search';waitrt?;move ('go black arch')"),
            [put("search"), go("go black arch")]
        );
        assert_eq!(
            actions(";e fput 'open hut'; waitrt?; move 'go hut';"),
            [put("open hut"), go("go hut")]
        );
        assert_eq!(
            actions(";e fput 'search';sleep 0.5;waitrt?; fput 'open grate'; move 'go grate'"),
            [
                put("search"),
                Action::Pause(500),
                put("open grate"),
                go("go grate")
            ]
        );
        assert_eq!(
            actions(";e move 'jump lava'; sleep 1; waitrt?"),
            [go("jump lava"), Action::Pause(1000)]
        );
        assert_eq!(
            actions(";e move 'climb dock'\nfill_hands"),
            [go("climb dock"), Action::FillHands]
        );
        assert_eq!(
            actions(";e move 'go door'; fput 'close door'"),
            [go("go door"), put("close door")]
        );
        assert_eq!(
            actions(";e move \"knock wall\"; fput\"stand\""),
            [go("knock wall"), put("stand")]
        );
        assert_eq!(
            actions(";e move 'swim opening'\nfput 'stand' unless standing?"),
            [go("swim opening"), put("stand")]
        );
        assert_eq!(
            actions(";e move 'go ring'; $go2_restart=true"),
            [go("go ring"), Action::Replan]
        );
        // A replan is the end of a crossing, or the script is not this shape.
        assert_eq!(
            actions(";e move 'go ring'; $go2_restart=true; move 'north'"),
            []
        );
    }

    #[test]
    fn with_no_move_the_last_fput_is_the_move() {
        assert_eq!(
            actions(";e fput \"get rope\";fput \"push rope\""),
            [put("get rope"), go("push rope")]
        );
        assert_eq!(
            actions(";e fput('get chain'); fput('push chain')"),
            [put("get chain"), go("push chain")]
        );
        assert_eq!(
            actions(";e fput 'lean wall';fput 'knock wall';fput 'knock wall';fput 'knock wall'"),
            [
                put("lean wall"),
                put("knock wall"),
                put("knock wall"),
                go("knock wall")
            ]
        );
    }

    #[test]
    fn a_line_may_open_with_a_cast() {
        let found = steps(
            ";e if celerity = Spell[506] and celerity.known? and celerity.affordable? and not \
             celerity.active?; celerity.cast; end; fput 'search'; waitrt?; move 'go crack'; \
             waitrt?",
            1,
            2,
        );
        let [cast, search, crack] = &found[..] else {
            panic!("{found:?}");
        };
        assert_eq!(cast.action, Action::Cast("Celerity".into()));
        assert!(cast.when.is_some());
        assert_eq!(
            (&search.action, &crack.action),
            (&put("search"), &go("go crack"))
        );

        let found = steps(
            ";e if resolve = Spell[9704] and resolve.known? and resolve.affordable? and not \
             resolve.active?; resolve.cast; end; empty_hands; move 'go water'; waitrt?",
            1,
            2,
        );
        let kinds: Vec<&Action> = found.iter().map(|step| &step.action).collect();
        assert_eq!(
            kinds,
            [
                &Action::Cast("Sigil of Resolve".into()),
                &Action::EmptyHands,
                &go("go water")
            ]
        );
    }

    #[test]
    fn a_line_that_decides_anything_is_not_a_line() {
        for near in [
            // A lone move is `plain_move`'s; a lone fput moves nothing known.
            ";e move 'west'",
            ";e fput 'west'",
            // Decisions, loops, a second statement inside a quote.
            ";e move 'out'; move 'north' if checkpaths.include?('n')",
            ";e move 'south' until Room.current.id == 5867",
            ";e fput 'search'; move 'go gap'; fput 'quit' if dead?",
            ";e move 'go gap'; fput 'quit'",
            ";e fput \"say lalk gno'ap renqua to!'\";move ('go arch');",
            // A doubted landing with no move before it; an unknown spacing.
            ";e fput 'go path';fput 'go path';$go2_restart = true",
            ";e fput 'search' ;move 'go gap'",
            ";e fput 'search';;move 'go gap'",
            ";e sleep 2; move 'go gap'",
        ] {
            assert_eq!(crossing(near, 1, 2), None, "{near}");
        }
    }
}
