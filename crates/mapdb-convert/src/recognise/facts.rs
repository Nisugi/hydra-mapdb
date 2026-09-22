//! Arms that ask about the walker itself: its posture, race, citizenship,
//! level, society and gender (`plan/21` step 7, the vocabulary the long tail
//! showed was missing).

use cena_map::{Action, Cond, Cost, Crossing, Step};

use super::RoomFacts;
use super::costs::gated;
use super::{always, holes, is_plain_argument, is_word, quoted};

const SITTING: &str = "sitting";
const KNEELING: &str = "kneeling";

fn not(cond: Cond) -> Cond {
    Cond::Not(Box::new(cond))
}

fn otherwise(cond: Cond) -> Cond {
    Cond::Otherwise(Box::new(cond))
}

fn seconds(hole: &str) -> Option<f64> {
    hole.trim()
        .parse()
        .ok()
        .filter(|seconds: &f64| seconds.is_finite() && *seconds >= 0.0)
}

/// A name the game prints: a town, a race. Apostrophes are part of them.
fn is_name(hole: &str) -> bool {
    !hole.is_empty() && !hole.contains(['"', ';', '\n', '#', '\\'])
}

pub(super) fn crossing(script: &str, from: u32) -> Option<Crossing> {
    rowboat(script, from).or_else(|| low_crawl(script))
}

pub(super) fn cost(script: &str, room: &RoomFacts<'_>) -> Option<Cost> {
    in_a_boat(script, room.climate)
        .or_else(|| only_from_here(script, room))
        .or_else(|| with_the_key(script))
        .or_else(|| race_or_gender(script))
        .or_else(|| citizenship(script))
        .or_else(|| level(script))
        .or_else(|| society(script))
}

/// The lake: row if seated in a boat, swim if not. 83 exits. Which one is a
/// fact about the walker when it gets there, so both are steps; whether the
/// exit can be used at all is the cost's business ([`in_a_boat`]).
fn rowboat(script: &str, from: u32) -> Option<Crossing> {
    const FORMS: [[&str; 4]; 2] = [
        [
            ";e if checksitting;while Room.current.id == ",
            ";fput('row ",
            "');waitrt?;end;else;move('swim ",
            "');end;",
        ],
        [
            ";e if checksitting;while Room.current.id == ",
            ";fput('row ",
            "');waitrt?;end;else;move('swim ",
            "');end;fill_hand;",
        ],
    ];
    let (found, refill) = FORMS
        .iter()
        .zip([false, true])
        .find_map(|(form, refill)| Some((holes(script, form)?, refill)))?;
    let [room, row, swim] = found[..] else {
        return None;
    };
    (room.parse() == Ok(from) && row == swim && is_word(row)).then_some(())?;
    let seated = || Cond::Posture(SITTING.to_owned());
    let mut steps = vec![
        Step {
            action: Action::KeepMoving(format!("row {row}")),
            when: Some(seated()),
        },
        Step {
            action: Action::Move(format!("swim {swim}")),
            when: Some(otherwise(seated())),
        },
    ];
    if refill {
        steps.push(always(Action::FillHands));
    }
    Some(Crossing::Steps(steps))
}

/// A passage too low to stand in: kneel first, unless already kneeling or of
/// a race short enough not to need to. 38 exits, two spellings.
///
/// The guard is `Otherwise`: a walker whose race is not known kneels, which
/// always works, rather than trying it upright.
fn low_crawl(script: &str) -> Option<Crossing> {
    const SHORT: [&str; 3] = ["Dwarf", "Halfling", "Gnome"];
    let found = holes(
        script,
        &[
            ";e fput 'kneel' unless kneeling? or (Stats.race =~ /Dwarf|Halfling|Gnome/); move '",
            "'",
        ],
    )
    .or_else(|| {
        holes(
            script,
            &[
                ";e if not kneeling?;\tfput 'kneel' if Stats.race !~ /dwarf|halfling|gnome/i;\
                 end;fput '",
                "'",
            ],
        )
    })?;
    let command = found
        .first()
        .copied()
        .filter(|hole| is_plain_argument(hole))?;
    let mut low_enough = vec![Cond::Posture(KNEELING.to_owned())];
    low_enough.extend(SHORT.iter().map(|race| Cond::Race((*race).to_owned())));
    Some(Crossing::Steps(vec![
        Step {
            action: Action::Put("kneel".to_owned()),
            when: Some(otherwise(Cond::Any(low_enough))),
        },
        always(Action::Move(command.to_owned())),
    ]))
}

/// `checksitting && Room.current.climate == 'freshwater' ? 10 : 0.2`, and the
/// same with `nil` for 10. 103 exits.
///
/// **The climate is the room's, and the converter knows the room**, so that
/// half is settled here and never reaches the map. Off fresh water the test
/// is false and the cost is its plain branch.
fn in_a_boat(script: &str, climate: Option<&str>) -> Option<Cost> {
    let found = quoted(
        script,
        &[
            ";e checksitting && Room.current.climate == '",
            "' ? ",
            " : ",
            "",
        ],
    )?;
    let [water, seated, afoot] = found[..] else {
        return None;
    };
    is_word(water).then_some(())?;
    let afoot = seconds(afoot)?;
    if climate != Some(water) {
        return Some(Cost::Fixed(afoot));
    }
    let sitting = Cond::Posture(SITTING.to_owned());
    Some(if seated == "nil" {
        // Not by boat: this one can refuse, so unknown refuses too.
        Cost::Gated {
            when: not(sitting),
            then: afoot,
            otherwise: None,
        }
    } else {
        // Slower by boat, never refused.
        Cost::Gated {
            when: otherwise(sitting),
            then: afoot,
            otherwise: Some(seconds(seated)?),
        }
    })
}

/// The ways back out of the Hinterwilds and the Red Forest: open to the side
/// the walker came in by, *and only in the right place*. 4 exits. The place
/// is the room's, so it is tested here; were it ever false the script would
/// never open, and the exit is left unported rather than priced shut.
fn only_from_here(script: &str, room: &RoomFacts<'_>) -> Option<Cost> {
    if let Some(found) = holes(
        script,
        &[
            ";e UserVars.mapdb_hinterwilds_location == '",
            "' and Map.current.location.to_s =~ /the Hinterwilds/ ? ",
            " : nil;",
        ],
    ) {
        let [end, then] = found[..] else {
            return None;
        };
        (is_word(end) && room.location?.contains("the Hinterwilds")).then_some(())?;
        let came_by = Cond::Remembered("hinterwilds_location".to_owned(), end.to_owned());
        return gated(came_by, then);
    }
    let found = holes(
        script,
        &[
            ";e if (checkroom.to_s =~ /^\\[Red Forest/) and (UserVars.mapdb_redforest_location \
             == '",
            "'); ",
            "; else; nil; end",
        ],
    )?;
    let [side, then] = found[..] else {
        return None;
    };
    (is_word(side) && room.title?.starts_with("[Red Forest")).then_some(())?;
    gated(
        Cond::Remembered("redforest_location".to_owned(), side.to_owned()),
        then,
    )
}

/// A door that opens to whoever wears its key: 8 exits.
fn with_the_key(script: &str) -> Option<Cost> {
    let found = holes(
        script,
        &[
            ";e key=GameObj.inv.find{|k| k.name=='",
            "';};if !key.nil? then ",
            " else nil end;",
        ],
    )?;
    let [key, then] = found[..] else {
        return None;
    };
    is_name(key).then_some(())?;
    gated(Cond::Wearing(key.to_owned()), then)
}

fn race_or_gender(script: &str) -> Option<Cost> {
    if let Some(found) = quoted(script, &[";e Stats.race == '", "' ? ", " : nil"]) {
        let [race, then] = found[..] else {
            return None;
        };
        is_name(race).then_some(())?;
        return gated(Cond::Race(race.to_owned()), then.trim());
    }
    // `!defined?(Stats.gender) or` lets an unknown walker through upstream;
    // here unknown is impassable, like every other unknown.
    let found = quoted(
        script,
        &[
            ";e ((!defined?(Stats.gender) or Stats.gender == '",
            "') ? ",
            " : nil);",
        ],
    )?;
    let [gender, then] = found[..] else {
        return None;
    };
    is_word(gender).then_some(())?;
    gated(Cond::Gender(gender.to_owned()), then)
}

/// Citizens only -- 7 exits, four spellings, two of which also let someone
/// else through: a walker the profile says Vornavis admits, or one unseen.
fn citizenship(script: &str) -> Option<Cost> {
    let citizen = |town: &str| is_name(town).then(|| Cond::Citizenship(town.to_owned()));
    if let Some(found) = holes(
        script,
        &[
            ";e ((Char.citizenship == \"",
            "\" or !UserVars.mapdb_",
            ".nil?) ? ",
            " : nil);",
        ],
    ) {
        let [town, setting, then] = found[..] else {
            return None;
        };
        is_word(setting).then_some(())?;
        let admitted = Cond::SettingIsSet(setting.to_owned());
        return gated(Cond::Any(vec![citizen(town)?, admitted]), then);
    }
    if let Some(found) = holes(
        script,
        &[
            ";e if Char.citizenship == \"",
            "\" || invisible? ; ",
            "; else; nil; end",
        ],
    ) {
        let [town, then] = found[..] else {
            return None;
        };
        let unseen = Cond::Flag("invisible".to_owned());
        return gated(Cond::Any(vec![citizen(town)?, unseen]), then);
    }
    let found = holes(script, &[";e Char.citizenship == \"", "\" ? ", " : nil"])
        .or_else(|| holes(script, &[";e Char.citizenship == \"", "\" ? ", " : nil;"]))?;
    let [town, then] = found[..] else {
        return None;
    };
    gated(citizen(town)?, then)
}

/// `XMLData.level > 19 ? 33.2 : nil ` -- with upstream's trailing space. 2 exits.
fn level(script: &str) -> Option<Cost> {
    let [over, then] = holes(script, &[";e XMLData.level > ", " ? ", " : nil "])?[..] else {
        return None;
    };
    let at_least = over.parse::<u32>().ok()?.checked_add(1)?;
    gated(Cond::LevelAtLeast(at_least), then)
}

/// The Order of Voln: 39 exits. `rank == 26` is the top of the order, so "at
/// least" says the same and survives the order gaining a rank.
fn society(script: &str) -> Option<Cost> {
    if let Some(found) = holes(
        script,
        &[
            ";e if Society.status == '",
            "' and Society.rank == ",
            " and $go2_use_seeking; ",
            "; else; nil; end",
        ],
    ) {
        let [order, rank, then] = found[..] else {
            return None;
        };
        is_name(order).then_some(())?;
        return gated(
            Cond::All(vec![
                Cond::Setting("use_seeking".to_owned(), "true".to_owned()),
                Cond::Society(order.to_owned()),
                Cond::SocietyRankAtLeast(rank.parse().ok()?),
            ]),
            then,
        );
    }
    if let Some(found) = holes(script, &[";e Society.rank == ", " ? ", " : nil"]) {
        let [rank, then] = found[..] else {
            return None;
        };
        return gated(Cond::SocietyRankAtLeast(rank.parse().ok()?), then);
    }
    let [order, then] = holes(script, &[";e Society.status == \"", "\" ? ", " : nil"])?[..] else {
        return None;
    };
    is_name(order).then_some(())?;
    gated(Cond::Society(order.to_owned()), then)
}

#[cfg(test)]
mod tests {
    use cena_map::{Walker, moves_whatever_is_known};

    use super::*;

    #[test]
    fn a_rowboat_rows_when_seated_and_swims_otherwise() {
        let script = ";e if checksitting;while Room.current.id == 13966;fput('row southeast');\
                      waitrt?;end;else;move('swim southeast');end;";
        let Some(Crossing::Steps(steps)) = crossing(script, 13966) else {
            panic!("not recognised");
        };
        assert!(
            moves_whatever_is_known(&steps),
            "nobody is left in the water"
        );
        assert_eq!(steps[0].action, Action::KeepMoving("row southeast".into()));
        assert_eq!(steps[1].action, Action::Move("swim southeast".into()));
        let seated = Walker {
            posture: Some("sitting".into()),
            ..Walker::default()
        };
        let fires = |walker: &Walker| {
            steps
                .iter()
                .map(|step| step.when.as_ref().is_none_or(|when| when.holds(walker)))
                .collect::<Vec<_>>()
        };
        assert_eq!(fires(&seated), [true, false]);
        assert_eq!(fires(&Walker::default()), [false, true]);
        assert_eq!(crossing(script, 1), None, "it names another room");
    }

    #[test]
    fn a_low_crawl_kneels_unless_known_not_to_need_to() {
        let script = ";e fput 'kneel' unless kneeling? or (Stats.race =~ \
                      /Dwarf|Halfling|Gnome/); move 'southeast'";
        let Some(Crossing::Steps(steps)) = crossing(script, 1) else {
            panic!("not recognised");
        };
        let kneels = |walker: &Walker| steps[0].when.as_ref().unwrap().holds(walker);
        let of = |race: &str| Walker {
            race: Some(race.into()),
            posture: Some("standing".into()),
            ..Walker::default()
        };
        assert!(kneels(&of("Giantman")));
        assert!(!kneels(&of("Forest Gnome")), "a word of the race is enough");
        assert!(
            kneels(&Walker::default()),
            "not known: kneel, it always works"
        );
        assert_eq!(steps[1].action, Action::Move("southeast".into()));
    }

    /// The room's half of the question is answered by the converter.
    #[test]
    fn a_boat_cost_depends_on_the_water_the_room_is_on() {
        let slower = ";e checksitting && Room.current.climate == 'freshwater' ? 10 : 0.2";
        let barred = ";e checksitting && Room.current.climate == 'freshwater' ? nil : 0.2";
        assert_eq!(
            cost(
                slower,
                &RoomFacts {
                    climate: Some("temperate"),
                    ..RoomFacts::default()
                }
            ),
            Some(Cost::Fixed(0.2))
        );
        assert_eq!(cost(barred, &RoomFacts::default()), Some(Cost::Fixed(0.2)));

        let seated = Walker {
            posture: Some("sitting".into()),
            ..Walker::default()
        };
        let afoot = Walker {
            posture: Some("standing".into()),
            ..Walker::default()
        };
        let nobody = Walker::default();
        let slower = cost(
            slower,
            &RoomFacts {
                climate: Some("freshwater"),
                ..RoomFacts::default()
            },
        )
        .unwrap();
        assert_eq!(slower.price(&seated), Some(10.0));
        assert_eq!(slower.price(&afoot), Some(0.2));
        assert_eq!(
            slower.price(&nobody),
            Some(0.2),
            "both pass, so nobody is refused"
        );
        let barred = cost(
            barred,
            &RoomFacts {
                climate: Some("freshwater"),
                ..RoomFacts::default()
            },
        )
        .unwrap();
        assert_eq!(barred.price(&seated), None);
        assert_eq!(barred.price(&afoot), Some(0.2));
        assert_eq!(
            barred.price(&nobody),
            None,
            "this one can refuse, so unknown does"
        );
    }

    #[test]
    fn who_the_walker_is() {
        let gate = |script| match cost(script, &RoomFacts::default()) {
            Some(Cost::Gated {
                when,
                then,
                otherwise: None,
            }) => Some((when, then)),
            _ => None,
        };
        assert_eq!(
            gate(";e Stats.race == 'Giantman' ? 0.2 : nil"),
            Some((Cond::Race("Giantman".into()), 0.2))
        );
        assert_eq!(
            gate(";e Char.citizenship == \"Wehnimer's Landing\" ? 0.2 : nil"),
            Some((Cond::Citizenship("Wehnimer's Landing".into()), 0.2))
        );
        assert_eq!(
            gate(";e XMLData.level > 19 ? 33.2 : nil "),
            Some((Cond::LevelAtLeast(20), 33.2))
        );
        assert_eq!(
            gate(";e Society.rank == 26 ? 0.2 : nil"),
            Some((Cond::SocietyRankAtLeast(26), 0.2))
        );
        let seeking = gate(
            ";e if Society.status == 'Order of Voln' and Society.rank == 26 and \
             $go2_use_seeking; 2.8; else; nil; end",
        );
        assert!(
            matches!(seeking, Some((Cond::All(parts), then)) if parts.len() == 3 && then > 2.0)
        );
        assert_eq!(gate(";e Stats.race == 'Giantman' ? 0.2 : 5"), None);
    }
}
