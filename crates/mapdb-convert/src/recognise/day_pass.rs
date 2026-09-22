//! The Chronomage day passes and the errands: crossings that stop to make
//! another trip. Named here and pinned verbatim, as the puzzles are
//! (`super::named`); run by the Travel behaviour's stack of trips.
//!
//! The day pass's **cost** is ported too, and is where upstream and Hydra
//! part ways: upstream's cost script opens the sack and reads every pass
//! before it answers. A price here cannot send commands, so which passes the
//! walker holds is a flag the planner fills in before it prices anything.

use cena_map::{Cond, Cost, Crossing, Errand, Routine, Rung};

use super::holes;

const TOWNS: [(&str, &str); 3] = [
    ("wl", "Wehnimer's Landing"),
    ("imt", "Icemule Trace"),
    ("sol", "Solhaven"),
];

pub(super) fn crossing(script: &str) -> Option<Crossing> {
    const PASSES: [(&str, &str); 6] = [
        (
            include_str!("../upstream_scripts/day_pass_wl_imt.rb"),
            "wl,imt",
        ),
        (
            include_str!("../upstream_scripts/day_pass_wl_sol.rb"),
            "wl,sol",
        ),
        (
            include_str!("../upstream_scripts/day_pass_sol_wl.rb"),
            "sol,wl",
        ),
        (
            include_str!("../upstream_scripts/day_pass_sol_imt.rb"),
            "sol,imt",
        ),
        (
            include_str!("../upstream_scripts/day_pass_imt_wl.rb"),
            "imt,wl",
        ),
        (
            include_str!("../upstream_scripts/day_pass_imt_sol.rb"),
            "imt,sol",
        ),
    ];
    const ERRANDS: [(&str, Errand); 5] = [
        (
            include_str!("../upstream_scripts/errand_giant_to_rr.rb"),
            Errand::GiantToRiversRest,
        ),
        (
            include_str!("../upstream_scripts/errand_giant_from_rr.rb"),
            Errand::GiantFromRiversRest,
        ),
        (
            include_str!("../upstream_scripts/errand_cutter_marshtown.rb"),
            Errand::CutterFromMarshtown,
        ),
        (
            include_str!("../upstream_scripts/errand_cutter_rivers_rest.rb"),
            Errand::CutterFromRiversRest,
        ),
        (
            include_str!("../upstream_scripts/errand_sword_gorge.rb"),
            Errand::SwordInTheGorge,
        ),
    ];
    let routine = if let Some((_, route)) = PASSES.iter().find(|(pinned, _)| *pinned == script) {
        Routine::DayPass {
            route: (*route).to_owned(),
        }
    } else {
        let (_, errand) = ERRANDS.iter().find(|(pinned, _)| *pinned == script)?;
        Routine::Errand { errand: *errand }
    };
    Some(Crossing::Routine(routine))
}

/// The flag that says the walker holds a valid pass between these two towns,
/// which is the same pass either way: the codes in alphabetical order.
fn holds_a_pass(from: &str, to: &str) -> Cond {
    let (first, second) = if from < to { (from, to) } else { (to, from) };
    Cond::Flag(format!("day_pass:{first},{second}"))
}

/// A town's name, quoted either way, as its code.
fn code_of(quoted: &str) -> Option<&'static str> {
    let name = quoted
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| quoted.strip_prefix('\'')?.strip_suffix('\''))?;
    TOWNS
        .iter()
        .find(|(_, town)| *town == name)
        .map(|(code, _)| *code)
}

/// 6 exits. With `use_day_pass` on: the first price for a walker holding a
/// valid pass, the second for one whose profile says to buy -- `yes`, `true`,
/// or this route's code. Upstream's setting may list several routes; here it
/// names one, or says yes to all. Anyone else is refused.
pub(super) fn cost(script: &str) -> Option<Cost> {
    const HEADS: [&str; 2] = [
        include_str!("../upstream_scripts/day_pass_cost_head.rb"),
        // The same, but for one line's indentation.
        include_str!("../upstream_scripts/day_pass_cost_head_b.rb"),
    ];
    let tail = HEADS.iter().find_map(|head| script.strip_prefix(head))?;
    let found = holes(
        tail,
        &[
            "   if $mapdb_day_passes.any? { |id,h| h[:towns].include?(",
            ") and h[:towns].include?(",
            ") and h[:expires] > (Time.now + 10) }\n      ",
            "\n   elsif UserVars.mapdb_buy_day_pass.to_s =~ /^(yes|true)$|\\b",
            "\\b/i\n      ",
            "\n   else\n      nil\n   end\nelse\n   nil\nend\n\n",
        ],
    )?;
    let [one, other, held, route, bought] = found[..] else {
        return None;
    };
    let (from, to) = route.split_once(',')?;
    let mut named = [code_of(one)?, code_of(other)?];
    let mut routed = [from, to];
    named.sort_unstable();
    routed.sort_unstable();
    (named == routed && from != to).then_some(())?;
    let setting = |name: &str, value: &str| Cond::Setting(name.to_owned(), value.to_owned());
    let using = || setting("use_day_pass", "true");
    let buying = Cond::Any(vec![
        setting("buy_day_pass", "yes"),
        setting("buy_day_pass", "true"),
        setting("buy_day_pass", route),
    ]);
    let price = |text: &str| {
        text.parse()
            .ok()
            .filter(|s: &f64| s.is_finite() && *s >= 0.0)
    };
    Some(Cost::Ladder {
        ladder: vec![
            Rung {
                when: Cond::All(vec![using(), holds_a_pass(from, to)]),
                then: price(held)?,
            },
            Rung {
                when: Cond::All(vec![using(), buying]),
                then: price(bought)?,
            },
        ],
        otherwise: None,
    })
}

#[cfg(test)]
mod tests {
    use cena_map::Walker;

    use super::*;

    fn script(tail: &str) -> String {
        format!(
            "{}{tail}",
            include_str!("../upstream_scripts/day_pass_cost_head.rb")
        )
    }

    const TAIL: &str = "   if $mapdb_day_passes.any? { |id,h| h[:towns].include?(\"Wehnimer's \
                        Landing\") and h[:towns].include?('Icemule Trace') and h[:expires] > \
                        (Time.now + 10) }\n      0.8\n   elsif UserVars.mapdb_buy_day_pass.to_s \
                        =~ /^(yes|true)$|\\bimt,wl\\b/i\n      8.8\n   else\n      nil\n   \
                        end\nelse\n   nil\nend\n\n";

    #[test]
    fn a_pass_held_is_cheap_one_to_buy_is_dearer_and_neither_is_shut() {
        let cost = cost(&script(TAIL)).unwrap();
        let mut walker = Walker::default();
        assert_eq!(cost.price(&walker), None);
        walker.settings.insert("use_day_pass".into(), "true".into());
        assert_eq!(
            cost.price(&walker),
            None,
            "nothing held, nothing to buy with"
        );
        walker
            .settings
            .insert("buy_day_pass".into(), "imt,wl".into());
        assert_eq!(cost.price(&walker), Some(8.8));
        // The same pass serves both ways, so the flag names the towns in order.
        walker.flags.insert("day_pass:imt,wl".into(), true);
        assert_eq!(cost.price(&walker), Some(0.8));
        walker
            .settings
            .insert("use_day_pass".into(), "false".into());
        assert_eq!(cost.price(&walker), None, "the profile says not to");
    }

    #[test]
    fn a_route_that_is_not_the_towns_named_is_another_script() {
        assert_eq!(cost(&script(&TAIL.replace("imt,wl", "imt,sol"))), None);
        assert_eq!(
            cost(&script(&TAIL.replace("Icemule Trace", "Ta'Illistim"))),
            None
        );
    }

    #[test]
    fn the_pinned_crossings_name_their_route_and_their_errand() {
        assert_eq!(
            crossing(include_str!("../upstream_scripts/day_pass_sol_imt.rb")),
            Some(Crossing::Routine(Routine::DayPass {
                route: "sol,imt".into()
            }))
        );
        assert_eq!(
            crossing(include_str!("../upstream_scripts/errand_sword_gorge.rb")),
            Some(Crossing::Routine(Routine::Errand {
                errand: Errand::SwordInTheGorge
            }))
        );
    }
}
