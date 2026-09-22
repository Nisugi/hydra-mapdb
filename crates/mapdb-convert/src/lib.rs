//! cena-mapdb-convert: the offline half of the map pipeline.
//!
//! `plan/21` §3a. The upstream map file is **input only**: this tool reads it
//! and writes one [`cena_map::Room`] per file, and the client never sees the
//! upstream file or a line of its Ruby.
//!
//! ```text
//! upstream map-*.json ──► this tool ──► rooms/NNN/<id>.json + index.json
//!                              └──────► report/  (what could not be converted)
//! ```
//!
//! [`run::convert`] is the pipeline as a pure function; [`output::write`] puts
//! it on disk. As of `plan/21` §5 step 2 only plain edges convert: every
//! scripted edge comes out `Unported`, carrying the id of its [`shape`], and
//! `tests/ratchet.rs` pins how many there are so the number can only fall.

pub mod convert;
pub mod delegate;
pub mod go2;
pub mod output;
pub mod recognise;
pub mod report;
pub mod run;
pub mod shape;
pub mod upstream;
