//! Corrections to the map, as submitted, validated, and folded.
//!
//! A corrections file carries **corrected inputs to a layout, never a
//! layout** (`hydra-mapper`'s `docs/corrections-format.md`). Everything is
//! keyed by uid -- the game's own room number -- because this repo assigns
//! its own room ids on every build, so a correction keyed any other way would
//! silently mean a different room after the next conversion.
//!
//! Three things happen here, in order:
//!
//! - [`file`] parses a submission and says what version it is.
//! - [`validate`] decides whether it may be accepted, against the current map
//!   and against the corrections already accepted.
//! - [`fold`] combines every accepted file into the one effective set a build
//!   reads, reporting any correction a later one overrode.
//!
//! [`file`], [`validate`] and [`fold`] are pure: they decide against plain
//! values and touch nothing. [`disk`] is the one module that reads the
//! repository, and the binaries in `src/bin` are thin wrappers over it --
//! everything decidable is decided where it can be tested without a
//! filesystem.

pub mod disk;
pub mod file;
pub mod fold;
pub mod validate;

pub use file::{Bearing, Correction, ParseError};
pub use fold::{Clash, Folded, fold};
pub use validate::{Refusal, Report, Warning, validate};

// The corrected values themselves are `cena_map`'s types, not this crate's:
// a correction is only ever on its way into a room record, so restating
// `Dirto` or `Placement` here would be the second definition the git
// dependency exists to prevent.
pub use cena_map::{Dirto, Placement, Sheet};
