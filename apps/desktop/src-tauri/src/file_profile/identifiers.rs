//! The identifiers a text or a cell holds, in the forms a person may type them
//! (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 4.5 and 7).
//!
//! `756.3052.5778.43`, `756 3052 5778 43` and `7563052577843` are one number written three ways, and a
//! full-text index over the words of the text finds one of them only. The profile therefore stores each
//! identifier **in its compact form** (the one the schemes compare on, lower case) and, when the pack
//! knows how it is grouped, **in its grouped form** as adjacent words. A search folds what the user typed
//! to the compact form and meets every spelling.
//!
//! What counts as an identifier is the packs' own schemes (`knowledge::packs`), cumulative across the
//! active countries. Documents already hold these numbers in clear in the passages' index, and the
//! workbook cells are already at rest in the same file, so storing the compact form adds no new
//! exposure; the knowledge base keeps its own keyed hash for entity identity, unchanged.

use crate::knowledge::packs::PackSet;

use super::vocabulary::ProfileVocabulary;

/// The most identifiers kept for one file. A document that holds more is recorded as truncated, and every
/// answer that rests on it says so.
pub const MAX_IDENTIFIERS_PER_FILE: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IdentifierForms {
    pub scheme_id: String,
    pub kind: String,
    /// Lower case, no separator: `7563052577843`.
    pub compact: String,
    /// The compact form cut in groups, one space between them: `756 3052 5778 43`. `None` when the
    /// scheme has no layout that fits this length.
    pub grouped: Option<String>,
}

impl IdentifierForms {
    /// The words the full-text row holds for this identifier: the compact form, then the groups.
    pub fn words(&self) -> String {
        match &self.grouped {
            Some(grouped) => format!("{} {grouped}", self.compact),
            None => self.compact.clone(),
        }
    }
}

/// The compact form cut by the first layout whose total is its length.
pub fn group(compact: &str, layouts: &[Vec<usize>]) -> Option<String> {
    let characters: Vec<char> = compact.chars().collect();
    let layout = layouts
        .iter()
        .find(|layout| layout.iter().sum::<usize>() == characters.len())?;
    let mut parts = Vec::with_capacity(layout.len());
    let mut start = 0;
    for size in layout {
        parts.push(characters[start..start + size].iter().collect::<String>());
        start += size;
    }
    Some(parts.join(" "))
}

/// Every identifier of the active packs' schemes in `text`, deduplicated, in order of first appearance.
/// The text is read, never kept.
pub fn identifiers_in(
    text: &str,
    packs: &PackSet,
    vocabulary: &ProfileVocabulary,
) -> Vec<IdentifierForms> {
    // Every hit with where it stands, so that a number found inside a longer one is dropped: the digits
    // of an IBAN contain a telephone-shaped run, and the run is not a telephone number.
    let mut hits: Vec<(
        std::ops::Range<usize>,
        &crate::knowledge::packs::IdentifierScheme,
        String,
    )> = Vec::new();
    for scheme in packs.identifier_schemes() {
        for hit in scheme.scan(text) {
            hits.push((hit.range, scheme, hit.normalized.to_lowercase()));
        }
    }
    let contained = |inner: &std::ops::Range<usize>, outer: &std::ops::Range<usize>| {
        outer.start <= inner.start
            && inner.end <= outer.end
            && (outer.end - outer.start) > (inner.end - inner.start)
    };
    let mut kept: Vec<&(
        std::ops::Range<usize>,
        &crate::knowledge::packs::IdentifierScheme,
        String,
    )> = hits
        .iter()
        .filter(|(range, _, _)| !hits.iter().any(|(other, _, _)| contained(range, other)))
        .collect();
    kept.sort_by_key(|(range, _, _)| range.start);

    let mut found: Vec<IdentifierForms> = Vec::new();
    for (_, scheme, compact) in kept {
        if found
            .iter()
            .any(|known| known.scheme_id == scheme.id && known.compact == *compact)
        {
            continue;
        }
        found.push(IdentifierForms {
            scheme_id: scheme.id.clone(),
            kind: scheme.kind.clone(),
            grouped: group(compact, vocabulary.identifier_layouts(&scheme.id)),
            compact: compact.clone(),
        });
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> (PackSet, ProfileVocabulary) {
        (
            PackSet::load("fr-FR", &["health-fr", "health-ch"]).expect("loads"),
            ProfileVocabulary::load(&["health-fr", "health-ch"]).expect("loads"),
        )
    }

    fn compacts(text: &str) -> Vec<String> {
        let (packs, vocabulary) = context();
        identifiers_in(text, &packs, &vocabulary)
            .into_iter()
            .map(|f| f.compact)
            .collect()
    }

    #[test]
    fn one_swiss_number_written_four_ways_has_one_compact_form() {
        for spelling in [
            "756.3052.5778.43",
            "756 3052 5778 43",
            "7563052577843",
            "756.3052 5778.43",
        ] {
            assert_eq!(
                compacts(&format!("AVS : {spelling}.")),
                vec!["7563052577843"],
                "{spelling}"
            );
        }
        let (packs, vocabulary) = context();
        let found = identifiers_in("756.3052.5778.43", &packs, &vocabulary);
        assert_eq!(found[0].grouped.as_deref(), Some("756 3052 5778 43"));
        assert_eq!(found[0].words(), "7563052577843 756 3052 5778 43");
        assert_eq!(found[0].scheme_id, "avs");
    }

    #[test]
    fn a_french_number_and_both_ibans_are_grouped_by_their_layouts() {
        let (packs, vocabulary) = context();
        let nir = identifiers_in("1 58 03 74 443 192 09", &packs, &vocabulary);
        assert_eq!(nir[0].compact, "158037444319209");
        assert_eq!(nir[0].grouped.as_deref(), Some("1 58 03 74 443 192 09"));
        let swiss = identifiers_in("CH93 0076 2011 6238 5295 7", &packs, &vocabulary);
        assert_eq!(
            swiss[0].grouped.as_deref(),
            Some("ch93 0076 2011 6238 5295 7")
        );
        let french = identifiers_in("FR14 2004 1010 0505 0001 3M02 606", &packs, &vocabulary);
        assert_eq!(
            french[0].grouped.as_deref(),
            Some("fr14 2004 1010 0505 0001 3m02 606"),
            "{french:?}"
        );
    }

    #[test]
    fn a_scheme_without_a_layout_is_stored_in_one_piece_and_a_repeat_is_one_identifier() {
        let (packs, vocabulary) = context();
        let found = identifiers_in("write to Jean.Dupont@example.test or jean.dupont@example.test; invoice FAC-2026-014 and FAC-2026-014", &packs, &vocabulary);
        let kinds: Vec<(&str, Option<&str>)> = found
            .iter()
            .map(|f| (f.scheme_id.as_str(), f.grouped.as_deref()))
            .collect();
        assert_eq!(
            kinds.iter().filter(|(id, _)| *id == "email").count(),
            1,
            "{kinds:?}"
        );
        assert_eq!(
            kinds
                .iter()
                .filter(|(id, _)| *id == "invoice_number")
                .count(),
            1,
            "{kinds:?}"
        );
        assert!(kinds.iter().all(|(_, grouped)| grouped.is_none()));
    }

    #[test]
    fn a_pack_that_is_not_active_reads_none_of_its_identifiers() {
        let packs = PackSet::load("fr-FR", &["health-fr"]).expect("loads");
        let vocabulary = ProfileVocabulary::load(&["health-fr"]).expect("loads");
        assert!(identifiers_in("756.3052.5778.43", &packs, &vocabulary).is_empty());
    }

    #[test]
    fn grouping_needs_a_layout_that_fits() {
        let layouts = vec![vec![3, 4, 4, 2]];
        assert_eq!(
            group("7563052577843", &layouts).as_deref(),
            Some("756 3052 5778 43")
        );
        assert_eq!(group("756305257784", &layouts), None);
        assert_eq!(group("", &layouts), None);
        assert_eq!(group("12", &[]), None);
    }
}
