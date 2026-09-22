//! What a conversion did, and -- the part that matters -- what it could not do.
//!
//! `plan/21` §3a: "Nothing is dropped silently." The report is the porting
//! worklist: every scripted shape, how many edges share it, and one edge to
//! look at. `tests/ratchet.rs` pins its totals so they can only fall.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use cena_map::{Cost, Crossing, Room, ShapeId};

use crate::convert::Problem;
use crate::shape::normalise;
use crate::upstream::{UpstreamCost, UpstreamRoom, is_script};

/// One scripted shape and the edges that share it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeRow {
    pub id: ShapeId,
    pub edges: usize,
    /// The normalised text, so the row can be read without the source.
    pub shape: String,
    /// One edge that has this shape, as `from->to`.
    pub sample: String,
}

/// Totals for one conversion.
#[derive(Debug, Default)]
pub struct Report {
    pub rooms: usize,
    pub rooms_without_uid: usize,
    pub exits: usize,
    /// Exits a pathfinder could use as converted: plain command, constant cost.
    pub routable: usize,
    /// Scripted upstream, and crossed by steps an arm produced.
    pub ported_crossings: usize,
    pub unported_crossings: usize,
    /// Costs that deferred to another exit's, replaced by it before conversion.
    pub delegations_resolved: usize,
    /// Scripted upstream, and priced by a gate an arm produced.
    pub ported_costs: usize,
    pub unported_costs: usize,
    /// Exits with no cost at all, which are impassable (`cena_map::Cost`).
    pub without_cost: usize,
    /// Exits whose destination is not a room in this map.
    pub dangling: usize,
    pub problems: Vec<Problem>,
    crossing_shapes: BTreeMap<ShapeId, ShapeRow>,
    cost_shapes: BTreeMap<ShapeId, ShapeRow>,
}

impl Report {
    /// Record the scripts of one upstream room. Called before conversion
    /// consumes it, because the normalised text is wanted alongside the id and
    /// only the source has it.
    pub fn see_scripts(&mut self, upstream: &UpstreamRoom) {
        for (to, command) in &upstream.wayto {
            // The table is the worklist, so a script an arm already knows is
            // not on it.
            if is_script(command)
                && to
                    .parse()
                    .ok()
                    .and_then(|to| crate::recognise::crossing(command, upstream.id, to))
                    .is_none()
            {
                note(&mut self.crossing_shapes, command, upstream.id, to);
            }
        }
        for (to, cost) in &upstream.timeto {
            // A cost for a destination with no `wayto` prices an exit that does
            // not exist: 61 upstream, 13 of them scripted. Conversion reports
            // each as a problem; counting them here would make this table
            // disagree with the exits it describes -- which is how the ratchet
            // first caught it, 1,873 shapes-side against 1,860 exits-side.
            if let Some(UpstreamCost::Script(script)) = cost
                && upstream.wayto.contains_key(to)
                && is_script(script)
                && crate::recognise::cost(script, &upstream.room_facts()).is_none()
            {
                note(&mut self.cost_shapes, script, upstream.id, to);
            }
        }
    }

    /// Record one converted room.
    pub fn see_room(&mut self, room: &Room) {
        self.rooms += 1;
        if room.uid.is_empty() {
            self.rooms_without_uid += 1;
        }
        for exit in &room.exits {
            self.exits += 1;
            if exit.is_routable() {
                self.routable += 1;
            }
            match exit.crossing {
                Crossing::Unported(_) => self.unported_crossings += 1,
                Crossing::Steps(_) | Crossing::Routine(_) | Crossing::PassThrough(_) => {
                    self.ported_crossings += 1;
                }
                Crossing::Command(_) | Crossing::Unknown(_) => {}
            }
            match exit.cost {
                Some(Cost::Unported { .. }) => self.unported_costs += 1,
                // `Unknown` is made only by the binary loader; the converter
                // writes costs, it never reads a map file.
                Some(
                    Cost::Gated { .. }
                    | Cost::Table { .. }
                    | Cost::Ladder { .. }
                    | Cost::Hasted { .. },
                ) => {
                    self.ported_costs += 1;
                }
                Some(Cost::Fixed(_) | Cost::Unknown(_)) => {}
                None => self.without_cost += 1,
            }
        }
    }

    /// Scripted crossing shapes, most edges first.
    #[must_use]
    pub fn crossing_shapes(&self) -> Vec<&ShapeRow> {
        by_edges(&self.crossing_shapes)
    }

    /// Scripted cost shapes, most edges first.
    #[must_use]
    pub fn cost_shapes(&self) -> Vec<&ShapeRow> {
        by_edges(&self.cost_shapes)
    }

    /// The summary a person reads.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut text = String::new();
        // Writing to a String cannot fail.
        let _ = writeln!(text, "rooms                 {}", self.rooms);
        let _ = writeln!(text, "  without a uid       {}", self.rooms_without_uid);
        let _ = writeln!(text, "exits                 {}", self.exits);
        let _ = writeln!(text, "  routable as is      {}", self.routable);
        let _ = writeln!(text, "  ported crossings    {}", self.ported_crossings);
        let _ = writeln!(
            text,
            "  unported crossings  {} in {} shapes",
            self.unported_crossings,
            self.crossing_shapes.len()
        );
        let _ = writeln!(
            text,
            "  delegated costs     {} resolved",
            self.delegations_resolved
        );
        let _ = writeln!(text, "  ported costs        {}", self.ported_costs);
        let _ = writeln!(
            text,
            "  unported costs      {} in {} shapes",
            self.unported_costs,
            self.cost_shapes.len()
        );
        let _ = writeln!(text, "  with no cost        {}", self.without_cost);
        let _ = writeln!(text, "  dangling            {}", self.dangling);
        let _ = writeln!(text, "problems              {}", self.problems.len());
        text
    }

    /// A shape table as tab-separated text: edges, id, sample, shape.
    #[must_use]
    pub fn shapes_tsv(rows: &[&ShapeRow]) -> String {
        let mut text = String::from("edges\tshape_id\tsample\tshape\n");
        for row in rows {
            let _ = writeln!(
                text,
                "{}\t{}\t{}\t{}",
                row.edges, row.id.0, row.sample, row.shape
            );
        }
        text
    }
}

fn note(shapes: &mut BTreeMap<ShapeId, ShapeRow>, script: &str, from: u32, to: &str) {
    let shape = normalise(script);
    let id = crate::shape::shape_id(script);
    shapes
        .entry(id.clone())
        .or_insert_with(|| ShapeRow {
            id,
            edges: 0,
            shape,
            sample: format!("{from}->{to}"),
        })
        .edges += 1;
}

/// Most edges first; ties by id, so the order is stable between runs.
fn by_edges(shapes: &BTreeMap<ShapeId, ShapeRow>) -> Vec<&ShapeRow> {
    let mut rows: Vec<&ShapeRow> = shapes.values().collect();
    rows.sort_by(|a, b| b.edges.cmp(&a.edges).then_with(|| a.id.cmp(&b.id)));
    rows
}
