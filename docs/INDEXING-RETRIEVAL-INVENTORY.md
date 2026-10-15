# Indexing and retrieval: the complete inventory of what is planned

14 October 2026 · built from `docs/SESSION-INDEXING-RETRIEVAL-MASTER.md` (**M**), `docs/RETRIEVAL-RESEARCH.md` (**R**, letters A to J), `docs/SESSION-RETRIEVAL-V2.md` (**S1**), `docs/SESSION-LONG-DOCUMENTS.md` (**S2**), the study `docs/test-reports/knowledge-base-pass-1/07-record-study-fixes-and-freeze.md` (**07**, fixes P-1 to P-9), the defects register (**KBD**), the fix battery (`docs/SESSION-KB-FIX-04-battery.md`, **F**) and the findings of the lab (`docs/RETRIEVAL-LAB.md`, **L**). Written for the owner and for the agent who builds it. English; interface strings stay French.

Status: **done** · **build** (decided, to build) · **measure** (built behind a switch, kept only on figures) · **owner** (a decision of the owner is needed first) · **later** · **rejected**. Nothing below the « done » lines is coded.

## Phase 0 - the lab (branch `feat/retrieval-lab`)

| # | Item | Source | Status |
| --- | --- | --- | --- |
| 0.1 | Scoring tool (recall@5/10/30, MRR, nDCG@10, wrong files, time) | M, S1 lot 0, R J2 | done |
| 0.2 | Replay harness without the model, Analyse timings split, index size | M, S1 lot 0 | done |
| 0.3 | Baseline recorded (205 questions, 200 documents; the six set sentences 36/111) | M | done |
| 0.4 | Passage-level truth: a separate small set (29 files) and a big set (500 files, 7 book extracts, 236 questions) | M, R J1 | done |
| 0.5 | Embedding comparison `nomic` / `bge-m3` / `qwen3-embedding:0.6b` on French data, quality and time | M, R B | done |
| 0.6 | Ablation switches: each method behind a switch, measured alone | R J3 | build (with each method) |

## Phase 1 - indexing quality (branch `feat/indexing-quality`, then `feat/long-documents`)

| # | Item | Source | Status |
| --- | --- | --- | --- |
| 1.1 | **Clean the text**: strip headers and footers repeated on every page, repair hyphenation across lines and pages, normalise Unicode and ligatures, spaces inside numbers, drop empty and boilerplate chunks, flag low-confidence OCR chunks | M 1, R A6 | build |
| 1.2 | **Metadata and author**, each field with its source and confidence: PDF Info/XMP/outline (`pdfium-render`), DOCX core and app properties and heading styles, front matter of TXT/MD, file name and folder, the **header block of a letter or form** (sender, recipient, place and date, « Objet : », references, signature) read with per-country pattern packs as data; the **author is an entity with the role `author`**; one `document_fields` table mirrored into full-text columns; document type and date usable as filters | M 2, R A5 and section 5 | build (the role-as-entity point: owner) |
| 1.3 | **Structure**: a sections table (parent, level, title, order, first/last page or offset, locator label) and `section_id` on chunks; title sources in order: PDF outline, DOCX styles, Markdown `#`, heading heuristics, slices of N pages; the locator shows a section label instead of « page 1 » for DOCX/TXT/MD | M 3, S2 lot 2, R A1 | build |
| 1.4 | **Chunking v2, only above a calibrated « long » threshold**: structure-aware, a table or a list is never cut and repeats its header row; chunk size is a measured parameter (about 800 / 1 200 / 1 600 characters, 10 to 20 % overlap); `chunker_version` joins `should_skip`; **golden test: below the threshold the chunks and the ranking are byte-identical to today's**. *L: the table split question L028 fails for every channel and model; this is its fix* | M 4, S2 lot 3, R A1 A2 | build; the threshold: owner |
| 1.5 | **Deterministic contextual prefix** (title, type, author, date, section path) added to the text that is embedded and indexed, not to the text that is cited | M 5, R A3 | build |
| 1.6 | **French lexical analysis**: elision and diacritics; Snowball French stemmer in Rust with the stemmed tokens in an extra full-text column and the query stemmed the same way (check the crate's licence); `bm25()` column weights (title, section path, author, subject above body); phrase and `NEAR` queries; **identifier normalisation** (AVS number, health insurance number, IBAN, invoice numbers stored with and without separators as one token); a **domain thesaurus per knowledge pack as data** (LAMal / assurance-maladie, CPAM / caisse primaire, franchise / participation); trigram table as a measured option; keyphrases per chunk (measure); the entities attached per passage through `kb_mentions.chunk_id`. *L: the AVS number written without dots is found for 1 question of 17: first item to build* | M 6, R C1 C2 C3 C4 C5 C6 A8 | build (trigram, keyphrases: measure) |
| 1.7 | **Document cards**: one short digest chunk per file (title, outline, first sentence of each section, counts), indexed and embedded; it is stage T0 of the summaries | M 7, R A9, S2 lot 8 | build |
| 1.8 | **Resumable, two-speed ingestion inside the Analyse**: phase 1 per file, cancellable (extraction in `spawn_blocking`): text, full-text index, sections, fields and base mentions written first, chunks marked « vector pending » (a clean schema, no empty-blob hack); phase 2 the vectors in batches (16 inputs, 20 000 characters), persisted per batch, « vectorisé à X % »; phase 1 of all files before phase 2; a failing file never blocks the pass (`MAX_CONSECUTIVE_FAILED_FILES`) | M 8, S2 lot 4 | build; the display « vectorisé à X % »: owner |
| 1.9 | **Base caps proportional** to the number of passages (`MAX_ENTITIES_PER_SOURCE`, `MAX_MENTIONS_PER_SOURCE`) with a visible `truncated`; the people-inventory question answered from the base | M 9, S2 lot 7 | build |
| 1.10 | **Vector store at scale**: int8 vectors (measure), the model and dimension recorded in the index with a **mismatch detected and stated, never searched silently**, ranking that no longer loads every chunk's text (score on the vectors of the scope, keep candidates, fetch text afterwards; restrict vector scoring to the other channels' candidates when the scope is large), **parity test** on small libraries, measure at 5 000 / 20 000 / 100 000 chunks | M 10, S2 lot 5, R D3 H1 H2 | build (int8, truncation: measure) |
| 1.11 | **Schema safety**: an old index is detected at launch by a cheap check (« L'index doit être refait : appuyez sur Analyser »), never rebuilt by itself, and an automatic test proves an old index is detected and not touched | M 6, S2 lot 7 | build |
| 1.12 | **The tabular data is never touched by the text re-indexing** (`tabular_inventories`, `tabular_workbooks`, `kb_column_semantics`); an automatic test and the `data` questions of the 205 exam replayed before and after every step | owner, 14 Oct (L section 7) | build |
| 1.13 | **The embedding model is chosen after the lexical improvements**, on the same two sets; a switch forces a full re-Analyse | M, L | measure |

## Phase 2 - retrieval and restitution (branch `feat/retrieval-quality`; **lexical channel first**, owner 14 Oct)

| # | Item | Source | Status |
| --- | --- | --- | --- |
| 2.1 | **Contracts**: `Locator` (page now, section label and page slices later) used by `Evidence`, the listing, « Sources », the known-figures check and `format_evidence`; `Evidence.matches`; `Listing`; `Coverage` (extend `EvidenceCoverage` with the code `partial` and « used / total », never « complet » without a count) | M 1, S1 section 4 | build |
| 2.2 | **Understand the question without a model**: strip the question frame (the router's pack already knows `filler`, subjects, operations) to keep the content terms; entities and their **roles** (« courriers du Dr X » = author); identifiers; dates and document types as filters (reuse the table engine's date parser) | M 2, R F1 F2 F3 | build |
| 2.3 | **P-1, the set question, answered by the index with no model and no embedding**: « Quels documents / courriers / résultats / factures parlent de, mentionnent, évoquent, viennent de, concernent X ? », « Y a-t-il des documents qui parlent de X ? », « Combien de documents mentionnent X ? »; the complete list, the count, one excerpt per file, the first 50 with the total; states that it is a word search; `knowledgeMaxCandidates` never truncates it. **Reverses the router guard: decided 14 Oct** (`docs/DECISIONS.md`) | M 6, S1 lot C, 07 P-1, KBD-53 | build |
| 2.4 | **P-2, the rank** (« le dernier courrier reçu »): from `WorkFolderInventory`, the `YYYY-MM-DD_` prefix else `modified_at`, filtered by the document kind, saying what was ranked | M 6, S1 lot C, 07 P-2, KBD-54 | build |
| 2.5 | **Section granularity** (« quelles sections parlent de X ») once sections exist | M 6, S2 lot 6 | build (after 1.3) |
| 2.6 | **Candidate generation in several channels** (lexical with the new analysis, entity files, metadata filters, vector on the candidate union) fused by **Reciprocal Rank Fusion** replacing the flat +0.2 bonus; optional weighted fusion tuned on the exam | M 3, R D1 D2 | measure (after the lexical work) |
| 2.7 | **Rerank** on the top 30 to 50 (`bge-reranker-v2-m3`, Apache 2.0): ONNX on the workstation (`fastembed-rs`, check Windows and macOS) or a service on the AI machine behind `AIProvider.rerank`; kept only if the figures say so | M 4, R E1 | measure; location: owner |
| 2.8 | **Assemble**: de-duplicate (one passage per section unless it is the answer), cap **per locator** instead of per file **without raising the total**, neighbouring passages inside the same character cap, best passages at the start and end, **few passages** | M 5, S2 lot 6, 07 P-5, R E2 G1 G2 G3 G4, KBD-56 | build |
| 2.9 | **« Passages trouvés » as a first-class display**, ahead of and independent of the answer: located passages with highlighted terms, section, page, author, date, « Ouvrir à la page »; matching files first, the others folded (never filter by matching the model's text to paths) | M 7, S1 lot D, 07 P-3, R I5, KBD-52 | build; the display: owner |
| 2.10 | **Constrain the model**: temperature 0 or 0.1 (P-4, KBD-57); the model cites **passage ids** and the application maps them; structured output through Ollama's `format` (measure on `ministral-3:3b`, plain fallback) | M 8, S1 lot A, 07 P-4, R I1 I2 I3 | build (structured output: measure) |
| 2.11 | **Verify after the answer**: figures (add the locator figures to the known ones: P-7, KBD-49), quotations must be substrings of the passages after normalising whitespace and quotes, ids must exist; anomalies appended, never rewriting the answer | M 8, S1 lot A, S2 lot 9, 07 P-7, R I4 | build |
| 2.12 | **Isolated content terms for the ordinary road's lexical half**, in shadow first | S1 lot E, R F1 | measure |
| 2.13 | **P-6**: a widened search adds to the narrowed passages instead of replacing them (`targeting.rs:598-602`; KBD-17, KBD-19) | M 9, 07 P-6 | owner |
| 2.14 | **Pseudo-relevance feedback** (RM3) | M 9, R F5 | measure, later |
| 2.15 | **Honest coverage everywhere**: « n passages sur N » and `partial` on any global question | M 10, S1 lot C, S2 lot 1 | build |
| 2.16 | **A refusal that does not rest on a similarity threshold** (none exists: *L* section 3.2): the application checks that the question's content terms or entities appear in the passages, plus the model | L | build (new) |

## Phase 3 - long documents, books, libraries (branch `feat/long-documents`)

| # | Item | Source | Status |
| --- | --- | --- | --- |
| 3.1 | Honest coverage on a long document (the same code as 2.15) | S2 lot 1 | build |
| 3.2 | Searching inside a long document: per-locator cap, neighbours, **per-passage targeting through `kb_mentions.chunk_id` in shadow mode first**, set questions at section granularity | M 3, S2 lot 6 | build; leaving shadow mode: owner |
| 3.3 | **Summaries in three stages, every heavy one started by hand**: T0 deterministic digest stored locally (free); T1 optional chapter summaries by the model with the estimated duration shown before it starts, cancellable, resumable, invalidated by the file hash; T2 optional full reading in batches, « complete » only if 100 % of the units were read without failure; the server stores nothing | M 3, S2 lot 8, 07 P-9 | build (T1, T2 on hand); storage and duration display: owner |
| 3.4 | Exact citations and « information absent » tests on every long fixture | S2 lot 9 | build |
| 3.5 | Latency and memory of the ranking at 100 000 chunks, before and after 1.10 | S2 section 6 | measure |

## Rejected, later, deferred

| Item | Source | Status |
| --- | --- | --- |
| Semantic chunking, late chunking, RAPTOR / GraphRAG, ColBERT, fine-tuning an embedding model, query rewriting or HyDE by the model | M 11, R A10 A11 A12 F4 H4 | rejected for now |
| A model-written context per chunk (contextual retrieval) and generated questions per chunk (doc2query) | M 11, R A4 A7 | later, heavy, started by hand, ideally on the Mac mini |
| Tantivy in place of FTS5; an ANN index (HNSW) | R C7 H3 | Tantivy rejected; the ANN index **measured 15 Oct 2026** on 120 000 fake-vector passages, not wired (`docs/RETRIEVAL-QUALITY.md` 12.11) |
| **Dates and amounts as facts (`ld_facts`)** | S2, owner | **built 15 Oct 2026** (`facts.rs`, `docs/RETRIEVAL-QUALITY.md` 12.9): the question's date or amount finds the passage that writes it another way; no answer from a fact alone |
| New readers: ODT, RTF, EML, VTT, ODP, PPTX, ODS (F-20, KBD-30) | owner, 13 Oct | **built 15 Oct 2026** (`readers/`, ODS on the tabular side; `docs/RETRIEVAL-QUALITY.md` 12.10) |
| Fixes that only serve `auto` (F-17, F-18, F-19) and F-21 (the model's sentence against the engine block) | owner, 12 and 13 Oct | not started |
| Books of 1 000 pages in the lab | owner, 14 Oct | not for now |

## Settled rules that bind every item

Windows and macOS both; business code imports no Ollama, Open WebUI or OCR client; the web view never calls the server; the server writes no document text anywhere; no heavy work outside an Analyse the user started; every string sent to a model is profession-neutral with a guard test; Rust and Python return machine codes, French only in the catalogues; free licences only (`models/LICENSES.md` gets a row before a model is used); a schema change needs the owner's go in the design note; one final pass at the end of a phase, one report, git commands for the owner.

## What the lab adds to the order of work

1. Identifier normalisation (1.6): the measured, model-independent failure. 2. The set-question road (2.3), with the question frame (2.2). 3. P-4 and P-7 (2.10, 2.11): an hour. 4. P-2 (2.4). 5. The table-aware chunking (1.4). 6. The remaining lexical analysis (stemming, weights, phrase), each measured alone against the baseline of `docs/RETRIEVAL-LAB.md`. 7. Only then the fusion, the rerank and the model choice.
