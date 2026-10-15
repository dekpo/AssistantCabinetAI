//! Retrieval: lexical plus vector search over the local index, merged into a capped set of
//! excerpts. Never the whole folder - only what is selected here leaves the machine
//! (`docs/RETRIEVAL.md`).

use serde::Serialize;

use crate::conversation::ContextBudget;
use crate::extraction::PageOrigin;
use crate::index_store::{cosine_similarity_with_norm, query_norm, IndexStore, LexicalHit};

/// How many excerpts an answer may cite at most, for a model whose window is the baseline
/// (`ContextBudget::DEFAULT`) or smaller. A larger window raises it (`RetrievalLimits`).
pub const MAX_EVIDENCE_CHUNKS: usize = 6;
/// The combined size of those excerpts, at the baseline window.
pub const MAX_EVIDENCE_CHARS: usize = 6_000;
/// Below this similarity a vector hit is noise rather than evidence.
const MIN_VECTOR_SCORE: f32 = 0.15;

/// The ceiling for an answer that must cover every document rather than the most relevant few, at
/// the baseline window.
/// Larger than `MAX_EVIDENCE_CHARS`, because one excerpt from each of ten files is legitimately
/// more text than six excerpts, and still well under the chat context cap so the question, the
/// instruction and the Work Folder context all fit beside it.
pub const MAX_PER_DOCUMENT_CHARS: usize = 12_000;

/// How much one search may send to the model: how many excerpts, and how many characters for the
/// most relevant ones and for one excerpt per document.
///
/// These used to be three constants, whatever the model. They are now a function of the window the
/// server publishes for the model chosen for the question (`for_budget`), so a larger window reads
/// more of the folder and a small one reads exactly what it always did. Before 13 October 2026 a
/// question about every document stopped after about ten files whatever the window
/// (`docs/CONTEXT-BUDGET.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetrievalLimits {
    pub max_chunks: usize,
    pub max_chars: usize,
    pub max_per_document_chars: usize,
}

impl RetrievalLimits {
    /// What every model got before the limits followed the window; still what a model at or below
    /// the baseline window gets.
    pub const BASELINE: Self = Self {
        max_chunks: MAX_EVIDENCE_CHUNKS,
        max_chars: MAX_EVIDENCE_CHARS,
        max_per_document_chars: MAX_PER_DOCUMENT_CHARS,
    };

    /// The baseline, scaled by how much more than the baseline prompt this model can read.
    pub fn for_budget(budget: &ContextBudget) -> Self {
        Self {
            max_chunks: budget.scaled_over_baseline(Self::BASELINE.max_chunks),
            max_chars: budget.scaled_over_baseline(Self::BASELINE.max_chars),
            max_per_document_chars: budget
                .scaled_over_baseline(Self::BASELINE.max_per_document_chars),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub chunk_id: String,
    pub relative_path: String,
    pub page_number: u32,
    /// The passage's rank inside its page (1, 2, ...): what tells two passages of one page apart. Not a
    /// section of the document; that is `section`.
    pub ordinal: u32,
    /// The section of the document the passage starts in, when the document has a structure.
    pub section: Option<EvidenceSection>,
    pub text: String,
    pub score: f32,
    pub origin: PageOrigin,
    pub confidence: Option<f32>,
}

/// The section a passage belongs to, as the citation shows it. `title` is empty for a slice of pages
/// (the interface says "pages 21 to 40" from the range).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceSection {
    pub title: String,
    pub level: u8,
    pub first_page: u32,
    pub last_page: u32,
}

/// Name the section of each passage, from the structure the Analyse stored. A passage of a document
/// with no structure keeps `None`; so does any passage if the lookup fails (a citation never depends on it).
pub fn attach_sections(index: &IndexStore, evidence: &mut [Evidence]) {
    let ids: Vec<String> = evidence.iter().map(|e| e.chunk_id.clone()).collect();
    let Ok(found) = index.sections_of_chunks(&ids) else {
        return;
    };
    for item in evidence.iter_mut() {
        item.section = found.get(&item.chunk_id).map(|section| EvidenceSection {
            title: section.title.clone(),
            level: section.level,
            first_page: section.first_page,
            last_page: section.last_page,
        });
    }
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
    /// The best cosine similarity any chunk of the scope reached, **without** the lexical bonus
    /// (0 for an empty scope). The bonus says a word of the question was found; this says how close
    /// the best passage is in meaning. The knowledge base reads it to decide that a reduced search was
    /// too narrow (`knowledge::targeting::search_documents`).
    pub best_vector_score: f32,
    /// Passages of the scope that have no vector yet (the Analyse stopped before computing it, or is
    /// still running). They are found by their words only. Non-zero is the machine code `partial_vectors`:
    /// the answer layer says so in its own words.
    pub unvectorised_chunks: usize,
    /// Passages whose stored vector has another length than the question's: they were made by another
    /// embedding model, and cannot be compared. Non-zero is the machine code `embedding_mismatch`: the
    /// meaning part of this search is unreliable until the Analyse recomputes the vectors, and the
    /// answer layer says so instead of searching silently.
    pub mismatched_chunks: usize,
}

/// `search_scoped`, reporting how many chunks were scored. Same evidence, same order.
pub fn search_scoped_counted(
    index: &IndexStore,
    query_text: &str,
    query_embedding: &[f32],
    scope: RetrievalScope<'_>,
) -> Result<SearchOutcome, crate::error::AppError> {
    search_scoped_counted_with_terms(
        index,
        query_text,
        &[],
        query_embedding,
        scope,
        RetrievalLimits::BASELINE,
    )
}

/// `search_scoped_counted`, with extra surface forms for the **lexical** part only: the other
/// spellings of a name the question resolved to (`knowledge::targeting`). The embedding text is the
/// question and nothing else, so what the vector half looks for is exactly what it was. With no
/// extra term and the baseline limits this is `search_scoped_counted`, byte for byte.
pub fn search_scoped_counted_with_terms(
    index: &IndexStore,
    query_text: &str,
    extra_terms: &[String],
    query_embedding: &[f32],
    scope: RetrievalScope<'_>,
    limits: RetrievalLimits,
) -> Result<SearchOutcome, crate::error::AppError> {
    let mut lexical_hits = lexical_hits_in(index, query_text, scope, limits)?;
    lexical_hits.extend(term_hits_in(index, extra_terms, scope, limits)?);
    search_scoped_with_hits(index, query_embedding, scope, &lexical_hits, limits)
}

/// The lexical bonus candidates for `scope`: the best full-text matches **among the scope's own
/// files**, so a file the user did not select can never use up the places that decide who gets
/// the bonus. `WholeFolder` has no restriction to apply and keeps the unrestricted search.
fn lexical_hits_in(
    index: &IndexStore,
    query_text: &str,
    scope: RetrievalScope<'_>,
    limits: RetrievalLimits,
) -> Result<Vec<LexicalHit>, crate::error::AppError> {
    match scope {
        RetrievalScope::WholeFolder => index.search_lexical(query_text, limits.max_chunks),
        RetrievalScope::File(relative_path) => {
            index.search_lexical_in(query_text, limits.max_chunks, &[relative_path.to_string()])
        }
        RetrievalScope::Files(relative_paths) | RetrievalScope::CurrentFolder(relative_paths) => {
            index.search_lexical_in(query_text, limits.max_chunks, relative_paths)
        }
    }
}

/// The same, for the surface forms of resolved entities. Always held to the scope's own files: the
/// whole index is never searched for a name (`WholeFolder` has no use for this and gets nothing).
fn term_hits_in(
    index: &IndexStore,
    extra_terms: &[String],
    scope: RetrievalScope<'_>,
    limits: RetrievalLimits,
) -> Result<Vec<LexicalHit>, crate::error::AppError> {
    if extra_terms.is_empty() {
        return Ok(Vec::new());
    }
    match scope {
        RetrievalScope::WholeFolder => Ok(Vec::new()),
        RetrievalScope::File(relative_path) => index.search_lexical_terms_in(
            extra_terms,
            limits.max_chunks,
            &[relative_path.to_string()],
        ),
        RetrievalScope::Files(relative_paths) | RetrievalScope::CurrentFolder(relative_paths) => {
            index.search_lexical_terms_in(extra_terms, limits.max_chunks, relative_paths)
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
    limits: RetrievalLimits,
) -> Result<SearchOutcome, crate::error::AppError> {
    // The ranking reads the vectors of the scope one row at a time and keeps, for each passage that
    // counts, its id, its score and its size: the text of a passage is read only for the few that are
    // kept. Same scores, same order, same caps as when every passage was loaded with its text
    // (`tests/ranking_parity.rs`).
    let query_length = query_embedding.len();
    let norm = query_norm(query_embedding);
    let lexical: std::collections::HashSet<&str> = lexical_hits
        .iter()
        .map(|hit| hit.chunk_id.as_str())
        .collect();
    let present: std::collections::HashSet<&str> = match scope {
        RetrievalScope::CurrentFolder(paths) => paths.iter().map(String::as_str).collect(),
        _ => std::collections::HashSet::new(),
    };
    let mut scored: Vec<(String, f32, usize)> = Vec::new();
    let mut chunks_considered = 0usize;
    let mut unvectorised_chunks = 0usize;
    let mut mismatched_chunks = 0usize;
    let mut best_vector_score = f32::NEG_INFINITY;
    let mut buffer: Vec<f32> = Vec::new();
    {
        let mut visit = |chunk_id: &str, path: &str, text_bytes: usize, raw: &[u8]| {
            if matches!(scope, RetrievalScope::CurrentFolder(_)) && !present.contains(path) {
                return;
            }
            chunks_considered += 1;
            buffer.clear();
            buffer.extend(
                raw.chunks_exact(4)
                    .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
            );
            if buffer.is_empty() {
                unvectorised_chunks += 1;
            } else if buffer.len() != query_length {
                mismatched_chunks += 1;
            }
            let vector_score = cosine_similarity_with_norm(&buffer, query_embedding, norm);
            best_vector_score = best_vector_score.max(vector_score);
            let lexical_bonus = if lexical.contains(chunk_id) { 0.2 } else { 0.0 };
            if vector_score < MIN_VECTOR_SCORE && lexical_bonus == 0.0 {
                return;
            }
            scored.push((
                chunk_id.to_string(),
                vector_score + lexical_bonus,
                text_bytes,
            ));
        };
        match scope {
            RetrievalScope::WholeFolder | RetrievalScope::CurrentFolder(_) => {
                index.for_each_vector(None, &mut visit)?
            }
            RetrievalScope::File(relative_path) => {
                index.for_each_vector(Some(relative_path), &mut visit)?
            }
            RetrievalScope::Files(relative_paths) => {
                for relative_path in relative_paths {
                    index.for_each_vector(Some(relative_path), &mut visit)?;
                }
            }
        }
    }

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut chosen: Vec<(String, f32)> = Vec::new();
    let mut total_chars = 0usize;
    for (chunk_id, score, text_bytes) in scored {
        if chosen.len() >= limits.max_chunks {
            break;
        }
        if total_chars + text_bytes > limits.max_chars {
            continue;
        }
        total_chars += text_bytes;
        chosen.push((chunk_id, score));
    }
    let ids: Vec<String> = chosen.iter().map(|(id, _)| id.clone()).collect();
    let mut passages = index.passages_by_id(&ids)?;
    let mut selected: Vec<Evidence> = Vec::with_capacity(chosen.len());
    for (chunk_id, score) in chosen {
        let Some(chunk) = passages.remove(&chunk_id) else {
            continue;
        };
        selected.push(Evidence {
            chunk_id: chunk.chunk_id,
            relative_path: chunk.relative_path,
            page_number: chunk.page_number,
            ordinal: chunk.section,
            section: None,
            text: chunk.text,
            score,
            origin: chunk.origin,
            confidence: chunk.confidence,
        });
    }
    attach_sections(index, &mut selected);
    Ok(SearchOutcome {
        evidence: selected,
        chunks_considered,
        best_vector_score: if best_vector_score.is_finite() {
            best_vector_score
        } else {
            0.0
        },
        unvectorised_chunks,
        mismatched_chunks,
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
    search_per_document_counted_with_terms(
        index,
        query_text,
        &[],
        query_embedding,
        relative_paths,
        RetrievalLimits::BASELINE,
    )
}

/// `search_per_document_counted`, with extra surface forms for the lexical part only (see
/// `search_scoped_counted_with_terms`). The files are taken in the order given.
pub fn search_per_document_counted_with_terms(
    index: &IndexStore,
    query_text: &str,
    extra_terms: &[String],
    query_embedding: &[f32],
    relative_paths: &[String],
    limits: RetrievalLimits,
) -> Result<SearchOutcome, crate::error::AppError> {
    let mut selected: Vec<Evidence> = Vec::new();
    let mut total_chars = 0usize;
    let mut chunks_considered = 0usize;
    let mut unvectorised_chunks = 0usize;
    let mut mismatched_chunks = 0usize;
    let mut best_vector_score = 0.0f32;

    // The full-text query is the same for every file, so it runs once, over all of them. Run per
    // file it repeated one query as many times as there were documents.
    let mut lexical_hits =
        index.search_lexical_in(query_text, limits.max_chunks, relative_paths)?;
    if !extra_terms.is_empty() {
        lexical_hits.extend(index.search_lexical_terms_in(
            extra_terms,
            limits.max_chunks,
            relative_paths,
        )?);
    }

    for relative_path in relative_paths {
        let found = search_scoped_with_hits(
            index,
            query_embedding,
            RetrievalScope::File(relative_path),
            &lexical_hits,
            limits,
        )?;
        chunks_considered += found.chunks_considered;
        unvectorised_chunks += found.unvectorised_chunks;
        mismatched_chunks += found.mismatched_chunks;
        best_vector_score = best_vector_score.max(found.best_vector_score);
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
        if total_chars + evidence.text.len() > limits.max_per_document_chars && !selected.is_empty()
        {
            break;
        }
        total_chars += evidence.text.len();
        selected.push(evidence);
    }

    Ok(SearchOutcome {
        evidence: selected,
        chunks_considered,
        best_vector_score,
        unvectorised_chunks,
        mismatched_chunks,
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
    let mut first: Vec<Evidence> = chunks
        .into_iter()
        .take(1)
        .map(|chunk| Evidence {
            chunk_id: chunk.chunk_id,
            relative_path: chunk.relative_path,
            page_number: chunk.page_number,
            ordinal: chunk.section,
            section: None,
            text: chunk.text,
            score: 0.0,
            origin: chunk.origin,
            confidence: chunk.confidence,
        })
        .collect();
    attach_sections(index, &mut first);
    Ok(first.pop())
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
        // The section is named when the document has one with a title: the model can then say "in the
        // section on X" and the reader can find it. A slice of pages has no title and adds nothing.
        let place = match item
            .section
            .as_ref()
            .filter(|section| !section.title.is_empty())
        {
            Some(section) => format!("page {}, section \"{}\"", item.page_number, section.title),
            None => format!("page {}", item.page_number),
        };
        text.push_str(&format!(
            "[{}] {} ({}):\n{}\n\n",
            index + 1,
            item.relative_path,
            place,
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
                index_text: None,
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
                    index_text: None,
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

    /// `count` files, each holding one passage of about a thousand characters that mentions the
    /// query word, so that nothing but the limits decides how many reach the model.
    fn folder_of(count: usize) -> (IndexStore, Vec<String>) {
        let paths: Vec<String> = (0..count).map(|n| format!("file-{n:03}.pdf")).collect();
        let ids: Vec<String> = (0..count).map(|n| format!("file-{n:03}#p1#s1")).collect();
        let text = format!("cephalees {}", "x".repeat(990));
        let store = store_with_documents(
            paths
                .iter()
                .zip(&ids)
                .map(|(path, id)| {
                    (
                        path.as_str(),
                        vec![(id.as_str(), text.as_str(), vec![1.0, 0.0])],
                    )
                })
                .collect(),
        );
        (store, paths)
    }

    fn limits_for_window(window_tokens: usize) -> RetrievalLimits {
        RetrievalLimits::for_budget(&ContextBudget {
            window_tokens,
            output_tokens: 2_048,
            gateway_cap_chars: 1_000_000,
        })
    }

    /// The defect this scaling exists for (13 October 2026, 200 documents): a question about every
    /// document stopped when twelve thousand characters were used, about a dozen files, whatever the
    /// window of the model. At the default window that is still true; a larger window reads more.
    #[test]
    fn a_larger_window_covers_more_documents_of_a_question_about_every_document() {
        let (store, files) = folder_of(80);
        let covered = |limits: RetrievalLimits| {
            search_per_document_counted_with_terms(
                &store,
                "cephalees",
                &[],
                &[1.0, 0.0],
                &files,
                limits,
            )
            .unwrap()
            .evidence
            .len()
        };

        let default_window = covered(limits_for_window(8_192));
        let twelve_k = covered(limits_for_window(12_288));
        let twenty_k = covered(limits_for_window(20_480));

        assert_eq!(default_window, 12, "the default window is unchanged");
        assert!(twelve_k > default_window, "{twelve_k}");
        assert_eq!(
            twenty_k, 36,
            "three times the default prompt, three times the files"
        );
    }

    #[test]
    fn a_larger_window_cites_more_passages_of_an_ordinary_question() {
        let (store, _) = folder_of(80);
        let cited = |limits: RetrievalLimits| {
            search_scoped_counted_with_terms(
                &store,
                "cephalees",
                &[],
                &[1.0, 0.0],
                RetrievalScope::WholeFolder,
                limits,
            )
            .unwrap()
            .evidence
            .len()
        };

        assert_eq!(
            cited(limits_for_window(4_096)),
            6,
            "a small window is unchanged"
        );
        assert_eq!(cited(limits_for_window(8_192)), 6);
        assert_eq!(cited(limits_for_window(20_480)), 18);
    }

    /// The knowledge base widens a reduced search on this number: it must be the closest passage by
    /// meaning, and the lexical bonus must not be in it.
    #[test]
    fn the_best_vector_score_is_the_closest_passage_without_the_lexical_bonus() {
        let store = store_with_chunks(vec![
            ("a#p1#s1", "a.pdf", "Dupont Dupont Dupont", vec![0.0, 1.0]),
            ("b#p1#s1", "b.pdf", "autre chose", vec![1.0, 0.0]),
        ]);
        // The question is close to b by meaning and says Dupont, which only a.pdf holds.
        let found = search_scoped_counted(
            &store,
            "Dupont",
            &[1.0, 0.0],
            RetrievalScope::Files(&["a.pdf".to_string()]),
        )
        .unwrap();

        assert_eq!(found.chunks_considered, 1);
        assert!(found.evidence[0].score >= 0.2, "the bonus is in the score");
        assert!(
            found.best_vector_score < 0.01,
            "and not in the closest passage"
        );

        let all = search_scoped_counted(&store, "Dupont", &[1.0, 0.0], RetrievalScope::WholeFolder)
            .unwrap();
        assert!((all.best_vector_score - 1.0).abs() < 1e-6);

        let nothing =
            search_scoped_counted(&store, "Dupont", &[1.0, 0.0], RetrievalScope::Files(&[]))
                .unwrap();
        assert_eq!(nothing.best_vector_score, 0.0);
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
            ordinal: 1,
            section: None,
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
