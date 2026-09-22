//! Arms for doors that want a key, and for the walker's own company: a
//! floating disk to wait for, a group to bring along.
//!
//! The keys are where commands carry placeholders (`cena_map::Action`):
//! `{item:…}` for a thing addressed by the game's id, `{setting:…}` for a name
//! only the owner of the house can supply.

use cena_map::{Action, Cond, Crossing, Routine, Step};

use super::{always, holes, is_plain_argument, is_word};

const DISK: &str = "Floating Disk"; // 511

pub(super) fn crossing(script: &str, from: u32) -> Option<Crossing> {
    flight_of_steps(script).or_else(|| {
        keyed_door(script)
            .or_else(|| key_in_a_sack(script))
            .or_else(|| key_named_by_the_profile(script))
            .or_else(|| with_own_disk(script))
            .or_else(|| with_company(script, from))
            .map(Crossing::Steps)
    })
}

fn plain(command: &str) -> Option<String> {
    is_plain_argument(command).then(|| command.to_owned())
}

fn put(command: String) -> Step {
    always(Action::Put(command))
}

fn when(action: Action, cond: Cond) -> Step {
    Step {
        action,
        when: Some(cond),
    }
}

/// A thing's full name: words, hyphens, an apostrophe.
fn is_name(hole: &str) -> bool {
    !hole.is_empty()
        && hole
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '\''))
}

/// `UserVars.Key_Sack`, as the profile names it.
fn setting(name: &str) -> Option<String> {
    is_word(name).then(|| format!("{{setting:{}}}", name.to_ascii_lowercase()))
}

/// Take the key off, unlock, open, go, close, lock, put the key back on. 8
/// exits; the cost asks that the key is worn (`facts::with_the_key`).
fn keyed_door(script: &str) -> Option<Vec<Step>> {
    let found = holes(
        script,
        &[
            ";e door='",
            "';key=GameObj.inv.find{|k| k.name=='",
            "';};if !key.nil? then multifput \"remove ##{key.id}\",  \"unlock #{door}\", \"open \
             #{door}\", \"go #{door}\", \"close #{door}\", \"lock #{door}\", \"wear ##{key.id}\"; \
             end;",
        ],
    )?;
    let [door, key] = found[..] else {
        return None;
    };
    (is_name(door) && is_name(key)).then_some(())?;
    Some(vec![
        put(format!("remove {{item:{key}}}")),
        put(format!("unlock {door}")),
        put(format!("open {door}")),
        always(Action::Move(format!("go {door}"))),
        put(format!("close {door}")),
        put(format!("lock {door}")),
        put(format!("wear {{item:{key}}}")),
    ])
}

/// A house key: worn, the door just opens; kept in a sack, it is taken out
/// for the door and put back. 6 exits. Upstream frees one hand; freeing both
/// gives it the one.
fn key_in_a_sack(script: &str) -> Option<Vec<Step>> {
    let found = holes(
        script,
        &[
            ";e if GameObj.inv.find {|obj| obj.noun ",
            "};fput \"",
            "\";else;empty_hand;multifput \"get my ",
            " from my #{UserVars.",
            "}\",\"",
            "\",\"put my key in my #{UserVars.",
            "}\";fill_hand;end",
        ],
    )?;
    let [test, go, key, sack, again, sack_again] = found[..] else {
        return None;
    };
    (go == again && sack == sack_again).then_some(())?;
    let worn = match test {
        "== \"key\"" => Cond::WearingNoun("key".to_owned()),
        "=~ /^key(?:ring)?$/" => Cond::Any(vec![
            Cond::WearingNoun("key".to_owned()),
            Cond::WearingNoun("keyring".to_owned()),
        ]),
        _ => return None,
    };
    let key = match key.strip_prefix("#{UserVars.") {
        Some(named) => setting(named.strip_suffix('}')?)?,
        None => (key == "key").then(|| key.to_owned())?,
    };
    let sack = setting(sack)?;
    let go = plain(go)?;
    let unworn = || Cond::Otherwise(Box::new(worn.clone()));
    Some(vec![
        when(Action::Move(go.clone()), worn.clone()),
        when(Action::EmptyHands, unworn()),
        when(
            Action::Put(format!("get my {key} from my {sack}")),
            unworn(),
        ),
        when(Action::Move(go), unworn()),
        when(Action::Put(format!("put my key in my {sack}")), unworn()),
        when(Action::FillHands, unworn()),
    ])
}

/// Once a key is off it is worn by nobody, so where it came from is written
/// down before it moves, and asked of the note after.
const NOTE: &str = "key_was_worn";

/// The same, where the profile names the key as well as the sack, and the
/// door is locked again behind the walker. 2 exits.
fn key_named_by_the_profile(script: &str) -> Option<Vec<Step>> {
    let found = holes(
        script,
        &[
            ";e refill_hand = false;key_worn = false;\n  (refill_hand = true;empty_hand) if \
             !checkleft.nil? and !checkright.nil?;\n  key_worn = true if GameObj.inv.find {|obj| \
             obj.name =~ /#{UserVars.key.split(' ').join('.*?')}/};\n  fput \"remove my \
             #{UserVars.key}\" if key_worn;\n  fput \"get my #{UserVars.key} from my \
             #{UserVars.key_sack}\" if !key_worn;\n  door = '",
            "';\n  multifput \"unlock #{door}\",\"open #{door}\",\"go #{door}\",\"close \
             #{door}\",\"lock #{door}\";\n  fput \"wear my #{UserVars.key}\" if key_worn;\n  fput \
             \"put my #{UserVars.key} in my #{UserVars.key_sack}\" if !key_worn;\n  fill_hand if \
             refill_hand;",
        ],
    )?;
    let [door] = found[..] else {
        return None;
    };
    is_name(door).then_some(())?;
    let worn = || Cond::WearingNamedBy("key".to_owned());
    let was_worn = || Cond::Remembered(NOTE.to_owned(), "yes".to_owned());
    let was_not = || Cond::Otherwise(Box::new(was_worn()));
    let send = |command: &str| Action::Put(command.to_owned());
    Some(vec![
        always(Action::EmptyHands),
        always(Action::Forget(NOTE.to_owned())),
        when(Action::Remember(NOTE.to_owned(), "yes".to_owned()), worn()),
        when(send("remove my {setting:key}"), was_worn()),
        when(
            send("get my {setting:key} from my {setting:key_sack}"),
            was_not(),
        ),
        put(format!("unlock {door}")),
        put(format!("open {door}")),
        always(Action::Move(format!("go {door}"))),
        put(format!("close {door}")),
        put(format!("lock {door}")),
        when(send("wear my {setting:key}"), was_worn()),
        when(
            send("put my {setting:key} in my {setting:key_sack}"),
            was_not(),
        ),
        always(Action::Forget(NOTE.to_owned())),
        always(Action::FillHands),
    ])
}

/// Wait for the walker's own disk to follow it in, and conjure another if it
/// is lost. 6 exits. Whether the disk in the room is *this* character's is
/// the walker's to work out -- its name holds the character's -- so it is a
/// flag, `own_disk_here`.
fn with_own_disk(script: &str) -> Option<Vec<Step>> {
    let found = holes(
        script,
        &[
            ";e 40.times { sleep 0.1; break if GameObj.loot.any? { |obj| obj.name =~ \
             /#{Char.name} disk$/ } }; unless GameObj.loot.any? { |obj| obj.name =~ \
             /#{Char.name} disk$/ }; disk = Spell[511]; wait_until { disk.affordable? }; \
             disk.cast; end; move '",
            "'",
        ],
    )?;
    let [go] = found[..] else {
        return None;
    };
    let here = || Cond::Flag("own_disk_here".to_owned());
    // Someone who can conjure a disk, and whose disk is not in the room.
    let lost = || {
        Cond::All(vec![
            Cond::SpellKnown(DISK.to_owned()),
            Cond::Not(Box::new(here())),
        ])
    };
    Some(vec![
        when(
            Action::WaitUntil(here()),
            Cond::SpellActive(DISK.to_owned()),
        ),
        // Upstream **waits for the mana** and then casts; it does not go on
        // without a disk because the walker is short just now. So being able
        // to pay is waited for, not asked as a guard that skips the cast.
        when(
            Action::WaitUntil(Cond::SpellAffordable(DISK.to_owned())),
            lost(),
        ),
        when(Action::Cast(DISK.to_owned()), lost()),
        always(Action::Move(plain(go)?)),
    ])
}

/// Ladders and a bridge a group or an escorted child crosses with the
/// walker. 6 exits. Upstream reads who followed out of the game's text; who
/// is following is the walker's to know (`Action::AwaitFollowers`).
fn with_company(script: &str, from: u32) -> Option<Vec<Step>> {
    if let Some(found) = holes(
        script,
        &[
            ";e move '",
            "'\nwaitrt?\nfill_hands\nif $group_members\nclear\necho \"Waiting for your group... \
             To ditch them, ;send go \"\nbegin\nline = get\nif line =~ /^(You reach out and hold \
             )?([A-z][a-z]+)('s hand| joins your group)\\.$/\n$group_members.delete $2\nelsif \
             line == 'go'\nbreak\nend\nend while $group_members.length > 0\nend",
        ],
    ) {
        let [go] = found[..] else {
            return None;
        };
        return Some(vec![
            always(Action::Move(plain(go)?)),
            always(Action::FillHands),
            always(Action::AwaitFollowers),
        ]);
    }
    if let Some(found) = holes(
        script,
        &[
            ";e $group_members = nil;clear.reverse.each { |line|;  if line =~ /^Obvious \
             (paths|exits)/;    break;  elsif line =~ /^([A-Za-z ,]+) followed\\.$/;    \
             $group_members = $1.split(/, | and /);    $group_members.delete_if { |m| m =~ \
             /^[Yy]our / };    $group_members = nil if $group_members.empty?;    break;  \
             end;};empty_hands;move '",
            "';waitrt?",
        ],
    ) {
        let [go] = found[..] else {
            return None;
        };
        // The hands come back at the top of the ladder, on the next exit.
        return Some(vec![
            always(Action::EmptyHands),
            always(Action::Move(plain(go)?)),
        ]);
    }
    let found = holes(
        script,
        &[
            ";e if (bounty? =~ /^You have made contact with the child/); child = \
             GameObj.npcs.find { |npc| npc.noun == 'child' }; else; child = nil; end; move '",
            "'; if Room.current == Room[",
            "]; 50.times { break if GameObj.npcs.any? { |npc| npc.id == child.id }; sleep 0.1 } \
             if child; move '",
            "'; end",
        ],
    )?;
    let [go, room, again] = found[..] else {
        return None;
    };
    (go == again && room.parse() == Ok(from)).then_some(())?;
    Some(vec![
        always(Action::TryMove(plain(go)?)),
        when(Action::AwaitFollowers, Cond::StillHere),
        when(Action::Move(plain(go)?), Cond::StillHere),
    ])
}

/// Four flights of steps listed in an order that changes: a routine, since
/// the command depends on what `look` says. 4 exits.
fn flight_of_steps(script: &str) -> Option<Crossing> {
    const FLIGHT: &str = "a flight of (ascending|descending) steps (curving along|leading \
                          straight through) the (northern|eastern|southern|western) wall";
    let looked = format!(
        ";e clear\nput 'look'\nloop {{\nline = get\nif line =~ /{FLIGHT}, {FLIGHT}, {FLIGHT} and \
         {FLIGHT}\\./\nif $3 == '"
    );
    let found = holes(
        script,
        &[
            &looked,
            "'\nmove 'climb steps'\nelsif $6 == '",
            "'\nmove 'climb second steps'\nelsif $9 == '",
            "'\nmove 'climb third steps'\nelsif $12 == '",
            "'\nmove 'climb fourth steps'\nelse\necho \"error: unique_map_movements",
            " can't find the right steps\"\nexit\nend\nbreak\nend\n}",
        ],
    )?;
    // One of the four names the file it was copied from in its error.
    let [wall, second, third, fourth, "" | ".txt"] = found[..] else {
        return None;
    };
    (matches!(wall, "northern" | "eastern" | "southern" | "western")
        && [second, third, fourth].iter().all(|other| *other == wall))
    .then(|| {
        Crossing::Routine(Routine::FlightOfSteps {
            wall: wall.to_owned(),
        })
    })
}

#[cfg(test)]
mod tests {
    use cena_map::moves_whatever_is_known;

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

    #[test]
    fn the_key_is_addressed_as_an_item_not_by_name() {
        let script = ";e door='oak door';key=GameObj.inv.find{|k| k.name=='cord-strung delicate \
                      brass key';};if !key.nil? then multifput \"remove ##{key.id}\",  \"unlock \
                      #{door}\", \"open #{door}\", \"go #{door}\", \"close #{door}\", \"lock \
                      #{door}\", \"wear ##{key.id}\"; end;";
        let found = steps(script, 1);
        assert_eq!(found.len(), 7);
        assert_eq!(
            found[0].action,
            Action::Put("remove {item:cord-strung delicate brass key}".into())
        );
        assert_eq!(found[3].action, Action::Move("go oak door".into()));
    }

    #[test]
    fn a_key_in_a_sack_is_named_by_the_profile() {
        let script = ";e if GameObj.inv.find {|obj| obj.noun == \"key\"};fput \"go beechwood \
                      door\";else;empty_hand;multifput \"get my #{UserVars.journeys_end} from my \
                      #{UserVars.keysack}\",\"go beechwood door\",\"put my key in my \
                      #{UserVars.keysack}\";fill_hand;end";
        let found = steps(script, 1);
        assert_eq!(found[0].when, Some(Cond::WearingNoun("key".into())));
        assert_eq!(
            found[2].action,
            Action::Put("get my {setting:journeys_end} from my {setting:keysack}".into())
        );
        // Going through one door and coming back by another is not this shape.
        let other = script.replacen("go beechwood door", "go oak door", 1);
        assert_eq!(steps(&other, 1), []);
    }

    #[test]
    fn a_lost_disk_is_conjured_again() {
        let script = ";e 40.times { sleep 0.1; break if GameObj.loot.any? { |obj| obj.name =~ \
                      /#{Char.name} disk$/ } }; unless GameObj.loot.any? { |obj| obj.name =~ \
                      /#{Char.name} disk$/ }; disk = Spell[511]; wait_until { disk.affordable? \
                      }; disk.cast; end; move 'up'";
        let found = steps(script, 1);
        assert_eq!(found.len(), 4);
        assert_eq!(found[3].action, Action::Move("up".into()));
        assert_eq!(found[3].when, None);

        // Short of mana with the disk gone: the mana is waited for and the
        // disk cast, as upstream does. It is not skipped for the move.
        let short = cena_map::Walker {
            flags: [("own_disk_here".to_owned(), false)].into(),
            known_spells: Some(["Floating Disk".to_owned()].into()),
            active_spells: Some(std::collections::HashSet::new()),
            affordable_spells: Some(std::collections::HashSet::new()),
            ..cena_map::Walker::default()
        };
        let eligible: Vec<&Action> = found
            .iter()
            .filter(|step| step.when.as_ref().is_none_or(|when| when.holds(&short)))
            .map(|step| &step.action)
            .collect();
        assert_eq!(
            eligible,
            [
                &Action::WaitUntil(Cond::SpellAffordable("Floating Disk".into())),
                &Action::Cast("Floating Disk".into()),
                &Action::Move("up".into()),
            ]
        );
    }

    /// Priced: the key must be named, and then either it is worn or there is
    /// a sack to fetch it from. A worn key needs no sack.
    #[test]
    fn a_worn_key_needs_no_sack_to_be_priced() {
        let script = ";e refill_hand = false;key_worn = false;\n  (refill_hand = \
                      true;empty_hand) if !checkleft.nil? and !checkright.nil?;\n  key_worn = \
                      true if GameObj.inv.find {|obj| obj.name =~ /#{UserVars.key.split(' \
                      ').join('.*?')}/};\n  fput \"remove my #{UserVars.key}\" if key_worn;\n  \
                      fput \"get my #{UserVars.key} from my #{UserVars.key_sack}\" if \
                      !key_worn;\n  door = 'bright door';\n  multifput \"unlock #{door}\",\"open \
                      #{door}\",\"go #{door}\",\"close #{door}\",\"lock #{door}\";\n  fput \"wear \
                      my #{UserVars.key}\" if key_worn;\n  fput \"put my #{UserVars.key} in my \
                      #{UserVars.key_sack}\" if !key_worn;\n  fill_hand if refill_hand;";
        let door = crossing(script, 1).unwrap();
        let cost =
            crate::recognise::priced_for_crossing(&door, Some(cena_map::Cost::Fixed(0.2))).unwrap();
        let mut walker = cena_map::Walker {
            worn: Some(["a small brass key".to_owned()].into()),
            ..cena_map::Walker::default()
        };
        assert_eq!(cost.price(&walker), None, "which key is not said");
        walker.settings.insert("key".into(), "brass key".into());
        assert_eq!(cost.price(&walker), Some(0.2), "worn, so no sack is needed");
        walker.worn = Some(std::collections::HashSet::new());
        assert_eq!(
            cost.price(&walker),
            None,
            "not worn, and no sack to look in"
        );
        walker.settings.insert("key_sack".into(), "cloak".into());
        assert_eq!(cost.price(&walker), Some(0.2));
    }

    #[test]
    fn a_child_is_waited_for_only_where_the_move_failed() {
        let script = ";e if (bounty? =~ /^You have made contact with the child/); child = \
                      GameObj.npcs.find { |npc| npc.noun == 'child' }; else; child = nil; end; \
                      move 'southwest'; if Room.current == Room[7313]; 50.times { break if \
                      GameObj.npcs.any? { |npc| npc.id == child.id }; sleep 0.1 } if child; move \
                      'southwest'; end";
        let found = steps(script, 7313);
        assert_eq!(found[1].action, Action::AwaitFollowers);
        assert_eq!(found[1].when, Some(Cond::StillHere));
        assert_eq!(steps(script, 7314), [], "it watches another room");
    }

    #[test]
    fn the_wall_is_the_routines_one_argument() {
        let flight = "a flight of (ascending|descending) steps (curving along|leading straight \
                      through) the (northern|eastern|southern|western) wall";
        let script = format!(
            ";e clear\nput 'look'\nloop {{\nline = get\nif line =~ /{flight}, {flight}, {flight} \
             and {flight}\\./\nif $3 == 'eastern'\nmove 'climb steps'\nelsif $6 == \
             'eastern'\nmove 'climb second steps'\nelsif $9 == 'eastern'\nmove 'climb third \
             steps'\nelsif $12 == 'eastern'\nmove 'climb fourth steps'\nelse\necho \"error: \
             unique_map_movements can't find the right steps\"\nexit\nend\nbreak\nend\n}}"
        );
        assert_eq!(
            crossing(&script, 1),
            Some(Crossing::Routine(Routine::FlightOfSteps {
                wall: "eastern".into()
            }))
        );
        assert_eq!(crossing(&script.replacen("eastern", "western", 1), 1), None);
    }
}
