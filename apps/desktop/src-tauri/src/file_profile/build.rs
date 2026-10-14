//! The profile of one file, layer 1: everything that needs **no knowledge base** (`docs/LEXICAL-RETRIEVAL-DESIGN.md`
//! 4.6 and 5). One pure function, used by the Analyse (the result is stored) and on the fly for a file
//! that was never analysed (only the name part is then available: no text, no properties).
//!
//! The inputs are plain values: the path, the name before cleaning, the format, the properties already
//! read, the text of the first page, and a workbook's sheet names and column headers. Nothing here
//! opens a file or a database, so the same input always gives the same profile, and a thousand-page
//! document costs the same as a page: only the first page's text is passed in.
//!
//! Layer 2 (links to the people and organisations the base already holds) reads the surfaces this layer
//! collects (`name.name_words`, `roles`, `metadata`, `header.letterhead`) and is a separate step, so that
//! the knowledge mode `off` still gives a complete layer 1.

use serde::{Deserialize, Serialize};

use super::dates::{collect_dates, DocumentDates};
use super::header::{read_header, HeaderReading, TitleSource, TITLE_FIRST_LINE_CONFIDENCE};
use super::metadata::{MetadataKind, MetadataReading};
use super::name::{name_identifiers, split_date_prefix, NameIdentifier, NameProfile};
use super::types::{classify, TypeAssignment, TypeInput};
use super::vocabulary::{words_key, ProfileVocabulary, TypeScope};
use crate::knowledge::packs::PackSet;

/// The number of characters of the first page read for the body markers.
pub const BODY_CHARS: usize = 1500;

/// Which of the two folders a file belongs to; the same relative path can exist in both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Domain {
    Documents,
    Data,
}

impl Domain {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Documents => "documents",
            Self::Data => "data",
        }
    }

    fn scope(self) -> TypeScope {
        match self {
            Self::Documents => TypeScope::Documents,
            Self::Data => TypeScope::Data,
        }
    }
}

/// A workbook's structure, as the Analyse already holds it (read, never written, by this step).
#[derive(Debug, Clone, Default)]
pub struct WorkbookFacts {
    pub sheet_names: Vec<String>,
    pub column_headers: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct ProfileInput<'a> {
    pub relative_path: &'a str,
    pub original_name: Option<&'a str>,
    pub domain: Domain,
    /// The text of the first page. `None` for a file never read, or a scan whose recognition failed.
    pub first_page_text: Option<&'a str>,
    pub metadata: Option<&'a MetadataReading>,
    pub workbook: Option<&'a WorkbookFacts>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TitleChoice {
    pub text: String,
    /// `metadata_title`, `heading` or `first_line`.
    pub source: &'static str,
    pub confidence: f32,
}

/// A name that a label of the header or a property gives a role, as written, not yet linked.
#[derive(Debug, Clone, PartialEq)]
pub struct RoleEvidence {
    pub role: &'static str,
    pub surface: String,
    /// `header_label`, `metadata_author`, `metadata_company`.
    pub source: &'static str,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct FileProfile {
    pub name: NameProfile,
    pub domain: Domain,
    pub header: HeaderReading,
    pub title: Option<TitleChoice>,
    pub document_type: Option<TypeAssignment>,
    pub dates: DocumentDates,
    pub name_identifiers: Vec<NameIdentifier>,
    pub roles: Vec<RoleEvidence>,
}

/// Build layer 1 of the profile of one file.
pub fn build_profile(
    input: &ProfileInput<'_>,
    vocabulary: &ProfileVocabulary,
    packs: &PackSet,
) -> FileProfile {
    let name = NameProfile::build(input.relative_path, input.original_name);
    let empty = MetadataReading::default();
    let metadata = input.metadata.unwrap_or(&empty);

    let header = match (input.domain, input.first_page_text) {
        (Domain::Documents, Some(text)) => read_header(text, vocabulary),
        _ => HeaderReading::default(),
    };
    let title = choose_title(&header, metadata, vocabulary);

    let (title_words, body_words) = (
        title
            .as_ref()
            .map(|t| super::name::words_of(&t.text))
            .unwrap_or_default(),
        input
            .first_page_text
            .map(|text| super::name::words_of(&text.chars().take(BODY_CHARS).collect::<String>()))
            .unwrap_or_default(),
    );
    let workbook = input.workbook.cloned().unwrap_or_default();
    let sheet_names: Vec<Vec<String>> = workbook
        .sheet_names
        .iter()
        .map(|s| super::name::words_of(s))
        .collect();
    let column_headers: Vec<Vec<String>> = workbook
        .column_headers
        .iter()
        .map(|h| super::name::words_of(h))
        .collect();
    let document_type = classify(
        vocabulary,
        &TypeInput {
            scope: input.domain.scope(),
            name_words: &name.name_words,
            title_words: &title_words,
            header_words: &header.header_words,
            body_words: &body_words,
            sheet_names: &sheet_names,
            column_headers: &column_headers,
        },
    );

    let dates = collect_dates(&header, &name, metadata);

    let stem = input
        .relative_path
        .rsplit('/')
        .next()
        .unwrap_or(input.relative_path);
    let stem = stem.rsplit_once('.').map_or(stem, |(stem, _)| stem);
    let name_identifiers = name_identifiers(split_date_prefix(stem).1, packs);

    let mut roles: Vec<RoleEvidence> = Vec::new();
    for hit in &header.roles {
        if let Some(role) = crate::file_profile::vocabulary::ROLES
            .iter()
            .find(|role| **role == hit.role)
        {
            roles.push(RoleEvidence {
                role,
                surface: hit.surface.clone(),
                source: "header_label",
                confidence: hit.confidence,
            });
        }
    }
    // The author of the properties, the last person to save it and the company stamped by the
    // software are weaker than a labelled header line and are kept as their own sources.
    for field in &metadata.fields {
        let (role, source) = match field.kind {
            MetadataKind::Author => ("author", "metadata_author"),
            MetadataKind::Company => ("issuer", "metadata_company"),
            _ => continue,
        };
        roles.push(RoleEvidence {
            role,
            surface: field.value.clone(),
            source,
            confidence: field.confidence,
        });
    }

    // The same name given the same role by two sources is one fact: keep the more confident reading.
    roles.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut seen: Vec<(&'static str, String)> = Vec::new();
    roles.retain(|role| {
        let key = (role.role, words_key(&role.surface));
        let new = !seen.contains(&key);
        if new {
            seen.push(key);
        }
        new
    });

    FileProfile {
        name,
        domain: input.domain,
        header,
        title,
        document_type,
        dates,
        name_identifiers,
        roles,
    }
}

/// The title of the document: the most confident of the properties' title, a Markdown heading and the
/// first line, a generic one (`Document1`, `Untitled`) never. A workbook has no title.
fn choose_title(
    header: &HeaderReading,
    metadata: &MetadataReading,
    vocabulary: &ProfileVocabulary,
) -> Option<TitleChoice> {
    let mut candidates: Vec<TitleChoice> = Vec::new();
    if let Some(field) = metadata.first(MetadataKind::Title) {
        candidates.push(TitleChoice {
            text: field.value.clone(),
            source: "metadata_title",
            confidence: field.confidence,
        });
    }
    if let Some(line) = &header.title {
        if !vocabulary.is_generic(&line.text) && !vocabulary.looks_like_file_name(&line.text) {
            candidates.push(TitleChoice {
                text: line.text.clone(),
                source: match line.source {
                    TitleSource::Heading => "heading",
                    TitleSource::FirstLine => "first_line",
                },
                confidence: line
                    .confidence
                    .min(if line.source == TitleSource::FirstLine {
                        TITLE_FIRST_LINE_CONFIDENCE
                    } else {
                        1.0
                    }),
            });
        }
    }
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_profile::metadata::read_metadata_bytes;

    fn context() -> (ProfileVocabulary, PackSet) {
        (
            ProfileVocabulary::load(&["health-fr", "health-ch"]).expect("loads"),
            PackSet::load("fr-FR", &["health-fr", "health-ch"]).expect("loads"),
        )
    }

    fn build(path: &str, text: Option<&str>) -> FileProfile {
        let (vocabulary, packs) = context();
        build_profile(
            &ProfileInput {
                relative_path: path,
                original_name: None,
                domain: Domain::Documents,
                first_page_text: text,
                metadata: None,
                workbook: None,
            },
            &vocabulary,
            &packs,
        )
    }

    #[test]
    fn an_invoice_gets_its_type_date_title_identifier_and_roles_from_its_text() {
        let text = "Facture FAC-2026-014\nMedSupply SA\nFacture n FAC-2026-014\nDate : 14 avril 2026\nClient : Orbital Clinic\nTotal HT : CHF 326.30\n";
        let profile = build(
            "comptabilite/factures/2026-04-14_facture-medsupply-fac-2026-014.pdf",
            Some(text),
        );
        assert_eq!(
            profile.document_type.as_ref().map(|t| t.code.as_str()),
            Some("invoice")
        );
        assert_eq!(
            profile.dates.chosen().map(|d| d.iso.as_str()),
            Some("2026-04-14")
        );
        assert_eq!(
            profile.title.as_ref().map(|t| t.text.as_str()),
            Some("Facture FAC-2026-014")
        );
        assert_eq!(profile.name_identifiers.len(), 1);
        assert_eq!(profile.name_identifiers[0].normalized, "FAC2026014");
        let roles: Vec<(&str, &str)> = profile
            .roles
            .iter()
            .map(|r| (r.role, r.surface.as_str()))
            .collect();
        assert_eq!(roles, vec![("recipient", "Orbital Clinic")]);
    }

    #[test]
    fn a_file_never_read_still_has_a_profile_from_its_name() {
        let profile = build(
            "laboratoire/2026-06/2026-06-20_resultats-analyses-favre-laurent.jpg",
            None,
        );
        assert_eq!(
            profile.document_type.as_ref().map(|t| t.code.as_str()),
            Some("lab_result")
        );
        assert_eq!(
            profile.dates.chosen().map(|d| (d.iso.as_str(), d.source)),
            Some(("2026-06-20", "filename_prefix"))
        );
        assert!(profile.title.is_none() && profile.roles.is_empty());
        assert!(profile.name.matches("favre laurent"));
    }

    #[test]
    fn a_file_with_nothing_to_say_has_an_empty_profile_and_no_made_up_field() {
        let profile = build("note.txt", Some("just a few words\n"));
        assert!(profile.document_type.is_none());
        assert!(profile.dates.all.is_empty() && profile.dates.chosen().is_none());
        assert!(profile.roles.is_empty() && profile.name_identifiers.is_empty());
    }

    #[test]
    fn a_property_title_and_author_are_used_when_they_say_something_and_ignored_when_generic() {
        let (vocabulary, packs) = context();
        let front_matter =
            "---\ntitle: Annual review\nauthor: Ylva Reviewer\n---\nSecond line of text\n";
        let metadata = read_metadata_bytes(front_matter.as_bytes(), "md", &vocabulary);
        let profile = build_profile(
            &ProfileInput {
                relative_path: "review.md",
                original_name: None,
                domain: Domain::Documents,
                first_page_text: Some(front_matter),
                metadata: Some(&metadata),
                workbook: None,
            },
            &vocabulary,
            &packs,
        );
        let title = profile.title.expect("a title");
        assert_eq!(
            (title.text.as_str(), title.source),
            ("Annual review", "metadata_title")
        );
        assert_eq!(profile.roles.len(), 1);
        assert_eq!(
            (profile.roles[0].role, profile.roles[0].source),
            ("author", "metadata_author")
        );

        let generic = build("a.txt", Some("Document1\nbody text here\n"));
        assert!(
            generic.title.is_none(),
            "a generic first line is not a title"
        );
    }

    #[test]
    fn a_workbook_is_typed_from_its_structure_and_has_no_title_or_header() {
        let (vocabulary, packs) = context();
        let facts = WorkbookFacts {
            sheet_names: vec!["Septembre".to_string(), "Octobre".to_string()],
            column_headers: vec!["Patient".to_string(), "Date du rendez-vous".to_string()],
        };
        let profile = build_profile(
            &ProfileInput {
                relative_path: "rendez-vous-2026.xlsx",
                original_name: None,
                domain: Domain::Data,
                first_page_text: Some("Patient;Montant\n"),
                metadata: None,
                workbook: Some(&facts),
            },
            &vocabulary,
            &packs,
        );
        assert_eq!(
            profile.document_type.as_ref().map(|t| t.code.as_str()),
            Some("schedule")
        );
        assert!(profile.title.is_none() && profile.header.dates.is_empty());
    }
}
