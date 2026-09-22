//! Arms for slice a of the long tail (`research/mapdb-inventory/tail/slice_a.tsv`).
//!
//! Same rules as every arm (`super`): the upstream script verbatim, with
//! holes; an exact match or none. This file's tests live at its foot.

use cena_map::{Action, Cond, Crossing, Step};

use super::moves::cast_if_able;
use super::{always, holes, is_plain_argument, quoted};

/// The steps for a crossing script in this slice, if an arm here knows it.
pub(super) fn crossing(script: &str, from: u32, to: u32) -> Option<Crossing> {
    let _ = (from, to);
    lone_fput(script)
        .or_else(|| fputs_then_move(script))
        .or_else(|| fputs_between_roundtimes(script))
        .or_else(|| fput_then_fput(script))
        .or_else(|| repeated_then_move(script))
        .or_else(|| unhide_then_move(script))
        .or_else(|| stand_then_move(script))
        .or_else(|| put_move_put(script))
        .or_else(|| hands_free(script))
        .or_else(|| hands_free_climbs(script))
        .or_else(|| boulder(script))
        .or_else(|| with_a_pause(script))
        .or_else(|| cart(script))
        .or_else(|| caravan(script))
        .or_else(|| current(script))
        .or_else(|| hatch(script))
}

fn put(command: &str) -> Step {
    always(Action::Put(command.to_owned()))
}

fn go(command: &str) -> Step {
    always(Action::Move(command.to_owned()))
}

/// Every hole a quoted command, or nothing.
fn plain(found: Vec<&str>) -> Option<Vec<&str>> {
    found
        .iter()
        .all(|hole| is_plain_argument(hole))
        .then_some(found)
}

/// Every command but the last sent where the walker stands; the last moves it.
fn puts_then_go(commands: &[&str]) -> Option<Crossing> {
    let (last, before) = commands.split_last()?;
    let mut steps: Vec<Step> = before.iter().map(|command| put(command)).collect();
    steps.push(go(last));
    Some(Crossing::Steps(steps))
}

/// `sleep`/`pause` in upstream's seconds, as milliseconds. Only the values
/// this slice uses: a table cannot misread a number the way a parser can.
fn millis(seconds: &str) -> Option<u32> {
    match seconds {
        "0.5" => Some(500),
        "1" => Some(1000),
        _ => None,
    }
}

/// `fput 'go hut'` and nothing else: 29 exits whose one command changes
/// rooms, sent with `fput` where `move` would have done.
fn lone_fput(script: &str) -> Option<Crossing> {
    let found = plain(quoted(script, &[";e fput '", "'"])?)?;
    puts_then_go(&found)
}

/// `fput 'unlock door'; fput 'open door'; move 'go door'`: one to seven
/// commands, then the move. 12 exits in four spellings of the separators.
fn fputs_then_move(script: &str) -> Option<Crossing> {
    const STARTS: [&str; 2] = [";e fput '", ";e  fput '"];
    const ENDS: [&str; 3] = ["'; move '", "';move '", "' ; move '"];
    (1..=7).find_map(|puts| {
        STARTS.iter().find_map(|start| {
            ENDS.iter().find_map(|end| {
                let mut parts = vec![*start];
                parts.extend(std::iter::repeat_n("'; fput '", puts - 1));
                parts.extend([*end, "'"]);
                // A hole that swallowed a whole `fput` holds a quote, and is
                // refused: only the right count matches.
                puts_then_go(&plain(quoted(script, &parts)?)?)
            })
        })
    })
}

/// The same on separate lines with `waitrt?` between: 2 exits. Waiting out
/// roundtime is the walker's business before it sends anything.
fn fputs_between_roundtimes(script: &str) -> Option<Crossing> {
    const FORMS: [&[&str]; 2] = [
        &[";e fput '", "'\nwaitrt?\nfput '", "'\nmove '", "'"],
        &[
            ";e fput '",
            "'\nwaitrt?\nfput '",
            "'\nwaitrt?\nfput '",
            "'\nmove '",
            "'",
        ],
    ];
    let found = FORMS.iter().find_map(|form| plain(quoted(script, form)?))?;
    puts_then_go(&found)
}

/// `fput 'search'; fput 'go crevice'`: the second `fput` is the move. 3 exits.
fn fput_then_fput(script: &str) -> Option<Crossing> {
    const FORMS: [[&str; 3]; 2] = [
        [";e fput '", "'; fput '", "'"],
        [";e fput '", "'; waitrt?; fput '", "'"],
    ];
    let found = FORMS.iter().find_map(|form| quoted(script, form))?;
    puts_then_go(&plain(found)?)
}

/// `3.times { fput 'knock ash door' };move 'go ash door'`: knock, dig or ask
/// a fixed number of times, then go. 5 exits. Where there is nothing after the
/// loop, its last send is the move (as `super::moves::event_transport`).
fn repeated_then_move(script: &str) -> Option<Crossing> {
    const THEN: [[&str; 4]; 3] = [
        [";e ", ".times { fput '", "' };move '", "'"],
        [";e ", ".times{fput '", "'};move '", "'"],
        [";e ", ".times{fput '", "'};fput '", "'"],
    ];
    let count = |times: &str| {
        times
            .parse::<usize>()
            .ok()
            .filter(|times| (1..=5).contains(times))
    };
    if let Some(found) = THEN.iter().find_map(|form| holes(script, form)) {
        let [times, knock, then] = found[..] else {
            return None;
        };
        plain(vec![knock, then])?;
        let mut steps = vec![put(knock); count(times)?];
        steps.push(go(then));
        return Some(Crossing::Steps(steps));
    }
    let [times, ask] = holes(script, &[";e ", ".times{fput '", "';}"])?[..] else {
        return None;
    };
    plain(vec![ask])?;
    let mut steps = vec![put(ask); count(times)? - 1];
    steps.push(go(ask));
    Some(Crossing::Steps(steps))
}

/// A gate that will not pass the unseen: `unhide` first. 6 exits. `invisible?`
/// is the status the game reports; `checkspell` asks after the spell itself.
fn unhide_then_move(script: &str) -> Option<Crossing> {
    const SPELL: &str = "Invisibility"; // 916
    const FORMS: [(&[&str], bool); 3] = [
        (&[";e fput 'unhide' if invisible?;move('", "')"], false),
        (&[";e fput 'unhide' if checkspell(916); move '", "'"], true),
        (
            &[";e fput 'unhide' if checkspell 'invisibility'\nmove '", "'"],
            true,
        ),
    ];
    let (found, spell) = FORMS
        .iter()
        .find_map(|(form, spell)| Some((holes(script, form)?, *spell)))?;
    let [command] = plain(found)?[..] else {
        return None;
    };
    let when = if spell {
        Cond::SpellActive(SPELL.to_owned())
    } else {
        Cond::Flag("invisible".to_owned())
    };
    Some(Crossing::Steps(vec![
        Step {
            action: Action::Put("unhide".to_owned()),
            when: Some(when),
        },
        go(command),
    ]))
}

/// `fput 'stand' unless standing?; move 'out'`: standing up is part of what
/// `Action::Move` means, so this is one move. 3 exits.
fn stand_then_move(script: &str) -> Option<Crossing> {
    const FORMS: [[&str; 2]; 2] = [
        [";e fput 'stand' unless standing?; move '", "'"],
        [";e fput 'stand' until standing?;move '", "'"],
    ];
    let found = FORMS.iter().find_map(|form| quoted(script, form))?;
    puts_then_go(&plain(found)?)
}

/// `fput 'kneel'; move 'go burrow'; fput 'stand'`. 2 exits.
fn put_move_put(script: &str) -> Option<Crossing> {
    let found = plain(holes(
        script,
        &[";e fput '", "'; move '", "'; fput '", "'"],
    )?)?;
    let [before, command, after] = found[..] else {
        return None;
    };
    Some(Crossing::Steps(vec![put(before), go(command), put(after)]))
}

/// Hands emptied, then the move, in the spellings `super::moves` does not
/// have. 12 exits.
///
/// `empty_hand` (one hand) is ported as `EmptyHands` (both): a walker with
/// both hands free has one free, and the walk refills whatever was stowed.
/// `empty_hands if <something is held>` is `EmptyHands`, which does nothing
/// to empty hands.
fn hands_free(script: &str) -> Option<Crossing> {
    // (template, refills, a `fput` comes before the move)
    const FORMS: [(&[&str], bool, bool); 9] = [
        (&[";e empty_hand;move('", "')"], false, false),
        (&[";e empty_hands\nmove '", "'"], false, false),
        (&[";e empty_hands;move '", "'"], false, false),
        (&[";e empty_hands;move \"", "\""], false, false),
        (&[";e empty_hands;move '", "';fill_hands"], true, false),
        (
            &[
                ";e empty_hands if GameObj.right_hand.id or GameObj.left_hand.id;move('",
                "')",
            ],
            false,
            false,
        ),
        (
            &[";e empty_hand; fput '", "'; move '", "'; fill_hand"],
            true,
            true,
        ),
        (&[";e empty_hand;fput('", "');move('", "')"], false, true),
        (&[";e empty_hands;fput \"", "\";move \"", "\""], false, true),
    ];
    let (found, refill) = FORMS.iter().find_map(|(form, refill, before)| {
        let found = plain(holes(script, form)?)?;
        (found.len() == 1 + usize::from(*before)).then_some((found, *refill))
    })?;
    let (command, before) = found.split_last()?;
    let mut steps = vec![always(Action::EmptyHands)];
    steps.extend(before.iter().map(|command| put(command)));
    steps.push(go(command));
    if refill {
        steps.push(always(Action::FillHands));
    }
    Some(Crossing::Steps(steps))
}

/// `empty_hands; 3.times { move 'climb wall' }; fill_hands`: each climb
/// reaches another room. 2 exits.
fn hands_free_climbs(script: &str) -> Option<Crossing> {
    let found = holes(
        script,
        &[";e empty_hands; ", ".times { move '", "' }; fill_hands"],
    )?;
    let [times, command] = found[..] else {
        return None;
    };
    let times: usize = times.parse().ok().filter(|times| (1..=5).contains(times))?;
    plain(vec![command])?;
    let mut steps = vec![always(Action::EmptyHands)];
    steps.extend(vec![go(command); times]);
    steps.push(always(Action::FillHands));
    Some(Crossing::Steps(steps))
}

/// A boulder climbed with Resolve if able, hands free, and in offensive
/// stance when the walker climbs badly. Upstream leaves it in defensive
/// stance whatever it arrived in, and so does this. 2 exits.
fn boulder(script: &str) -> Option<Crossing> {
    const BEFORE: &str = ";e Spell[9704].cast if Spell[9704].known? and !Spell[9704].active? \
        and Spell[9704].affordable?; empty_hands; fput 'stance offensive' if \
        Skills.climbing < 20; move '";
    const AFTER: &str = "'; waitrt?; fput 'stance defensive'; fill_hands";
    let [command] = plain(holes(script, &[BEFORE, AFTER])?)?[..] else {
        return None;
    };
    Some(Crossing::Steps(vec![
        cast_if_able("Sigil of Resolve"),
        always(Action::EmptyHands),
        Step {
            action: Action::Put("stance offensive".to_owned()),
            when: Some(Cond::SkillUnder("climbing".to_owned(), 20)),
        },
        go(command),
        put("stance defensive"),
        always(Action::FillHands),
    ]))
}

/// A beat before or after the move. 4 exits in three shapes.
fn with_a_pause(script: &str) -> Option<Crossing> {
    if let Some(found) = holes(script, &[";e fput '", "'; pause ", "; waitrt?"]) {
        let [command, seconds] = found[..] else {
            return None;
        };
        plain(vec![command])?;
        return Some(Crossing::Steps(vec![
            go(command),
            always(Action::Pause(millis(seconds)?)),
        ]));
    }
    if let Some(found) = holes(
        script,
        &[";e fput '", "'; move '", "'; sleep ", "; waitrt?"],
    ) {
        let [first, command, seconds] = found[..] else {
            return None;
        };
        plain(vec![first, command])?;
        return Some(Crossing::Steps(vec![
            put(first),
            go(command),
            always(Action::Pause(millis(seconds)?)),
        ]));
    }
    let found = holes(
        script,
        &[";e fput '", "'; sleep ", ";waitrt?; move('", "')"],
    )?;
    let [first, seconds, command] = found[..] else {
        return None;
    };
    plain(vec![first, command])?;
    Some(Crossing::Steps(vec![
        put(first),
        always(Action::Pause(millis(seconds)?)),
        go(command),
    ]))
}

/// The mine carts: buy a ticket or a pass, and wait out the ride. 6 exits.
fn cart(script: &str) -> Option<Crossing> {
    const ARRIVED: &str = "You hastily exit the cart";
    let after = format!("\"\nwaitfor \"{ARRIVED}\"");
    let [buy] = plain(holes(script, &[";e fput \"", &after])?)?[..] else {
        return None;
    };
    Some(Crossing::Steps(vec![
        put(buy),
        always(Action::Await(ARRIVED.to_owned())),
    ]))
}

/// The caravan to the Hinterwilds, remembering which town it left from so the
/// way back can be priced. 1 exit. Upstream writes the memory first; here it
/// is written once the caravan has arrived (`Action::Remember`).
fn caravan(script: &str) -> Option<Crossing> {
    const ARRIVED: &str = "With a groan and a creak, the caravan comes to a stop.";
    let after = format!("';waitfor '{ARRIVED}'");
    let found = holes(
        script,
        &[
            ";e UserVars.mapdb_hinterwilds_location = '",
            "';multifput '",
            "','",
            "','",
            &after,
        ],
    )?;
    let [town, inquire, order, confirm] = plain(found)?[..] else {
        return None;
    };
    Some(Crossing::Steps(vec![
        put(inquire),
        put(order),
        put(confirm),
        always(Action::Await(ARRIVED.to_owned())),
        always(Action::Remember(
            "hinterwilds_location".to_owned(),
            town.to_owned(),
        )),
    ]))
}

/// A river that carries the walker on by itself. 1 exit.
fn current(script: &str) -> Option<Crossing> {
    (script
        == ";e echo \"Waiting for current to carry you to the new room...\";\
            line = get until line =~ /sturdy ladder/")
        .then(|| Crossing::Steps(vec![always(Action::Await("sturdy ladder".to_owned()))]))
}

/// Three rooms out to rub the blood, three back to rub the hatch. 1 exit.
fn hatch(script: &str) -> Option<Crossing> {
    (script
        == ";e ['west','west','northwest'].each{|d| move(d)};fput 'rub blood';\
            ['southeast','east','east'].each{|d| move(d)};fput 'rub hatch';move('go hatch');")
        .then(|| {
            Crossing::Steps(vec![
                go("west"),
                go("west"),
                go("northwest"),
                put("rub blood"),
                go("southeast"),
                go("east"),
                go("east"),
                put("rub hatch"),
                go("go hatch"),
            ])
        })
}

#[cfg(test)]
mod tests {
    use cena_map::moves_whatever_is_known;

    use super::*;

    fn steps(script: &str) -> Option<Vec<Step>> {
        match crossing(script, 1, 2)? {
            Crossing::Steps(steps) => {
                assert!(moves_whatever_is_known(&steps), "{script}");
                Some(steps)
            }
            _ => None,
        }
    }

    /// Steps in shorthand, `|` between: `p:` put, `m:` move, `a:` await,
    /// `w:` pause in ms, `e` empty hands, `f` fill hands.
    fn seq(text: &str) -> Vec<Step> {
        text.split('|')
            .filter_map(|step| {
                let (kind, rest) = step.split_once(':').unwrap_or((step, ""));
                Some(always(match kind {
                    "p" => Action::Put(rest.to_owned()),
                    "m" => Action::Move(rest.to_owned()),
                    "a" => Action::Await(rest.to_owned()),
                    "w" => Action::Pause(rest.parse().ok()?),
                    "e" => Action::EmptyHands,
                    "f" => Action::FillHands,
                    _ => return None,
                }))
            })
            .collect()
    }

    fn ported(script: &str, expected: &str) {
        assert_eq!(steps(script), Some(seq(expected)), "{script}");
    }

    fn refused(script: &str) {
        assert_eq!(steps(script), None, "{script}");
    }

    #[test]
    fn a_lone_fput_is_the_move() {
        ported(";e fput 'go hut'", "m:go hut");
        ported(";e fput \"go gang\"", "m:go gang");
        refused(";e fput 'go hut'; exit");
        refused(";e fput 'go hut\"");
    }

    #[test]
    fn fputs_then_a_move() {
        ported(
            ";e fput 'pull bin'; fput 'open trapdoor'; move 'go trapdoor'",
            "p:pull bin|p:open trapdoor|m:go trapdoor",
        );
        ported(
            ";e fput 'turn candelabra'; fput 'pull black book';move 'go door'",
            "p:turn candelabra|p:pull black book|m:go door",
        );
        ported(
            ";e  fput 'search'; move 'go trapdoor'",
            "p:search|m:go trapdoor",
        );
        ported(
            ";e fput 'look barrel' ; move 'go chute'",
            "p:look barrel|m:go chute",
        );
        ported(
            ";e fput 'push gargoyl'; fput 'push letter i'; fput 'push letter w'; \
             fput 'push letter n'; fput 'push letter b'; fput 'push letter t'; \
             fput 'push letter c'; move 'go door'",
            "p:push gargoyl|p:push letter i|p:push letter w|p:push letter n|p:push letter b|\
             p:push letter t|p:push letter c|m:go door",
        );
        refused(";e fput 'a'; fput 'b' if hidden?; move 'c'");
        refused(";e fput 'a'; fput 'b'; move 'c'; exit");
    }

    #[test]
    fn fputs_with_roundtime_between() {
        ported(
            ";e fput 'search'\nwaitrt?\nfput 'open trapdoor'\nmove 'go trapdoor'",
            "p:search|p:open trapdoor|m:go trapdoor",
        );
        ported(
            ";e fput \"search\"\nwaitrt?\nfput \"search\"\nwaitrt?\nfput \"search\"\n\
             move \"go torch-lit passageway\"",
            "p:search|p:search|p:search|m:go torch-lit passageway",
        );
        refused(";e fput 'crawl crack'\nwaitrt?\nfput 'stand' unless standing?");
    }

    #[test]
    fn the_second_fput_moves() {
        ported(
            ";e fput 'search'; fput 'go crevice'",
            "p:search|m:go crevice",
        );
        ported(
            ";e fput 'search'; waitrt?; fput 'go gash'",
            "p:search|m:go gash",
        );
        refused(";e fput 'search'; fput 'go crevice' if hidden?");
    }

    #[test]
    fn repeated_then_the_move() {
        ported(
            ";e 3.times { fput 'knock ash door' };move 'go ash door'",
            "p:knock ash door|p:knock ash door|p:knock ash door|m:go ash door",
        );
        ported(
            ";e 3.times{fput 'knock door'};fput 'go door'",
            "p:knock door|p:knock door|p:knock door|m:go door",
        );
        ported(
            ";e 2.times{fput 'dig opening'};move 'go opening'",
            "p:dig opening|p:dig opening|m:go opening",
        );
        ported(
            ";e 2.times{fput 'ask sailor about boat';}",
            "p:ask sailor about boat|m:ask sailor about boat",
        );
        refused(";e 50.times{fput 'dig opening'};move 'go opening'");
        refused(";e 0.times{fput 'ask sailor about boat';}");
    }

    #[test]
    fn unhide_when_unseen() {
        let expected = |when, command| {
            let unhide = Step {
                action: Action::Put("unhide".to_owned()),
                when: Some(when),
            };
            Some(vec![unhide, go(command)])
        };
        let spell = || Cond::SpellActive("Invisibility".to_owned());
        assert_eq!(
            steps(";e fput 'unhide' if invisible?;move('go gate')"),
            expected(Cond::Flag("invisible".to_owned()), "go gate")
        );
        assert_eq!(
            steps(";e fput 'unhide' if checkspell(916); move 'go gate'"),
            expected(spell(), "go gate")
        );
        assert_eq!(
            steps(";e fput 'unhide' if checkspell 'invisibility'\nmove 'go rope'"),
            expected(spell(), "go rope")
        );
        refused(";e fput 'unhide' if checkspell(917); move 'go gate'");
    }

    #[test]
    fn standing_is_part_of_the_move() {
        ported(";e fput 'stand' unless standing?; move 'out'", "m:out");
        ported(";e fput \"stand\" until standing?;move \"out\"", "m:out");
        refused(";e fput 'stand' unless standing?;move('jump'); $go2_restart=true");
    }

    #[test]
    fn kneel_go_stand() {
        ported(
            ";e fput 'kneel'; move 'go burrow'; fput 'stand'",
            "p:kneel|m:go burrow|p:stand",
        );
        refused(";e fput 'kneel'; move 'go burrow'; fput 'stand' unless standing?");
    }

    #[test]
    fn hands_emptied_first() {
        ported(";e empty_hand;move('go deep water')", "e|m:go deep water");
        ported(";e empty_hands\nmove 'climb pathway'", "e|m:climb pathway");
        ported(";e empty_hands;move \"go gondolas\"", "e|m:go gondolas");
        ported(
            ";e empty_hands;move 'go slope';fill_hands",
            "e|m:go slope|f",
        );
        ported(
            ";e empty_hands if GameObj.right_hand.id or GameObj.left_hand.id;move('go opening')",
            "e|m:go opening",
        );
        ported(
            ";e empty_hand; fput 'turn ring'; move 'go hole'; fill_hand",
            "e|p:turn ring|m:go hole|f",
        );
        ported(
            ";e empty_hand;fput('go river');move('go river')",
            "e|p:go river|m:go river",
        );
        ported(
            ";e empty_hands;fput \"stance offensive\";move \"climb rope ladder\"",
            "e|p:stance offensive|m:climb rope ladder",
        );
        refused(";e empty_hands\nfput 'unhide' if checkspell 'invisibility'\nmove 'go river'");
        refused(";e empty_hands; 50.times { move 'climb wall' }; fill_hands");
        ported(
            ";e empty_hands; 2.times { move 'climb wall' }; fill_hands",
            "e|m:climb wall|m:climb wall|f",
        );
    }

    #[test]
    fn the_boulder() {
        let script = ";e Spell[9704].cast if Spell[9704].known? and !Spell[9704].active? and \
             Spell[9704].affordable?; empty_hands; fput 'stance offensive' if \
             Skills.climbing < 20; move 'climb boulder'; waitrt?; fput 'stance defensive'; \
             fill_hands";
        let mut expected = seq("e|p:stance offensive|m:climb boulder|p:stance defensive|f");
        expected.insert(0, cast_if_able("Sigil of Resolve"));
        if let Some(stance) = expected.get_mut(2) {
            stance.when = Some(Cond::SkillUnder("climbing".to_owned(), 20));
        }
        assert_eq!(steps(script), Some(expected));
        refused(&script.replace("< 20", "< 30"));
    }

    #[test]
    fn pauses() {
        ported(
            ";e fput 'go fissure'; pause 0.5; waitrt?",
            "m:go fissure|w:500",
        );
        ported(
            ";e fput 'search wall'; move 'swim crevice'; sleep 1; waitrt?",
            "p:search wall|m:swim crevice|w:1000",
        );
        ported(
            ";e fput 'search'; sleep 0.5;waitrt?; move('go mouth')",
            "p:search|w:500|m:go mouth",
        );
        refused(";e fput 'go fissure'; pause 0.7; waitrt?");
    }

    #[test]
    fn rides() {
        ported(
            ";e fput \"buy pass\"\nwaitfor \"You hastily exit the cart\"",
            "p:buy pass|a:You hastily exit the cart",
        );
        refused(";e fput \"buy pass\"\nwaitfor \"You exit the cart\"");

        let mut expected = seq("p:inquire|p:order 1|p:order confirm|\
             a:With a groan and a creak, the caravan comes to a stop.");
        expected.push(always(Action::Remember(
            "hinterwilds_location".to_owned(),
            "EN".to_owned(),
        )));
        assert_eq!(
            steps(
                ";e UserVars.mapdb_hinterwilds_location = 'EN';multifput 'inquire','order 1',\
                 'order confirm';waitfor 'With a groan and a creak, the caravan comes to a stop.'"
            ),
            Some(expected)
        );
        // The Icemule wagon waits for any of three lines, which `Await` cannot say.
        refused(
            ";e UserVars.mapdb_hinterwilds_location = 'IM';multifput 'inquire','order 3',\
             'order confirm';waitfor 'The wagon comes to a halt','The wagon draws to an \
             abrupt halt','the caravan comes to a stop';",
        );

        ported(
            ";e echo \"Waiting for current to carry you to the new room...\";line = get \
             until line =~ /sturdy ladder/",
            "a:sturdy ladder",
        );
        refused(";e line = get until line =~ /sturdy ladder/");
    }

    #[test]
    fn the_hatch() {
        ported(
            ";e ['west','west','northwest'].each{|d| move(d)};fput 'rub blood';\
             ['southeast','east','east'].each{|d| move(d)};fput 'rub hatch';move('go hatch');",
            "m:west|m:west|m:northwest|p:rub blood|m:southeast|m:east|m:east|p:rub hatch|\
             m:go hatch",
        );
        refused(";e ['west','west'].each{|d| move(d)};fput 'rub hatch';move('go hatch');");
    }
}
