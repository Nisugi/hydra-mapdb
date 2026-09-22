//! Arms for slice d of the long tail (`research/mapdb-inventory/tail/slice_d_costs.tsv`).
//!
//! Same rules as every arm (`super`): the upstream script verbatim, with
//! holes; an exact match or none. This file's tests live at its foot.

use cena_map::{Cond, Cost};

use super::costs::gated;
use super::{holes, is_word};

/// The gate for a cost script in this slice, if an arm here knows it.
pub(super) fn cost(script: &str) -> Option<Cost> {
    premium(script)
        .or_else(|| water_walking(script))
        .or_else(|| setting_is(script))
        .or_else(|| setting_true_or_yes(script))
        .or_else(|| setting_named(script))
        .or_else(|| spell_known(script))
        .or_else(|| skill_at_least(script))
}

fn seconds(hole: &str) -> Option<f64> {
    hole.parse()
        .ok()
        .filter(|seconds: &f64| seconds.is_finite() && *seconds >= 0.0)
}

/// `UserVars.mapdb_premium.nil? ? 10 : 0.2` -- 9 exits. Open to everyone,
/// quicker for a profile that says the account is premium. A profile always
/// answers "is it set", so this is never impassable.
fn premium(script: &str) -> Option<Cost> {
    let [without, with] = holes(script, &[";e UserVars.mapdb_premium.nil? ? ", " : ", ";"])?[..]
    else {
        return None;
    };
    Some(Cost::Gated {
        when: Cond::SettingIsSet("premium".to_owned()),
        then: seconds(with)?,
        otherwise: Some(seconds(without)?),
    })
}

/// `checkspell(112) ? 0.2 : 2.0` -- 8 exits: across the water quickly with
/// Water Walking up, slowly without. The number is part of the template;
/// spells are named here, not numbered.
///
/// **Both prices are passable, so this gate must never refuse anyone**
/// (`cena_map::step`, the two kinds of check: this only changes *how long*).
/// Asked as "is Water Walking up", a walker whose spells are unknown would be
/// priced impassable; asked as `Otherwise`, which is never unknown, that
/// walker pays the slow price.
fn water_walking(script: &str) -> Option<Cost> {
    let [quick, slow] = holes(script, &[";e checkspell(112) ? ", " : ", ""])?[..] else {
        return None;
    };
    let walking = Cond::SpellActive("Water Walking".to_owned());
    Some(Cost::Gated {
        when: Cond::Otherwise(Box::new(walking)),
        then: seconds(slow)?,
        otherwise: Some(seconds(quick)?),
    })
}

/// A profile setting compared with one value, four ways -- 9 exits. Three
/// of the four names (`Cuddfan_Hollow` and the like) carry no `mapdb_` prefix
/// upstream and are kept as written. `UserVars.mapdb_car_from_sos ? ..` is
/// Ruby truthiness; its partner `car_to_sos` tests `== true`, so both are read
/// as "the setting is true".
fn setting_is(script: &str) -> Option<Cost> {
    const EQUALS: [&str; 4] = [";e UserVars.mapdb_", " == '", "' ? ", " : nil"];
    const NOT_EQUALS: [&str; 4] = [";e (UserVars.", ".to_s != '", "' ? nil : ", ")"];
    const TRUTHY: [&str; 3] = [";e UserVars.mapdb_", " ? ", " : nil"];
    const IS_TRUE: [&str; 3] = [";e if UserVars.mapdb_", " == true;", ";else;nil;end"];
    let (name, value, then) =
        if let Some(found) = holes(script, &EQUALS).or_else(|| holes(script, &NOT_EQUALS)) {
            let [name, value, then] = found[..] else {
                return None;
            };
            (name, value, then)
        } else {
            let found = holes(script, &TRUTHY).or_else(|| holes(script, &IS_TRUE))?;
            let [name, then] = found[..] else {
                return None;
            };
            (name, "true", then)
        };
    if !is_word(name) || !is_word(value) {
        return None;
    }
    gated(Cond::Setting(name.to_owned(), value.to_owned()), then)
}

/// `if (UserVars.mapdb_black_swan == true) or (.. == 'yes')` -- 2 exits.
fn setting_true_or_yes(script: &str) -> Option<Cost> {
    let [name, again, then] = holes(
        script,
        &[
            ";e if (UserVars.mapdb_",
            " == true) or (UserVars.mapdb_",
            " == 'yes'); ",
            "; else; nil; end",
        ],
    )?[..] else {
        return None;
    };
    if !is_word(name) || name != again {
        return None;
    }
    let is = |value: &str| Cond::Setting(name.to_owned(), value.to_owned());
    gated(Cond::Any(vec![is("true"), is("yes")]), then)
}

/// `UserVars.Shivergale.nil? ? nil : 0.2` -- 4 exits into private property:
/// open once the profile names it at all.
fn setting_named(script: &str) -> Option<Cost> {
    let found = holes(script, &[";e UserVars.", ".nil? ? nil : ", ";"])
        .or_else(|| holes(script, &[";e UserVars.", ".nil? ? nil : ", ""]))?;
    let [name, then] = found[..] else {
        return None;
    };
    is_word(name).then_some(())?;
    gated(Cond::SettingIsSet(name.to_owned()), then)
}

/// `Spell[920].known? ? 20 : nil` -- 1 exit.
fn spell_known(script: &str) -> Option<Cost> {
    let [then] = holes(script, &[";e Spell[920].known? ? ", " : nil"])?[..] else {
        return None;
    };
    gated(Cond::SpellKnown("Call Familiar".to_owned()), then)
}

/// A skill floor -- 5 exits. `>= 30` is "not under 30"; `> 99` is "not under
/// 100". One form opens with `!defined?(Skills) or`, which lets a walker of
/// unknown skills through; here unknown is impassable like every other
/// unknown.
fn skill_at_least(script: &str) -> Option<Cost> {
    const AT_LEAST: [&str; 4] = [";e if Skills.", " >= ", "; ", "; else nil; end"];
    const OVER: [&str; 5] = [
        ";e ((defined?(Skills.",
        ") and Skills.",
        " > ",
        ") ? ",
        " : nil)",
    ];
    const OVER_OR_UNDEFINED: [&str; 3] = [
        ";e ((!defined?(Skills) or !defined?(Skills.climbing) or (defined?(Skills) and \
         defined?(Skills.climbing) and Skills.climbing > ",
        ")) ? ",
        " : nil)",
    ];
    let (skill, floor, then) = if let Some(found) = holes(script, &AT_LEAST) {
        let [skill, ranks, then] = found[..] else {
            return None;
        };
        (skill, ranks.parse::<u32>().ok()?, then)
    } else if let Some(found) = holes(script, &OVER) {
        let [skill, again, ranks, then] = found[..] else {
            return None;
        };
        (skill == again).then_some(())?;
        (skill, ranks.parse::<u32>().ok()?.checked_add(1)?, then)
    } else {
        let [ranks, then] = holes(script, &OVER_OR_UNDEFINED)?[..] else {
            return None;
        };
        ("climbing", ranks.parse::<u32>().ok()?.checked_add(1)?, then)
    };
    is_word(skill).then_some(())?;
    let under = Cond::SkillUnder(skill.to_owned(), floor);
    gated(Cond::Not(Box::new(under)), then)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate(when: Cond, then: f64, otherwise: Option<f64>) -> Cost {
        Cost::Gated {
            when,
            then,
            otherwise,
        }
    }

    fn setting(name: &str, value: &str) -> Cond {
        Cond::Setting(name.into(), value.into())
    }

    #[test]
    fn premium_is_quicker_and_nobody_is_refused() {
        assert_eq!(
            cost(";e UserVars.mapdb_premium.nil? ? 10 : 0.2;"),
            Some(gate(Cond::SettingIsSet("premium".into()), 0.2, Some(10.0)))
        );
        assert_eq!(cost(";e UserVars.mapdb_premium.nil? ? 10 : nil;"), None);
        assert_eq!(cost(";e UserVars.mapdb_premium.nil? ? -1 : 0.2;"), None);
    }

    #[test]
    fn water_walking_is_the_quick_way_across() {
        assert_eq!(
            cost(";e checkspell(112) ? 0.2 : 2.0"),
            Some(gate(
                Cond::Otherwise(Box::new(Cond::SpellActive("Water Walking".into()))),
                2.0,
                Some(0.2)
            ))
        );
        // Nobody is refused: not even a walker whose spells nobody has read.
        let priced = cost(";e checkspell(112) ? 0.2 : 2.0").unwrap();
        assert_eq!(priced.price(&cena_map::Walker::default()), Some(2.0));
        let up = cena_map::Walker {
            active_spells: Some(["Water Walking".to_owned()].into()),
            ..cena_map::Walker::default()
        };
        assert_eq!(priced.price(&up), Some(0.2));
        assert_eq!(cost(";e checkspell(113) ? 0.2 : 2.0"), None);
        assert_eq!(cost(";e checkspell(112) ? 0.2 : 2.0; exit"), None);
    }

    #[test]
    fn a_setting_is_compared_with_one_value() {
        for (script, name, value, then) in [
            (
                ";e UserVars.mapdb_annoy_ylandra == 'yes' ? 0.2 : nil",
                "annoy_ylandra",
                "yes",
                0.2,
            ),
            (
                ";e (UserVars.Cuddfan_Hollow.to_s != 'true' ? nil : 0.2)",
                "Cuddfan_Hollow",
                "true",
                0.2,
            ),
            (
                ";e UserVars.mapdb_car_from_sos ? 120 : nil",
                "car_from_sos",
                "true",
                120.0,
            ),
            (
                ";e if UserVars.mapdb_car_to_sos == true;120;else;nil;end",
                "car_to_sos",
                "true",
                120.0,
            ),
        ] {
            assert_eq!(
                cost(script),
                Some(gate(setting(name, value), then, None)),
                "{script}"
            );
        }
        // A second test in the hole is not a name.
        assert_eq!(
            cost(
                ";e UserVars.mapdb_hinterwilds_location == 'EN' and Map.current.location.to_s =~ /the Hinterwilds/ ? 240 : nil;"
            ),
            None
        );
        assert_eq!(cost(";e UserVars.mapdb_a and b ? 120 : nil"), None);
    }

    #[test]
    fn true_or_yes_is_either() {
        assert_eq!(
            cost(
                ";e if (UserVars.mapdb_black_swan == true) or (UserVars.mapdb_black_swan == 'yes'); 0.2; else; nil; end"
            ),
            Some(gate(
                Cond::Any(vec![
                    setting("black_swan", "true"),
                    setting("black_swan", "yes")
                ]),
                0.2,
                None
            ))
        );
        assert_eq!(
            cost(
                ";e if (UserVars.mapdb_black_swan == true) or (UserVars.mapdb_other == 'yes'); 0.2; else; nil; end"
            ),
            None
        );
    }

    #[test]
    fn a_named_property_is_open() {
        for script in [
            ";e UserVars.Shivergale.nil? ? nil : 0.2;",
            ";e UserVars.Shivergale.nil? ? nil : 0.2",
        ] {
            assert_eq!(
                cost(script),
                Some(gate(Cond::SettingIsSet("Shivergale".into()), 0.2, None))
            );
        }
        assert_eq!(cost(";e UserVars.Shiver gale.nil? ? nil : 0.2"), None);
    }

    #[test]
    fn a_known_spell_opens_the_exit() {
        assert_eq!(
            cost(";e Spell[920].known? ? 20 : nil"),
            Some(gate(Cond::SpellKnown("Call Familiar".into()), 20.0, None))
        );
        assert_eq!(cost(";e Spell[921].known? ? 20 : nil"), None);
    }

    #[test]
    fn a_skill_floor_is_not_under() {
        let at_least =
            |skill: &str, ranks| Cond::Not(Box::new(Cond::SkillUnder(skill.into(), ranks)));
        assert_eq!(
            cost(";e if Skills.perception >= 30; 0.2; else nil; end"),
            Some(gate(at_least("perception", 30), 0.2, None))
        );
        assert_eq!(
            cost(";e ((defined?(Skills.climbing) and Skills.climbing > 99) ? 0.2 : nil)"),
            Some(gate(at_least("climbing", 100), 0.2, None))
        );
        assert_eq!(
            cost(
                ";e ((!defined?(Skills) or !defined?(Skills.climbing) or (defined?(Skills) and defined?(Skills.climbing) and Skills.climbing > 14)) ? 0.2 : nil)"
            ),
            Some(gate(at_least("climbing", 15), 0.2, None))
        );
        // Arithmetic in the hole is not a number of ranks.
        assert_eq!(
            cost(
                ";e if Skills.climbing >= [XMLData.encumbrance_value/1.25,12].max; 3.0; else; nil; end"
            ),
            None
        );
        assert_eq!(
            cost(";e ((defined?(Skills.climbing) and Skills.swimming > 99) ? 0.2 : nil)"),
            None
        );
    }
}
