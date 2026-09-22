//! cena-map-combine: per-room JSON in, one map binary out.
//!
//! `plan/21` §3a. The converter's output is thousands of small, diffable files;
//! the client wants one file it can load in a single pass. [`combine::combine`]
//! reads the first, and `cena_map::binary` defines the second.
//!
//! ```text
//! <dir>/index.json + rooms/NNN/<id>.json ──► this tool ──► hydra.map
//! ```

pub mod combine;
