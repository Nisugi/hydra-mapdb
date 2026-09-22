//! Arms whose crossing is a list of steps: moves, and what is done around them.

use cena_map::{Action, Cond, Crossing, Step};

use super::{always, holes, is_plain_argument, is_word, quoted};

/// `move 'X'`, written as a script where a plain command would have done, and
/// the same with a trailing `waitrt?` -- waiting out roundtime is part of what
/// `Action::Move` means, so it adds nothing.
pub(super) fn plain_move(script: &str) -> Option<Crossing> {
    const ENDINGS: [&str; 5] = ["'", "';", "'; waitrt?", "';waitrt", "'; waitrt"];
    let found = ENDINGS
        .iter()
        .find_map(|ending| quoted(script, &[";e move '", ending]))
        .or_else(|| quoted(script, &[";e move('", "')"]))
        // A command that changes rooms, sent with `fput` after a beat.
        .or_else(|| quoted(script, &[";e pause 0.2; waitrt?; fput '", "'"]))
        // `push south` until the game says it worked or cannot: a move, and
        // retrying it is what `Action::Move` already does.
        .or_else(|| {
            quoted(
                script,
                &[
                    ";e dothistimeout '",
                    "',5,/you push|you can't push/i;waitrt?",
                ],
            )
        })?;
    let command = found
        .first()
        .copied()
        .filter(|hole| is_plain_argument(hole))?;
    Some(Crossing::Steps(vec![always(Action::Move(
        command.to_owned(),
    ))]))
}

/// `fput 'open gate'; move 'go gate'`.
pub(super) fn put_then_move(script: &str) -> Option<Crossing> {
    const FORMS: [[&str; 3]; 4] = [
        [";e fput '", "'; move '", "'"],
        [";e fput '", "';move '", "'"],
        [";e fput '", "'\nmove '", "'"],
        [";e fput '", "';move('", "')"],
    ];
    let found = FORMS.iter().find_map(|form| quoted(script, form))?;
    let [first, then] = found[..] else {
        return None;
    };
    if !is_plain_argument(first) || !is_plain_argument(then) {
        return None;
    }
    Some(Crossing::Steps(vec![
        always(Action::Put(first.to_owned())),
        always(Action::Move(then.to_owned())),
    ]))
}

/// `multifput 'put 1 coin in almsbox', 'go pillar'`: every command but the
/// last is sent where the walker stands, and the last one moves it.
pub(super) fn puts_then_move(script: &str) -> Option<Crossing> {
    let sent = script.strip_prefix(";e multifput ")?;
    let sent = sent.strip_suffix("; waitrt?;").unwrap_or(sent);
    let mut commands = quoted_list(sent)?;
    let last = commands.pop()?;
    (!commands.is_empty()).then_some(())?;
    let mut steps: Vec<Step> = commands
        .into_iter()
        .map(|command| always(Action::Put(command.to_owned())))
        .collect();
    steps.push(always(Action::Move(last.to_owned())));
    Some(Crossing::Steps(steps))
}

/// `'a', "b"` -- quoted either way, separated by commas -- as its parts.
pub(super) fn quoted_list(text: &str) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    let mut rest = text.trim();
    loop {
        let quote = rest.chars().next().filter(|c| matches!(c, '\'' | '"'))?;
        let (part, after) = rest.get(1..)?.split_once(quote)?;
        is_plain_argument(part).then_some(())?;
        parts.push(part);
        rest = after.trim_start();
        match rest.strip_prefix(',') {
            Some(more) => rest = more.trim_start(),
            None => return rest.is_empty().then_some(parts),
        }
    }
}

/// The way out of an event ground: `move('go wagon');UserVars.mapdb_x = nil;`.
pub(super) fn move_and_forget(script: &str) -> Option<Crossing> {
    let found = quoted(script, &[";e move('", "');UserVars.mapdb_", " = nil;"])?;
    let [command, name] = found[..] else {
        return None;
    };
    (is_plain_argument(command) && is_word(name)).then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::Move(command.to_owned())),
        always(Action::Forget(name.to_owned())),
    ]))
}

/// `empty_hands; move 'climb wall'; fill_hands`, six ways. Four end without
/// `fill_hands`; they are ported as written, because the walk refills the
/// hands before it ends whatever the steps say (`Action::EmptyHands`).
pub(super) fn hands_free_move(script: &str) -> Option<Crossing> {
    const REFILLING: [[&str; 2]; 4] = [
        [";e empty_hands\nmove '", "'\nwaitrt?\nfill_hands"],
        [";e empty_hands; move '", "'; fill_hands"],
        [";e empty_hands\nmove '", "'\nfill_hands"],
        [";e empty_hands; move '", "'; waitrt?; fill_hands"],
    ];
    const NOT_REFILLING: [[&str; 2]; 2] = [
        [";e empty_hands; move '", "'; waitrt?"],
        [";e empty_hands; move '", "'"],
    ];
    let find = |forms: &[[&str; 2]]| forms.iter().find_map(|form| quoted(script, form));
    let (found, refill) = match find(&REFILLING) {
        Some(found) => (found, true),
        None => (find(&NOT_REFILLING)?, false),
    };
    let command = found
        .first()
        .copied()
        .filter(|hole| is_plain_argument(hole))?;
    let mut steps = vec![
        always(Action::EmptyHands),
        always(Action::Move(command.to_owned())),
    ];
    if refill {
        steps.push(always(Action::FillHands));
    }
    Some(Crossing::Steps(steps))
}

/// An inn's tables: `go Cat's Paw table`, and again if someone seated there
/// answers with an invitation instead of the walker simply heading over.
///
/// Ported as one move. **Sending it again on an invitation is a reaction of
/// the move itself** -- like opening a door that turned out to be closed --
/// and belongs with the walker's other reactions, not in 478 copies here.
pub(super) fn inn_table(script: &str) -> Option<Crossing> {
    const REST: &str = "\"; fput \"go #{table} table\" if dothistimeout(\"go #{table} table\", 25, \
        /You (?:and your group )?head over to|waves.*you.*(?:invites|inviting) you\
        (?: and your group)? to (?:join|come sit at)/) =~ /inviting you|invites you/";
    let [table] = holes(script, &[";e table = \"", REST])?[..] else {
        return None;
    };
    (!table.is_empty() && !table.contains(['"', ';', '\n', '#'])).then_some(())?;
    Some(Crossing::Steps(vec![always(Action::Move(format!(
        "go {table} table"
    )))]))
}

/// `2.times{fput "event transport duskruin"};UserVars.mapdb_duskruin_origin = 7;`
///
/// The game asks once and goes on the second asking, so the last send is the
/// move. What is remembered is always the room being left: upstream writes
/// its id, or `Map.current.id`, and an arm that saw anything else refuses.
pub(super) fn event_transport(script: &str, from: u32) -> Option<Crossing> {
    let found = holes(
        script,
        &[";e ", ".times{fput \"", "\"};UserVars.mapdb_", " = ", ";"],
    )?;
    let [times, command, name, value] = found[..] else {
        return None;
    };
    let times: usize = times.parse().ok().filter(|times| (1..=3).contains(times))?;
    if !is_plain_argument(command) || !is_word(name) {
        return None;
    }
    if value != "Map.current.id" && value.parse() != Ok(from) {
        return None;
    }
    let mut steps = vec![always(Action::Put(command.to_owned())); times - 1];
    steps.push(always(Action::Move(command.to_owned())));
    steps.push(always(Action::Remember(name.to_owned(), from.to_string())));
    Some(Crossing::Steps(steps))
}

/// A portmaster's ship: ask, ask again to confirm, and wait out the voyage.
pub(super) fn portmaster(script: &str) -> Option<Crossing> {
    const LANDED: &str = "A crew member escorts you off the ship.";
    let [ask, again] = holes(
        script,
        &[";e multifput '", "','", &format!("';waitfor '{LANDED}'")],
    )?[..] else {
        return None;
    };
    (ask == again && is_plain_argument(ask) && ask.starts_with("ask portmaster about travel "))
        .then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::Put(ask.to_owned())),
        always(Action::Put(ask.to_owned())),
        always(Action::Await(LANDED.to_owned())),
    ]))
}

/// Upstream names spells by number here; the map names them, as the icy paths
/// do. The numbers are checked against `cena-model/data/spells.tsv`.
const RESOLVE: &str = "Sigil of Resolve"; // 9704

const WATER_WALKING: &str = "Water Walking"; // 112

/// Cast `spell` if the walker knows it, can pay for it, and has not got it up.
pub(super) fn cast_if_able(spell: &str) -> Step {
    let named = || spell.to_owned();
    Step {
        action: Action::Cast(named()),
        when: Some(Cond::All(vec![
            Cond::SpellKnown(named()),
            Cond::SpellAffordable(named()),
            Cond::Not(Box::new(Cond::SpellActive(named()))),
        ])),
    }
}

/// `upstream's name for a spell, tested and cast` -- the text of one
/// `if x = Spell[n] and x.known? and x.affordable? and not x.active?; x.cast; end`.
pub(super) fn cast_clause(variable: &str, number: u32) -> String {
    format!(
        "if {variable} = Spell[{number}] and {variable}.known? and {variable}.affordable? \
         and not {variable}.active?; {variable}.cast; end; "
    )
}

/// A hard climb: Resolve if able, then the move. 26 exits, on the roads to the
/// Nations.
pub(super) fn resolve_then_move(script: &str) -> Option<Crossing> {
    let before = format!(";e {}move '", cast_clause("resolve", 9704));
    let [command] = holes(script, &[&before, "'; waitrt?"])?[..] else {
        return None;
    };
    is_plain_argument(command).then_some(())?;
    Some(Crossing::Steps(vec![
        cast_if_able(RESOLVE),
        always(Action::Move(command.to_owned())),
    ]))
}

/// The arctic waters: Resolve and Water Walking if able, then walk across if
/// Water Walking is up and swim if it is not -- a question asked *after* the
/// cast, which is why a step's guard is asked when the step is reached.
pub(super) fn arctic_waters(script: &str) -> Option<Crossing> {
    let before = format!(
        ";e {}{}fput (Spell[112].active? ? 'go ",
        cast_clause("resolve", 9704),
        cast_clause("waterwalking", 112)
    );
    let [walk, swim] = holes(script, &[&before, "' : 'swim ", "')"])?[..] else {
        return None;
    };
    (walk == swim && is_word(walk)).then_some(())?;
    let walking = || Cond::SpellActive(WATER_WALKING.to_owned());
    Some(Crossing::Steps(vec![
        cast_if_able(RESOLVE),
        cast_if_able(WATER_WALKING),
        Step {
            action: Action::Move(format!("go {walk}")),
            when: Some(walking()),
        },
        Step {
            action: Action::Move(format!("swim {swim}")),
            when: Some(Cond::Otherwise(Box::new(walking()))),
        },
    ]))
}

/// The icy paths: 171 exits in two upstream shapes.
///
/// **What upstream does**, and where this departs from it. The common shape
/// (150) waits four seconds when the walker is heavy or unskilled and not
/// hasted. The other (21, the glacier) waits six on a simpler test, and
/// otherwise casts Sigil of Resolve if it can.
///
/// **One profile setting, `ice_mode`: `run`, `wait` or `auto`** -- go2's
/// values, kept by the author's choice. `run` does nothing special: no cast,
/// no wait, just the move. `wait` always waits. `auto` waits when the shape's
/// own test says the walker is likely to slip.
///
/// **The author's rule: all 171 cast Sigil of Resolve when it is known and
/// affordable -- unless `ice_mode` is `run`.** So both shapes become the same three steps -- cast,
/// wait, move -- each keeping its own wait and its own test for it. The cast
/// comes first and does not depend on the wait: Resolve is what makes the
/// crossing likelier to succeed either way.
///
/// Dropped: the `echo`, which talks to a Lich user; and the glacier shape's
/// reaction to `Rushing heedlessly` (cast Haste, stand, replan) -- a fall is
/// the `move` step's to recover from, as it is on any other exit.
///
/// **A guard that cannot be answered does not fire** (`cena_map::cond`), so a
/// walker whose skills are unknown neither casts nor waits. That errs toward a
/// fall, which the walk recovers from, rather than toward refusing a route.
pub(super) fn icy_path(script: &str) -> Option<Crossing> {
    const TRAIL: &str = ";e if (UserVars.mapdb_ice_mode == 'wait') or \
        ((UserVars.mapdb_ice_mode != 'run') and ((XMLData.encumbrance_value > 50) or \
        ((Skills.survival < 50) and not Spell['Haste'].active?))); \
        sleep 0.2; echo 'trying not to slip...'; sleep 4; end; move '";
    const GLACIER: &str = ";e \n\t\tresolve=Spell['Sigil of Resolve']\n\t\thaste=Spell['Haste']\n\t\t\
        if UserVars.mapdb_ice_mode == 'wait' || Skills.survival < 50 || \
        XMLData.encumbrance_value >= 50\n\t\t\techo 'trying not to slip...'; sleep 6\n\t\t\
        elsif resolve.known? && resolve.affordable? && !resolve.active?\n\t\t\tresolve.cast\n\t\t\
        end\n\t\tresult = fput '";
    const GLACIER_AFTER: &str = "'\n\t\tif result =~ /^Rushing heedlessly/\n\t\t\t\
        haste.cast if haste.known? && haste.affordable? && !haste.active?\n\t\t\t\
        fput 'stand'\n\t\t\t$go2_restart = true\n\t\tend\n\t";

    // go2's own words, kept (author, 2026-09-20: "more informative" than
    // off/on). The third value, `auto`, is never tested for: it is what is left.
    const RUN: &str = "run";
    const WAIT: &str = "wait";
    let setting = |value: &str| Cond::Setting("ice_mode".to_owned(), value.to_owned());
    let unskilled = Cond::SkillUnder("survival".to_owned(), 50);
    let (direction, pause, slippery) = if let Some(found) = holes(script, &[TRAIL, "'"]) {
        let heavy_or_slow = Cond::Any(vec![
            Cond::EncumbranceOver(50),
            Cond::All(vec![
                unskilled,
                Cond::Not(Box::new(Cond::SpellActive("Haste".to_owned()))),
            ]),
        ]);
        let unless_running = Cond::All(vec![Cond::Not(Box::new(setting(RUN))), heavy_or_slow]);
        let slippery = Cond::Any(vec![setting(WAIT), unless_running]);
        (*found.first()?, 4200, slippery)
    } else {
        let found = holes(script, &[GLACIER, GLACIER_AFTER])?;
        // `>= 50` upstream, and whole percents: over 49.
        let slippery = Cond::Any(vec![setting(WAIT), unskilled, Cond::EncumbranceOver(49)]);
        (*found.first()?, 6000, slippery)
    };
    if !is_plain_argument(direction) {
        return None;
    }

    let resolve = || "Sigil of Resolve".to_owned();
    let can_cast = Cond::All(vec![
        Cond::Not(Box::new(setting(RUN))),
        Cond::SpellKnown(resolve()),
        Cond::SpellAffordable(resolve()),
        Cond::Not(Box::new(Cond::SpellActive(resolve()))),
    ]);
    let step = |action, when| Step { action, when };
    Some(Crossing::Steps(vec![
        step(Action::Cast(resolve()), Some(can_cast)),
        step(Action::Pause(pause), Some(slippery)),
        step(Action::Move(direction.to_owned()), None),
    ]))
}

/// The pedal boats: `pedal west` until the boat is somewhere else. 173 exits,
/// two spacings. It takes several pedals to cross one room and none of them
/// fails, which is why this is `KeepMoving` and not `Move`.
pub(super) fn pedal_boat(script: &str) -> Option<Crossing> {
    const FORMS: [[&str; 2]; 2] = [
        [
            ";e direction=\"",
            "\";start=Room.current.id; dothistimeout \"pedal #{direction}\", 2, /pedal/ \
             while Room.current.id == start",
        ],
        [
            ";e direction=\"",
            "\";start=Room.current.id;dothistimeout \"pedal #{direction}\", 2, /pedal/ \
             while Room.current.id == start",
        ],
    ];
    let found = FORMS.iter().find_map(|form| holes(script, form))?;
    let direction = found.first().copied().filter(|hole| is_word(hole))?;
    Some(Crossing::Steps(vec![always(Action::KeepMoving(format!(
        "pedal {direction}"
    )))]))
}

/// One command, sent until the walker is at the exit's own destination: 68
/// exits, five spellings. 60 are one forest, `go forest` up to fifty times.
/// The room upstream names must be the room the exit reaches.
pub(super) fn until_there(script: &str, to: u32) -> Option<Crossing> {
    const FORMS: [[&str; 3]; 5] = [
        [
            ";e 50.times { move '",
            "'; break if Room.current.id == ",
            " }",
        ],
        [";e move '", "' until Room.current.id == ", ""],
        [";e fput '", "' until Room.current.id == ", ""],
        [";e begin\nmove '", "'\nend until Room.current.id == ", ""],
        [
            ";e begin\nfput '",
            "'\nwaitrt?\nend until Room.current.id == ",
            "",
        ],
    ];
    let found = FORMS.iter().find_map(|form| quoted(script, form))?;
    let [command, room] = found[..] else {
        return None;
    };
    (is_plain_argument(command) && room.parse() == Ok(to)).then_some(())?;
    Some(Crossing::Steps(vec![always(Action::MoveUntilThere(
        command.to_owned(),
    ))]))
}
