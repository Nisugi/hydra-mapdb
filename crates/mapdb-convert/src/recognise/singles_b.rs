//! More crossings upstream wrote once (`super::singles` has the rule): the
//! last of the group's ways, the caravans, and two gates that are routines.

use cena_map::{Action, Cond, Crossing, Routine, Step};

use super::always;
use super::moves::cast_if_able;

pub(super) fn crossing(script: &str) -> Option<Crossing> {
    verbatim(script).or_else(|| caravan(script))
}

fn when(action: Action, cond: Cond) -> Step {
    Step {
        action,
        when: Some(cond),
    }
}

fn put(command: &str) -> Step {
    always(Action::Put(command.to_owned()))
}

fn go(command: &str) -> Step {
    always(Action::Move(command.to_owned()))
}

fn stand_up() -> Step {
    when(
        Action::Put("stand".to_owned()),
        Cond::Not(Box::new(Cond::Posture("standing".to_owned()))),
    )
}

fn unstunned() -> Step {
    always(Action::WaitUntil(Cond::Otherwise(Box::new(Cond::Flag(
        "stunned".to_owned(),
    )))))
}

fn verbatim(script: &str) -> Option<Crossing> {
    const CREVICE: &str = include_str!("../upstream_scripts/crevice.rb");
    const ABYSS: &str = include_str!("../upstream_scripts/abyss.rb");
    const DARK_OPENING: &str = include_str!("../upstream_scripts/dark_opening.rb");
    const CLIMB_OPENING: &str = include_str!("../upstream_scripts/climb_opening.rb");
    const SHADOW: &str = include_str!("../upstream_scripts/shadow.rb");
    const GATE_IN: &str = include_str!("../upstream_scripts/bronze_gate_in.rb");
    const GATE_OUT: &str = include_str!("../upstream_scripts/bronze_gate_out.rb");
    const COLOUR_BARRIER: &str = include_str!("../upstream_scripts/colour_barrier.rb");
    let steps = match script {
        // Only the way out has upstream's Warrior batter it.
        GATE_IN => return Some(Crossing::Routine(Routine::BronzeGate { batter: false })),
        GATE_OUT => return Some(Crossing::Routine(Routine::BronzeGate { batter: true })),
        COLOUR_BARRIER => return Some(Crossing::Routine(Routine::ColourBarrier)),
        // The crevice shows to a search, sometimes; stepping out and back
        // between searches is what upstream does to try again. It points the
        // crevice out to a group, which whoever leads one has a flag for.
        CREVICE => vec![
            cast_if_able("Celerity"),
            always(Action::RoundUntil {
                commands: vec!["north".to_owned(), "south".to_owned(), "search".to_owned()],
                until: vec!["you discover a narrow crevice".to_owned()],
                tries: None,
            }),
            when(
                Action::Put("point crevice".to_owned()),
                Cond::Flag("leading_group".to_owned()),
            ),
            go("go crevice"),
            always(Action::AwaitFollowers),
        ],
        ABYSS => vec![
            stand_up(),
            go("jump abyss"),
            unstunned(),
            stand_up(),
            always(Action::AwaitFollowers),
        ],
        // In the dark: yell the names, feel for the opening, crawl until
        // there is a way south.
        DARK_OPENING => vec![
            put("yell jaron galarn"),
            always(Action::PutUntil {
                command: "search".to_owned(),
                until: vec!["you stumble upon a low opening".to_owned()],
                tries: None,
            }),
            always(Action::MoveWhile(
                "go opening".to_owned(),
                Cond::Not(Box::new(Cond::Exit("s".to_owned()))),
            )),
            stand_up(),
            always(Action::AwaitFollowers),
        ],
        CLIMB_OPENING => vec![
            always(Action::EmptyHands),
            go("climb opening"),
            always(Action::FillHands),
            always(Action::AwaitFollowers),
        ],
        SHADOW => vec![
            go("say shadow bind my soul"),
            always(Action::AwaitFollowers),
        ],
        _ => return None,
    };
    Some(Crossing::Steps(steps))
}

/// The caravans to and from the Sea of Fire: order by the destination's name,
/// confirm, ride. 4 exits in two scripts that differ only in the name. The
/// wagon is a room of its own, so the ride is two arrivals. **It can be
/// waylaid**, which lands the walker somewhere else with bandits in the room:
/// upstream shouts and stops, and what a walker does about hostiles is the
/// walker's, so here it only plans again. The exits' costs ask that the fare
/// is arranged (`car_to_sos`, `car_from_sos`).
fn caravan(script: &str) -> Option<Crossing> {
    const FROM: (&str, &str) = (
        include_str!("../upstream_scripts/caravan_from_sos.rb"),
        "Wehnimer's Landing",
    );
    // The same, but for how it marks its shout bold.
    const FROM_B: (&str, &str) = (
        include_str!("../upstream_scripts/caravan_from_sos_b.rb"),
        "Vornavis",
    );
    const TO: (&str, &str) = (
        include_str!("../upstream_scripts/caravan_to_sos.rb"),
        "the Sea of Fire",
    );
    let (_, after) = script.split_once("\"inquire\", 5, /(\\d)\\) ")?;
    let (named, _) = after.split_once("/;if res")?;
    named
        .chars()
        .all(|c| c.is_ascii_alphabetic() || matches!(c, ' ' | '\''))
        .then_some(())?;
    [FROM, FROM_B, TO]
        .iter()
        .any(|(template, town)| template.replace(town, named) == script)
        .then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::OrderByName(named.to_owned())),
        put("order confirm"),
        always(Action::AwaitArrival),
        always(Action::AwaitArrival),
        always(Action::Replan),
    ]))
}

#[cfg(test)]
mod tests {
    use cena_map::moves_whatever_is_known;

    use super::*;

    #[test]
    fn every_verbatim_script_is_ported_and_moves_the_walker() {
        for script in [
            include_str!("../upstream_scripts/crevice.rb"),
            include_str!("../upstream_scripts/abyss.rb"),
            include_str!("../upstream_scripts/dark_opening.rb"),
            include_str!("../upstream_scripts/climb_opening.rb"),
            include_str!("../upstream_scripts/shadow.rb"),
            include_str!("../upstream_scripts/caravan_to_sos.rb"),
        ] {
            let Some(Crossing::Steps(steps)) = crossing(script) else {
                panic!("unported: {script}");
            };
            assert!(moves_whatever_is_known(&steps), "{script}");
            assert_eq!(crossing(&format!("{script} ")), None);
        }
    }

    #[test]
    fn a_caravan_is_ordered_by_the_name_of_where_it_goes() {
        let landing = include_str!("../upstream_scripts/caravan_from_sos.rb");
        let vornavis = landing.replace("Wehnimer's Landing", "Vornavis");
        for (script, named) in [(landing, "Wehnimer's Landing"), (&vornavis, "Vornavis")] {
            let Some(Crossing::Steps(steps)) = crossing(script) else {
                panic!("unported: {named}");
            };
            assert_eq!(steps[0].action, Action::OrderByName(named.to_owned()));
        }
        // Asking about one town and ordering another is not this script.
        assert_eq!(crossing(&landing.replacen("Landing", "Harbor", 1)), None);
    }

    #[test]
    fn only_the_way_out_of_the_graveyard_is_battered() {
        assert_eq!(
            crossing(include_str!("../upstream_scripts/bronze_gate_out.rb")),
            Some(Crossing::Routine(Routine::BronzeGate { batter: true }))
        );
        assert_eq!(
            crossing(include_str!("../upstream_scripts/bronze_gate_in.rb")),
            Some(Crossing::Routine(Routine::BronzeGate { batter: false }))
        );
    }
}
