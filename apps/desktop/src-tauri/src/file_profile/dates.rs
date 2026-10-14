//! The date of a document: every date found with its source and confidence, and the one chosen only
//! when the evidence agrees (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 5, "document date").
//!
//! **Never the file's modification time.** The operating system's date says when a file was last saved,
//! which is often years after the document was written; it is not offered here at all. Order of trust:
//! a labelled header date, a letter's `place, marker date`, a dated title, the file-name prefix, the date in
//! the first lines, the creation date in the properties, and last the properties' modification date
//! (kept, never chosen). A date the header gives and the name contradicts is kept twice and chosen once,
//! by confidence. Two different dates at the top confidence mean **no chosen date**: the finder filters
//! on this field, and a wrong value is worse than an empty one.

use super::header::HeaderReading;
use super::metadata::{MetadataKind, MetadataReading};
use super::name::{NameDateSource, NameProfile, NAME_DATE_CONFIDENCE};

/// A date is chosen only at or above this confidence: a modification date and a camera's clock are
/// kept as evidence and never presented as the document's date.
pub const CHOOSE_MIN_CONFIDENCE: f32 = 0.50;

#[derive(Debug, Clone, PartialEq)]
pub struct DateEvidence {
    /// `YYYY-MM-DD`.
    pub iso: String,
    /// A machine code: `header_label`, `header_dated`, `header_title`, `header_text`,
    /// `filename_prefix`, `metadata_created`, `metadata_modified`.
    pub source: &'static str,
    pub confidence: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocumentDates {
    /// Every date found, most confident first.
    pub all: Vec<DateEvidence>,
    /// Index into `all` of the chosen date, when the evidence allows one.
    pub chosen: Option<usize>,
}

impl DocumentDates {
    pub fn chosen(&self) -> Option<&DateEvidence> {
        self.chosen.map(|index| &self.all[index])
    }
}

/// Gather the dates of a document from its header, its name and its properties.
pub fn collect_dates(
    header: &HeaderReading,
    name: &NameProfile,
    metadata: &MetadataReading,
) -> DocumentDates {
    let mut all: Vec<DateEvidence> = Vec::new();
    for date in &header.dates {
        all.push(DateEvidence {
            iso: date.iso.clone(),
            source: date.class.source(),
            confidence: date.class.confidence(),
        });
    }
    if let Some(date) = name
        .date
        .as_ref()
        .filter(|d| d.source == NameDateSource::FilenamePrefix)
    {
        all.push(DateEvidence {
            iso: date.value.clone(),
            source: "filename_prefix",
            confidence: NAME_DATE_CONFIDENCE,
        });
    }
    for field in &metadata.fields {
        let source = match field.kind {
            MetadataKind::Created => "metadata_created",
            MetadataKind::Modified => "metadata_modified",
            _ => continue,
        };
        all.push(DateEvidence {
            iso: field.value.clone(),
            source,
            confidence: field.confidence,
        });
    }
    all.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let chosen = all.first().and_then(|top| {
        if top.confidence < CHOOSE_MIN_CONFIDENCE {
            return None;
        }
        let rival = all
            .iter()
            .take_while(|other| (other.confidence - top.confidence).abs() < 0.001)
            .any(|other| other.iso != top.iso);
        (!rival).then_some(0)
    });
    DocumentDates { all, chosen }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_profile::header::read_header;
    use crate::file_profile::metadata::read_metadata_bytes;
    use crate::file_profile::vocabulary::ProfileVocabulary;

    fn vocabulary() -> ProfileVocabulary {
        ProfileVocabulary::load(&[]).expect("loads")
    }

    fn dates(path: &str, text: &str, front_matter: &str) -> DocumentDates {
        let vocabulary = vocabulary();
        let header = read_header(text, &vocabulary);
        let metadata = read_metadata_bytes(front_matter.as_bytes(), "md", &vocabulary);
        collect_dates(&header, &NameProfile::build(path, None), &metadata)
    }

    fn chosen(dates: &DocumentDates) -> Option<(&str, &str)> {
        dates.chosen().map(|d| (d.iso.as_str(), d.source))
    }

    #[test]
    fn the_header_outranks_the_name_which_outranks_the_properties() {
        let labelled = dates(
            "2026-04-01_invoice.txt",
            "Invoice\nDate : 14 avril 2026\n",
            "",
        );
        assert_eq!(
            chosen(&labelled),
            Some(("2026-04-14", "header_label")),
            "{:?}",
            labelled.all
        );
        assert_eq!(
            labelled.all.len(),
            2,
            "the name's date is kept as evidence too"
        );

        let from_name = dates("2026-04-01_note.txt", "Short note\nwithout any date\n", "");
        assert_eq!(chosen(&from_name), Some(("2026-04-01", "filename_prefix")));

        let from_created = dates("note.txt", "Short note\n", "---\ndate: 2026-05-02\n---\n");
        assert_eq!(
            chosen(&from_created),
            Some(("2026-05-02", "metadata_created"))
        );
    }

    #[test]
    fn two_different_dates_at_the_same_confidence_choose_nothing() {
        let both = dates(
            "note.txt",
            "Short note\nPlanned for 3 mars 2026 or 9 mars 2026\n",
            "",
        );
        assert_eq!(both.all.len(), 2);
        assert!(both.chosen().is_none(), "ambiguous: {:?}", both.all);
        // The same date twice is one fact.
        let twice = dates(
            "note.txt",
            "Short note\nSee 3 mars 2026 and 03/03/2026\n",
            "",
        );
        assert_eq!(chosen(&twice), Some(("2026-03-03", "header_text")));
    }

    #[test]
    fn a_modification_date_and_a_camera_clock_are_kept_but_never_chosen() {
        let vocabulary = vocabulary();
        let mut metadata = MetadataReading::default();
        metadata
            .fields
            .push(crate::file_profile::metadata::MetadataField {
                kind: MetadataKind::Modified,
                value: "2026-06-01".to_string(),
                source: crate::file_profile::metadata::MetadataSource::DocxCore,
                confidence: 0.30,
            });
        let header = read_header("Short note\n", &vocabulary);
        let found = collect_dates(&header, &NameProfile::build("note.txt", None), &metadata);
        assert_eq!(found.all.len(), 1);
        assert!(found.chosen().is_none());
    }

    #[test]
    fn a_folder_that_is_a_month_is_not_a_document_date() {
        let found = dates("reports/2026-03/note.txt", "Short note\n", "");
        assert!(found.all.is_empty() && found.chosen().is_none());
    }

    #[test]
    fn no_date_anywhere_is_an_empty_field() {
        let found = dates("note.txt", "Short note\nnothing here\n", "");
        assert_eq!(found, DocumentDates::default());
    }
}
