//! Puzzles that belong to one place: named here, run by the Travel behaviour
//! (`cena_map::Puzzle`). Each is matched against upstream's script verbatim,
//! so the copy under `src/upstream_scripts/` is the reference for what the
//! routine must do, and an upstream edit un-ports the exit.
//!
//! The two engines are here too: the Confluence's and the seeking's scripts
//! sit on an exit from a room to itself, which nothing walks, and other exits
//! call them. Pinning them means a change to either is noticed.

use cena_map::{Crossing, Puzzle, Routine};

pub(super) fn crossing(script: &str) -> Option<Crossing> {
    const PUZZLES: [(&str, Puzzle); 12] = [
        (
            include_str!("../upstream_scripts/rolaren_gate.rb"),
            Puzzle::RolarenGate,
        ),
        (
            include_str!("../upstream_scripts/rune_staircase.rb"),
            Puzzle::RuneStaircase,
        ),
        (
            include_str!("../upstream_scripts/three_pillars.rb"),
            Puzzle::ThreePillars,
        ),
        (
            include_str!("../upstream_scripts/vaalorn_door.rb"),
            Puzzle::VaalornDoor,
        ),
        (
            include_str!("../upstream_scripts/mural_of_deities.rb"),
            Puzzle::MuralOfDeities,
        ),
        (
            include_str!("../upstream_scripts/altar_levers.rb"),
            Puzzle::AltarLevers,
        ),
        (
            include_str!("../upstream_scripts/workshop_pillars.rb"),
            Puzzle::WorkshopPillars,
        ),
        (
            include_str!("../upstream_scripts/eye_spy_runes.rb"),
            Puzzle::EyeSpyRunes,
        ),
        (
            include_str!("../upstream_scripts/crown_door.rb"),
            Puzzle::CrownDoor,
        ),
        (
            include_str!("../upstream_scripts/bridge_wheel.rb"),
            Puzzle::BridgeWheel,
        ),
        (
            include_str!("../upstream_scripts/familiar_doors.rb"),
            Puzzle::FamiliarDoors,
        ),
        (
            include_str!("../upstream_scripts/labyrinth_entry.rb"),
            Puzzle::LabyrinthEntry,
        ),
    ];
    const CONFLUENCE: &str = include_str!("../upstream_scripts/confluence_engine.rb");
    const SEEKING: &str = include_str!("../upstream_scripts/seeking_engine.rb");
    let routine = match script {
        CONFLUENCE => Routine::Confluence { leave: false },
        SEEKING => Routine::Seeking { remember: None },
        _ => {
            let (_, puzzle) = PUZZLES.iter().find(|(pinned, _)| *pinned == script)?;
            Routine::Puzzle { puzzle: *puzzle }
        }
    };
    Some(Crossing::Routine(routine))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pinned_script_is_its_puzzle_and_nothing_near_it_is() {
        let script = include_str!("../upstream_scripts/altar_levers.rb");
        assert_eq!(
            crossing(script),
            Some(Crossing::Routine(Routine::Puzzle {
                puzzle: Puzzle::AltarLevers
            }))
        );
        assert_eq!(crossing(&script.replacen("altar", "table", 1)), None);
    }

    #[test]
    fn the_engines_are_pinned_as_the_routines_they_run() {
        assert_eq!(
            crossing(include_str!("../upstream_scripts/confluence_engine.rb")),
            Some(Crossing::Routine(Routine::Confluence { leave: false }))
        );
    }
}
