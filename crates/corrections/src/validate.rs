//! Whether a submission may be accepted.
//!
//! This is `corrections-format.md`'s "Validating a submission" section in
//! code. Each check quotes the rule it enforces, so the document and this
//! file can be read against each other when either changes.
//!
//! Two outcomes, deliberately distinct. A [`Refusal`] means the file must not
//! be merged -- it would apply a correction to the wrong room, or apply two
//! that contradict. A [`Warning`] means a reviewer should look, but a person
//! may accept it: an orphaned plate is legal, and a stale `source_map` only
//! says the submitter was reasoning about a different build.
//!
//! The checks report **every** problem rather than stopping at the first.
//! Someone who has to resubmit learns all of it at once.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use cena_map::{Dirto, Uid};

use crate::file::{Bearing, Correction};

/// A reason a submission must not be merged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A uid absent from the current map. Usually a stale submission: the
    /// room was renumbered or removed upstream since the export was made.
    UnknownRoom { field: &'static str, uid: Uid },
    /// A `map_membership` slug with no entry in `maps`, which is not an
    /// `<area>.interiors` shelf either. Nothing says what grid this is.
    UnknownSlug { uid: Uid, slug: String },
    /// A bearing on `a -> b` with nothing on `b -> a`. The engine reads the
    /// entry on whichever room it is resolving from, so a one-sided
    /// correction would apply only half the time.
    OneSided { from: Uid, to: Uid, bearing: Dirto },
    /// `a -> b` east with `b -> a` anything but west.
    Disagree {
        from: Uid,
        to: Uid,
        bearing: Dirto,
        reverse: Dirto,
    },
    /// A room placed relative to itself.
    SelfAnchored { uid: Uid },
    /// An anchor that is itself placed. **A bug in the producer**, not
    /// something to resolve by ordering: a consumer would apply the anchor's
    /// offset, then measure from where it now is, and land somewhere neither
    /// correction asked for.
    CompoundingAnchor { uid: Uid, anchor: Uid },
    /// This submission corrects something an already-accepted correction
    /// corrects differently. Surfaced here, while the person who wrote it is
    /// still in the conversation, rather than at fold time.
    Conflict {
        what: String,
        submitted: String,
        accepted: String,
        source: String,
    },
    /// Nothing in the file would change anything. An export carrying only
    /// pictures reports itself as empty, and so does one whose corrections
    /// all match what is already accepted.
    Empty,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::UnknownRoom { field, uid } => write!(
                f,
                "`{field}` corrects room {}, which is not in the current map. This usually means \
                 the submission was made against an older build.",
                uid.0
            ),
            Refusal::UnknownSlug { uid, slug } => write!(
                f,
                "room {} is placed on grid `{slug}`, but nothing says what that grid is: it has \
                 no `maps` entry and is not an `<area>.interiors` shelf.",
                uid.0
            ),
            Refusal::OneSided { from, to, bearing } => write!(
                f,
                "`dirto` {} -> {} is {}, but {} -> {} says nothing. Entries are written on \
                 both rooms, because the engine reads whichever room it is resolving from -- a \
                 one-sided correction would apply only half the time.",
                from.0,
                to.0,
                bearing.name(),
                to.0,
                from.0
            ),
            Refusal::Disagree {
                from,
                to,
                bearing,
                reverse,
            } => write!(
                f,
                "`dirto` {} -> {} is {}, so {} -> {} must be {}, but it is {}.",
                from.0,
                to.0,
                bearing.name(),
                to.0,
                from.0,
                bearing.opposite().name(),
                reverse.name()
            ),
            Refusal::SelfAnchored { uid } => {
                write!(f, "room {} is placed relative to itself.", uid.0)
            }
            Refusal::CompoundingAnchor { uid, anchor } => write!(
                f,
                "room {} is placed against anchor {}, which is itself placed. The two offsets \
                 would compound and land somewhere neither correction asked for. This is a bug \
                 in whatever produced the file, not something to fix by ordering.",
                uid.0, anchor.0
            ),
            Refusal::Conflict {
                what,
                submitted,
                accepted,
                source,
            } => write!(
                f,
                "{what}: this submission says {submitted}, but the already-accepted `{source}` \
                 says {accepted}. One of the two is wrong; a reviewer decides which."
            ),
            Refusal::Empty => f.write_str(
                "this file would not change anything. Pictures alone are not a correction, and \
                 every correction here already matches what is accepted.",
            ),
        }
    }
}

/// Something a reviewer should see, which does not stop a merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// The submission was reasoned about against an older build.
    StaleSourceMap { named: String, current: String },
    /// A plate with no area: legal, but a consumer cannot tell where it
    /// attaches.
    OrphanedPlate { slug: String },
    /// A `maps` entry no room references.
    UnusedPlate { slug: String },
    /// A room put on a plate with no `area` entry saying where it still
    /// belongs. Without that, a consumer reading `map_membership` alone
    /// concludes the room left its area.
    PlacedWithoutArea { uid: Uid, slug: String },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Warning::StaleSourceMap { named, current } => write!(
                f,
                "made against `{named}`, but the current build is `{current}`. The corrections \
                 are uid-keyed so they do not depend on it, but it is worth knowing the \
                 submitter was looking at different data."
            ),
            Warning::OrphanedPlate { slug } => write!(
                f,
                "plate `{slug}` has no `area`, so nothing says which area it is a sheet of."
            ),
            Warning::UnusedPlate { slug } => {
                write!(f, "`maps` describes plate `{slug}`, which no room is on.")
            }
            Warning::PlacedWithoutArea { uid, slug } => write!(
                f,
                "room {} is placed on plate `{slug}` with no `area` entry. A plate is a grid, not \
                 a place -- without one, a consumer concludes the room left its area.",
                uid.0
            ),
        }
    }
}

/// What validation decided.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub refusals: Vec<Refusal>,
    pub warnings: Vec<Warning>,
}

impl Report {
    /// Whether the submission may be merged.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.refusals.is_empty()
    }
}

/// What the current map knows, as validation needs it.
///
/// A trait rather than a `&Map` so the checks can be tested without building
/// one, and so the binary can answer from `index.json` and the room files
/// without loading the whole map twice.
pub trait Known {
    /// Whether any room in the current map carries this uid.
    fn has_uid(&self, uid: Uid) -> bool;
    /// What the current build is, for the `source_map` freshness warning.
    fn source_map(&self) -> Option<&str>;
}

impl Known for BTreeSet<Uid> {
    fn has_uid(&self, uid: Uid) -> bool {
        self.contains(&uid)
    }
    fn source_map(&self) -> Option<&str> {
        None
    }
}

/// Validate a submission against the current map and what is already accepted.
///
/// `accepted` is the fold of every correction already in the repo, and the
/// name each came from, so a conflict can say which file it clashes with.
#[must_use]
pub fn validate(
    submission: &Correction,
    map: &impl Known,
    accepted: &crate::fold::Folded,
) -> Report {
    let mut report = Report::default();

    rooms_exist(submission, map, &mut report);
    slugs_are_known(submission, &mut report);
    dirto_agrees(submission, &mut report);
    placements_resolve(submission, &mut report);
    conflicts(submission, accepted, &mut report);
    advisories(submission, map, &mut report);

    // An empty file is only worth saying so about when nothing else is
    // wrong; after a refusal it is noise.
    if report.refusals.is_empty() && changes_nothing(submission, accepted) {
        report.refusals.push(Refusal::Empty);
    }
    report
}

/// "a room key ... names a uid absent from the current map (report which --
/// this usually means a stale submission)".
fn rooms_exist(submission: &Correction, map: &impl Known, report: &mut Report) {
    let mut check = |field: &'static str, uid: Uid| {
        if !map.has_uid(uid) {
            let refusal = Refusal::UnknownRoom { field, uid };
            if !report.refusals.contains(&refusal) {
                report.refusals.push(refusal);
            }
        }
    };
    for (from, destinations) in &submission.dirto {
        check("dirto", *from);
        for to in destinations.keys() {
            check("dirto", *to);
        }
    }
    for uid in submission.map_membership.keys() {
        check("map_membership", *uid);
    }
    for uid in submission.area.keys() {
        check("area", *uid);
    }
    // "both `anchor` and the room key must resolve to rooms in the current
    // map".
    for (uid, placement) in &submission.placement {
        check("placement", *uid);
        check("placement", placement.anchor);
    }
}

/// "a `map_membership` slug has no entry in `maps` and is not an
/// `<area>.interiors` slug".
fn slugs_are_known(submission: &Correction, report: &mut Report) {
    for (uid, slug) in &submission.map_membership {
        if !submission.maps.contains_key(slug) && !slug.ends_with(".interiors") {
            report.refusals.push(Refusal::UnknownSlug {
                uid: *uid,
                slug: slug.clone(),
            });
        }
    }
}

/// "`dirto` entries are one-sided, or disagree with each other -- `a -> b`
/// east with `b -> a` anything but west."
///
/// A reverse entry is **not** invented for an exit the map does not have: a
/// one-way passage stays one-way. But the producer writes both ends when both
/// ends exist, so a missing reverse here means the file is malformed, not that
/// the passage is one-way -- the mapper would have written nothing at all.
fn dirto_agrees(submission: &Correction, report: &mut Report) {
    for (from, destinations) in &submission.dirto {
        for (to, bearing) in destinations {
            match submission.dirto.get(to).and_then(|back| back.get(from)) {
                Some(reverse) if *reverse == bearing.opposite() => {}
                Some(reverse) => {
                    // Reported once, from the lower uid, rather than twice
                    // from each end of the same disagreement.
                    if from < to {
                        report.refusals.push(Refusal::Disagree {
                            from: *from,
                            to: *to,
                            bearing: *bearing,
                            reverse: *reverse,
                        });
                    }
                }
                None => report.refusals.push(Refusal::OneSided {
                    from: *from,
                    to: *to,
                    bearing: *bearing,
                }),
            }
        }
    }
}

/// "a room must not be its own anchor" and "an anchor that is itself a
/// `placement` key is a bug in the producer ... refuse it, because the
/// offsets would compound."
fn placements_resolve(submission: &Correction, report: &mut Report) {
    for (uid, placement) in &submission.placement {
        if placement.anchor == *uid {
            report.refusals.push(Refusal::SelfAnchored { uid: *uid });
        } else if submission.placement.contains_key(&placement.anchor) {
            report.refusals.push(Refusal::CompoundingAnchor {
                uid: *uid,
                anchor: placement.anchor,
            });
        }
    }
}

/// Flag a submission that contradicts one already accepted, before the PR is
/// opened rather than at fold time -- the person who made it is still here to
/// say which is right.
fn conflicts(submission: &Correction, accepted: &crate::fold::Folded, report: &mut Report) {
    for (from, destinations) in &submission.dirto {
        for (to, bearing) in destinations {
            if let Some((was, source)) = accepted.dirto_with_source(*from, *to)
                && was != *bearing
            {
                report.refusals.push(Refusal::Conflict {
                    what: format!("`dirto` {} -> {}", from.0, to.0),
                    submitted: bearing.name().to_owned(),
                    accepted: was.name().to_owned(),
                    source: source.to_owned(),
                });
            }
        }
    }
    for (uid, slug) in &submission.map_membership {
        if let Some((was, source)) = accepted.membership_with_source(*uid)
            && was != slug
        {
            report.refusals.push(Refusal::Conflict {
                what: format!("room {} is on grid", uid.0),
                submitted: slug.clone(),
                accepted: was.to_owned(),
                source: source.to_owned(),
            });
        }
    }
    for (uid, area) in &submission.area {
        if let Some((was, source)) = accepted.area_with_source(*uid)
            && was != area
        {
            report.refusals.push(Refusal::Conflict {
                what: format!("room {} is in area", uid.0),
                submitted: area.clone(),
                accepted: was.to_owned(),
                source: source.to_owned(),
            });
        }
    }
    for (uid, placement) in &submission.placement {
        if let Some((was, source)) = accepted.placement_with_source(*uid)
            && was != *placement
        {
            report.refusals.push(Refusal::Conflict {
                what: format!("room {} is placed", uid.0),
                submitted: format!(
                    "{} east, {} south of {}",
                    placement.dx, placement.dy, placement.anchor.0
                ),
                accepted: format!("{} east, {} south of {}", was.dx, was.dy, was.anchor.0),
                source: source.to_owned(),
            });
        }
    }
}

/// The "accept but flag" list.
fn advisories(submission: &Correction, map: &impl Known, report: &mut Report) {
    if let (Some(named), Some(current)) = (&submission.source_map, map.source_map())
        && named != current
    {
        report.warnings.push(Warning::StaleSourceMap {
            named: named.clone(),
            current: current.to_owned(),
        });
    }
    for (slug, sheet) in &submission.maps {
        if sheet.area.is_none() {
            report
                .warnings
                .push(Warning::OrphanedPlate { slug: slug.clone() });
        }
        if !submission.map_membership.values().any(|on| on == slug) {
            report
                .warnings
                .push(Warning::UnusedPlate { slug: slug.clone() });
        }
    }
    // "written for every room the export puts on a plate" -- an interiors
    // shelf is automatic and stays in its own area, so only plates are
    // expected to carry one.
    for (uid, slug) in &submission.map_membership {
        if submission.maps.contains_key(slug) && !submission.area.contains_key(uid) {
            report.warnings.push(Warning::PlacedWithoutArea {
                uid: *uid,
                slug: slug.clone(),
            });
        }
    }
}

/// Whether every correction here already says what is accepted.
fn changes_nothing(submission: &Correction, accepted: &crate::fold::Folded) -> bool {
    let dirto_new = submission.dirto.iter().any(|(from, destinations)| {
        destinations.iter().any(|(to, bearing)| {
            accepted.dirto_with_source(*from, *to).map(|(was, _)| was) != Some(*bearing)
        })
    });
    let membership_new = submission.map_membership.iter().any(|(uid, slug)| {
        accepted.membership_with_source(*uid).map(|(was, _)| was) != Some(slug.as_str())
    });
    let area_new = submission.area.iter().any(|(uid, area)| {
        accepted.area_with_source(*uid).map(|(was, _)| was) != Some(area.as_str())
    });
    let placement_new = submission.placement.iter().any(|(uid, placement)| {
        accepted.placement_with_source(*uid).map(|(was, _)| was) != Some(*placement)
    });
    !(dirto_new || membership_new || area_new || placement_new)
}

/// Every uid in a conversion directory's room files, for [`Known`].
#[derive(Debug, Default)]
pub struct CurrentMap {
    pub uids: BTreeSet<Uid>,
    pub source_map: Option<String>,
}

impl Known for CurrentMap {
    fn has_uid(&self, uid: Uid) -> bool {
        self.uids.contains(&uid)
    }
    fn source_map(&self) -> Option<&str> {
        self.source_map.as_deref()
    }
}

/// A convenience for callers holding a plain map of accepted corrections.
impl Correction {
    /// Whether this file carries any correction at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dirto.is_empty()
            && self.map_membership.is_empty()
            && self.area.is_empty()
            && self.placement.is_empty()
    }

    /// Every uid this file mentions, in any field.
    #[must_use]
    pub fn rooms(&self) -> BTreeSet<Uid> {
        let mut rooms = BTreeSet::new();
        for (from, destinations) in &self.dirto {
            rooms.insert(*from);
            rooms.extend(destinations.keys().copied());
        }
        rooms.extend(self.map_membership.keys().copied());
        rooms.extend(self.area.keys().copied());
        for (uid, placement) in &self.placement {
            rooms.insert(*uid);
            rooms.insert(placement.anchor);
        }
        rooms
    }
}

/// Summarise what a file would change, for a PR body.
#[must_use]
pub fn summary(submission: &Correction) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    let edges: usize = submission.dirto.values().map(BTreeMap::len).sum();
    if edges > 0 {
        counts.insert("direction corrections", edges);
    }
    if !submission.map_membership.is_empty() {
        counts.insert(
            "rooms moved to another grid",
            submission.map_membership.len(),
        );
    }
    if !submission.area.is_empty() {
        counts.insert("area assignments", submission.area.len());
    }
    if !submission.placement.is_empty() {
        counts.insert("placements", submission.placement.len());
    }
    if !submission.maps.is_empty() {
        counts.insert("plates", submission.maps.len());
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::parse;
    use crate::fold::Folded;
    use cena_map::Placement;

    fn map(uids: &[i64]) -> BTreeSet<Uid> {
        uids.iter().copied().map(Uid).collect()
    }

    fn check(text: &str, uids: &[i64]) -> Report {
        validate(&parse(text).unwrap(), &map(uids), &Folded::default())
    }

    #[test]
    fn a_consistent_pair_of_bearings_is_accepted() {
        let report = check(
            r#"{"version":2,"dirto":{"1":{"2":"east"},"2":{"1":"west"}}}"#,
            &[1, 2],
        );
        assert!(report.accepted(), "{:?}", report.refusals);
    }

    #[test]
    fn a_one_sided_bearing_is_refused() {
        let report = check(r#"{"version":2,"dirto":{"1":{"2":"east"}}}"#, &[1, 2]);
        assert_eq!(
            report.refusals,
            vec![Refusal::OneSided {
                from: Uid(1),
                to: Uid(2),
                bearing: Dirto::East
            }]
        );
    }

    #[test]
    fn disagreeing_bearings_are_refused_once_not_twice() {
        let report = check(
            r#"{"version":2,"dirto":{"1":{"2":"east"},"2":{"1":"north"}}}"#,
            &[1, 2],
        );
        assert_eq!(report.refusals.len(), 1);
        assert!(matches!(report.refusals[0], Refusal::Disagree { .. }));
    }

    #[test]
    fn cross_group_must_be_written_both_ways() {
        let ok = check(
            r#"{"version":2,"dirto":{"1":{"2":"cross-group"},"2":{"1":"cross-group"}}}"#,
            &[1, 2],
        );
        assert!(ok.accepted(), "{:?}", ok.refusals);
    }

    #[test]
    fn a_uid_the_map_does_not_have_is_refused() {
        let report = check(r#"{"version":2,"area":{"999":"nowhere"}}"#, &[1]);
        assert_eq!(
            report.refusals,
            vec![Refusal::UnknownRoom {
                field: "area",
                uid: Uid(999)
            }]
        );
    }

    #[test]
    fn an_interiors_shelf_needs_no_maps_entry() {
        let report = check(
            r#"{"version":2,"map_membership":{"1":"icemule.interiors"}}"#,
            &[1],
        );
        assert!(report.accepted(), "{:?}", report.refusals);
    }

    #[test]
    fn a_plate_with_no_maps_entry_is_refused() {
        let report = check(
            r#"{"version":2,"map_membership":{"1":"landing.well"}}"#,
            &[1],
        );
        assert_eq!(
            report.refusals,
            vec![Refusal::UnknownSlug {
                uid: Uid(1),
                slug: "landing.well".to_owned()
            }]
        );
    }

    #[test]
    fn a_room_may_not_be_its_own_anchor() {
        let report = check(
            r#"{"version":2,"placement":{"1":{"anchor":1,"dx":3,"dy":-2}}}"#,
            &[1],
        );
        assert_eq!(report.refusals, vec![Refusal::SelfAnchored { uid: Uid(1) }]);
    }

    /// The offsets would compound: apply the anchor's, then measure from
    /// where it now is.
    #[test]
    fn an_anchor_that_is_itself_placed_is_refused() {
        let report = check(
            r#"{"version":2,"placement":{
                "1":{"anchor":2,"dx":3,"dy":-2},
                "2":{"anchor":3,"dx":1,"dy":1}}}"#,
            &[1, 2, 3],
        );
        assert!(report.refusals.contains(&Refusal::CompoundingAnchor {
            uid: Uid(1),
            anchor: Uid(2)
        }));
    }

    #[test]
    fn an_anchor_absent_from_the_map_is_refused() {
        let report = check(
            r#"{"version":2,"placement":{"1":{"anchor":42,"dx":1,"dy":1}}}"#,
            &[1],
        );
        assert!(report.refusals.contains(&Refusal::UnknownRoom {
            field: "placement",
            uid: Uid(42)
        }));
    }

    #[test]
    fn pictures_alone_are_not_a_submission() {
        let report = check(r#"{"version":2,"pictures":{"a":"<svg/>"}}"#, &[1]);
        assert_eq!(report.refusals, vec![Refusal::Empty]);
    }

    #[test]
    fn an_orphaned_plate_is_flagged_not_refused() {
        let report = check(
            r#"{"version":2,
                "map_membership":{"1":"landing.well"},
                "area":{"1":"wehnimers"},
                "maps":{"landing.well":{"name":"The Town Well"}}}"#,
            &[1],
        );
        assert!(report.accepted(), "{:?}", report.refusals);
        assert!(report.warnings.contains(&Warning::OrphanedPlate {
            slug: "landing.well".to_owned()
        }));
    }

    #[test]
    fn a_plate_move_with_no_area_is_flagged() {
        let report = check(
            r#"{"version":2,
                "map_membership":{"1":"landing.well"},
                "maps":{"landing.well":{"name":"The Well","area":"wehnimers"}}}"#,
            &[1],
        );
        assert!(report.accepted(), "{:?}", report.refusals);
        assert!(report.warnings.contains(&Warning::PlacedWithoutArea {
            uid: Uid(1),
            slug: "landing.well".to_owned()
        }));
    }

    #[test]
    fn contradicting_an_accepted_correction_is_refused_with_its_source() {
        let accepted = crate::fold::fold(&[(
            "2026-09-01-first.json".to_owned(),
            parse(r#"{"version":2,"dirto":{"1":{"2":"east"},"2":{"1":"west"}}}"#).unwrap(),
        )]);
        let submission =
            parse(r#"{"version":2,"dirto":{"1":{"2":"north"},"2":{"1":"south"}}}"#).unwrap();
        let report = validate(&submission, &map(&[1, 2]), &accepted.folded);
        assert!(matches!(
            report.refusals.first(),
            Some(Refusal::Conflict { source, .. }) if source == "2026-09-01-first.json"
        ));
    }

    #[test]
    fn resubmitting_what_is_already_accepted_changes_nothing() {
        let text = r#"{"version":2,"dirto":{"1":{"2":"east"},"2":{"1":"west"}}}"#;
        let accepted = crate::fold::fold(&[("first.json".to_owned(), parse(text).unwrap())]);
        let report = validate(&parse(text).unwrap(), &map(&[1, 2]), &accepted.folded);
        assert_eq!(report.refusals, vec![Refusal::Empty]);
    }

    #[test]
    fn every_problem_is_reported_not_just_the_first() {
        let report = check(
            r#"{"version":2,
                "dirto":{"1":{"2":"east"}},
                "placement":{"1":{"anchor":1,"dx":0,"dy":0}}}"#,
            &[1, 2],
        );
        assert_eq!(report.refusals.len(), 2);
    }

    #[test]
    fn a_stale_source_map_is_flagged() {
        struct Build;
        impl Known for Build {
            fn has_uid(&self, _: Uid) -> bool {
                true
            }
            fn source_map(&self) -> Option<&str> {
                Some("2026-09-20")
            }
        }
        let submission =
            parse(r#"{"version":2,"source_map":"2026-08-01","area":{"1":"x"}}"#).unwrap();
        let report = validate(&submission, &Build, &Folded::default());
        assert!(report.accepted());
        assert!(matches!(
            report.warnings.first(),
            Some(Warning::StaleSourceMap { .. })
        ));
    }

    #[test]
    fn placement_survives_a_valid_anchor() {
        let submission =
            parse(r#"{"version":2,"placement":{"4124007":{"anchor":4124001,"dx":3,"dy":-2}}}"#)
                .unwrap();
        assert_eq!(
            submission.placement[&Uid(4_124_007)],
            Placement {
                anchor: Uid(4_124_001),
                dx: 3,
                dy: -2
            }
        );
        let report = validate(
            &submission,
            &map(&[4_124_007, 4_124_001]),
            &Folded::default(),
        );
        assert!(report.accepted(), "{:?}", report.refusals);
    }
}
