//! Retrieval: lexical plus vector search over the local index, merged into a capped set of
//! excerpts. Never the whole folder - only what is selected here leaves the machine
//! (`docs/RETRIEVAL.md`).

use serde::Serialize;

use crate::index_store::{cosine_similarity, IndexStore};

/// How many excerpts an answer may cite at most.
pub const MAX_EVIDENCE_CHUNKS: usize = 6;
/// A hard ceiling on the combined size sent to the gateway, well under the chat context cap.
pub const MAX_EVIDENCE_CHARS: usize = 6_000;
/// Below this similarity a vector hit is noise rather than evidence.
const MIN_VECTOR_SCORE: f32 = 0.15;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub chunk_id: String,
    pub relative_path: String,
    pub page_number: u32,
    pub section: u32,
    pub text: String,
    pub score: f32,
}

/// Rank stored chunks against a query embedding plus its lexical text, merge the two rankings,
/// and cap the result on count and total characters.
pub fn search(
    index: &IndexStore,
    query_text: &str,
    query_embedding: &[f32],
) -> Result<Vec<Evidence>, crate::error::AppError> {
    let lexical_hits = index.search_lexical(query_text, MAX_EVIDENCE_CHUNKS)?;
    let all_chunks = index.all_chunks()?;

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

/// The English instruction plus the excerpts, ready to send as one turn to the gateway. The
/// instruction is English (`docs/LANGUAGE-AND-LOCALE.md`); the excerpts are copied verbatim,
/// whatever language the source document is in - they are data, never rewritten.
pub fn build_context_turn(evidence: &[Evidence]) -> String {
    let mut text = String::from(
        "You are given excerpts retrieved from the practice's own documents. Answer only from \
         these excerpts, citing the file and page for every fact. If the excerpts do not contain \
         the answer, say so instead of guessing.\n\n",
    );
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
            };
            store
                .replace_document(path, "hash", false, &[chunk], &[embedding])
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
        }];

        let turn = build_context_turn(&evidence);

        assert!(turn.contains("inbox/letter.pdf"));
        assert!(turn.contains("page 2"));
        assert!(turn.contains("HbA1c a 6.8 pourcent"));
    }
}
