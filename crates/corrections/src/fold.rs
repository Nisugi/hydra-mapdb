//! Combining every accepted correction into the one set a build reads.
//!
//! `corrections/` is **append-only**: one file per accepted submission, named
//! for when it landed, never rewritten. That is what makes a bad correction
//! revertible by reverting its commit rather than by recomputing a merged
//! file by hand, and it is why two submissions never conflict in git.
//!
//! The cost of append-only is that the effective correction for a room is not
//! visible in any one file, so it is computed here -- and where two files
//! correct the same thing differently, the later one wins and the override is
//! **reported** rather than applied silently. Validation refuses a conflict
//! before it is ever accepted ([`crate::validate`]), so a clash at this point
//! means two submissions were merged too close together for that check to
//! have compared them. It is the backstop, not the first line.

use std::collections::BTreeMap;
use std::fmt;

use cena_map::{Dirto, Placement, Sheet, Uid};

use crate::file::Correction;

/// Two accepted files correct the same thing differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash {
    /// What was corrected twice, in words.
    pub what: String,
    /// The file that lost, and what it said.
    pub overridden: String,
    pub was: String,
    /// The file that won, and what it says.
    pub winner: String,
    pub now: String,
}

impl fmt::Display for Clash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: `{}` says {}, overridden by the later `{}` saying {}",
            self.what, self.overridden, self.was, self.winner, self.now
        )
    }
}

/// One correction, and the file it came from.
type Sourced<T> = (T, String);

/// The effective correction set: what every room ends up corrected to, and
/// which file said so.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Folded {
    dirto: BTreeMap<Uid, BTreeMap<Uid, Sourced<Dirto>>>,
    map_membership: BTreeMap<Uid, Sourced<String>>,
    area: BTreeMap<Uid, Sourced<String>>,
    placement: BTreeMap<Uid, Sourced<Placement>>,
    maps: BTreeMap<String, Sourced<Sheet>>,
}

impl Folded {
    /// The effective bearing for an edge, and the file that set it.
    #[must_use]
    pub fn dirto_with_source(&self, from: Uid, to: Uid) -> Option<(Dirto, &str)> {
        self.dirto
            .get(&from)?
            .get(&to)
            .map(|(bearing, source)| (*bearing, source.as_str()))
    }

    /// The effective grid for a room, and the file that set it.
    #[must_use]
    pub fn membership_with_source(&self, uid: Uid) -> Option<(&str, &str)> {
        self.map_membership
            .get(&uid)
            .map(|(slug, source)| (slug.as_str(), source.as_str()))
    }

    /// The effective area for a room, and the file that set it.
    #[must_use]
    pub fn area_with_source(&self, uid: Uid) -> Option<(&str, &str)> {
        self.area
            .get(&uid)
            .map(|(area, source)| (area.as_str(), source.as_str()))
    }

    /// The effective placement for a room, and the file that set it.
    #[must_use]
    pub fn placement_with_source(&self, uid: Uid) -> Option<(Placement, &str)> {
        self.placement
            .get(&uid)
            .map(|(placement, source)| (*placement, source.as_str()))
    }

    /// Every effective direction correction, as the applier wants it.
    #[must_use]
    pub fn dirto(&self) -> BTreeMap<Uid, BTreeMap<Uid, Dirto>> {
        self.dirto
            .iter()
            .map(|(from, destinations)| {
                let destinations = destinations
                    .iter()
                    .map(|(to, (bearing, _))| (*to, *bearing))
                    .collect();
                (*from, destinations)
            })
            .collect()
    }

    /// Every effective grid assignment.
    #[must_use]
    pub fn map_membership(&self) -> BTreeMap<Uid, &str> {
        plain(&self.map_membership)
    }

    /// Every effective area assignment.
    #[must_use]
    pub fn area(&self) -> BTreeMap<Uid, &str> {
        plain(&self.area)
    }

    /// Every effective placement.
    #[must_use]
    pub fn placement(&self) -> BTreeMap<Uid, Placement> {
        self.placement
            .iter()
            .map(|(uid, (placement, _))| (*uid, *placement))
            .collect()
    }

    /// Every plate the accepted set describes.
    #[must_use]
    pub fn maps(&self) -> BTreeMap<&str, &Sheet> {
        self.maps
            .iter()
            .map(|(slug, (sheet, _))| (slug.as_str(), sheet))
            .collect()
    }

    /// Whether anything is corrected at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dirto.is_empty()
            && self.map_membership.is_empty()
            && self.area.is_empty()
            && self.placement.is_empty()
    }

    /// How many corrections of each kind, for a report.
    #[must_use]
    pub fn counts(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        counts.insert(
            "direction corrections",
            self.dirto.values().map(BTreeMap::len).sum(),
        );
        counts.insert("grid assignments", self.map_membership.len());
        counts.insert("area assignments", self.area.len());
        counts.insert("placements", self.placement.len());
        counts.insert("plates", self.maps.len());
        counts.retain(|_, count| *count > 0);
        counts
    }
}

fn plain(sourced: &BTreeMap<Uid, Sourced<String>>) -> BTreeMap<Uid, &str> {
    sourced
        .iter()
        .map(|(uid, (value, _))| (*uid, value.as_str()))
        .collect()
}

/// What a fold produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Result_ {
    pub folded: Folded,
    /// Every place a later file overrode an earlier one. Empty in the normal
    /// case, because validation refuses a conflicting submission before it is
    /// accepted.
    pub clashes: Vec<Clash>,
}

/// Fold accepted corrections, **in the order given**, later winning.
///
/// Callers pass files sorted by name, which is chronological because the
/// names begin with the date the submission was accepted.
#[must_use]
pub fn fold(files: &[(String, Correction)]) -> Result_ {
    let mut out = Result_::default();

    for (source, correction) in files {
        for (from, destinations) in &correction.dirto {
            for (to, bearing) in destinations {
                let slot = out.folded.dirto.entry(*from).or_default();
                if let Some((was, overridden)) = slot.get(to)
                    && was != bearing
                {
                    out.clashes.push(Clash {
                        what: format!("`dirto` {} -> {}", from.0, to.0),
                        overridden: overridden.clone(),
                        was: was.name().to_owned(),
                        winner: source.clone(),
                        now: bearing.name().to_owned(),
                    });
                }
                slot.insert(*to, (*bearing, source.clone()));
            }
        }

        replace(
            &mut out.folded.map_membership,
            &correction.map_membership,
            source,
            &mut out.clashes,
            |uid| format!("room {} is on grid", uid.0),
            Clone::clone,
        );
        replace(
            &mut out.folded.area,
            &correction.area,
            source,
            &mut out.clashes,
            |uid| format!("room {} is in area", uid.0),
            Clone::clone,
        );
        replace(
            &mut out.folded.placement,
            &correction.placement,
            source,
            &mut out.clashes,
            |uid| format!("room {} is placed", uid.0),
            |placement: &Placement| *placement,
        );

        for (slug, sheet) in &correction.maps {
            out.folded
                .maps
                .insert(slug.clone(), (sheet.clone(), source.clone()));
        }
    }

    out
}

/// Insert every entry of `from` into `into`, reporting anything overridden.
fn replace<T, U>(
    into: &mut BTreeMap<Uid, Sourced<T>>,
    from: &BTreeMap<Uid, U>,
    source: &str,
    clashes: &mut Vec<Clash>,
    what: impl Fn(Uid) -> String,
    convert: impl Fn(&U) -> T,
) where
    T: PartialEq + fmt::Debug,
{
    for (uid, value) in from {
        let value = convert(value);
        if let Some((was, overridden)) = into.get(uid)
            && *was != value
        {
            clashes.push(Clash {
                what: what(*uid),
                overridden: overridden.clone(),
                was: format!("{was:?}"),
                winner: source.to_owned(),
                now: format!("{value:?}"),
            });
        }
        into.insert(*uid, (value, source.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::parse;

    fn file(name: &str, text: &str) -> (String, Correction) {
        (name.to_owned(), parse(text).unwrap())
    }

    #[test]
    fn independent_corrections_all_survive() {
        let out = fold(&[
            file("a.json", r#"{"version":2,"area":{"1":"icemule"}}"#),
            file("b.json", r#"{"version":2,"area":{"2":"landing"}}"#),
        ]);
        assert!(out.clashes.is_empty());
        assert_eq!(out.folded.area().len(), 2);
    }

    #[test]
    fn a_later_file_wins_and_the_override_is_reported() {
        let out = fold(&[
            file(
                "2026-09-01-a.json",
                r#"{"version":2,"area":{"1":"icemule"}}"#,
            ),
            file(
                "2026-09-08-b.json",
                r#"{"version":2,"area":{"1":"landing"}}"#,
            ),
        ]);
        assert_eq!(out.folded.area()[&Uid(1)], "landing");
        assert_eq!(out.clashes.len(), 1);
        assert_eq!(out.clashes[0].overridden, "2026-09-01-a.json");
        assert_eq!(out.clashes[0].winner, "2026-09-08-b.json");
    }

    /// Two files saying the same thing is not a clash -- only a
    /// disagreement is.
    #[test]
    fn agreeing_files_do_not_clash() {
        let out = fold(&[
            file("a.json", r#"{"version":2,"area":{"1":"icemule"}}"#),
            file("b.json", r#"{"version":2,"area":{"1":"icemule"}}"#),
        ]);
        assert!(out.clashes.is_empty());
    }

    #[test]
    fn a_dirto_override_names_both_files() {
        let out = fold(&[
            file(
                "a.json",
                r#"{"version":2,"dirto":{"1":{"2":"east"},"2":{"1":"west"}}}"#,
            ),
            file(
                "b.json",
                r#"{"version":2,"dirto":{"1":{"2":"north"},"2":{"1":"south"}}}"#,
            ),
        ]);
        assert_eq!(
            out.folded.dirto_with_source(Uid(1), Uid(2)).unwrap().0,
            Dirto::North
        );
        assert_eq!(out.clashes.len(), 2);
    }

    #[test]
    fn a_placement_override_is_reported() {
        let out = fold(&[
            file(
                "a.json",
                r#"{"version":2,"placement":{"1":{"anchor":9,"dx":1,"dy":1}}}"#,
            ),
            file(
                "b.json",
                r#"{"version":2,"placement":{"1":{"anchor":9,"dx":4,"dy":0}}}"#,
            ),
        ]);
        assert_eq!(out.clashes.len(), 1);
        assert_eq!(out.folded.placement()[&Uid(1)].dx, 4);
    }

    #[test]
    fn an_empty_set_folds_to_nothing() {
        assert!(fold(&[]).folded.is_empty());
    }
}
