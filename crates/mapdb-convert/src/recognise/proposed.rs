//! Arms for the vocabulary round two of the long tail proposed
//! (`research/mapdb-inventory/tail/PROPOSALS.md`): repeat until the game says
//! so, cast at a thing, a stance held for one crossing, turns taken between
//! commands, and waiting for whoever follows.

use cena_map::{Action, Crossing, Step};

use super::{always, holes, is_plain_argument, is_word, quoted};

pub(super) fn crossing(script: &str, from: u32) -> Option<Crossing> {
    phase_through(script)
        .or_else(|| search_until_found(script))
        .or_else(|| climb_in_stance(script))
        .or_else(|| taking_turns(script, from))
        .or_else(|| with_the_group(script))
        .or_else(|| with_an_escort(script))
}

fn plain(command: &str) -> Option<String> {
    is_plain_argument(command).then(|| command.to_owned())
}

/// A line of the game's that a hole holds: no quote to end it early.
fn line(text: &str) -> Option<String> {
    (!text.is_empty() && !text.contains(['\'', '"', '/', '\n'])).then(|| text.to_owned())
}

fn search(until: String, tries: Option<u32>) -> Step {
    always(Action::PutUntil {
        command: "search".to_owned(),
        until: vec![until],
        tries,
    })
}

/// Phase (704) cast at a thing in the room carries the walker through it. 12
/// exits in two spellings. Upstream waits for the mana and, in one of them,
/// recasts on `Spell Hindrance`; both are what `Action::CastAt` means.
fn phase_through(script: &str) -> Option<Crossing> {
    let [target] = holes(
        script,
        &[
            ";e loop { wait_until { Spell[704].affordable? }; result = cast(704, '",
            "'); break unless result =~ /Spell Hindrance/ } ",
        ],
    )
    .or_else(|| {
        holes(
            script,
            &[
                ";e phase = Spell[704]; unless phase.affordable?; echo 'waiting for mana...'; \
                 wait_until { phase.affordable? }; end; phase.cast('",
                "')",
            ],
        )
    })?[..] else {
        return None;
    };
    // A noun, or a name: `oil painting`.
    target.split(' ').all(is_word).then_some(())?;
    Some(Crossing::Steps(vec![always(Action::CastAt(
        "Phase".to_owned(),
        target.to_owned(),
    ))]))
}

/// `search` until the game says the way is found, then take it. 14 exits in
/// four spellings; the other lines upstream waits on (`don't find anything`,
/// `Roundtime`) only end one try, which the step does itself.
fn search_until_found(script: &str) -> Option<Crossing> {
    if let Some(found) = holes(
        script,
        &[
            ";e begin; fput 'search'; search_result = waitfor \"don't find anything\", \"",
            "\", \"Roundtime\", \"Round time\"; waitrt?; end until search_result =~ /",
            "/; move '",
            "'",
        ],
    ) {
        let [said, again, go] = found[..] else {
            return None;
        };
        (said == again).then_some(())?;
        return Some(Crossing::Steps(vec![
            search(line(said)?, None),
            always(Action::Move(plain(go)?)),
        ]));
    }
    if let Some(found) = holes(
        script,
        &[
            ";e 10.times { result = dothistimeout 'search', 5, /don't find anything|",
            "|Round ?time|...[Ww]ait/; waitrt?; break if result =~ /",
            "/ };move('",
            "')",
        ],
    ) {
        let [said, again, go] = found[..] else {
            return None;
        };
        (said == again).then_some(())?;
        return Some(Crossing::Steps(vec![
            search(line(said)?, Some(10)),
            always(Action::Move(plain(go)?)),
        ]));
    }
    if let Some(found) = holes(
        script,
        &[
            ";e \nbegin; \n  search_result = dothistimeout 'search', 5, /don't find \
             anything|discover.*?",
            "|Round ?tim/; \n  waitrt?; \nend until search_result =~ /discover/; \nmove('",
            "')\n",
        ],
    ) {
        // What is discovered: a `crack`, an `opening`.
        let [thing, go] = found[..] else {
            return None;
        };
        is_word(thing).then_some(())?;
        return Some(Crossing::Steps(vec![
            search("discover".to_owned(), None),
            always(Action::Move(plain(go)?)),
        ]));
    }
    // Lying down to look under something.
    let found = quoted(
        script,
        &[
            ";e fput 'lie' until checkprone; result = dothistimeout 'search',3, /",
            "/ until result;waitrt?;fput 'stand' until standing?;waitrt?;fput '",
            "'",
        ],
    )?;
    let [said, go] = found[..] else {
        return None;
    };
    Some(Crossing::Steps(vec![
        always(Action::Put("lie".to_owned())),
        search(line(said)?, None),
        always(Action::Put("stand".to_owned())),
        always(Action::Move(plain(go)?)),
    ]))
}

/// A hard climb in offensive stance, hands free, the stance put back after. 6
/// exits. It lands somewhere the map does not promise, so it replans.
fn climb_in_stance(script: &str) -> Option<Crossing> {
    let [command] = holes(
        script,
        &[
            ";e cur_stance = XMLData.stance_text;empty_hands;fput('stance offensive') if \
             cur_stance != 'offensive';move('",
            "');fill_hands;fput('stance ' + cur_stance) if cur_stance != \
             'offensive';$go2_restart = true",
        ],
    )?[..] else {
        return None;
    };
    Some(Crossing::Steps(vec![
        always(Action::EmptyHands),
        always(Action::Stance("offensive".to_owned())),
        always(Action::Move(plain(command)?)),
        always(Action::FillHands),
        always(Action::RestoreStance),
        always(Action::Replan),
    ]))
}

/// `south`, then `southwest`, then `south` again, until the room changes. 4
/// exits. The room named must be the one being left.
fn taking_turns(script: &str, from: u32) -> Option<Crossing> {
    let found = holes(
        script,
        &[
            ";e loop { move '",
            "'; break if Room.current.id != ",
            "; move '",
            "'; break if Room.current.id != ",
            "; }",
        ],
    )?;
    let [first, room, second, again] = found[..] else {
        return None;
    };
    (room == again && room.parse() == Ok(from)).then_some(())?;
    Some(Crossing::Steps(vec![always(Action::KeepMovingAny(vec![
        plain(first)?,
        plain(second)?,
    ]))]))
}

/// A move, then waiting for the group that followed to take hands again. 6
/// exits. Upstream reads who followed out of the game's text before it moves;
/// here who is following is the walker's to know.
fn with_the_group(script: &str) -> Option<Crossing> {
    let [command] = holes(
        script,
        &[
            ";e group_members = nil; clear.reverse.each { |line| if line =~ /^Obvious \
             (paths|exits)/; break; elsif line =~ /^([A-Za-z ,]+) followed\\.$/; group_members = \
             $1.split(/, | and /); group_members.delete_if { |m| m =~ /^[Yy]our / }; \
             group_members = nil if group_members.empty?; break; end }; move '",
            "'; if group_members; echo \"Waiting for your group... \"; begin; if get =~ /^(You \
             reach out and hold )?([A-z][a-z]+)('s hand| joins your group)\\.$/; \
             group_members.delete $2; end; end while group_members.length > 0; end; waitrt?",
        ],
    )?[..] else {
        return None;
    };
    Some(Crossing::Steps(vec![
        always(Action::Move(plain(command)?)),
        always(Action::AwaitFollowers),
    ]))
}

/// Several moves, waiting before and after each for a child or an official
/// being escorted to catch up. 9 exits.
fn with_an_escort(script: &str) -> Option<Crossing> {
    const ESCORTING: &str = ";e if ((bounty? =~ /^You have made contact with the child/)||\
        (Society.task =~ /You have been tasked to find and rescue an official who was \
        captured/)); mynpc = GameObj.npcs.find { |npc| npc.noun =~ /child|official/ }; else;  \
        mynpc = nil; end;  ";
    const WAIT: &str =
        "50.times { break if GameObj.npcs.any? { |npc| npc.id == mynpc.id }; sleep 0.1 } if mynpc;";
    let mut rest = script.strip_prefix(ESCORTING)?.strip_prefix(WAIT)?;
    let mut steps = vec![always(Action::AwaitFollowers)];
    while !rest.is_empty() {
        let (command, after) = rest.strip_prefix(" move '")?.split_once("'; ")?;
        steps.push(always(Action::Move(plain(command)?)));
        steps.push(always(Action::AwaitFollowers));
        rest = after.strip_prefix(WAIT)?;
    }
    (steps.len() > 1).then_some(Crossing::Steps(steps))
}

#[cfg(test)]
mod tests {
    use cena_map::moves_whatever_is_known;

    use super::*;

    fn actions(script: &str, from: u32) -> Vec<Action> {
        match crossing(script, from) {
            Some(Crossing::Steps(steps)) => {
                assert!(moves_whatever_is_known(&steps), "{script}");
                steps.into_iter().map(|step| step.action).collect()
            }
            _ => Vec::new(),
        }
    }

    #[test]
    fn phase_is_cast_at_the_thing_and_carries_the_walker() {
        let script = ";e loop { wait_until { Spell[704].affordable? }; result = cast(704, \
                      'insignia'); break unless result =~ /Spell Hindrance/ } ";
        assert_eq!(
            actions(script, 1),
            [Action::CastAt("Phase".into(), "insignia".into())]
        );
        assert_eq!(actions(&script.replace("704", "705"), 1), []);
    }

    #[test]
    fn a_search_repeats_until_the_game_says_the_way_is_found() {
        let script = ";e begin; fput 'search'; search_result = waitfor \"don't find anything\", \
                      \"discover a northwest path\", \"Roundtime\", \"Round time\"; waitrt?; end \
                      until search_result =~ /discover a northwest path/; move 'go path'";
        assert_eq!(
            actions(script, 1),
            [
                Action::PutUntil {
                    command: "search".into(),
                    until: vec!["discover a northwest path".into()],
                    tries: None
                },
                Action::Move("go path".into())
            ]
        );
        // Waiting for one line and testing for another is not this shape.
        let crossed = script.replacen("northwest path", "northeast path", 1);
        assert_eq!(actions(&crossed, 1), []);
    }

    #[test]
    fn a_stance_is_taken_for_the_climb_and_put_back() {
        let script = ";e cur_stance = XMLData.stance_text;empty_hands;fput('stance offensive') \
                      if cur_stance != 'offensive';move('climb rockslide');fill_hands;\
                      fput('stance ' + cur_stance) if cur_stance != 'offensive';\
                      $go2_restart = true";
        let found = actions(script, 1);
        assert_eq!(found.len(), 6);
        assert_eq!(found[1], Action::Stance("offensive".into()));
        assert_eq!(found[4], Action::RestoreStance);
        assert_eq!(found[5], Action::Replan, "and it is last");
    }

    #[test]
    fn turns_are_taken_until_the_room_changes() {
        let script = ";e loop { move 'south'; break if Room.current.id != 4136; move \
                      'southwest'; break if Room.current.id != 4136; }";
        assert_eq!(
            actions(script, 4136),
            [Action::KeepMovingAny(vec![
                "south".into(),
                "southwest".into()
            ])]
        );
        assert_eq!(actions(script, 4137), [], "it watches another room");
    }

    #[test]
    fn an_escort_is_waited_for_around_every_move() {
        let wait = "50.times { break if GameObj.npcs.any? { |npc| npc.id == mynpc.id }; sleep \
                    0.1 } if mynpc;";
        let script = format!(
            ";e if ((bounty? =~ /^You have made contact with the child/)||(Society.task =~ /You \
             have been tasked to find and rescue an official who was captured/)); mynpc = \
             GameObj.npcs.find {{ |npc| npc.noun =~ /child|official/ }}; else;  mynpc = nil; \
             end;  {wait} move 'southeast'; {wait} move 'southwest'; {wait}"
        );
        assert_eq!(
            actions(&script, 1),
            [
                Action::AwaitFollowers,
                Action::Move("southeast".into()),
                Action::AwaitFollowers,
                Action::Move("southwest".into()),
                Action::AwaitFollowers,
            ]
        );
        let cut_short = script.trim_end_matches(wait);
        assert_eq!(actions(cut_short, 1), [], "a move with no wait after it");
    }
}
