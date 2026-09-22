//! What go2 does to the map **before it plans**, which the map file alone
//! does not show.
//!
//! Nearly every special case lives in the mapdb's own scripts (`plan/21`
//! §2c). This is the exception: `go2.lic` rewrites two `timeto` values in
//! memory at startup, from a setting, so the file on disk prices an exit that
//! go2 -- by default -- never offers.
//!
//! Found 2026-09-21 by reading every setting go2 uses against every setting
//! the converted map asks about (author's question: "have you looked at go2
//! and seen what all settings it uses?"). It was the only one missing.

use cena_map::{Cond, Cost};

/// The rocky trail between Ta'Vaalor and Ta'Illistim, 16745 and 16746.
///
/// `go2.lic:952`: `change_map_vaalor_shortcut` sets both directions to 15
/// seconds when `vaalor shortcut` is on and to **`nil` -- impassable -- when
/// it is off**, and `go2.lic:854` makes off the default. go2's own help says
/// why: climbing and swimming are "recommended to be trained".
const VAALOR_SHORTCUT: [(u32, u32); 2] = [(16745, 16746), (16746, 16745)];

/// `cost`, as go2 would have priced this exit by the time it planned.
#[must_use]
pub fn as_go2_prices_it(from: u32, to: u32, cost: Option<Cost>) -> Option<Cost> {
    if !VAALOR_SHORTCUT.contains(&(from, to)) {
        return cost;
    }
    let on = Cond::Setting("vaalor_shortcut".to_owned(), "true".to_owned());
    Some(match cost? {
        Cost::Fixed(seconds) => Cost::Gated {
            when: on,
            then: seconds,
            otherwise: None,
        },
        Cost::Gated {
            when,
            then,
            otherwise: None,
        } => Cost::Gated {
            when: Cond::All(vec![on, when]),
            then,
            otherwise: None,
        },
        // Anything else is a price this override was not written for. Shut
        // is the safe reading of a trail go2 shuts by default.
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use cena_map::Walker;

    use super::*;

    #[test]
    fn the_shortcut_is_shut_unless_the_profile_opens_it() {
        let trail = as_go2_prices_it(16745, 16746, Some(Cost::Fixed(15.0))).unwrap();
        let mut walker = Walker::default();
        assert_eq!(trail.price(&walker), None, "go2's default is off");
        walker
            .settings
            .insert("vaalor_shortcut".into(), "true".into());
        assert_eq!(trail.price(&walker), Some(15.0));
        // Both ways, and nothing else.
        assert!(matches!(
            as_go2_prices_it(16746, 16745, Some(Cost::Fixed(15.0))),
            Some(Cost::Gated { .. })
        ));
        assert_eq!(
            as_go2_prices_it(16745, 16744, Some(Cost::Fixed(1.0))),
            Some(Cost::Fixed(1.0))
        );
    }
}
