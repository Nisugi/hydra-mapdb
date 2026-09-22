//! Arms for slice h of the long tail, round two
//! (`research/mapdb-inventory/tail/slice_h_costs.tsv`).
//!
//! Same rules as every arm (`super`): the upstream script verbatim, with
//! holes; an exact match or none. This file's tests live at its foot.

use cena_map::{Cond, Cost};

use super::costs::gated;
use super::{holes, is_word};

/// The gate for a cost script in this slice, if an arm here knows it.
pub(super) fn cost(script: &str) -> Option<Cost> {
    silverwood(script)
        .or_else(|| premium_hall(script))
        .or_else(|| not_while_hunting(script))
        .or_else(|| no_such_script(script))
}

/// `$SILVERWOOD_TOWN == :wl ? 0.2 : nil` -- 4 exits, the manor's four doors
/// out. The door in writes the global (`$SILVERWOOD_TOWN=:wl;move 'go door'`,
/// one per town: `wl`, `imt`, `va`, `zul`) and the door out clears it, so it
/// is a memory like any other; only upstream's choice of a Ruby global over
/// `UserVars` differs, and that loses it at logout, which nobody intended.
fn silverwood(script: &str) -> Option<Cost> {
    let [town, then] = holes(script, &[";e $SILVERWOOD_TOWN == :", " ? ", " : nil"])?[..] else {
        return None;
    };
    is_word(town).then_some(())?;
    gated(
        Cond::Remembered("silverwood_town".to_owned(), town.to_owned()),
        then,
    )
}

/// The urchins' two ways into the premium hall -- 18 exits, 9 each, all but
/// two by delegation. `Account` is always defined in the Lich this map is
/// written for, so the trinket branch is dead and the test is "is the account
/// premium": a flag, because the planner knows and the map cannot.
///
/// The second half, `Map[30714].timeto['7'].call`, is another exit's cost and
/// is part of the template: it is `costs::only_when_travelling`, always true
/// here. Were upstream to point it elsewhere this arm would stop matching.
fn premium_hall(script: &str) -> Option<Cost> {
    const PREMIUM: [&str; 2] = [
        ";e ((defined?(Account) ? Account.subscription.downcase == 'premium': \
         UserVars.mapdb_fwi_trinket.to_s != '') and Map[30714].timeto['7'].call) ? ",
        " : nil",
    ];
    const NOT_PREMIUM: [&str; 2] = [
        ";e (!(defined?(Account) ? Account.subscription.downcase == 'premium': \
         UserVars.mapdb_fwi_trinket.to_s != '') and Map[30714].timeto['7'].call) ? ",
        " : nil",
    ];
    let premium = Cond::Flag("premium_account".to_owned());
    if let Some(found) = holes(script, &PREMIUM) {
        let [then] = found[..] else {
            return None;
        };
        return gated(premium, then);
    }
    let [then] = holes(script, &NOT_PREMIUM)?[..] else {
        return None;
    };
    gated(Cond::Not(Box::new(premium)), then)
}

/// `(Script.running?('bigshot') || Script.running?('wander'))? nil : 0.2` --
/// 6 exits that a hunting script must not wander through. There are no
/// scripts here; what upstream means is "the character is out hunting", which
/// is a flag the planner works out. This one refuses, so unknown refuses too.
fn not_while_hunting(script: &str) -> Option<Cost> {
    const FORMS: [[&str; 2]; 2] = [
        [
            ";e (Script.running?('bigshot') || Script.running?('wander'))? nil : ",
            "",
        ],
        [
            ";e (Script.running?('bigshot') or Script.running?('wander')) ? nil : ",
            "",
        ],
    ];
    let found = FORMS.iter().find_map(|form| holes(script, form))?;
    let [then] = found[..] else {
        return None;
    };
    let hunting = Cond::Flag("hunting".to_owned());
    gated(Cond::Not(Box::new(hunting)), then)
}

/// `if Script.running? 'ego2';nil;else;180.0;end` -- 2 exits closed to one
/// other Lich walker. Nothing here is ever that script, so the test is always
/// false and the cost is constant (as `costs::only_when_travelling`).
fn no_such_script(script: &str) -> Option<Cost> {
    let [then] = holes(script, &[";e if Script.running? 'ego2';nil;else;", ";end"])?[..] else {
        return None;
    };
    let then: f64 = then.parse().ok()?;
    (then.is_finite() && then >= 0.0).then_some(Cost::Fixed(then))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cena_map::Walker;

    fn gate(when: Cond, then: f64) -> Cost {
        Cost::Gated {
            when,
            then,
            otherwise: None,
        }
    }

    #[test]
    fn the_manor_lets_you_out_where_you_came_in() {
        assert_eq!(
            cost(";e $SILVERWOOD_TOWN == :imt ? 0.2 : nil"),
            Some(gate(
                Cond::Remembered("silverwood_town".into(), "imt".into()),
                0.2
            ))
        );
        assert_eq!(cost(";e $SILVERWOOD_TOWN == :imt ? 0.2 : 5"), None);
        assert_eq!(
            cost(";e $SILVERWOOD_TOWN == :imt or true ? 0.2 : nil"),
            None
        );
        assert_eq!(cost(";e $OTHER_TOWN == :imt ? 0.2 : nil"), None);
    }

    #[test]
    fn the_premium_hall_has_a_door_for_each_kind_of_account() {
        let premium = ";e ((defined?(Account) ? Account.subscription.downcase == 'premium': UserVars.mapdb_fwi_trinket.to_s != '') and Map[30714].timeto['7'].call) ? 0.1 : nil";
        let not_premium = ";e (!(defined?(Account) ? Account.subscription.downcase == 'premium': UserVars.mapdb_fwi_trinket.to_s != '') and Map[30714].timeto['7'].call) ? 0.1 : nil";
        let flag = Cond::Flag("premium_account".into());
        assert_eq!(cost(premium), Some(gate(flag.clone(), 0.1)));
        assert_eq!(
            cost(not_premium),
            Some(gate(Cond::Not(Box::new(flag)), 0.1))
        );
        // Another exit's cost is not the one this template was read against.
        assert_eq!(cost(&premium.replace("['7']", "['8']")), None);
        assert_eq!(cost(&premium.replace("premium'", "platinum'")), None);
    }

    #[test]
    fn a_hunter_is_kept_out_and_an_unknown_walker_too() {
        let expected = |then| gate(Cond::Not(Box::new(Cond::Flag("hunting".into()))), then);
        assert_eq!(
            cost(";e (Script.running?('bigshot') || Script.running?('wander'))? nil : 6.0"),
            Some(expected(6.0))
        );
        let priced =
            cost(";e (Script.running?('bigshot') or Script.running?('wander')) ? nil : 0.2");
        assert_eq!(priced, Some(expected(0.2)));
        let walker = |hunting| Walker {
            flags: [("hunting".to_owned(), hunting)].into(),
            ..Walker::default()
        };
        let priced = priced.unwrap();
        assert_eq!(priced.price(&walker(false)), Some(0.2));
        assert_eq!(priced.price(&walker(true)), None);
        assert_eq!(priced.price(&Walker::default()), None);
        assert_eq!(
            cost(";e (Script.running?('bigshot') || Script.running?('go2'))? nil : 0.2"),
            None
        );
        assert_eq!(
            cost(";e (Script.running?('bigshot') || Script.running?('wander'))? nil : nil"),
            None
        );
    }

    #[test]
    fn a_script_that_never_runs_here_closes_nothing() {
        assert_eq!(
            cost(";e if Script.running? 'ego2';nil;else;180.0;end"),
            Some(Cost::Fixed(180.0))
        );
        assert_eq!(cost(";e if Script.running? 'go2';nil;else;180.0;end"), None);
        assert_eq!(cost(";e if Script.running? 'ego2';nil;else;nil;end"), None);
    }
}
