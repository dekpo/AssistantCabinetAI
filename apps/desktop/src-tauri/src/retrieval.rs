//! Retrieval: lexical plus vector search over the local index, merged into a capped set of
//! excerpts. Never the whole folder - only what is selected here leaves the machine
//! (`docs/RETRIEVAL.md`).

use serde::Serialize;

use crate::extraction::PageOrigin;
use crate::index_store::{cosine_similarity, IndexStore};

/// How many excerpts an answer may cite at most.
pub const MAX_EVIDENCE_CHUNKS: usize = 6;
/// A hard ceiling on the combined size sent to the gateway, well under the chat context cap.
pub const MAX_EVIDENCE_CHARS: usize = 6_000;
/// Below this similarity a vector hit is noise rather than evidence.
const MIN_VECTOR_SCORE: f32 = 0.15;

/// The ceiling for an answer that must cover every document rather than the most relevant few.
/// Larger than `MAX_EVIDENCE_CHARS`, because one excerpt from each of ten files is legitimately
/// more text than six excerpts, and still well under the chat context cap so the question, the
/// instruction and the Work Folder context all fit beside it.
pub const MAX_PER_DOCUMENT_CHARS: usize = 12_000;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub chunk_id: String,
    pub relative_path: String,
    pub page_number: u32,
    pub section: u32,
    pub text: String,
    pub score: f32,
    pub origin: PageOrigin,
    pub confidence: Option<f32>,
}

/// Which part of the corpus a search may draw on.
///
/// `File` is what an explicitly named document produces: the user said which file they meant, so
/// searching the rest of the folder can only contaminate the answer with a passage from a
/// different patient, a different year or a different correspondent
/// (`docs/WORK-FOLDER-INVENTORY.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrievalScope<'a> {
    WholeFolder,
    File(&'a str),
}

/// Rank stored chunks against a query embedding plus its lexical text, merge the two rankings,
/// and cap the result on count and total characters. Searches the whole folder.
pub fn search(
    index: &IndexStore,
    query_text: &str,
    query_embedding: &[f32],
) -> Result<Vec<Evidence>, crate::error::AppError> {
    search_scoped(
        index,
        query_text,
        query_embedding,
        RetrievalScope::WholeFolder,
    )
}

/// The same ranking, restricted to `scope`. The caps are unchanged: a narrower scope means
/// better evidence, never more of it.
pub fn search_scoped(
    index: &IndexStore,
    query_text: &str,
    query_embedding: &[f32],
    scope: RetrievalScope<'_>,
) -> Result<Vec<Evidence>, crate::error::AppError> {
    let lexical_hits = index.search_lexical(query_text, MAX_EVIDENCE_CHUNKS)?;
    let all_chunks = match scope {
        RetrievalScope::WholeFolder => index.all_chunks()?,
        RetrievalScope::File(relative_path) => index.chunks_for_document(relative_path)?,
    };

    let mut scored: Vec<Evidence> = Vec::new();
    for chunk in &all_chunks {
        let vector_score = cosine_similarity(&chunk.embedding, query_embedding);
        let lexical_bonus = if lexical_hits
            .iter()
            .any(|hit| hit.chunk_id == chunk.chunk_id)
        {
            0.2
        } else {
            0.0
        };
        let score = vector_score + lexical_bonus;
        if vector_score < MIN_VECTOR_SCORE && lexical_bonus == 0.0 {
            continue;
        }
        scored.push(Evidence {
            chunk_id: chunk.chunk_id.clone(),
            relative_path: chunk.relative_path.clone(),
            page_number: chunk.page_number,
            section: chunk.section,
            text: chunk.text.clone(),
            score,
            origin: chunk.origin,
            confidence: chunk.confidence,
        });
    }

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut selected = Vec::new();
    let mut total_chars = 0usize;
    for evidence in scored {
        if selected.len() >= MAX_EVIDENCE_CHUNKS {
            break;
        }
        if total_chars + evidence.text.len() > MAX_EVIDENCE_CHARS {
            continue;
        }
        total_chars += evidence.text.len();
        selected.push(evidence);
    }
    Ok(selected)
}

/// One excerpt from **each** of `relative_paths`, in the order given.
///
/// This is the difference between "what is most relevant in this folder" and "what does each of
/// these documents say", and it is why the caller passes a list from the Work Folder inventory
/// instead of letting the ranker choose. Ranking still decides *which* passage inside a file
/// answers best; it no longer decides which files get a voice.
///
/// Files are taken in the order given until the character budget runs out, so the result is
/// deterministic. Whole stored chunks only: an excerpt is never truncated to make it fit, because
/// a citation must point at a passage that really exists. When the budget cannot cover every
/// file, the caller learns it from `EvidenceCoverage` and says so rather than implying it read
/// everything.
pub fn search_per_document(
    index: &IndexStore,
    query_text: &str,
    query_embedding: &[f32],
    relative_paths: &[String],
) -> Result<Vec<Evidence>, crate::error::AppError> {
    let mut selected: Vec<Evidence> = Vec::new();
    let mut total_chars = 0usize;

    for relative_path in relative_paths {
        let mut inside = search_scoped(
            index,
            query_text,
            query_embedding,
            RetrievalScope::File(relative_path),
        )?;
        // Ranking inside one file can find nothing above the noise floor - a summary question
        // shares few words with a lab report. The file still has to be represented, so fall back
        // to its first stored chunk rather than dropping it from an answer that claims to cover
        // every document.
        let best = match inside.is_empty() {
            false => Some(inside.remove(0)),
            true => first_chunk_of(index, relative_path)?,
        };
        let Some(evidence) = best else {
            continue;
        };
        if total_chars + evidence.text.len() > MAX_PER_DOCUMENT_CHARS && !selected.is_empty() {
            break;
        }
        total_chars += evidence.text.len();
        selected.push(evidence);
    }

    Ok(selected)
}

/// The opening passage of a file, used when nothing in it ranks above the noise floor. Its score
/// is zero: it is there for coverage, and the score says so.
fn first_chunk_of(
    index: &IndexStore,
    relative_path: &str,
) -> Result<Option<Evidence>, crate::error::AppError> {
    let mut chunks = index.chunks_for_document(relative_path)?;
    chunks.sort_by_key(|chunk| (chunk.page_number, chunk.section));
    Ok(chunks.into_iter().next().map(|chunk| Evidence {
        chunk_id: chunk.chunk_id,
        relative_path: chunk.relative_path,
        page_number: chunk.page_number,
        section: chunk.section,
        text: chunk.text,
        score: 0.0,
        origin: chunk.origin,
        confidence: chunk.confidence,
    }))
}

/// How much of the corpus an answer actually rests on.
///
/// Computed here, from the evidence that was really assembled and the inventory's own counts -
/// never asked of the model, which only knows what it was sent. The interface shows it so an
/// answer cannot imply it read documents it never received (`docs/WORK-FOLDER-INVENTORY.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceCoverage {
    /// Distinct files the excerpts came from.
    pub files_covered: usize,
    /// Files the local index holds usable content for.
    pub indexed_files: usize,
    /// Files in the folder nothing could be read from.
    pub unreadable_files: usize,
}

impl EvidenceCoverage {
    pub fn of(evidence: &[Evidence], indexed_files: usize, unreadable_files: usize) -> Self {
        let distinct: std::collections::BTreeSet<&str> = evidence
            .iter()
            .map(|item| item.relative_path.as_str())
            .collect();
        Self {
            files_covered: distinct.len(),
            indexed_files,
            unreadable_files,
        }
    }
}

/// The English instruction plus the excerpts, ready to send as one turn to the gateway. The
/// instruction is English (`docs/LANGUAGE-AND-LOCALE.md`); the excerpts are copied verbatim,
/// whatever language the source document is in - they are data, never rewritten.
pub const RETRIEVAL_INSTRUCTION: &str =
    "You are given excerpts retrieved from the practice's own documents. Answer only from \
     these excerpts, citing the file and page for every fact. If the excerpts do not contain \
     the answer, say so instead of guessing.";

pub fn build_context_turn(evidence: &[Evidence]) -> String {
    format!("{RETRIEVAL_INSTRUCTION}\n\n{}", format_evidence(evidence))
}

/// The excerpts alone, numbered and cited. Split out from the instruction so a caller that has
/// more to say in the system turn - the Work Folder knowledge contract, for instance - can order
/// the parts itself rather than concatenate two instructions.
pub fn format_evidence(evidence: &[Evidence]) -> String {
    let mut text = String::new();
    for (index, item) in evidence.iter().enumerate() {
        text.push_str(&format!(
            "[{}] {} (page {}):\n{}\n\n",
            index + 1,
            item.relative_path,
            item.page_number,
            item.text
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::Chunk;
    use crate::index_store::IndexStore;

    fn store_with_chunks(chunks: Vec<(&str, &str, &str, Vec<f32>)>) -> IndexStore {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        for (chunk_id, path, text, embedding) in chunks {
            let chunk = Chunk {
                chunk_id: chunk_id.to_string(),
                relative_path: path.to_string(),
                page_number: 1,
                section: 1,
                text: text.to_string(),
                origin: crate::extraction::PageOrigin::TextLayer,
                confidence: None,
            };
            store
                .replace_document(path, "hash", false, &[chunk], &[embedding], None, None)
                .unwrap();
        }
        store
    }

    #[test]
    fn a_close_vector_match_is_returned() {
        let store = store_with_chunks(vec![
            ("a#p1#s1", "a.pdf", "HbA1c a 6.8 pourcent", vec![1.0, 0.0]),
            (
                "b#p1#s1",
                "b.pdf",
                "sujet totalement different",
                vec![0.0, 1.0],
            ),
        ]);

        let hits = search(&store, "HbA1c", &[1.0, 0.0]).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk_id, "a#p1#s1");
        assert_eq!(hits[0].origin, PageOrigin::TextLayer);
    }

    #[test]
    fn a_scoped_search_returns_evidence_from_that_file_only() {
        let store = store_with_chunks(vec![
            (
                "mars#p1#s1",
                "2026/mars/neurologie.pdf",
                "Cephalees episodiques depuis six semaines",
                vec![1.0, 0.0],
            ),
            (
                "janvier#p1#s1",
                "2026/janvier/neurologie.pdf",
                "Cephalees episodiques depuis deux semaines",
                vec![1.0, 0.0],
            ),
        ]);

        let whole = search(&store, "cephalees", &[1.0, 0.0]).unwrap();
        assert_eq!(whole.len(), 2, "the whole folder still answers as it did");

        let scoped = search_scoped(
            &store,
            "cephalees",
            &[1.0, 0.0],
            RetrievalScope::File("2026/mars/neurologie.pdf"),
        )
        .unwrap();

        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].relative_path, "2026/mars/neurologie.pdf");
    }

    #[test]
    fn a_scope_naming_a_file_with_no_chunks_returns_nothing_rather_than_the_folder() {
        let store = store_with_chunks(vec![(
            "a#p1#s1",
            "a.pdf",
            "HbA1c a 6.8 pourcent",
            vec![1.0, 0.0],
        )]);

        let scoped =
            search_scoped(&store, "HbA1c", &[1.0, 0.0], RetrievalScope::File("b.pdf")).unwrap();

        assert!(scoped.is_empty());
    }

    #[test]
    fn an_off_topic_query_finds_nothing() {
        let store = store_with_chunks(vec![(
            "a#p1#s1",
            "a.pdf",
            "HbA1c a 6.8 pourcent",
            vec![1.0, 0.0],
        )]);

        let hits = search(&store, "recette de cuisine", &[0.0, 1.0]).unwrap();

        assert!(hits.is_empty());
    }

    #[test]
    fn the_context_turn_cites_file_and_page() {
        let evidence = vec![Evidence {
            chunk_id: "a#p2#s1".into(),
            relative_path: "inbox/letter.pdf".into(),
            page_number: 2,
            section: 1,
            text: "HbA1c a 6.8 pourcent".into(),
            score: 0.9,
            origin: PageOrigin::TextLayer,
            confidence: None,
        }];

        let turn = build_context_turn(&evidence);

        assert!(turn.contains("inbox/letter.pdf"));
        assert!(turn.contains("page 2"));
        assert!(turn.contains("HbA1c a 6.8 pourcent"));
    }
}
