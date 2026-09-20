//! Chunking that keeps file, page and section.
//!
//! A chunk is the unit the index stores and a citation points at, so it must carry enough
//! locator to be shown to her as "file, page, passage" (`docs/ARCHITECTURE.md`, the `Source`
//! model). Sections are paragraph-based here: boring on purpose, replaceable later.

use serde::Serialize;

use crate::extraction::{ExtractedDocument, PageOrigin};

/// Kept small so retrieval sends only a few chunks per answer, never the whole file.
const MAX_CHUNK_CHARS: usize = 1_200;
const MIN_CHUNK_CHARS: usize = 200;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub chunk_id: String,
    pub relative_path: String,
    pub page_number: u32,
    /// 1-indexed position of the paragraph group inside the page, so two chunks on the same
    /// page remain distinguishable in a citation.
    pub section: u32,
    pub text: String,
    pub origin: PageOrigin,
    pub confidence: Option<f32>,
}

/// Split each page into paragraphs, then group consecutive paragraphs up to `MAX_CHUNK_CHARS`
/// so short paragraphs (a date line, a heading) do not become their own noisy chunk.
pub fn chunk(document: &ExtractedDocument) -> Vec<Chunk> {
    let mut chunks = Vec::new();

    for page in &document.pages {
        let paragraphs: Vec<&str> = page
            .text
            .split("\n\n")
            .flat_map(|block| block.split('\n'))
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();

        let mut section: u32 = 0;
        let mut current = String::new();
        for paragraph in paragraphs {
            if !current.is_empty() && current.len() + paragraph.len() + 1 > MAX_CHUNK_CHARS {
                section += 1;
                chunks.push(new_chunk(document, page, section, &current));
                current.clear();
            }
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(paragraph);
            if current.len() >= MAX_CHUNK_CHARS {
                section += 1;
                chunks.push(new_chunk(document, page, section, &current));
                current.clear();
            }
        }
        if !current.is_empty() {
            section += 1;
            chunks.push(new_chunk(document, page, section, &current));
        }
    }

    // A very short trailing chunk (e.g. one date line left alone on a page) is folded into the
    // previous one from the same page rather than shipped as its own weak vector.
    merge_short_trailing_chunks(chunks)
}

fn new_chunk(
    document: &ExtractedDocument,
    page: &crate::extraction::ExtractedPage,
    section: u32,
    text: &str,
) -> Chunk {
    Chunk {
        chunk_id: format!("{}#p{}#s{}", document.relative_path, page.page_number, section),
        relative_path: document.relative_path.clone(),
        page_number: page.page_number,
        section,
        text: text.to_string(),
        origin: page.origin,
        confidence: page.confidence,
    }
}

fn merge_short_trailing_chunks(chunks: Vec<Chunk>) -> Vec<Chunk> {
    let mut merged: Vec<Chunk> = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        if chunk.text.len() < MIN_CHUNK_CHARS {
            if let Some(previous) = merged.last_mut() {
                if previous.page_number == chunk.page_number {
                    previous.text.push('\n');
                    previous.text.push_str(&chunk.text);
                    continue;
                }
            }
        }
        merged.push(chunk);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extraction::{ExtractedPage, PageOrigin};

    fn document(pages: Vec<(u32, &str)>) -> ExtractedDocument {
        ExtractedDocument {
            relative_path: "inbox/letter.pdf".into(),
            pages: pages
                .into_iter()
                .map(|(page_number, text)| ExtractedPage {
                    page_number,
                    text: text.to_string(),
                    origin: PageOrigin::TextLayer,
                    confidence: None,
                })
                .collect(),
            empty: false,
            used_ocr: false,
            low_confidence: false,
        }
    }

    #[test]
    fn each_chunk_keeps_file_page_and_section() {
        let document = document(vec![(1, "Bonjour Camille.\n\nDate : 12/03/2026.")]);

        let chunks = chunk(&document);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].relative_path, "inbox/letter.pdf");
        assert_eq!(chunks[0].page_number, 1);
        assert_eq!(chunks[0].chunk_id, "inbox/letter.pdf#p1#s1");
        assert!(chunks[0].text.contains("Bonjour Camille"));
        assert!(chunks[0].text.contains("12/03/2026"));
        assert_eq!(chunks[0].origin, PageOrigin::TextLayer);
        assert_eq!(chunks[0].confidence, None);
    }

    #[test]
    fn a_long_page_becomes_several_chunks_with_increasing_sections() {
        let long_paragraph = "x".repeat(900);
        let text = format!("{long_paragraph}\n\n{long_paragraph}\n\n{long_paragraph}");
        let document = document(vec![(1, &text)]);

        let chunks = chunk(&document);

        assert!(chunks.len() >= 2);
        for (index, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.section, (index + 1) as u32);
            assert_eq!(chunk.page_number, 1);
        }
    }

    #[test]
    fn distinct_pages_produce_distinct_locators() {
        let document = document(vec![
            (1, "Page one content here."),
            (2, "Page two content here."),
        ]);

        let chunks = chunk(&document);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].page_number, 1);
        assert_eq!(chunks[1].page_number, 2);
        assert_ne!(chunks[0].chunk_id, chunks[1].chunk_id);
    }

    #[test]
    fn an_empty_document_produces_no_chunks() {
        let document = document(vec![(1, "")]);

        assert!(chunk(&document).is_empty());
    }

    #[test]
    fn an_ocr_page_carries_origin_and_confidence_onto_its_chunks() {
        let document = ExtractedDocument {
            relative_path: "inbox/scan.pdf".into(),
            pages: vec![ExtractedPage {
                page_number: 1,
                text: "Recognised letter body with enough words to become a chunk.".into(),
                origin: PageOrigin::Ocr,
                confidence: Some(0.81),
            }],
            empty: false,
            used_ocr: true,
            low_confidence: false,
        };

        let chunks = chunk(&document);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].origin, PageOrigin::Ocr);
        assert_eq!(chunks[0].confidence, Some(0.81));
    }

    #[test]
    fn a_below_threshold_page_with_no_text_produces_no_chunk() {
        let document = ExtractedDocument {
            relative_path: "inbox/scan.pdf".into(),
            pages: vec![ExtractedPage {
                page_number: 1,
                text: String::new(),
                origin: PageOrigin::Ocr,
                confidence: Some(0.2),
            }],
            empty: true,
            used_ocr: true,
            low_confidence: true,
        };

        assert!(chunk(&document).is_empty());
    }
}
