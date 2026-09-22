//! Arms for crossings that **repeat something until the game says so** -- a
//! search that finds the way, a lever that finally gives -- and for the hard
//! climbs around them that take a stance or a spell first.

use cena_map::{Action, Cond, Crossing, Step};

use super::moves::{cast_clause, cast_if_able};
use super::{always, holes, is_plain_argument, is_word, quoted};

const CELERITY: &str = "Celerity"; // 506

pub(super) fn crossing(script: &str, from: u32, to: u32) -> Option<Crossing> {
    search_spellings(script)
        .or_else(|| with_celerity(script))
        .or_else(|| lever(script))
        .or_else(|| in_stance(script))
        .or_else(|| from_the_profile(script))
        .or_else(|| carried_off(script, from, to))
        .or_else(|| ferry(script))
        .map(Crossing::Steps)
}

fn plain(command: &str) -> Option<String> {
    is_plain_argument(command).then(|| command.to_owned())
}

fn when(action: Action, cond: Cond) -> Step {
    Step {
        action,
        when: Some(cond),
    }
}

/// A pattern that is only alternatives of plain text, as the lines it names.
/// `^` at the start and `.`, `\.` or `?` at the end are dropped -- a line is
/// asked to *hold* the text -- and anything else a pattern can do is refused.
fn lines(pattern: &str) -> Option<Vec<String>> {
    pattern
        .split('|')
        .map(|part| {
            let part = part.strip_prefix('^').unwrap_or(part);
            // `\.` is a full stop, and says so; a bare `.` matches one too.
            let part = part.replace("\\.", ".");
            let part = part.trim_end_matches(['?', '.']);
            (!part.is_empty()
                && !part.contains([
                    '\\', '^', '$', '*', '+', '?', '(', ')', '[', ']', '{', '}', '/',
                ]))
            .then(|| part.to_owned())
        })
        .collect()
}

fn put_until(command: &str, until: Vec<String>, tries: Option<u32>) -> Option<Step> {
    Some(always(Action::PutUntil {
        command: plain(command)?,
        until,
        tries,
    }))
}

/// `search` until told the way is found, in the spellings round two's arm
/// (`proposed::search_until_found`) did not meet. 7 exits.
fn search_spellings(script: &str) -> Option<Vec<Step>> {
    if let Some(found) = holes(
        script,
        &[
            ";e begin; fput 'search'; search_result = waitfor 'don\\'t find anything', '",
            "', '",
            "', 'Roundtime:'; waitrt?; end until search_result =~ /",
            "/; move '",
            "'",
        ],
    ) {
        let [first, second, word, go] = found[..] else {
            return None;
        };
        // Both lines that hold the word end the search; the rest end a try.
        (is_word(word) && first.contains(word) && second.contains(word)).then_some(())?;
        let until = lines(&format!("{first}|{second}"))?;
        return Some(vec![
            put_until("search", until, None)?,
            always(Action::Move(plain(go)?)),
        ]);
    }
    if let Some(found) = holes(
        script,
        &[
            ";e 10.times { result = dothistimeout 'search', 5, /don't find anything|",
            "|Round ?time/; waitrt?; break if result =~ /",
            "/ }; move '",
            "'",
        ],
    ) {
        let [said, again, go] = found[..] else {
            return None;
        };
        (said == again).then_some(())?;
        return Some(vec![
            put_until("search", lines(said)?, Some(10))?,
            always(Action::Move(plain(go)?)),
        ]);
    }
    if let Some(found) = holes(
        script,
        &[";e line = fput \"search\" until line =~ /", "/;move '", "'"],
    ) {
        let [said, go] = found[..] else {
            return None;
        };
        return Some(vec![
            put_until("search", lines(said)?, None)?,
            always(Action::Move(plain(go)?)),
        ]);
    }
    // Rubbish turns up first; each find but the trapdoor means search again.
    let found = holes(
        script,
        &[
            ";e put \"search\";while line=get;if line=~ /(",
            ")!$/;put \"search\";elsif line=~/",
            "!$/;put \"",
            "\";break;end;end",
        ],
    )?;
    let [rubbish, said, go] = found[..] else {
        return None;
    };
    lines(rubbish)?;
    Some(vec![
        put_until("search", lines(&format!("{said}!"))?, None)?,
        always(Action::Move(plain(go)?)),
    ])
}

/// Searches made under Celerity (506), cast if the walker can. 8 exits.
/// Upstream casts it again each time round; once is what the step says.
fn with_celerity(script: &str) -> Option<Vec<Step>> {
    let clause = cast_clause("celerity", 506);
    let clause = clause.trim_end();
    if let Some(found) = holes(
        script,
        &[
            &format!(";e {clause} until dothistimeout('search', 3, /"),
            ") =~ /",
            "/ do end; move '",
            "'",
        ],
    ) {
        let [all, said, go] = found[..] else {
            return None;
        };
        all.strip_suffix('/')?
            .split('|')
            .any(|part| part == said)
            .then_some(())?;
        return Some(vec![
            cast_if_able(CELERITY),
            put_until("search", lines(said)?, None)?,
            always(Action::Move(plain(go)?)),
        ]);
    }
    if let Some(found) = holes(
        script,
        &[
            &format!(
                ";e 10.times {{ {clause} dothistimeout 'search', 3, /^You (?:carefully \
                 )?search|^You don't find anything|^You find nothing else/; waitrt?; r = \
                 dothistimeout '"
            ),
            "', 3, /^I could not find|^You reach out|^Get what?/; break if r =~ /^You reach out/ \
             }; empty_hands; move '",
            "'; waitrt?; fill_hands",
        ],
    ) {
        let [get, go] = found[..] else {
            return None;
        };
        return Some(vec![
            cast_if_able(CELERITY),
            always(Action::RoundUntil {
                commands: vec!["search".to_owned(), plain(get)?],
                until: vec!["You reach out".to_owned()],
                tries: Some(10),
            }),
            always(Action::EmptyHands),
            always(Action::Move(plain(go)?)),
            always(Action::FillHands),
        ]);
    }
    // Upstream stops at the first answer that is NOT the opening
    // (`break if res !~`), which reads like an inverted test; searching until
    // the opening is found is right under either reading.
    let found = holes(
        script,
        &[
            ";e 10.times { fput 'stand' unless standing?; waitrt? }; fput '",
            &format!(
                "'; {clause} 10.times {{ res = dothistimeout 'search', 3, /own leg|You take small \
                 comfort|find nothing|"
            ),
            "/; break if res !~ /",
            "/}; move '",
            "'",
        ],
    )?;
    let [first, said, again, go] = found[..] else {
        return None;
    };
    (said == again).then_some(())?;
    Some(vec![
        always(Action::TryMove(plain(first)?)),
        cast_if_able(CELERITY),
        put_until("search", lines(said)?, Some(10))?,
        always(Action::Move(plain(go)?)),
    ])
}

/// A lever or a ring worked until it gives -- or until the game says it never
/// will, which ends the trying and leaves the move to fail. 3 exits.
fn lever(script: &str) -> Option<Vec<Step>> {
    if let Some(found) = holes(
        script,
        &[
            ";e loop { result = dothistimeout '",
            "', 3, /",
            "/; waitrt?; break if result =~ /",
            "/ }; move '",
            "'",
        ],
    )
    .or_else(|| {
        holes(
            script,
            &[
                ";e loop { result = dothistimeout '",
                "', 3, /",
                "/; break if result =~ /",
                "/ }; waitrt?; move '",
                "'",
            ],
        )
    }) {
        let [command, all, stop, go] = found[..] else {
            return None;
        };
        let until = lines(stop)?;
        let all = lines(all)?;
        until.iter().all(|line| all.contains(line)).then_some(())?;
        return Some(vec![
            put_until(command, until, None)?,
            always(Action::Move(plain(go)?)),
        ]);
    }
    // Upstream frees the hands only when both are full; one free hand is what
    // the ring needs, and freeing both gives it.
    let found = holes(
        script,
        &[
            ";e if checkleft and checkright; empty_hands; need_fill_hands = true; else; \
             need_fill_hands = false; end; result = dothistimeout '",
            "', 8, /",
            "/ until result =~ /",
            "/; waitrt?; fill_hands if need_fill_hands; move '",
            "'",
        ],
    )?;
    let [command, all, stop, go] = found[..] else {
        return None;
    };
    let until = lines(stop)?;
    let all = lines(all)?;
    until.iter().all(|line| all.contains(line)).then_some(())?;
    Some(vec![
        always(Action::EmptyHands),
        put_until(command, until, None)?,
        always(Action::FillHands),
        always(Action::Move(plain(go)?)),
    ])
}

/// Climbs made in offensive stance. 5 exits.
fn in_stance(script: &str) -> Option<Vec<Step>> {
    const SAVE: &str = ";e save_stance = XMLData.stance_text;fput 'stance offensive' if \
                        save_stance != 'offensive';";
    const RESTORE: &str = "');fput \"stance #{save_stance}\" if save_stance != \
                           XMLData.stance_text;;$go2_restart = true;";
    let offensive = || always(Action::Stance("offensive".to_owned()));
    if let Some(found) = holes(script, &[SAVE, "move('", RESTORE]) {
        let [sent, command] = found[..] else {
            return None;
        };
        let mut steps = vec![offensive()];
        if sent.is_empty() {
            steps.push(always(Action::Move(plain(command)?)));
        } else {
            // `put 'climb pit'; move('climb pit')`: sent twice, and either
            // send may be the one that moves.
            (sent == format!("put '{command}';")).then_some(())?;
            steps.push(always(Action::TryMove(plain(command)?)));
            steps.push(when(Action::Move(plain(command)?), Cond::StillHere));
        }
        steps.push(always(Action::RestoreStance));
        steps.push(always(Action::Replan));
        return Some(steps);
    }
    // Up a ledge until the room lists no exits at all; the hands stay empty
    // for the next climb, and the walk gives them back when it ends.
    if let Some(found) = holes(
        script,
        &[
            ";e empty_hands\nold_stance = XMLData.stance_text\nfput 'stance offensive' unless \
             old_stance == 'offensive'\nuntil !checkpaths\nfput 'stand' unless standing?\nfput '",
            "'\nsleep 0.1\nwaitrt?\nend\nfput 'stance ' + old_stance unless old_stance == \
             'offensive'",
        ],
    ) {
        let [command] = found[..] else {
            return None;
        };
        return Some(vec![
            always(Action::EmptyHands),
            offensive(),
            always(Action::MoveWhile(plain(command)?, Cond::ExitsOver(0))),
            always(Action::RestoreStance),
        ]);
    }
    if let Some(found) = holes(
        script,
        &[
            ";e while XMLData.room_title == '[",
            "]'; fput '",
            "'; move '",
            "'; waitrt?; end; fill_hands",
        ],
    ) {
        let [_title, first, command] = found[..] else {
            return None;
        };
        return Some(vec![
            always(Action::RoundWhile(
                vec![plain(first)?, plain(command)?],
                Cond::StillHere,
            )),
            always(Action::FillHands),
        ]);
    }
    let found = holes(
        script,
        &[
            ";e loop { fput '",
            "'; sleep 1; waitrt?; break if (checkpaths('west') || checkpaths('east')); }; \
             fill_hands",
        ],
    )?;
    let [command] = found[..] else {
        return None;
    };
    let ashore = Cond::Any(vec![Cond::Exit("w".to_owned()), Cond::Exit("e".to_owned())]);
    Some(vec![
        always(Action::MoveWhile(
            plain(command)?,
            Cond::Not(Box::new(ashore)),
        )),
        always(Action::FillHands),
    ])
}

/// The way into private property, which the profile spells out. 4 exits.
fn from_the_profile(script: &str) -> Option<Vec<Step>> {
    let found = holes(
        script,
        &[
            ";e if UserVars.",
            "; UserVars.",
            ".each{|c| fput \"#{c}\" } end",
        ],
    )?;
    let [name, again] = found[..] else {
        return None;
    };
    (is_word(name) && name == again).then_some(())?;
    Some(vec![always(Action::MovesFromSetting(
        name.to_ascii_lowercase(),
    ))])
}

/// The walker does something and is then carried to the destination. 2 exits.
fn carried_off(script: &str, from: u32, to: u32) -> Option<Vec<Step>> {
    if let Some(found) = holes(
        script,
        &[
            ";e (fput 'stand' until standing?;fput '",
            "') until Room.current.id != ",
            ";wait_until{Room.current.id == ",
            "}",
        ],
    ) {
        let [command, left, reached] = found[..] else {
            return None;
        };
        (left.parse() == Ok(from) && reached.parse() == Ok(to)).then_some(())?;
        return Some(vec![
            always(Action::KeepMoving(plain(command)?)),
            always(Action::AwaitArrival),
        ]);
    }
    let found = quoted(
        script,
        &[
            ";e 2.times { fput '",
            "'; sleep 1; wait_while { stunned? } }; fput '",
            "'; fput '",
            "'; wait_until { Room.current.id == ",
            " }",
        ],
    )?;
    let [twice, third, fourth, reached] = found[..] else {
        return None;
    };
    (reached.parse() == Ok(to)).then_some(())?;
    let unstunned = || {
        always(Action::WaitUntil(Cond::Otherwise(Box::new(Cond::Flag(
            "stunned".to_owned(),
        )))))
    };
    Some(vec![
        always(Action::Put(plain(twice)?)),
        unstunned(),
        always(Action::Put(plain(twice)?)),
        unstunned(),
        always(Action::Put(plain(third)?)),
        always(Action::Put(plain(fourth)?)),
        always(Action::AwaitArrival),
    ])
}

/// The ferry between River's Rest and Solhaven: wait for the gangplank or the
/// call ashore, then step out. 2 exits. The second line upstream matches as
/// `Solhaven....+ ashore!`; a line holding ` ashore!` is the part that is plain.
fn ferry(script: &str) -> Option<Vec<Step>> {
    let [town] = holes(
        script,
        &[
            ";e while line = get;if line=~ /The crew hurriedly puts out the gangplank\\.|",
            "\\.\\.\\..+ ashore!/;fput \"out\";break;end;end",
        ],
    )?[..] else {
        return None;
    };
    town.chars()
        .all(|c| c.is_ascii_alphabetic() || matches!(c, ' ' | '\''))
        .then_some(())?;
    Some(vec![
        always(Action::AwaitAny(vec![
            "The crew hurriedly puts out the gangplank.".to_owned(),
            " ashore!".to_owned(),
        ])),
        always(Action::Move("out".to_owned())),
    ])
}

#[cfg(test)]
mod tests {
    use cena_map::moves_whatever_is_known;

    use super::*;

    fn actions(script: &str, from: u32, to: u32) -> Vec<Action> {
        match crossing(script, from, to) {
            Some(Crossing::Steps(steps)) => {
                assert!(moves_whatever_is_known(&steps), "{script}");
                steps.into_iter().map(|step| step.action).collect()
            }
            _ => Vec::new(),
        }
    }

    #[test]
    fn a_pattern_of_plain_alternatives_is_its_lines() {
        assert_eq!(lines("a/b"), None, "a slash would end the pattern early");
        assert_eq!(
            lines("^Ok, you pull.  Now what?|just enough\\."),
            Some(vec!["Ok, you pull.  Now what".into(), "just enough".into()])
        );
        assert_eq!(lines("discover.*?opening"), None);
    }

    #[test]
    fn both_lines_that_hold_the_word_end_the_search() {
        let script = ";e begin; fput 'search'; search_result = waitfor 'don\\'t find anything', \
                      'the opening of an escape tunnel', 'nothing other than the tunnel', \
                      'Roundtime:'; waitrt?; end until search_result =~ /tunnel/; move 'go tunnel'";
        assert_eq!(
            actions(script, 1, 2),
            [
                Action::PutUntil {
                    command: "search".into(),
                    until: vec![
                        "the opening of an escape tunnel".into(),
                        "nothing other than the tunnel".into()
                    ],
                    tries: None
                },
                Action::Move("go tunnel".into())
            ]
        );
        assert_eq!(actions(&script.replace("/tunnel/", "/opening/"), 1, 2), []);
    }

    #[test]
    fn a_lever_is_worked_until_a_line_that_ends_it() {
        let script = ";e loop { result = dothistimeout 'pull lever', 3, /Ok, you pull.  Now \
                      what|just enough to raise|but it will not budge/; waitrt?; break if result \
                      =~ /Ok, you pull.  Now what?|just enough to raise/ }; move 'go portcullis'";
        let found = actions(script, 1, 2);
        assert_eq!(
            found[0],
            Action::PutUntil {
                command: "pull lever".into(),
                until: vec![
                    "Ok, you pull.  Now what".into(),
                    "just enough to raise".into()
                ],
                tries: None
            }
        );
        // Stopping on a line the command was never listening for is not this.
        assert_eq!(actions(&script.replace("raise/ }", "lower/ }"), 1, 2), []);
    }

    #[test]
    fn a_command_sent_twice_is_tried_then_made() {
        let script = ";e save_stance = XMLData.stance_text;fput 'stance offensive' if \
                      save_stance != 'offensive';put 'climb latrine pit';move('climb latrine \
                      pit');fput \"stance #{save_stance}\" if save_stance != \
                      XMLData.stance_text;;$go2_restart = true;";
        let found = actions(script, 1, 2);
        assert_eq!(found[1], Action::TryMove("climb latrine pit".into()));
        assert_eq!(found[2], Action::Move("climb latrine pit".into()));
        assert_eq!(found.last(), Some(&Action::Replan));
        let other = script.replace("put 'climb latrine pit'", "put 'climb wall'");
        assert_eq!(actions(&other, 1, 2), []);
    }

    #[test]
    fn the_profile_spells_out_the_way_in() {
        let script = ";e if UserVars.Peregrine; UserVars.Peregrine.each{|c| fput \"#{c}\" } end";
        assert_eq!(
            actions(script, 1, 2),
            [Action::MovesFromSetting("peregrine".into())]
        );
        let script = script.replacen("Peregrine", "Other", 1);
        assert_eq!(actions(&script, 1, 2), []);
    }

    #[test]
    fn being_carried_names_both_rooms() {
        let script = ";e (fput 'stand' until standing?;fput 'climb root') until Room.current.id \
                      != 24241;wait_until{Room.current.id == 313}";
        assert_eq!(
            actions(script, 24241, 313),
            [
                Action::KeepMoving("climb root".into()),
                Action::AwaitArrival
            ]
        );
        assert_eq!(actions(script, 24241, 314), []);
    }
}
