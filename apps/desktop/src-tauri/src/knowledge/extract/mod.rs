//! The extraction port: how a text or a table turns into candidate entities.
//!
//! An [`EntityExtractor`] reads one source and returns what it found as [`Candidate`]s - a surface
//! (the name as written), a type, a role, where it was found, how it was found. It does **not**
//! decide whether a candidate is a new entity or one the knowledge base already holds: that is the
//! resolver's job (`knowledge::resolve`), and keeping the two apart is what lets either be replaced.
//!
//! This lot ships the port and the plumbing around it, not an extractor that reads a file. The
//! deterministic text extractor is lot 4, the table extractor lot 5. A local NER model or a spaCy
//! adapter would implement the same trait later and be registered beside them in a
//! [`CompositeExtractor`]; no NLP engine is a dependency of the client, and business code never
//! imports one.
//!
//! Inputs are plain data owned by the caller and borrowed for the duration of the call. Text is
//! read and dropped: a [`Candidate`] holds a name, a locator and a count, never a passage.

use super::gazetteer::Gazetteer;
use super::normalize::fold;
use super::packs::PackSet;
use super::phonetic::PhoneticEncoder;
use super::{Confidence, EntityTypeId, MentionLocator, Method, RoleId, SignalDraft, SourceRef};

/// One chunk of a document, as the index stores it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkInput {
    pub chunk_id: String,
    pub text: String,
}

/// A document to read: its source and its chunks, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentInput {
    pub source: SourceRef,
    pub chunks: Vec<ChunkInput>,
}

/// One column of a sheet: its header as written and its cells as text, row by row (the first value
/// is the first data row). Lot 5 may widen this with the typed cells; the port only needs text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnInput {
    pub header: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetInput {
    pub name: String,
    pub columns: Vec<ColumnInput>,
}

/// A workbook to read: its source and its sheets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbookInput {
    pub source: SourceRef,
    pub sheets: Vec<SheetInput>,
}

/// What an extractor may consult. Built once per pass and shared by every file.
pub struct ExtractContext<'a> {
    pub packs: &'a PackSet,
    pub locale: &'a str,
    pub gazetteer: &'a Gazetteer,
    pub encoder: &'a dyn PhoneticEncoder,
}

/// Where a candidate sits inside the text it came from, in characters. Used to tell whether two
/// extractors found the same thing; never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn overlaps(&self, other: &Span) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// An identifier an extractor read next to, or as, a candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateIdentifier {
    /// The id of the pack's identifier scheme (`IdentifierScheme::id`).
    pub scheme: String,
    /// The reduced value (`IdentifierScheme::scan`).
    pub normalized: String,
}

/// A name, an organisation, an identifier or an item found in a source, not yet resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// The name as written: "Dr. Jean Dupont". It becomes the entity's display name if the entity
    /// is new.
    pub surface: String,
    pub type_id: EntityTypeId,
    pub subtype: Option<String>,
    pub role: Option<RoleId>,
    pub method: Method,
    pub confidence: Confidence,
    pub locator: MentionLocator,
    pub span: Option<Span>,
    pub occurrence_count: u32,
    pub identifier: Option<CandidateIdentifier>,
    /// The titles seen with the name (folded), so a title-and-surname alias can be made.
    pub titles_seen: Vec<String>,
}

impl Candidate {
    /// A candidate found once, with the confidence of its method and nothing optional set.
    pub fn new(
        surface: &str,
        type_id: EntityTypeId,
        method: Method,
        locator: MentionLocator,
    ) -> Self {
        Self {
            surface: surface.to_string(),
            type_id,
            subtype: None,
            role: None,
            method,
            confidence: method.default_confidence(),
            locator,
            span: None,
            occurrence_count: 1,
            identifier: None,
            titles_seen: Vec::new(),
        }
    }
}

/// A link a source states by its own structure (two role columns of one table), between two
/// candidates of the same [`ExtractedKnowledge`], by index. Never inferred from co-occurrence.
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateRelation {
    pub subject: usize,
    pub predicate: String,
    pub object: usize,
    pub confidence: Confidence,
}

/// Everything an extractor found in one source.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExtractedKnowledge {
    pub candidates: Vec<Candidate>,
    pub relations: Vec<CandidateRelation>,
    pub signals: Vec<SignalDraft>,
}

impl ExtractedKnowledge {
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty() && self.relations.is_empty() && self.signals.is_empty()
    }
}

/// The extraction port. The defaults find nothing, so an implementation overrides only the kind of
/// source it reads.
pub trait EntityExtractor: Send + Sync {
    /// Stable identifier of the extractor, for example `deterministic-text`.
    fn id(&self) -> &str;

    /// Raised whenever the output for some input changes. Together with the other extractors'
    /// versions it is what `kb_meta.extractor_version` stores, so improving an extractor makes the
    /// next pass read the sources again.
    fn version(&self) -> u32;

    fn extract_document(
        &self,
        _input: &DocumentInput,
        _context: &ExtractContext<'_>,
    ) -> ExtractedKnowledge {
        ExtractedKnowledge::default()
    }

    fn extract_workbook(
        &self,
        _input: &WorkbookInput,
        _context: &ExtractContext<'_>,
    ) -> ExtractedKnowledge {
        ExtractedKnowledge::default()
    }
}

/// Several extractors behind one, so a model-based extractor can be registered beside the
/// deterministic one without the pass knowing. Candidates two extractors found at the same place
/// are merged: the more confident one stays (the earlier registered one on a tie).
#[derive(Default)]
pub struct CompositeExtractor {
    extractors: Vec<Box<dyn EntityExtractor>>,
}

impl CompositeExtractor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, extractor: Box<dyn EntityExtractor>) -> Self {
        self.extractors.push(extractor);
        self
    }

    pub fn len(&self) -> usize {
        self.extractors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.extractors.is_empty()
    }

    /// `id:version` of every member, comma-separated, for the record of what read a source.
    pub fn signature(&self) -> String {
        self.extractors
            .iter()
            .map(|extractor| format!("{}:{}", extractor.id(), extractor.version()))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn combine(&self, parts: Vec<ExtractedKnowledge>) -> ExtractedKnowledge {
        let mut merged = ExtractedKnowledge::default();
        // The extractor each surviving candidate came from, to merge across extractors only.
        let mut origin: Vec<usize> = Vec::new();

        for (extractor, part) in parts.into_iter().enumerate() {
            let mut remap: Vec<usize> = Vec::with_capacity(part.candidates.len());
            for candidate in part.candidates {
                let twin = merged
                    .candidates
                    .iter()
                    .enumerate()
                    .position(|(position, kept)| {
                        origin[position] != extractor && same_place(kept, &candidate)
                    });
                match twin {
                    Some(position) => {
                        if candidate.confidence > merged.candidates[position].confidence {
                            merged.candidates[position] = candidate;
                            origin[position] = extractor;
                        }
                        remap.push(position);
                    }
                    None => {
                        merged.candidates.push(candidate);
                        origin.push(extractor);
                        remap.push(merged.candidates.len() - 1);
                    }
                }
            }
            for relation in part.relations {
                let (Some(&subject), Some(&object)) =
                    (remap.get(relation.subject), remap.get(relation.object))
                else {
                    continue;
                };
                let relation = CandidateRelation {
                    subject,
                    object,
                    ..relation
                };
                if subject != object && !merged.relations.contains(&relation) {
                    merged.relations.push(relation);
                }
            }
            for signal in part.signals {
                match merged
                    .signals
                    .iter_mut()
                    .find(|known| known.pack_id == signal.pack_id)
                {
                    Some(known) => {
                        known.hits = known.hits.max(signal.hits);
                        known.distinct_terms = known.distinct_terms.max(signal.distinct_terms);
                    }
                    None => merged.signals.push(signal),
                }
            }
        }
        merged
    }
}

/// Two candidates found at the same place: the same locator and either overlapping spans or, when
/// a span is missing, the same name once folded.
fn same_place(a: &Candidate, b: &Candidate) -> bool {
    if a.locator != b.locator {
        return false;
    }
    match (a.span, b.span) {
        (Some(left), Some(right)) => left.overlaps(&right),
        _ => fold(&a.surface) == fold(&b.surface),
    }
}

impl EntityExtractor for CompositeExtractor {
    fn id(&self) -> &str {
        "composite"
    }

    fn version(&self) -> u32 {
        self.extractors
            .iter()
            .map(|extractor| extractor.version())
            .sum()
    }

    fn extract_document(
        &self,
        input: &DocumentInput,
        context: &ExtractContext<'_>,
    ) -> ExtractedKnowledge {
        let parts = self
            .extractors
            .iter()
            .map(|extractor| extractor.extract_document(input, context))
            .collect();
        self.combine(parts)
    }

    fn extract_workbook(
        &self,
        input: &WorkbookInput,
        context: &ExtractContext<'_>,
    ) -> ExtractedKnowledge {
        let parts = self
            .extractors
            .iter()
            .map(|extractor| extractor.extract_workbook(input, context))
            .collect();
        self.combine(parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::phonetic::FrenchPhonetic;
    use crate::knowledge::Domain;

    /// An extractor that returns what it was given, to test the plumbing.
    struct Scripted {
        id: &'static str,
        version: u32,
        found: ExtractedKnowledge,
    }

    impl EntityExtractor for Scripted {
        fn id(&self) -> &str {
            self.id
        }
        fn version(&self) -> u32 {
            self.version
        }
        fn extract_document(
            &self,
            _input: &DocumentInput,
            _context: &ExtractContext<'_>,
        ) -> ExtractedKnowledge {
            self.found.clone()
        }
    }

    /// Reads nothing: relies on the trait defaults.
    struct Silent;
    impl EntityExtractor for Silent {
        fn id(&self) -> &str {
            "silent"
        }
        fn version(&self) -> u32 {
            1
        }
    }

    fn document() -> DocumentInput {
        DocumentInput {
            source: SourceRef {
                domain: Domain::Documents,
                relative_path: "a.txt".to_string(),
                content_id: "sha-a".to_string(),
            },
            chunks: vec![ChunkInput {
                chunk_id: "a.txt#p1#s1".to_string(),
                text: "irrelevant".to_string(),
            }],
        }
    }

    fn at_chunk() -> MentionLocator {
        MentionLocator::Chunk {
            chunk_id: "a.txt#p1#s1".to_string(),
        }
    }

    fn candidate(surface: &str, method: Method, span: Option<(usize, usize)>) -> Candidate {
        let mut found = Candidate::new(surface, EntityTypeId::person(), method, at_chunk());
        found.span = span.map(|(start, end)| Span { start, end });
        found
    }

    fn run(extractor: &dyn EntityExtractor) -> ExtractedKnowledge {
        let packs = PackSet::load("en-US", &[]).unwrap();
        let gazetteer = Gazetteer::new();
        let encoder = FrenchPhonetic;
        extractor.extract_document(
            &document(),
            &ExtractContext {
                packs: &packs,
                locale: "en-US",
                gazetteer: &gazetteer,
                encoder: &encoder,
            },
        )
    }

    #[test]
    fn the_defaults_find_nothing() {
        let found = run(&Silent);
        assert!(found.is_empty());
    }

    #[test]
    fn a_composite_with_no_member_finds_nothing() {
        let composite = CompositeExtractor::new();
        assert!(composite.is_empty());
        assert!(run(&composite).is_empty());
        assert_eq!(composite.version(), 0);
    }

    #[test]
    fn a_composite_reports_every_member_in_its_signature_and_its_version() {
        let composite = CompositeExtractor::new()
            .with(Box::new(Silent))
            .with(Box::new(Scripted {
                id: "other",
                version: 4,
                found: ExtractedKnowledge::default(),
            }));
        assert_eq!(composite.signature(), "silent:1,other:4");
        assert_eq!(composite.version(), 5);
        assert_eq!(composite.len(), 2);
    }

    #[test]
    fn the_more_confident_extractor_wins_a_shared_span() {
        let weak = candidate("Jean Dupont", Method::CapitalisedName, Some((10, 21)));
        let strong = candidate("Dr Jean Dupont", Method::TitleFullName, Some((7, 21)));
        let composite = CompositeExtractor::new()
            .with(Box::new(Scripted {
                id: "first",
                version: 1,
                found: ExtractedKnowledge {
                    candidates: vec![weak],
                    ..ExtractedKnowledge::default()
                },
            }))
            .with(Box::new(Scripted {
                id: "second",
                version: 1,
                found: ExtractedKnowledge {
                    candidates: vec![strong],
                    ..ExtractedKnowledge::default()
                },
            }));
        let found = run(&composite);
        assert_eq!(found.candidates.len(), 1);
        assert_eq!(found.candidates[0].method, Method::TitleFullName);
    }

    #[test]
    fn the_earlier_extractor_wins_a_tie() {
        let first = candidate("Jean Dupont", Method::GazetteerHit, Some((0, 11)));
        let mut second = candidate("Jean Dupont", Method::GazetteerHit, Some((0, 11)));
        second.subtype = Some("from_second".to_string());
        let composite = CompositeExtractor::new()
            .with(Box::new(Scripted {
                id: "first",
                version: 1,
                found: ExtractedKnowledge {
                    candidates: vec![first],
                    ..ExtractedKnowledge::default()
                },
            }))
            .with(Box::new(Scripted {
                id: "second",
                version: 1,
                found: ExtractedKnowledge {
                    candidates: vec![second],
                    ..ExtractedKnowledge::default()
                },
            }));
        let found = run(&composite);
        assert_eq!(found.candidates.len(), 1);
        assert_eq!(found.candidates[0].subtype, None);
    }

    #[test]
    fn different_places_and_one_extractors_own_neighbours_are_kept() {
        let near = candidate("Jean Dupont", Method::CapitalisedName, Some((0, 11)));
        let overlapping = candidate("Dupont", Method::CapitalisedName, Some((5, 11)));
        let elsewhere = candidate("Jean Dupont", Method::CapitalisedName, Some((50, 61)));
        let composite = CompositeExtractor::new()
            .with(Box::new(Scripted {
                id: "one",
                version: 1,
                // Two overlapping spans from the same extractor are its own business.
                found: ExtractedKnowledge {
                    candidates: vec![near, overlapping],
                    ..ExtractedKnowledge::default()
                },
            }))
            .with(Box::new(Scripted {
                id: "two",
                version: 1,
                found: ExtractedKnowledge {
                    candidates: vec![elsewhere],
                    ..ExtractedKnowledge::default()
                },
            }));
        assert_eq!(run(&composite).candidates.len(), 3);
    }

    #[test]
    fn spanless_candidates_are_the_same_when_the_folded_name_is() {
        let a = candidate("Jean Dupont", Method::ColumnValue, None);
        let b = candidate("JEAN DUPONT", Method::CapitalisedName, None);
        let c = candidate("Marie Dupont", Method::CapitalisedName, None);
        let composite = CompositeExtractor::new()
            .with(Box::new(Scripted {
                id: "one",
                version: 1,
                found: ExtractedKnowledge {
                    candidates: vec![a],
                    ..ExtractedKnowledge::default()
                },
            }))
            .with(Box::new(Scripted {
                id: "two",
                version: 1,
                found: ExtractedKnowledge {
                    candidates: vec![b, c],
                    ..ExtractedKnowledge::default()
                },
            }));
        let found = run(&composite);
        assert_eq!(found.candidates.len(), 2);
        assert_eq!(found.candidates[0].method, Method::ColumnValue);
    }

    #[test]
    fn relations_follow_their_candidates_through_the_merge() {
        let a = candidate("Alice Archer", Method::ColumnValue, None);
        let b = candidate("Bob Baker", Method::ColumnValue, None);
        let a_again = candidate("Alice Archer", Method::CapitalisedName, None);
        let c = candidate("Carol Clark", Method::ColumnValue, None);
        let first = ExtractedKnowledge {
            candidates: vec![a, b],
            relations: vec![CandidateRelation {
                subject: 0,
                predicate: "served_by".to_string(),
                object: 1,
                confidence: Confidence::new(0.9),
            }],
            signals: Vec::new(),
        };
        let second = ExtractedKnowledge {
            // Index 0 is a twin of the first extractor's candidate 0; index 1 is new.
            candidates: vec![a_again, c],
            relations: vec![
                CandidateRelation {
                    subject: 0,
                    predicate: "served_by".to_string(),
                    object: 1,
                    confidence: Confidence::new(0.9),
                },
                // A relation whose two ends merged into one candidate disappears.
                CandidateRelation {
                    subject: 0,
                    predicate: "same".to_string(),
                    object: 0,
                    confidence: Confidence::new(0.9),
                },
                // A relation that points outside the candidates is dropped, not trusted.
                CandidateRelation {
                    subject: 0,
                    predicate: "broken".to_string(),
                    object: 9,
                    confidence: Confidence::new(0.9),
                },
            ],
            signals: Vec::new(),
        };
        let composite = CompositeExtractor::new()
            .with(Box::new(Scripted {
                id: "one",
                version: 1,
                found: first,
            }))
            .with(Box::new(Scripted {
                id: "two",
                version: 1,
                found: second,
            }));
        let found = run(&composite);
        assert_eq!(found.candidates.len(), 3);
        let pairs: Vec<(usize, usize, &str)> = found
            .relations
            .iter()
            .map(|r| (r.subject, r.object, r.predicate.as_str()))
            .collect();
        assert_eq!(pairs, [(0, 1, "served_by"), (0, 2, "served_by")]);
    }

    #[test]
    fn signals_of_the_same_pack_keep_the_larger_count() {
        let signal = |hits, distinct_terms| SignalDraft {
            pack_id: "health".to_string(),
            hits,
            distinct_terms,
        };
        let composite = CompositeExtractor::new()
            .with(Box::new(Scripted {
                id: "one",
                version: 1,
                found: ExtractedKnowledge {
                    signals: vec![signal(3, 2)],
                    ..ExtractedKnowledge::default()
                },
            }))
            .with(Box::new(Scripted {
                id: "two",
                version: 1,
                found: ExtractedKnowledge {
                    signals: vec![signal(5, 1)],
                    ..ExtractedKnowledge::default()
                },
            }));
        let found = run(&composite);
        assert_eq!(found.signals, [signal(5, 2)]);
    }
}
