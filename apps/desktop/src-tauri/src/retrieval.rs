//! Retrieval: lexical plus vector search over the local index, merged into a capped set of
//! excerpts. Never the whole folder - only what is selected here leaves the machine
//! (`docs/RETRIEVAL.md`).

use serde::Serialize;

use crate::extraction::PageOrigin;
use crate::index_store::{cosine_similarity, IndexStore, LexicalHit};

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
    /// A user-chosen set of files (`AnalysisScope`): nothing outside it may be searched.
    Files(&'a [String]),
    /// The whole folder as it is now: every stored chunk whose file is still among these paths,
    /// in the index's own order. Identical to `WholeFolder` whenever the index describes the
    /// current folder. The difference is a previous folder's passages, or a deleted file's, which
    /// the index keeps until the next Analyse and which must not be cited meanwhile
    /// (`docs/SELECTION-AND-MEMORY.md`).
    CurrentFolder(&'a [String]),
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
    search_scoped_counted(index, query_text, query_embedding, scope).map(|found| found.evidence)
}

/// What a search found, with how much it had to look at to find it. The count is for the
/// diagnostics (`knowledge::diagnostics`): it says whether a slow search is slow because the scope
/// is large.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchOutcome {
    pub evidence: Vec<Evidence>,
    /// Stored chunks of the scope that were scored, before the cut to the caps.
    pub chunks_considered: usize,
}

/// `search_scoped`, reporting how many chunks were scored. Same evidence, same order.
pub fn search_scoped_counted(
    index: &IndexStore,
    query_text: &str,
    query_embedding: &[f32],
    scope: RetrievalScope<'_>,
) -> Result<SearchOutcome, crate::error::AppError> {
    let lexical_hits = lexical_hits_in(index, query_text, scope)?;
    search_scoped_with_hits(index, query_embedding, scope, &lexical_hits)
}

/// The lexical bonus candidates for `scope`: the best full-text matches **among the scope's own
/// files**, so a file the user did not select can never use up the places that decide who gets
/// the bonus. `WholeFolder` has no restriction to apply and keeps the unrestricted search.
fn lexical_hits_in(
    index: &IndexStore,
    query_text: &str,
    scope: RetrievalScope<'_>,
) -> Result<Vec<LexicalHit>, crate::error::AppError> {
    match scope {
        RetrievalScope::WholeFolder => index.search_lexical(query_text, MAX_EVIDENCE_CHUNKS),
        RetrievalScope::File(relative_path) => index.search_lexical_in(
            query_text,
            MAX_EVIDENCE_CHUNKS,
            &[relative_path.to_string()],
        ),
        RetrievalScope::Files(relative_paths) | RetrievalScope::CurrentFolder(relative_paths) => {
            index.search_lexical_in(query_text, MAX_EVIDENCE_CHUNKS, relative_paths)
        }
    }
}

/// The ranking itself, given the lexical hits already computed: so `search_per_document` can run
/// the full-text query once for all its files instead of once per file.
fn search_scoped_with_hits(
    index: &IndexStore,
    query_embedding: &[f32],
    scope: RetrievalScope<'_>,
    lexical_hits: &[LexicalHit],
) -> Result<SearchOutcome, crate::error::AppError> {
    let all_chunks = match scope {
        RetrievalScope::WholeFolder => index.all_chunks()?,
        RetrievalScope::File(relative_path) => index.chunks_for_document(relative_path)?,
        RetrievalScope::Files(relative_paths) => {
            let mut chunks = Vec::new();
            for relative_path in relative_paths {
                chunks.extend(index.chunks_for_document(relative_path)?);
            }
            chunks
        }
        RetrievalScope::CurrentFolder(relative_paths) => {
            let present: std::collections::HashSet<&str> =
                relative_paths.iter().map(String::as_str).collect();
            index
                .all_chunks()?
                .into_iter()
                .filter(|chunk| present.contains(chunk.relative_path.as_str()))
                .collect()
        }
    };

    let chunks_considered = all_chunks.len();

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
    Ok(SearchOutcome {
        evidence: selected,
        chunks_considered,
    })
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
    search_per_document_counted(index, query_text, query_embedding, relative_paths)
        .map(|found| found.evidence)
}

/// `search_per_document`, reporting how many chunks were scored across the files. Same evidence,
/// same order.
pub fn search_per_document_counted(
    index: &IndexStore,
    query_text: &str,
    query_embedding: &[f32],
    relative_paths: &[String],
) -> Result<SearchOutcome, crate::error::AppError> {
    let mut selected: Vec<Evidence> = Vec::new();
    let mut total_chars = 0usize;
    let mut chunks_considered = 0usize;

    // The full-text query is the same for every file, so it runs once, over all of them. Run per
    // file it repeated one query as many times as there were documents.
    let lexical_hits = index.search_lexical_in(query_text, MAX_EVIDENCE_CHUNKS, relative_paths)?;

    for relative_path in relative_paths {
        let found = search_scoped_with_hits(
            index,
            query_embedding,
            RetrievalScope::File(relative_path),
            &lexical_hits,
        )?;
        chunks_considered += found.chunks_considered;
        let mut inside = found.evidence;
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

    Ok(SearchOutcome {
        evidence: selected,
        chunks_considered,
    })
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
///
/// Tier 1 of the grounding priority chain (`docs/SELECTION-AND-MEMORY.md`): a document is
/// selected, so the answer is held to it, unconditionally - never softened into a suggestion the
/// model is free to weigh against its own general knowledge. Never mentions who the user is or
/// what profession they practise: the wording must read the same for a doctor, a lawyer, a notary
/// or an accountant, because none of that is this product's business to assume.
pub const RETRIEVAL_INSTRUCTION: &str =
    "You are given excerpts retrieved from the user's own documents. Answer only from these \
     excerpts, citing the file and page for every fact. If the excerpts do not contain the \
     answer, say so instead of guessing.";

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

    /// Several chunks per file, which `store_with_chunks` cannot build: it replaces a document each
    /// time it is given the same path.
    fn store_with_documents(documents: Vec<(&str, Vec<(&str, &str, Vec<f32>)>)>) -> IndexStore {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        for (path, chunks) in documents {
            let stored: Vec<Chunk> = chunks
                .iter()
                .enumerate()
                .map(|(position, (chunk_id, text, _))| Chunk {
                    chunk_id: chunk_id.to_string(),
                    relative_path: path.to_string(),
                    page_number: 1,
                    section: position as u32 + 1,
                    text: text.to_string(),
                    origin: crate::extraction::PageOrigin::TextLayer,
                    confidence: None,
                })
                .collect();
            let vectors: Vec<Vec<f32>> = chunks.into_iter().map(|(_, _, vector)| vector).collect();
            store
                .replace_document(path, "hash", false, &stored, &vectors, None, None)
                .unwrap();
        }
        store
    }

    /// The defect `search_lexical_in` exists for. Eight files outside the selection are stuffed
    /// with the query's words; the one match inside the selection says each word once and its
    /// vector is orthogonal to the question's, so only the lexical bonus can bring it back.
    #[test]
    fn a_scoped_search_gets_lexical_bonus_only_from_its_own_files() {
        let mut documents = vec![(
            "inside.pdf",
            vec![(
                "inside#p1#s1",
                "Cephalees signalees pendant ce rendez-vous du mois dernier",
                vec![0.0, 1.0],
            )],
        )];
        let stuffed_ids: Vec<String> = (0..8).map(|n| format!("stuffed{n}#p1#s1")).collect();
        let stuffed_paths: Vec<String> = (0..8).map(|n| format!("stuffed{n}.pdf")).collect();
        for n in 0..8 {
            documents.push((
                stuffed_paths[n].as_str(),
                vec![(
                    stuffed_ids[n].as_str(),
                    "cephalees episodiques cephalees episodiques cephalees episodiques",
                    vec![0.0, 1.0],
                )],
            ));
        }
        let store = store_with_documents(documents);
        let query = "cephalees episodiques";

        // The trap is real: the unrestricted full-text search spends all six places on the
        // stuffed files, so the in-scope chunk gets no bonus from it.
        let unrestricted = store.search_lexical(query, MAX_EVIDENCE_CHUNKS).unwrap();
        assert_eq!(unrestricted.len(), MAX_EVIDENCE_CHUNKS);
        assert!(
            unrestricted
                .iter()
                .all(|hit| hit.chunk_id.starts_with("stuffed")),
            "the fixture must reproduce the crowding: {unrestricted:?}"
        );

        let allowed = vec!["inside.pdf".to_string()];
        let hits =
            search_scoped(&store, query, &[1.0, 0.0], RetrievalScope::Files(&allowed)).unwrap();
        assert_eq!(hits.len(), 1, "the in-scope match keeps its lexical bonus");
        assert_eq!(hits[0].chunk_id, "inside#p1#s1");

        let named = search_scoped(
            &store,
            query,
            &[1.0, 0.0],
            RetrievalScope::File("inside.pdf"),
        )
        .unwrap();
        assert_eq!(
            named.len(),
            1,
            "a named file is held to its own lexical hits too"
        );

        let mut present = allowed.clone();
        present.extend(stuffed_paths.iter().take(1).cloned());
        let current = search_scoped(
            &store,
            query,
            &[1.0, 0.0],
            RetrievalScope::CurrentFolder(&present),
        )
        .unwrap();
        assert!(
            current.iter().any(|hit| hit.chunk_id == "inside#p1#s1"),
            "the current folder is held to its own files as well: {current:?}"
        );
    }

    /// `WholeFolder` keeps the unrestricted search: with nothing to restrict, nothing changes.
    #[test]
    fn the_whole_folder_keeps_the_unrestricted_lexical_search() {
        let store = store_with_documents(vec![
            (
                "a.pdf",
                vec![("a#p1#s1", "Cephalees episodiques", vec![0.0, 1.0])],
            ),
            (
                "b.pdf",
                vec![("b#p1#s1", "Sujet sans rapport", vec![0.0, 1.0])],
            ),
        ]);

        let hits = search(&store, "cephalees", &[1.0, 0.0]).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk_id, "a#p1#s1");
    }

    /// "Each document" evidence must be what it was when every file ran its own query. The
    /// fixture puts the query word in the weaker-looking chunk of one file, so the answer depends
    /// on the lexical bonus reaching the per-file ranking.
    #[test]
    fn each_document_search_returns_the_same_evidence_with_one_lexical_query() {
        let store = store_with_documents(vec![
            (
                "a.pdf",
                vec![("a#p1#s1", "Bilan annuel de synthese", vec![1.0, 0.0])],
            ),
            (
                "b.pdf",
                vec![
                    ("b#p1#s1", "Introduction generale", vec![0.9, 0.1]),
                    ("b#p1#s2", "Cephalees episodiques", vec![0.8, 0.2]),
                ],
            ),
            (
                "c.pdf",
                vec![("c#p1#s1", "Courrier sans rapport", vec![0.0, 1.0])],
            ),
        ]);
        let files = vec![
            "a.pdf".to_string(),
            "b.pdf".to_string(),
            "c.pdf".to_string(),
        ];

        let found = search_per_document_counted(&store, "cephalees", &[1.0, 0.0], &files).unwrap();

        let ids: Vec<&str> = found
            .evidence
            .iter()
            .map(|item| item.chunk_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec!["a#p1#s1", "b#p1#s2", "c#p1#s1"],
            "a's best chunk, b's lexical match over its closer vector, c's opening chunk as a \
             fallback with score 0"
        );
        assert_eq!(found.evidence[2].score, 0.0);
        assert_eq!(found.chunks_considered, 4);
        assert_eq!(
            search_per_document(&store, "cephalees", &[1.0, 0.0], &files).unwrap(),
            found.evidence
        );
    }

    #[test]
    fn a_counted_search_reports_how_many_chunks_it_scored() {
        let store = store_with_chunks(vec![
            ("a#p1#s1", "a.pdf", "HbA1c a 6.8 pourcent", vec![1.0, 0.0]),
            (
                "b#p1#s1",
                "b.pdf",
                "sujet totalement different",
                vec![0.0, 1.0],
            ),
            ("c#p1#s1", "c.pdf", "autre sujet", vec![0.0, 1.0]),
        ]);

        let whole =
            search_scoped_counted(&store, "HbA1c", &[1.0, 0.0], RetrievalScope::WholeFolder)
                .unwrap();
        assert_eq!(whole.chunks_considered, 3);
        assert_eq!(whole.evidence.len(), 1);

        let paths = vec!["a.pdf".to_string(), "b.pdf".to_string()];
        let files =
            search_scoped_counted(&store, "HbA1c", &[1.0, 0.0], RetrievalScope::Files(&paths))
                .unwrap();
        assert_eq!(files.chunks_considered, 2);
    }

    /// A guard against exactly the regression `RETRIEVAL_INSTRUCTION` was rewritten for on
    /// 27 September 2026: any hint that the person on the other side of this product is a medical
    /// professional, which would be false for the lawyers, notaries and accountants it is meant to
    /// serve too (`docs/DECISIONS.md`, "profession-neutral model-facing text").
    #[test]
    fn the_retrieval_instruction_stays_neutral_about_who_the_user_is() {
        let lower = RETRIEVAL_INSTRUCTION.to_lowercase();
        for word in ["patient", "doctor", "practitioner", "practice", "gp"] {
            assert!(
                !lower.contains(word),
                "{word:?} found in RETRIEVAL_INSTRUCTION"
            );
        }
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
    fn a_file_set_scope_never_returns_evidence_from_outside_the_set() {
        let store = store_with_chunks(vec![
            ("a#p1#s1", "a.pdf", "Cephalees episodiques", vec![1.0, 0.0]),
            ("b#p1#s1", "b.pdf", "Cephalees episodiques", vec![1.0, 0.0]),
            ("c#p1#s1", "c.pdf", "Cephalees episodiques", vec![1.0, 0.0]),
        ]);
        let allowed = vec!["a.pdf".to_string(), "c.pdf".to_string()];

        let hits = search_scoped(
            &store,
            "cephalees",
            &[1.0, 0.0],
            RetrievalScope::Files(&allowed),
        )
        .unwrap();

        let mut paths: Vec<_> = hits.iter().map(|hit| hit.relative_path.as_str()).collect();
        paths.sort();
        assert_eq!(paths, vec!["a.pdf", "c.pdf"]);

        let none =
            search_scoped(&store, "cephalees", &[1.0, 0.0], RetrievalScope::Files(&[])).unwrap();
        assert!(
            none.is_empty(),
            "an empty set allows nothing, not everything"
        );
    }

    #[test]
    fn the_current_folder_answers_exactly_as_the_whole_index_when_they_match() {
        let store = store_with_chunks(vec![
            ("a#p1#s1", "a.pdf", "Cephalees episodiques", vec![1.0, 0.0]),
            ("b#p1#s1", "b.pdf", "Cephalees episodiques", vec![0.9, 0.1]),
            ("c#p1#s1", "c.pdf", "Cephalees depuis hier", vec![0.8, 0.2]),
        ]);
        let present = vec![
            "a.pdf".to_string(),
            "b.pdf".to_string(),
            "c.pdf".to_string(),
        ];

        let whole = search(&store, "cephalees", &[1.0, 0.0]).unwrap();
        let current = search_scoped(
            &store,
            "cephalees",
            &[1.0, 0.0],
            RetrievalScope::CurrentFolder(&present),
        )
        .unwrap();

        assert_eq!(current, whole, "no regression: same evidence, same order");
    }

    #[test]
    fn the_current_folder_never_cites_a_file_that_is_no_longer_in_it() {
        // `old.pdf` belongs to the previous folder, or was deleted since the last Analyse: the
        // index still holds it until the next pass, and it must not reach an answer meanwhile.
        let store = store_with_chunks(vec![
            ("a#p1#s1", "a.pdf", "Cephalees episodiques", vec![1.0, 0.0]),
            (
                "old#p1#s1",
                "old.pdf",
                "Cephalees episodiques",
                vec![1.0, 0.0],
            ),
        ]);
        let present = vec!["a.pdf".to_string()];

        let hits = search_scoped(
            &store,
            "cephalees",
            &[1.0, 0.0],
            RetrievalScope::CurrentFolder(&present),
        )
        .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].relative_path, "a.pdf");
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
