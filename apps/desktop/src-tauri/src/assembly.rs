//! What the model reads, assembled from the ranked passages (retrieval-quality programme, lot C). Each
//! method sits behind its own switch in `AssemblyOptions`, is measured alone against the baseline
//! (`docs/RETRIEVAL-QUALITY.md`) and is kept only if it helps and nothing regresses. With every switch off
//! the passages are exactly the ones the ranking gives.
//!
//! **The total never grows.** `RetrievalLimits` (the number of passages and their characters) is the
//! budget of the tower, which reads about ninety tokens a second: a cap per section moves places from a
//! section that already has some to another that has none, a neighbour takes the place of the weakest
//! passage, and the order changes nothing about how much is sent.

use std::collections::HashMap;
use std::sync::RwLock;

/// One switch per method. `Default` is the ranking as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AssemblyOptions {
    /// A passage that holds an identifier written in the question (`FAC-2026-014`) comes first.
    pub pin_identifier: bool,
    /// The search hands its first thirty candidates to the gateway's reranker, which reads each with the
    /// question; the best are kept. Without a reranker on the gateway, or when it fails, the order is the
    /// search's own.
    pub rerank: bool,
    /// At most this many passages from one section (or one slice of pages) of one document. A passage with
    /// no section is not capped. `None` is no cap.
    pub per_section_cap: Option<usize>,
    /// The passage that follows the best one is read with it, in the place of the weakest passage.
    pub neighbours: bool,
    /// The best passages are first and last in what the model reads: a long list is read worse in the middle.
    pub edges_first: bool,
    /// A passage that states a date or an amount written in the question, in any way of writing it, comes
    /// first, like an identifier, when at most three passages state it (`facts`).
    pub facts: bool,
}

impl AssemblyOptions {
    pub const OFF: Self = Self {
        pin_identifier: false,
        rerank: false,
        per_section_cap: None,
        neighbours: false,
        edges_first: false,
        facts: false,
    };
}

static OPTIONS: RwLock<Option<AssemblyOptions>> = RwLock::new(None);

/// The options the product runs with: `PRODUCT_CHOICE` unless a measurement set something else.
pub fn assembly_options() -> AssemblyOptions {
    OPTIONS
        .read()
        .ok()
        .and_then(|guard| *guard)
        .unwrap_or(PRODUCT_CHOICE)
}

/// The methods kept by the measures of lots C to E (`docs/RETRIEVAL-QUALITY.md`, section 12). Kept: the
/// identifier pinned first (never worse in any family, one gain). Kept at lot "facts" (15 October 2026): a date
/// or an amount written in the question finds the passage that writes it another way (normal_form family +0.18
/// on the sample set and +0.55 on the big set, real vectors; nothing else moved, no family fell; 0.4 to 0.6 % of
/// an Analyse). Left off: the cap, the neighbour and the edges (no gain on the fixtures), and the reranker
/// (measured: a gain, but a family falls, section 12). Removed on 15 October 2026 by the owner, on a judgement of
/// the answers and not on a measure: the word bonus and the fusion by rank, the generic phrases and the
/// refusal before the model (section 12.7).
const PRODUCT_CHOICE: AssemblyOptions = AssemblyOptions {
    pin_identifier: true,
    facts: true,
    ..AssemblyOptions::OFF
};

/// Measurement only: run with other options, or `None` for the product's.
pub fn set_assembly_options_for_measurement(options: Option<AssemblyOptions>) {
    if let Ok(mut guard) = OPTIONS.write() {
        *guard = options;
    }
}

/// Counts the passages taken from each section and says whether one more may be taken.
#[derive(Debug, Default)]
pub struct SectionCap {
    cap: Option<usize>,
    taken: HashMap<(String, i64), usize>,
}

impl SectionCap {
    pub fn new(cap: Option<usize>) -> Self {
        Self {
            cap,
            taken: HashMap::new(),
        }
    }

    /// Whether the cap is on at all (the caller looks the section up only then).
    pub fn is_on(&self) -> bool {
        self.cap.is_some()
    }

    /// Take a place in `section` (a document and a section position) if there is one. A passage with no
    /// section always has a place. The place is counted only when it is given.
    pub fn admit(&mut self, section: Option<(String, i64)>) -> bool {
        let (Some(cap), Some(section)) = (self.cap, section) else {
            return true;
        };
        let taken = self.taken.entry(section).or_insert(0);
        if *taken >= cap {
            return false;
        }
        *taken += 1;
        true
    }
}

/// `ranked` (best first) reordered so that the best are at the two ends: 1, 3, 5, ... 6, 4, 2.
pub fn edges_first<T>(ranked: Vec<T>) -> Vec<T> {
    let mut front = Vec::new();
    let mut back = Vec::new();
    for (position, item) in ranked.into_iter().enumerate() {
        if position % 2 == 0 {
            front.push(item);
        } else {
            back.push(item);
        }
    }
    back.reverse();
    front.extend(back);
    front
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_best_passages_take_the_two_ends() {
        assert_eq!(edges_first(vec![1, 2, 3, 4, 5, 6]), vec![1, 3, 5, 6, 4, 2]);
        assert_eq!(edges_first(vec![1, 2, 3]), vec![1, 3, 2]);
        assert_eq!(edges_first(vec![1, 2]), vec![1, 2]);
        assert_eq!(edges_first(vec![1]), vec![1]);
        assert_eq!(edges_first(Vec::<u8>::new()), Vec::<u8>::new());
    }

    #[test]
    fn a_section_gives_at_most_its_cap_and_a_passage_with_no_section_is_never_refused() {
        let mut cap = SectionCap::new(Some(2));
        let section = |n: i64| Some(("book.pdf".to_string(), n));
        assert!(cap.admit(section(1)));
        assert!(cap.admit(section(1)));
        assert!(!cap.admit(section(1)), "the third of the same section");
        assert!(cap.admit(section(2)), "another section has its own places");
        assert!(
            cap.admit(Some(("other.pdf".to_string(), 1))),
            "another document too"
        );
        assert!(cap.admit(None));
        assert!(cap.admit(None));
    }

    #[test]
    fn without_a_cap_every_passage_is_admitted() {
        let mut cap = SectionCap::new(None);
        assert!(!cap.is_on());
        for _ in 0..10 {
            assert!(cap.admit(Some(("book.pdf".to_string(), 1))));
        }
    }
}
