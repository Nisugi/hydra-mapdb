//! Arms for slice g of the long tail, round two
//! (`research/mapdb-inventory/tail/slice_g.tsv`).
//!
//! Same rules as every arm (`super`): the upstream script verbatim, with
//! holes; an exact match or none. This file's tests live at its foot.

use cena_map::{Action, Cond, Crossing, Step};

use super::{always, holes, is_plain_argument, quoted};

/// The steps for a crossing script in this slice, if an arm here knows it.
pub(super) fn crossing(script: &str, from: u32, to: u32) -> Option<Crossing> {
    let _ = to;
    silverwood_out(script)
        .or_else(|| fog(script))
        .or_else(|| asked_twice(script))
        .or_else(|| lands_badly(script, from))
        .or_else(|| niche(script, from))
        .or_else(|| splinter(script))
        .or_else(|| hands_free_climbs(script))
}

fn moved(command: &str) -> Option<Step> {
    is_plain_argument(command).then(|| always(Action::Move(command.to_owned())))
}

fn put(command: &str) -> Step {
    always(Action::Put(command.to_owned()))
}

/// What is left to do when the first try did not move the walker.
fn if_still_here(action: Action) -> Step {
    Step {
        action,
        when: Some(Cond::StillHere),
    }
}

/// The one door out of Silverwood Manor, which leads back to whichever town
/// the walker came in from: forget the town, and replan. 4 exits. Upstream
/// clears the global after asking for the restart; `Replan` is always last.
fn silverwood_out(script: &str) -> Option<Crossing> {
    let found = quoted(
        script,
        &[";e move '", "';$go2_restart=true;$SILVERWOOD_TOWN=nil"],
    )?;
    let [command] = found[..] else {
        return None;
    };
    Some(Crossing::Steps(vec![
        moved(command)?,
        always(Action::Forget("silverwood_town".to_owned())),
        always(Action::Replan),
    ]))
}

/// The Red Forest's fog, for the far sides `super::tail_c` does not name:
/// `go fog` until the room's paths are the other side's. 5 exits. The paths
/// differ by exit and are written three times; all three must agree. One
/// also forgets where in the forest the walker was.
fn fog(script: &str) -> Option<Crossing> {
    const TURNED: &str = "You attempt to navigate your way through the fog, but get turned \
        around and come right back out where you started!";
    const FORGET: &str = ";UserVars.mapdb_redforest_location = nil; $go2_restart=true";
    let (body, forget) = match script.strip_suffix(FORGET) {
        Some(body) => (body, true),
        None => (script, false),
    };
    let turned = format!(";if result =~ /{TURNED}/;sleep 0.5;waitrt?;end;end");
    let asked = format!(
        "/;fput \"stand\" until standing?;result = dothistimeout \"go fog\", 5, /{TURNED}|"
    );
    let found = holes(
        body,
        &[";e result = nil;until result =~ /", &asked, "/", &turned],
    )?;
    let [first, second, nothing] = found[..] else {
        return None;
    };
    let paths = first.strip_prefix("Obvious paths: ")?;
    let listed = !paths.is_empty()
        && paths
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b == b',' || b == b' ');
    (listed && first == second && nothing.is_empty()).then_some(())?;
    let mut steps = vec![always(Action::KeepMoving("go fog".to_owned()))];
    if forget {
        steps.push(always(Action::Forget("redforest_location".to_owned())));
        steps.push(always(Action::Replan));
    }
    Some(Crossing::Steps(steps))
}

/// `put 'go field';put 'go field'`: the tug-of-war fields, entered by asking
/// twice. 4 exits in three spellings. Which send moves the walker is not
/// written down, so the first is tried and the second made only if it is
/// still where it was.
fn asked_twice(script: &str) -> Option<Crossing> {
    let found = quoted(script, &[";e put '", "';put '", "'"])
        .or_else(|| quoted(script, &[";e put '", "'; put '", "'"]))?;
    let [first, again] = found[..] else {
        return None;
    };
    (first == again && is_plain_argument(first)).then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::TryMove(first.to_owned())),
        if_still_here(Action::Move(again.to_owned())),
    ]))
}

/// A jump and a crawl that may stun the walker and leave it lying down. 2
/// exits. Standing is sent unguarded, as `super::tail_c` does after a crawl;
/// waiting out the stun is left to whatever sends the next command.
fn lands_badly(script: &str, from: u32) -> Option<Crossing> {
    if let Some(found) = quoted(
        script,
        &[
            ";e move '",
            "'; waitrt?; wait_while { stunned? }; fput 'stand' unless standing?; waitrt?",
        ],
    ) {
        let [command] = found[..] else {
            return None;
        };
        return Some(Crossing::Steps(vec![moved(command)?, put("stand")]));
    }
    // Sent until it answers or the room changes: `KeepMoving`.
    let found = holes(
        script,
        &[
            ";e result = nil;while result.nil? && Room.current.id == ",
            ";result = dothistimeout \"",
            "\",3,/^You crawl under the low overhang/;end;wait_while{stunned?};\
             fput \"stand\" until standing?",
        ],
    )?;
    let [room, command] = found[..] else {
        return None;
    };
    (room.parse() == Ok(from) && is_plain_argument(command)).then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::KeepMoving(command.to_owned())),
        put("stand"),
    ]))
}

/// A niche too dark to cross until the eyes adjust: try it; if still here,
/// wait for the line that says they have, and go. 1 exit.
fn niche(script: &str, from: u32) -> Option<Crossing> {
    let found = holes(
        script,
        &[
            ";e move '",
            "';while Room.current.id == ",
            ";line = get?;if line.nil?;sleep 0.2;elsif line == '",
            "';move '",
            "';break;end;end;",
        ],
    )?;
    let [first, room, line, again] = found[..] else {
        return None;
    };
    let said = !line.is_empty() && !line.contains(['\'', ';', '\n']);
    (first == again && room.parse() == Ok(from) && said).then_some(())?;
    is_plain_argument(first).then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::TryMove(first.to_owned())),
        if_still_here(Action::Await(line.to_owned())),
        if_still_here(Action::Move(again.to_owned())),
    ]))
}

/// Sit, pull the splinter, and stand up wherever that drops the walker. 1
/// exit. The pull is taken to be what changes rooms: it is the last thing
/// sent before the walker has to stand again.
fn splinter(script: &str) -> Option<Crossing> {
    let found = holes(
        script,
        &[
            ";e multifput('",
            "', '",
            "'); waitrt?; fput('stand'); waitrt?",
        ],
    )?;
    let [sit, pull] = found[..] else {
        return None;
    };
    is_plain_argument(sit).then_some(())?;
    Some(Crossing::Steps(vec![put(sit), moved(pull)?, put("stand")]))
}

/// `waitrt?; empty_hands; 4.times { move 'climb wall' }; fill_hands`:
/// `super::tail_a`'s climbs with a roundtime waited out first, which
/// `Action::Move` already means. 1 exit.
fn hands_free_climbs(script: &str) -> Option<Crossing> {
    let found = holes(
        script,
        &[
            ";e waitrt?; empty_hands; ",
            ".times { move '",
            "' }; fill_hands",
        ],
    )?;
    let [times, command] = found[..] else {
        return None;
    };
    let times: usize = times.parse().ok().filter(|times| (1..=5).contains(times))?;
    let mut steps = vec![always(Action::EmptyHands)];
    steps.extend(vec![moved(command)?; times]);
    steps.push(always(Action::FillHands));
    Some(Crossing::Steps(steps))
}

#[cfg(test)]
mod tests {
    use cena_map::moves_whatever_is_known;

    use super::*;

    fn steps(script: &str, from: u32) -> Option<Vec<Step>> {
        match crossing(script, from, 0)? {
            Crossing::Steps(steps) => {
                assert!(moves_whatever_is_known(&steps), "{script}");
                Some(steps)
            }
            _ => None,
        }
    }

    fn mv(command: &str) -> Step {
        always(Action::Move(command.to_owned()))
    }

    #[test]
    fn leaving_silverwood_forgets_the_town_and_replans_last() {
        let script = ";e move 'go door';$go2_restart=true;$SILVERWOOD_TOWN=nil";
        assert_eq!(
            steps(script, 13532),
            Some(vec![
                mv("go door"),
                always(Action::Forget("silverwood_town".into())),
                always(Action::Replan),
            ])
        );
        // Setting the town is another script.
        assert_eq!(steps(";e $SILVERWOOD_TOWN=:wl;move 'go door'", 1), None);
    }

    fn fog_script(first: &str, second: &str) -> String {
        format!(
            ";e result = nil;until result =~ /{first}/;fput \"stand\" until standing?;\
             result = dothistimeout \"go fog\", 5, /You attempt to navigate your way through \
             the fog, but get turned around and come right back out where you started!|\
             {second}/;if result =~ /You attempt to navigate your way through the fog, but get \
             turned around and come right back out where you started!/;sleep 0.5;waitrt?;end;end"
        )
    }

    #[test]
    fn the_fog_is_kept_at_until_it_lets_go() {
        let keep = always(Action::KeepMoving("go fog".into()));
        let paths = "Obvious paths: southwest, northwest";
        assert_eq!(
            steps(&fog_script(paths, paths), 24696),
            Some(vec![keep.clone()])
        );
        let out = format!(
            "{};UserVars.mapdb_redforest_location = nil; $go2_restart=true",
            fog_script("Obvious paths: northwest", "Obvious paths: northwest")
        );
        assert_eq!(
            steps(&out, 24675),
            Some(vec![
                keep,
                always(Action::Forget("redforest_location".into())),
                always(Action::Replan),
            ])
        );
        // Waiting for one room and asking after another is not this shape,
        // nor is a pattern that is more than a list of paths.
        assert_eq!(steps(&fog_script(paths, "Obvious paths: east"), 1), None);
        assert_eq!(
            steps(&fog_script("Obvious paths: .*", "Obvious paths: .*"), 1),
            None
        );
    }

    #[test]
    fn a_field_asked_for_twice_is_tried_then_entered() {
        let expected = Some(vec![
            always(Action::TryMove("go field".into())),
            if_still_here(Action::Move("go field".into())),
        ]);
        assert_eq!(steps(";e put \"go field\";put \"go field\"", 1), expected);
        assert_eq!(steps(";e put 'go field'; put 'go field'", 1), expected);
        // Two different commands: which one moves is another arm's business.
        assert_eq!(steps(";e put 'search';put 'go field'", 1), None);
    }

    #[test]
    fn a_hard_landing_ends_standing() {
        let jump = ";e move 'jump ledge'; waitrt?; wait_while { stunned? }; fput 'stand' unless \
                    standing?; waitrt?";
        assert_eq!(
            steps(jump, 9651),
            Some(vec![mv("jump ledge"), put("stand")])
        );
        let crawl = ";e result = nil;while result.nil? && Room.current.id == 7393;result = \
                     dothistimeout \"crawl hole\",3,/^You crawl under the low overhang/;end;\
                     wait_while{stunned?};fput \"stand\" until standing?";
        assert_eq!(
            steps(crawl, 7393),
            Some(vec![
                always(Action::KeepMoving("crawl hole".into())),
                put("stand")
            ])
        );
        // The room named must be the one being left.
        assert_eq!(steps(crawl, 7394), None);
    }

    #[test]
    fn a_dark_niche_is_tried_then_waited_for() {
        let script = ";e move 'go niche';while Room.current.id == 5922;line = get?;if line.nil?;\
                      sleep 0.2;elsif line == 'Your eyes have recovered from the poor light \
                      within the niche.';move 'go niche';break;end;end;";
        assert_eq!(
            steps(script, 5922),
            Some(vec![
                always(Action::TryMove("go niche".into())),
                if_still_here(Action::Await(
                    "Your eyes have recovered from the poor light within the niche.".into()
                )),
                if_still_here(Action::Move("go niche".into())),
            ])
        );
        assert_eq!(steps(script, 5923), None);
        assert_eq!(
            steps(&script.replacen("go niche", "go arch", 1), 5922),
            None
        );
    }

    #[test]
    fn a_splinter_pulled_from_a_chair() {
        let script = ";e multifput('sit chair', 'pull splinter'); waitrt?; fput('stand'); waitrt?";
        assert_eq!(
            steps(script, 36166),
            Some(vec![put("sit chair"), mv("pull splinter"), put("stand")])
        );
        assert_eq!(steps(&script.replace("'); waitrt?; f", "'); f"), 1), None);
    }

    #[test]
    fn four_climbs_with_empty_hands() {
        let script = ";e waitrt?; empty_hands; 4.times { move 'climb wall' }; fill_hands";
        let found = steps(script, 22007).unwrap_or_default();
        assert_eq!(found.len(), 6);
        assert_eq!(found[1..5], vec![mv("climb wall"); 4]);
        assert_eq!(steps(&script.replace('4', "40"), 1), None);
    }
}
