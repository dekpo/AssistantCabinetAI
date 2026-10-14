//! The business type of a file: `letter`, `invoice`, `lab_result`, ... for documents; `patient_list`,
//! `invoicing`, `schedule`, ... for workbooks (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 5.1).
//!
//! A type is a **rule as data** (`vocabulary.rs`: a code and the phrases that are evidence for it in the
//! file name, the title, the header lines, the first words of the text, a workbook's sheet names and
//! column headers). The user's own types are the same rules stored as manual rows (step 4); this file
//! only weighs the evidence.
//!
//! **A type is assigned only with enough evidence, otherwise there is none (`other`).** Never forced:
//!
//! - each place has its own confidence (a title phrase 0.90, a header phrase 0.80, a workbook's sheet name
//!   0.80, a file-name word 0.70, a column header 0.65, a phrase deep in the text 0.60), and two places that
//!   agree add 0.05 each;
//! - below 0.70 nothing is assigned: a phrase found only in the body is a hint, not a type;
//! - the generic type (a letter) is the catch-all and loses to **any** specific type that has evidence;
//! - two different specific types within 0.05 of each other are an ambiguity, and the file gets no type.

use super::vocabulary::{contains_phrase, DocumentTypeRule, Phrase, ProfileVocabulary, TypeScope};

/// A type below this confidence is not assigned.
pub const ASSIGN_MIN_CONFIDENCE: f32 = 0.70;
/// Two specific types closer than this are an ambiguity.
const AMBIGUITY_MARGIN: f32 = 0.05;
/// Each additional agreeing place.
const AGREEMENT_BONUS: f32 = 0.05;
const CONFIDENCE_CAP: f32 = 0.97;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TypeSource {
    Name,
    Title,
    Header,
    Body,
    Sheet,
    Column,
}

impl TypeSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Name => "filename",
            Self::Title => "title",
            Self::Header => "header",
            Self::Body => "body",
            Self::Sheet => "sheet_name",
            Self::Column => "column_header",
        }
    }

    fn confidence(self) -> f32 {
        match self {
            Self::Title => 0.90,
            Self::Header | Self::Sheet => 0.80,
            Self::Name => 0.70,
            Self::Column => 0.65,
            Self::Body => 0.60,
        }
    }
}

/// What is known about a file, as folded words.
#[derive(Debug, Clone, Copy)]
pub struct TypeInput<'a> {
    pub scope: TypeScope,
    /// The words of the file name, without its date prefix.
    pub name_words: &'a [String],
    /// The words of the title (empty when there is none).
    pub title_words: &'a [String],
    /// The words of the first lines.
    pub header_words: &'a [String],
    /// The words of the first part of the text.
    pub body_words: &'a [String],
    /// A workbook's sheet names, one phrase each.
    pub sheet_names: &'a [Vec<String>],
    /// A workbook's column headers, one phrase each.
    pub column_headers: &'a [Vec<String>],
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeAssignment {
    pub code: String,
    pub confidence: f32,
    /// The places that gave evidence, most trustworthy first.
    pub sources: Vec<TypeSource>,
}

/// The type of a file, or `None` when the evidence is not enough or is ambiguous.
pub fn classify(vocabulary: &ProfileVocabulary, input: &TypeInput<'_>) -> Option<TypeAssignment> {
    classify_among(vocabulary, input, &|_| true)
}

/// [`classify`] over the rules `accept` keeps: the same weights, the same threshold, the same ambiguity.
pub fn classify_among(
    vocabulary: &ProfileVocabulary,
    input: &TypeInput<'_>,
    accept: &dyn Fn(&DocumentTypeRule) -> bool,
) -> Option<TypeAssignment> {
    let mut candidates: Vec<(&DocumentTypeRule, TypeAssignment)> = Vec::new();
    for rule in vocabulary
        .document_types()
        .filter(|rule| rule.scope == input.scope && accept(rule))
    {
        let mut sources: Vec<TypeSource> = Vec::new();
        let mut consider = |source: TypeSource, found: bool| {
            if found {
                sources.push(source);
            }
        };
        consider(TypeSource::Name, any_phrase(&rule.name, input.name_words));
        consider(
            TypeSource::Title,
            any_phrase(&rule.title, input.title_words),
        );
        consider(
            TypeSource::Header,
            any_phrase(&rule.header, input.header_words),
        );
        consider(TypeSource::Body, any_phrase(&rule.body, input.body_words));
        consider(
            TypeSource::Sheet,
            any_phrase_in(&rule.sheet, input.sheet_names),
        );
        consider(
            TypeSource::Column,
            any_phrase_in(&rule.header, input.column_headers),
        );
        if sources.is_empty() {
            continue;
        }
        sources.sort_by(|a, b| {
            b.confidence()
                .partial_cmp(&a.confidence())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let confidence = (sources[0].confidence() + AGREEMENT_BONUS * (sources.len() - 1) as f32)
            .min(CONFIDENCE_CAP);
        candidates.push((
            rule,
            TypeAssignment {
                code: rule.code.clone(),
                confidence,
                sources,
            },
        ));
    }

    let best = |generic: bool| -> Vec<&TypeAssignment> {
        let mut found: Vec<&TypeAssignment> = candidates
            .iter()
            .filter(|(rule, assignment)| {
                rule.generic == generic && assignment.confidence >= ASSIGN_MIN_CONFIDENCE
            })
            .map(|(_, assignment)| assignment)
            .collect();
        found.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        found
    };
    let specific = best(false);
    if let Some(top) = specific.first() {
        let rival = specific
            .iter()
            .skip(1)
            .any(|other| top.confidence - other.confidence < AMBIGUITY_MARGIN);
        return if rival { None } else { Some((*top).clone()) };
    }
    best(true).first().map(|assignment| (*assignment).clone())
}

/// The type a file is left with when a user type (`user`, a specific one) meets the type it already had
/// (`existing`, `existing_generic` when it is the catch-all). The same rules as [`classify`]: a specific type
/// beats the generic one, two specific types within the margin are an ambiguity and leave none, otherwise the
/// more confident wins.
pub fn combine(
    existing: Option<TypeAssignment>,
    user: Option<TypeAssignment>,
    existing_generic: bool,
) -> Option<TypeAssignment> {
    match (existing, user) {
        (existing, None) => existing,
        (None, Some(user)) => Some(user),
        (Some(_), Some(user)) if existing_generic => Some(user),
        (Some(existing), Some(user)) => {
            if (existing.confidence - user.confidence).abs() < AMBIGUITY_MARGIN {
                None
            } else if user.confidence > existing.confidence {
                Some(user)
            } else {
                Some(existing)
            }
        }
    }
}

fn any_phrase(phrases: &[Phrase], words: &[String]) -> bool {
    !words.is_empty() && phrases.iter().any(|phrase| contains_phrase(words, phrase))
}

/// A phrase of the rule equals (or is contained in) one of a workbook's sheet names or column headers.
fn any_phrase_in(phrases: &[Phrase], places: &[Vec<String>]) -> bool {
    places.iter().any(|place| any_phrase(phrases, place))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_profile::name::words_of;

    fn vocabulary() -> ProfileVocabulary {
        ProfileVocabulary::load(&["health-fr"]).expect("loads")
    }

    fn document(name: &str, title: &str, header: &str, body: &str) -> Option<TypeAssignment> {
        let (name_words, title_words, header_words, body_words) = (
            words_of(name),
            words_of(title),
            words_of(header),
            words_of(body),
        );
        classify(
            &vocabulary(),
            &TypeInput {
                scope: TypeScope::Documents,
                name_words: &name_words,
                title_words: &title_words,
                header_words: &header_words,
                body_words: &body_words,
                sheet_names: &[],
                column_headers: &[],
            },
        )
    }

    fn code(assignment: Option<TypeAssignment>) -> Option<String> {
        assignment.map(|a| a.code)
    }

    #[test]
    fn a_title_phrase_is_the_strongest_evidence() {
        let found =
            document("scan-17", "Resultats d'analyses du 20 juin 2026", "", "").expect("a type");
        assert_eq!(found.code, "lab_result");
        assert!((found.confidence - 0.90).abs() < 0.001);
        assert_eq!(found.sources, vec![TypeSource::Title]);
    }

    #[test]
    fn a_word_of_the_file_name_is_enough_and_agreement_adds_confidence() {
        let name_only = document("facture-orange-fac-2026-116", "", "", "").expect("a type");
        assert_eq!(
            (name_only.code.as_str(), name_only.sources.clone()),
            ("invoice", vec![TypeSource::Name])
        );
        let agreed = document("facture-orange", "Facture FAC-2026-116", "", "").expect("a type");
        assert_eq!(agreed.code, "invoice");
        assert!(agreed.confidence > 0.90, "{}", agreed.confidence);
        assert!(agreed.confidence <= CONFIDENCE_CAP);
    }

    #[test]
    fn a_phrase_only_in_the_body_is_a_hint_and_not_a_type() {
        assert!(document(
            "note",
            "",
            "",
            "the total ttc is high and the validite du devis is long"
        )
        .is_none());
        let agreed = document("devis-leman", "", "", "validite du devis: three months")
            .expect("name and body agree");
        assert_eq!(agreed.code, "quote");
    }

    #[test]
    fn a_specific_type_beats_the_generic_letter() {
        // The name says courrier (a letter); the title says what it is.
        let found = document(
            "courrier-harmonie",
            "Decision de prise en charge, 7 octobre 2026",
            "objet",
            "",
        )
        .expect("a type");
        assert_eq!(found.code, "insurer_statement");
        // A letter stays a letter when nothing more specific is said.
        assert_eq!(
            code(document("courrier-orl-dupont", "", "objet demande", "")),
            Some("letter".to_string())
        );
        assert_eq!(
            code(document("note", "Demande de copie", "cher confrere", "")),
            Some("letter".to_string())
        );
        // `Objet` alone is not a letter: a contract has an article called Objet, a mail has a subject.
        assert!(document("note", "Contrat de maintenance", "objet du contrat", "").is_none());
    }

    #[test]
    fn two_specific_types_that_disagree_give_no_type() {
        // The name says invoice, the title says quote: 0.70 against 0.90 is a clear winner...
        assert_eq!(
            code(document("facture-x", "Devis n 12", "", "")),
            Some("quote".to_string())
        );
        // ...but a name that says both and nothing else decides nothing.
        assert!(document("facture-devis", "", "", "").is_none());
    }

    #[test]
    fn no_evidence_is_no_type() {
        assert!(document("note-interne-photocopieuse", "Photocopier", "", "").is_none());
        assert!(document("", "", "", "").is_none());
    }

    #[test]
    fn a_pack_that_is_not_active_adds_no_types() {
        let neutral = ProfileVocabulary::load(&[]).expect("loads");
        let name_words = words_of("ordonnance-martin");
        let found = classify(
            &neutral,
            &TypeInput {
                scope: TypeScope::Documents,
                name_words: &name_words,
                title_words: &[],
                header_words: &[],
                body_words: &[],
                sheet_names: &[],
                column_headers: &[],
            },
        );
        assert!(
            found.is_none(),
            "prescription belongs to the health pack: {found:?}"
        );
    }

    #[test]
    fn a_workbook_is_typed_from_its_sheet_names_its_headers_and_its_name() {
        let vocabulary = vocabulary();
        let sheets = vec![words_of("Septembre"), words_of("Octobre")];
        let headers = vec![
            words_of("Patient"),
            words_of("Medecin traitant"),
            words_of("Date du rendez-vous"),
        ];
        let name_words = words_of("rendez-vous-2026");
        let found = classify(
            &vocabulary,
            &TypeInput {
                scope: TypeScope::Data,
                name_words: &name_words,
                title_words: &[],
                header_words: &[],
                body_words: &[],
                sheet_names: &sheets,
                column_headers: &headers,
            },
        )
        .expect("a type");
        assert_eq!(found.code, "schedule", "name and a header agree: {found:?}");

        // A patient column alone is a patient list at the column header's confidence: below the bar.
        let only_headers = vec![words_of("Patient"), words_of("Montant")];
        let none = classify(
            &vocabulary,
            &TypeInput {
                scope: TypeScope::Data,
                name_words: &[],
                title_words: &[],
                header_words: &[],
                body_words: &[],
                sheet_names: &[],
                column_headers: &only_headers,
            },
        );
        assert!(none.is_none(), "{none:?}");
    }

    #[test]
    fn a_document_never_gets_a_data_type_and_the_reverse() {
        let found = document("planning-garde", "Planning de garde", "", "");
        assert!(found.is_none(), "schedule is a workbook type: {found:?}");
    }

    fn assigned(code: &str, confidence: f32) -> TypeAssignment {
        TypeAssignment {
            code: code.to_string(),
            confidence,
            sources: vec![TypeSource::Name],
        }
    }

    #[test]
    fn a_type_of_the_user_meets_the_type_a_file_already_has_under_the_rules_of_a_full_reading() {
        let user = Some(assigned("user_x", 0.80));
        // No type yet: the user's wins. The catch-all: a specific type beats it.
        assert_eq!(combine(None, user.clone(), false), user);
        assert_eq!(
            combine(Some(assigned("letter", 0.70)), user.clone(), true),
            user
        );
        // Two specific types close to each other are an ambiguity: none.
        assert_eq!(
            combine(Some(assigned("invoice", 0.78)), user.clone(), false),
            None
        );
        // Otherwise the more confident one.
        assert_eq!(
            combine(Some(assigned("invoice", 0.70)), user.clone(), false),
            user
        );
        let stronger = Some(assigned("invoice", 0.90));
        assert_eq!(combine(stronger.clone(), user, false), stronger);
        // No user type: what the file had.
        assert_eq!(
            combine(Some(assigned("invoice", 0.70)), None, false),
            Some(assigned("invoice", 0.70))
        );
    }
}
