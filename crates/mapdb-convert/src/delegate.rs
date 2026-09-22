//! Costs that say "whatever that other exit costs" (`plan/21` §4.1).
//!
//! Upstream writes `;e Map[7].timeto['30714'].call;` on 957 exits: nearly every
//! way into an urchin hub defers to one exit's gate, and every way out of one
//! defers to another. At run time that is a call through the map; here it is
//! resolved **once, before conversion**, by replacing the deferring cost with
//! the cost it points at. After this pass the rest of the converter never sees
//! a delegation, and no primitive is spent on one.
//!
//! A delegation whose target is missing, or is itself still a delegation after
//! a few hops, is left as it is -- and so stays on the unported worklist.

use std::collections::BTreeMap;

use crate::upstream::{UpstreamCost, UpstreamRoom};

/// Upstream chains are one hop deep today; this is the bound, not a guess at
/// the depth.
const MAX_HOPS: usize = 4;

/// `(room, destination)` for `;e Map[room].timeto['destination'].call;`.
fn target(script: &str) -> Option<(u32, &str)> {
    let rest = script.strip_prefix(";e Map[")?;
    let (room, rest) = rest.split_once("].timeto['")?;
    let (destination, rest) = rest.split_once("'].call")?;
    matches!(rest, "" | ";").then_some(())?;
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    (digits(room) && digits(destination)).then_some(())?;
    Some((room.parse().ok()?, destination))
}

/// Replace every delegating cost with what it points at. Returns how many were
/// replaced.
pub fn resolve(rooms: &mut [UpstreamRoom]) -> usize {
    let costs: BTreeMap<(u32, String), Option<UpstreamCost>> = rooms
        .iter()
        .flat_map(|room| {
            room.timeto
                .iter()
                .map(|(to, cost)| ((room.id, to.clone()), cost.clone()))
        })
        .collect();

    let follow = |start: &UpstreamCost| -> Option<UpstreamCost> {
        let mut cost = start.clone();
        for _ in 0..MAX_HOPS {
            let UpstreamCost::Script(script) = &cost else {
                return Some(cost);
            };
            let Some((room, destination)) = target(script) else {
                return Some(cost);
            };
            cost = costs.get(&(room, destination.to_owned()))?.clone()?;
        }
        None
    };

    let mut resolved = 0;
    for room in rooms {
        for cost in room.timeto.values_mut().flatten() {
            let UpstreamCost::Script(script) = &*cost else {
                continue;
            };
            if target(script).is_none() {
                continue;
            }
            if let Some(found) = follow(cost) {
                *cost = found;
                resolved += 1;
            }
        }
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_exact_form_is_a_delegation() {
        assert_eq!(
            target(";e Map[7].timeto['30714'].call;"),
            Some((7, "30714"))
        );
        assert_eq!(target(";e Map[7].timeto['30714'].call"), Some((7, "30714")));
        assert_eq!(target(";e Map[7].timeto['30714'].call ? 1 : nil"), None);
        assert_eq!(target(";e Map[x].timeto['30714'].call;"), None);
    }

    fn room(json: &str) -> UpstreamRoom {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn a_delegation_becomes_what_it_points_at() {
        let mut rooms = vec![
            room(r#"{"id":7,"wayto":{"30714":";e true"},"timeto":{"30714":";e the_gate"}}"#),
            room(
                r#"{"id":11,"wayto":{"30714":";e true","12":"north","13":"east"},
                    "timeto":{"30714":";e Map[7].timeto['30714'].call;",
                              "12":";e Map[99].timeto['1'].call;",
                              "13":";e Map[11].timeto['30714'].call;"}}"#,
            ),
        ];
        assert_eq!(resolve(&mut rooms), 2);
        let cost = |to: &str| match rooms[1].timeto.get(to) {
            Some(Some(UpstreamCost::Script(script))) => script.clone(),
            other => format!("{other:?}"),
        };
        assert_eq!(cost("30714"), ";e the_gate");
        assert_eq!(cost("13"), ";e the_gate", "two hops");
        assert_eq!(
            cost("12"),
            ";e Map[99].timeto['1'].call;",
            "a target that is not there is left on the worklist"
        );
    }
}
