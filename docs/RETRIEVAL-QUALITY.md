# Retrieval quality: design note, "before" figures, then results (tab 3)

Started 15 October 2026 (branch `feat/retrieval-quality`) for the owner, who decides section 8, and then for the developer of lots A to H. English, like all of `docs/`; the interface strings quoted here are French because the user reads French. **No product code has been written yet.** What exists so far: the "before" measures (section 2) and this note. Lot A is built and measured (section 10).

The aim, binding (owner, 14 October 2026): on a library of **30** or **1 000** documents, and on **one document of 1 000 pages or more**, a question reads **a handful of precise, located passages**; the answer says **what share of the documents it rests on**; a list question is answered by the index with no model; an absent fact is refused; an "everything" question about a book is answered from a stored digest with an optional, honest, hand-started deeper pass. The model writes the answer and nothing else.

## 1. Which figure is real

| Figure | Real or faked |
| --- | --- |
| The 205-question exam, all four channels (`pipeline`, `hybrid30`, `vector30`, `lexical30`) | **real**: the embedding model `assistant-embed` (`nomic-embed-text`, 768 numbers) on the LAN tower, 965 passages |
| The six set sentences, name queries, `knowledge_*` invariants, golden chunks, spreadsheet references, schema safety | deterministic: no model is involved (the vectors, when a test needs them, are fake). The numbers are exact, not estimates |
| Latency and memory of the ranking at 100 000 passages | real CPU time on this workstation, **synthetic** passages with 768 random numbers (`ranking_scale_bench`) |
| The planted-fact exam on the long sets | **real** only on the scale-0.1 set (800 pages, 2 549 passages, real vectors). On the 8 000-page set and the 1 007-document library, vectors are **faked** (the word channel is real, the meaning channels are meaningless there), exactly as in `docs/LONG-DOCUMENTS.md` section 1 |
| Times of an Analyse with the real model on 8 000 pages, and of the three-model comparison on the long set | **projected** from the measured seconds per passage, until measured; each projection says so |
| Generation time and answer text (lots E, G) | real, `ministral-3:3b` on the tower, only when a lot claims something about the answer |

## 2. The "before" (15 October 2026, 07:54 to 08:00, no product file edited)

Elapsed, wall clock: the exam **2 min 43 s** (release build already cached; of which the Analyse with the real model 118 s, 0.122 s a passage); the six sentences 17 s; the validation, tabular, golden and schema-safety binaries 45 s; the 18 other `knowledge_*` binaries 1 min 43 s (section 2.4).

### 2.1 The 205-question exam, real vectors (`data/retrieval-lab/ld3-before-exam`)

Library of 200 documents, 176 read by the product, 965 passages, perfect text recognition for the scans. **Not comparable with the 13 October baseline of `docs/RETRIEVAL-LAB.md`**: that replay answered 194 questions through the retrieval; the finder now answers the set, rank and listing sentences itself, so **138 questions are replayed (57 are answered by the folder router and left out) and 131 are scored** (183 on 13 October). Every "after" of this tab is compared with the figures below, never with the old table.

| Channel | R@5 | R@10 | R@30 | MRR | nDCG@10 | fact@10 | files found / expected | median / p95 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `pipeline` (6 passages) | 0.679 | 0.784 | 0.784 | 0.703 | 0.679 | 0.714 | 255 / 471 | 11 / 25 ms |
| `hybrid30` | 0.571 | 0.663 | 0.940 | 0.642 | 0.606 | 0.714 | 445 / 471 | 12 / 23 ms |
| `vector30` | 0.480 | 0.579 | 0.740 | 0.556 | 0.514 | 0.500 | 300 / 471 | 1.4 / 2.8 ms |
| `lexical30` | 0.780 | 0.843 | 0.932 | 0.844 | 0.837 | 0.714 | 438 / 471 | 0.9 / 5.3 ms |

Question embedding: median 197 ms (138 questions). The 5 questions the documents cannot answer still receive **6.0 passages** on `pipeline` (24 on `lexical30`, 30 on the others): nothing in the retrieval says "no" (lot E).

`pipeline`, recall@10 by family: alias 0.652, entity_free 0.833, follow_up 0.357, homonym 0.250, **identifier 0.515 (MRR 0.331)**, leak 0.833, long_document 1.000, mixed 0.694, multi_entity 0.500, named_file 1.000, ocr 1.000, org_vs_person 1.000, organization 0.867, person 0.961, practitioner 0.682, small_scope 1.000, term 1.000. The word search alone beats the pipeline at depth 10 again (0.843 against 0.784), and the identifier family is still the weakest.

### 2.2 The six set sentences, the names, the rank

Test `set_search::the_six_set_sentences_of_the_final_test_find_their_files_and_nothing_else`, through the application's path (router, finder, names port), 200-document library: **111 of 111, 0 wrong file, 0 gateway call**; LAMal 13/13, franchise annuelle 13/13, Genève 33/33, Lausanne 20/20, Nyon 8/8, Annecy 24/24. The latest letter (77 readable) is `courrier-harmonie.pdf` of 2026-10-07; the 37 AVS numbers typed without separators are found in 48 of 48 files, 0 wrong file. The 767 name queries are the finder suite (`file_finder`), run at the end with the rest.

### 2.3 Invariants, spreadsheets, golden, schema

| Binary | Result |
| --- | --- |
| `knowledge_validation` (I1 to I4 on 30 documents, the manifest) | 3 passed, 0 failed (4 ignored reports) |
| `knowledge_validation_walk` (lifecycle: I3, I14, no orphan row) | 1 passed (8 ignored reports) |
| `tabular_reference` (106 cases, cold and warm) | 3 passed |
| `chunking_golden` (146 short documents byte-identical) | 1 passed |
| `schema_safety` (old index untouched, spreadsheet tables identical, `data` questions answered the same, nothing heavy at open) | 4 passed |

### 2.4 The other `knowledge_*` binaries (I5 to I13)

The 18 `knowledge_*` test binaries (`book`, `documents`, `fix_names`, `fix_retrieval`, `fix_swiss_first`, `isolation`, `manage`, `manage_walk`, `names`, `names_calibration`, `packs`, `picker`, `resolver`, `store`, `surfaces`, `surfaces_walk`, `tables`, `targeting`), run as one block in 1 min 43 s: **412 passed, 0 failed, 22 ignored** (the ignored ones are the printing reports). Together with section 2.3 they are the "before" of the invariants I1 to I14; the same block is replayed once at the end.

## 3. What tab 2 left, checked in the code before relying on it

Nothing named in the handoff is missing. Verified: `Evidence.ordinal` and `Evidence.section` (`retrieval.rs:63-87`), the finder's `Locator { page, ordinal, sectionLabel }` with `locators` and `locatorsTotal`, chunker v2 with `LongOptions.prefix` off by default, document cards in their own table, `vector_counts`, `SearchOutcome.unvectorised_chunks` and `mismatched_chunks`, the lean ranking and its parity test, the corpus and libraries under `data/` (`long-documents`, `long-documents-tenth`, `long-library-1000`), the golden test.

Gaps that lot A has to close, read in the code:

1. `Evidence` carries one page (`page_number`, where the passage starts). The chunker knows the page of every line, so it knows where a passage ends, but nothing stores it (`chunks` has no such column). A fact after a page break inside a split table, a list or a hyphenated word is cited on the page of the header.
2. `EvidenceCoverage` has `files_covered`, `indexed_files`, `unreadable_files` and is sent **only** for `Plan::EveryDocument` (`commands.rs:2060`). An ordinary question over the whole library sends no coverage at all; `partial_vectors` and `embedding_mismatch` (which `SearchOutcome` already knows) never reach the answer.
3. `coverageLine` returns nothing when every analysed file was covered and none is unreadable. It never says "complet", which is right, but it also cannot say "3 of 885".
4. `Evidence` is built in three places (`search_scoped_with_hits`, `first_chunk_of`, test helpers in `mixed_answer`) and mirrored by `Evidence` in `ipc.ts`.

## 4. The switches, and how a method is judged

**One struct, no environment variable in product code.** `RetrievalOptions` (all `bool` or small enums, `Copy`, default = today's behaviour exactly) is carried inside `RetrievalLimits`, which already travels from `commands.rs` through every search function, so no signature changes. `RetrievalLimits::for_budget` returns the product's chosen options; the lab builds its own with `with_options`. The lab reads environment variables **in the test file only** (as it does today for `LongOptions`), so a measure is "all switches off except one".

| Switch | Lot | Needs a new Analyse? | Where it acts |
| --- | --- | --- | --- |
| `french_stems` | B | **yes** (an extra full-text column, rebuilt from stored text: no re-extraction, no re-embedding) | `chunks_fts`, `file_profile_fts`, query stemmed the same way |
| `field_weights` | B | yes (title and section columns in `chunks_fts`, same rebuild) | `bm25()` weights |
| `phrase_and_near` | B | no | query builder |
| `thesaurus` | B | no (data per pack) | query builder, OR-groups |
| `content_terms` (off, shadow, on) | B | no | the ordinary road's lexical half |
| `trigram` | B | yes (a second full-text table) | measured option |
| `per_locator_cap`, `neighbours`, `one_per_section`, `edges_first` | C | no | the cap loop and the assembly |
| `passage_targeting` (off, shadow) | C | no | `kb_mentions.chunk_id` read in the diagnostics only |
| `fusion` (flat bonus, RRF) | D | no | replaces the +0.2 bonus; `ranking_parity` stays the guard of the default |
| `pin_identifier` | D | no | exact identifier first on the ordinary road |
| `rerank` | D | no | after the candidate list |
| `refusal_check` | E | no | after retrieval, before generation |

**An index-time switch never acts silently on an index that lacks its structure.** The layout is recorded in `file_profile_meta` (the table that already carries the profile and embedding versions); a query-side switch whose column is absent falls back to today's query, and the old-index line (« L'index doit être refait : appuyez sur Analyser. ») is the way back. The rebuild of a full-text table from stored text runs inside an Analyse the user started, shows in its summary, and costs seconds, not the 45 minutes of the embeddings (to be measured, not assumed).

**The protocol, per method.** (1) All switches off = the "before" of section 2 (the code must reproduce it: a method that changes the OFF path is a bug). (2) One switch on, everything else off, the same measures: the 205-question exam (real), the planted-fact exam on the 800-page set (real) and on the 8 000-page set and the 1 007-document library (word channel only, vectors faked), the six sentences, the name queries. (3) The result goes in a table per method: overall and per family/kind, with the question-level list of what moved. (4) **Kept** only if recall@10 (exam) or fact@10 (planted) rises **and no family or kind is lower**. A family of fewer than 10 questions moves 0.05 per question: a one-question loss is inspected and reported to the owner, who decides; it is not silently called noise. (5) The kept methods are then run **together** once, because two methods that help alone may not add up; that run is the "after" of the lot.

Real elapsed cost of a measure: about 3 minutes for the exam (real), about 5 for the 800-page planted set (real, dominated by the embeddings), seconds for the rest.

## 5. Lot A: contracts and honest coverage

**What changes.**

1. **`Evidence.matches`.** The query's content terms that the passage holds, as character ranges in `text` (`MatchSpan { start, end }`, folded comparison: case and accents ignored, the same folding the full-text index uses). Computed in Rust by one pure function over the passage and the question's terms; empty for a passage found by meaning alone, which is the honest answer ("no word of the question is in it"). It feeds the highlighted display (lot F), the refusal check (lot E), and the statement that a set sentence "searches the word itself". Terms come from the same splitting rule as the lexical channel (`fts_match_expression`: words of four letters or more), moved into one shared function so the two cannot drift.
2. **A page range on the passage.** `Evidence.last_page` and `Locator.lastPage`, equal to the first page for nearly every passage. The chunker (`emit` in `chunking_long.rs`) stores, for a passage that crosses a page break, the page of its last line. **Storage: a new small table `chunk_pages (chunk_id, last_page)` with a row only for passages that cross a break** (decision A1). `chunks` is not touched; an old index has no such rows and reads as "one page"; a document read again by an Analyse gets exact ranges. The citation says « pages 41 à 42 » when the range has two pages, and the known-figures check (P-7) accepts both pages.
3. **Coverage on any search over several documents, with `partial`.** `EvidenceCoverage` gains `partial: bool` and the three counts that explain it, as machine codes only: `unvectorised_chunks` (code `partial_vectors`), `mismatched_chunks` (code `embedding_mismatch`), and `files_cut` (files the budget stopped before reaching). `partial` is true when any of the three is non-zero or when `files_covered < indexed_files` on a question that asked about every document. The line is written by the client from the counts: « Réponse fondée sur 3 des 885 documents analysés. », and, when partial, a second sentence (« La recherche par sens est incomplète : 40 % des passages sont vectorisés. »). **The word "complet" is never produced without a count**, and never produced by the line at all in this tab; a test pins it. When the line appears at all is decision A2.
4. **Both catalogues** (`fr-FR.json`, `en-US.json`) get the new sentences; `catalogues.test.ts` already pins their parity.

**Tests, written first and seen failing** (each fails before the code for a reason other than a missing symbol where possible: they are written against the existing public functions, then the new field is added to the assertion):

- `retrieval::tests`: matches of a passage (accents and case ignored, ranges on character boundaries, empty for a vector-only passage, same terms as the lexical channel).
- `tests/page_range.rs`: on a generated PDF whose table header is the last line of page 10 and whose rows start page 11, the passage carries pages 10 to 11 and the fact's page is inside; a plain passage has one page; an old index (no `chunk_pages`) reads one page; the golden test of short documents is untouched.
- `tests/coverage_honest.rs`: `EvidenceCoverage::of` with the new counts (partial vectors, mismatch, files cut); the 30- and 1 000-document libraries produce "n of N"; no path produces the word "complete" without a count.
- `src/lib/coverage.test.ts`: the line in both languages; the partial sentence; never "complet" alone.
- Lab: `retrieval_lab.rs` writes `lastPage`; `retrieval_lab_score.py --planted` gains `page@k` counted on the range (tests added first in `test_retrieval_lab_score.py`).

**Measured alone against the tab 2 base.** The planted set (800 pages, real vectors): `page@10` for `list_split`, `table_split`, `hyphen_page` must rise from 0.00 to 1.00 with every other figure identical (the ranking does not change: lot A adds fields, it reorders nothing). The exam, the six sentences and the golden test must be identical to section 2. Expected time: about 1 h 15 including the front-end and the catalogues.

## 6. Lots B to H: design in short, each measured alone

**B. The lexical channel, precise.** Elision and diacritics; a Snowball French stemmer in Rust stored as an extra full-text column, the query stemmed the same way (**the crate's licence is checked from its manifest before it is added; free licences only**); `bm25()` weights on `file_profile_fts` and `chunks_fts` (title, section path, author, subject above the body); quoted phrases and `NEAR`; a thesaurus per pack as data under `resources/`, cumulative (a pack adds groups, never a French assumption in code); isolated content terms for the ordinary road's lexical half (shadow first: the diagnostics record what would change); a trigram table as a measured option. A set sentence keeps saying that it searches the word itself unless the thesaurus is on, and then it names the groups it used. Risk: stemming merges unrelated words (« porte », « portée »); the exam's `identifier` and `person` families must not drop. Expected: about 2 h 30 with six measures.

**C. Searching inside a long document.** A cap per locator (section, or a slice of pages) replaces the idea of a cap per file, **inside the same total** (`RetrievalLimits` is not raised: the tower reads about 90 tokens a second); neighbours of a kept passage inside the same character cap; one passage per section unless it is the answer; the best passages at the start and end; per-passage targeting through `kb_mentions.chunk_id` **in shadow only** (the diagnostics record what a passage-level reduction would have kept); "quelles sections parlent de X ?" answered from the index grouped by section, no model; the document card is never a passage (it has its own table, so no passage query can return one: a test already pins it). Measured on the 1 000-page fixtures: the right section in the first 6 passages, with the total number of passages unchanged. About 2 h.

**D. Fusion, rerank, assemble (after B).** Candidates from lexical, entity files, finder filters, and vector on the union, fused by Reciprocal Rank Fusion in place of the flat +0.2; `pin_identifier` kept only if the `identifier` family rises and nothing falls (the item the finder branch left open); the rerank `bge-reranker-v2-m3` (Apache 2.0) on the top 30 to 50. **Where it runs is decision 1.** The embedding model is chosen last: `nomic`, `bge-m3`, `qwen3-embedding:0.6b` on the small, big and long (800-page) sets, with quality and Analyse time for each; `models/LICENSES.md` gets a row before a model is proposed. Projection (not measured): at the tower's present speed an Analyse of the 8 000 pages is about 45 min with `nomic`, about 1 h 50 with `bge-m3` (2.4x) and about 3 h 20 with `qwen3` (4.5x); the 800-page set is a tenth of that. About 3 h, plus the tower's time for the comparison.

**E. Refusal and exact citations.** The application checks that the question's content terms or entities appear in the passages (folded, stem-aware once B is on); if none does, it refuses with a machine code and **no model call**. This is measured on both sides: false refusals on every answerable exam question (must be 0 to keep it, paraphrased questions being the danger) and true refusals on the 5 absent exam questions, the 4 absent planted facts on each long fixture, and the 12 absent questions of the big set. The model cites passage ids and the application maps them to file, section and pages; Ollama's `format` is measured on `ministral-3:3b` with a plain-text fallback. A quotation between guillemets must be a substring of the supplied passages after normalising whitespace, quotes and line breaks (the normaliser is shared with `number_check.rs`); an anomaly is **appended**, never rewritten. About 2 h; the answer-text claims are measured with the real model.

**F. « Passages trouvés ».** Layout in section 7; built only after the owner's decision 2.

**G. Summaries in three stages.** T0 is tab 2's card: « donne-moi le plan » and « résume ce document » read it at once with its coverage line. T1 (chapter summaries by the model) and T2 (a full reading in batches) are started by hand, show their estimated duration **before** starting (a floor of 16 to 18 s plus about 90 tokens a second on the tower, recomputed from the measured speed of the machine in use), run visibly, can be cancelled and resumed, are invalidated when the file's hash changes, and call the answer "complete" **only if 100 % of the units were read without failure**, otherwise they say how many. Stored in the workstation's index in their own table (decision 5); the server stores nothing. About 2 h 30.

**H. Interface leftovers.** Labels of the finder's file states and the vector share in both catalogues; the filter controls over the facets the engine returns; the screen for the user's own business types; the per-file vector share; and, by a human test, seeing the Stop button, the two progress lines and the card line « Recherche par sens » run (KBD-62). The unified left-panel field only if the owner asks (decision 6). About 1 h 30.

## 7. Proposed layout for « Passages trouvés » (lot F, decision 2)

Placed **above the answer, inside the chat message**, built from the retrieval alone, so it is complete before the model writes a word and stays useful if the model fails or is stopped. Matching files first (a word of the question is in the passage), the others folded.

```text
Passages trouvés — 6 passages dans 3 documents sur 885 analysés
┌────────────────────────────────────────────────────────────────────────┐
│ recueil-procedures.pdf   Dr M. Bersier · 12 mars 2026                  │
│   Chapitre 3.2 « Franchise », pages 41 à 42            [Ouvrir à la page 41] │
│   « …la ‹franchise› annuelle est de 300 francs pour… »                 │
│   Chapitre 3.4, page 44                                [Ouvrir à la page 44] │
│   « …le montant de la ‹franchise› est révisé chaque… »                  │
├────────────────────────────────────────────────────────────────────────┤
│ 2026-03-09_decompte-css.pdf   CSS · 9 mars 2026                        │
│   page 1                                               [Ouvrir à la page 1]  │
│   « …‹franchise› de l'année en cours… »                                 │
├────────────────────────────────────────────────────────────────────────┤
│ ▸ Autres documents (1)   trouvés par le sens, aucun mot de la question │
└────────────────────────────────────────────────────────────────────────┘
```

Highlighted terms come from `Evidence.matches`; the section label and the page range from lot A; the author and date from the file profile; « Ouvrir à la page » uses the viewer the application already has. A passage with no matching word is labelled « trouvé par le sens », never highlighted, never presented as a word hit. The alternative is a side panel next to the chat (more room for a long list, less visible on a small screen); the recommendation is the block above the answer because the practice PC is small and the reader looks at the answer first.

## 8. Decisions for the owner

**Answered on 15 October 2026: all recommendations accepted** (A1 the `chunk_pages` table, A2 the line on every search over several documents, 1 the rerank on the AI machine, 2 the block above the answer, 3 per-passage reduction stays in shadow, 4 the thesaurus seed, 5 summaries stored in the workstation's index with the duration shown first). Decision 6, the unified left-panel field: **not now**; the owner asked for an explanation of what is planned, given in the report and recorded in section 10.

The original text of each decision follows, unchanged.

**Blocking lot A (answer these first):**

- **A1. Where the end page of a passage is stored.** Recommended: a new table `chunk_pages` with a row only for passages that cross a page break; `chunks` untouched; an old index keeps working (one page); exact ranges appear when a document is read again. The alternative (an extra nullable column in `chunks`) rewrites the table's shape for every index. A third way (no storage, a range estimated up to the next passage's start page) overstates by up to one page and is not recommended.
- **A2. When the line "Réponse fondée sur n des N documents" appears.** Recommended (it is your aim of 14 October): on **every** answer whose search covered more than one document, always with the count, and with a second sentence when `partial`; no line for a question about one named file. Today it appears only for "every document" questions; the cost of the recommendation is one more line under most answers.

**Needed when their lot comes (answer now or later):**

1. **Where the rerank runs.** Recommended: on the AI machine, behind `AIProvider.rerank` (a small reranker service next to Ollama, which has no rerank endpoint; `providers/ollama.py:113` refuses it today). Reasons: the practice PC is the 2019 machine, the reranker model is large (about 570 M parameters), an ONNX runtime would have to ship for both Windows and macOS, and the AI machine already does the model work. The passages go to the server as they already do for the chat prompt, and the server stores nothing. The workstation variant is measured only if the AI-machine one does not fit.
2. **The layout of « Passages trouvés »** (section 7): block above the answer (recommended) or side panel.
3. **Per-passage reduction.** Recommended: it stays in shadow mode in this tab (KBD-17, KBD-19, P-6: a widened search adds to the narrowed passages); the diagnostics show what it would have changed, and you decide from the figures at the end of lot C.
4. **The first thesaurus seed per pack.** Recommended: about ten groups per pack, listed in the lot B report for your validation before they are switched on: `health-ch` (LAMal, assurance-maladie obligatoire, caisse maladie, franchise and participation, AVS and AHV, AI and assurance-invalidité), `health-fr` (CPAM and caisse primaire d'assurance maladie, mutuelle and complémentaire santé, ordonnance and prescription, arrêt de travail and arrêt maladie, ALD). Grown afterwards from the misses of the exam.
5. **Summaries.** Recommended: stored in the workstation's index in their own table, keyed by the file hash and a prompt version, with the estimated duration shown before T1 and T2 start.
6. **The unified left-panel field.** Recommended: not now.

## 9. Plan, with times (estimates at my pace; the real times are recorded here as each lot ends)

| Step | Estimate | Real |
| --- | --- | --- |
| "Before" measures | 35 min | **6 min 30 s** (07:54 to 08:00:22 by the session's stamps; builds were cached; the exam 2 min 43 s was the longest) |
| Reading and this note | 55 min | **about 2 min** (08:00 to 08:02 by the stamps; the reading ran beside the measures) |
| Lot A (tests, code, catalogues, one measure) | 1 h 15 | **about 45 min** (08:03 to 08:48 by the stamps), of which waiting: the planted run 4 min 52 s, the full Rust suite 11 min, the exam 2 min 32 s |
| Lot B | 2 h 30 | |
| Lot C | 2 h | |
| Lot D (without the three-model comparison's tower time) | 3 h | |
| Lot E | 2 h | |
| Lots F, G, H | 5 h 30 | |

Final pass, once, after the last edit: `cargo test --no-fail-fast`, `clippy`, `rustfmt --edition 2021 --check` on my files only (never plain `cargo fmt`), `pnpm run build`, `pnpm run test`, then the exam, the long fixtures and the libraries once with the kept switches on, compared with section 2.

## 10. Lot A: result (15 October 2026, 08:03 to 08:48)

**What was built.**

| Piece | Where |
| --- | --- |
| The end page of a passage: `Chunked.ends` and `Structure.chunk_ends` from the long chunker (the page of the passage's last line, only when it is past the first), the table `chunk_pages` written with the structure, dropped with the document and by `clear`, read by `IndexStore::last_pages_of_chunks` | `chunking_long.rs`, `structure.rs`, `indexing.rs`, `index_store.rs` |
| `Evidence.last_page`, `Evidence.matches` (`MatchSpan`, character offsets), `query_phrases`, `match_spans`, `attach_places` (section and end page), `cited_pages`; the citation sent to the model reads « pages 10-11 » for a range | `retrieval.rs` |
| `lexical_words`: the one rule for the words the word search uses, shared by the query and by the marking | `index_store.rs` |
| `EvidenceCoverage` with `unvectorisedChunks`, `mismatchedChunks`, `partial`; `SearchShape` and `search_coverage`; sent for every search that is not a named file, denominator = the documents of the user's selection | `retrieval.rs`, `commands.rs` |
| The finder's `Locator.lastPage` | `finder/contracts.rs`, `finder/engine.rs` |
| The client: types, `coverageLine` (count always given, never « complet », the two vector sentences), `groupSources` lists every page of a range, four new catalogue sentences in both languages | `ipc.ts`, `coverage.ts`, `sourceGroups.ts`, `fr-FR.json`, `en-US.json` |
| The lab writes `last_page`; the scorer counts `page@10` on the whole range | `retrieval_lab.rs`, `retrieval_lab_score.py` |

**Tests, written first.** Chunker (2): a split table reports page 2 as its end; every passage reports the last page it reaches and only the crossing ones are listed. Store and search (`page_range`, 5): stored and read; an old index reads one page; the search returns the range; a document written again loses its old ranges; `clear` forgets them. Retrieval (12): the question's words marked without case or accents, none for a passage found by meaning, the four-letter rule, a resolved name marked whole, offsets in characters, both pages in the citation, the figure check knows every page, the coverage in four shapes and its serialised fields. Finder (1). Client (`coverage.test.ts` rewritten to 10 cases, 1 in `sourceGroups`). Scorer (3).

**Seen failing before the code, and how honestly.** The data shapes were added first with neutral values, so the tests failed on their assertions, not on a missing name: chunker 2, retrieval 9 of 11, `page_range` 2 of 5, client 5, scorer 1 of 3. **Five tests passed before the code, by design:** two guards that pin what must not change (a named file has no coverage; a passage found by meaning has no marked word), and three that pin cleanup and compatibility (an old index reads one page; a document written again loses its ranges; `clear` forgets them). **One test was red for the wrong reason at first** (the chunker fixture had no passage over a break, so the test failed on its own "the fixture must have one" check); it was corrected, then shown failing for the right reason by switching the code off and back on. The finder test (`lastPage`) was written beside its code and was red only because the field did not exist yet.

**Measured alone, against the end of tab 2 (planted facts, 800 pages, 2 549 passages, real vectors, 50 questions; `final-real` against `ld3-a-real`).**

| `page@10` (the fact's page lies inside the passage's range) | Tab 2 end | Lot A |
| --- | --- | --- |
| `pipeline`, all 50 | 0.462 | **1.000** |
| `lexical30` | 0.462 | **1.000** |
| `hybrid30` | 0.462 | **1.000** |
| `vector30` | 0.423 | 0.962 (its fact@10 is 0.920: it does not find every fact) |
| by kind on `pipeline` | `list_split` 0.00, `table_split` 0.00, `hyphen_page` 0.00, `prose` 0.375 | **all 1.00** |

Every other column is identical: recall@10 1.000, MRR 0.739, fact@10 1.000 (`pipeline`); 0.676 and 0.980 (`hybrid30`); 0.847 and 1.000 (`lexical30`); the 4 absent questions still receive 6.0 passages (lot E). The ranking did not change: lot A adds fields and reorders nothing. The prose kind rose too because a sentence that runs over a page break is a passage over a break.

**Exam, 205 questions, real vectors, before and after:** all four channels identical to the third decimal (R@10, R@30, MRR, nDCG@10, fact@10); no question lost recall@10 (`ld3-before-exam` against `ld3-a-exam`).

**Final pass, once, after the last edit.**

| Check | Result | Time |
| --- | --- | --- |
| `cargo test --no-fail-fast` | **1 557 passed, 0 failed, 49 ignored** (79 test binaries), including the six sentences (111 of 111, 0 wrong file), the golden chunks of the 146 short documents, `tabular_reference`, `schema_safety` (the spreadsheet tables identical), the 18 `knowledge_*` binaries | 11 min (1 537 passed in tab 2; the new tests are 20; the time is longer because the library changed and every binary was rebuilt) |
| `cargo clippy --release --all-targets` | no warning in the code of this lot; the others sit in files not touched here (one is in a test helper of `retrieval.rs` that was already there) | 30 s |
| `rustfmt --edition 2021 --check` on the files written or changed | clean for `retrieval.rs`, `index_store.rs`, `chunker_long.rs`, `page_range.rs`, `retrieval_lab.rs`, `chunking_long.rs`, `structure.rs`, `finder/*`, `indexing.rs`, `commands.rs`; `mixed_answer.rs` (src and test) were not clean before and were left as they were | |
| `pnpm run build` and `pnpm run test` | build passes; **700 passed**, 41 files (includes the language guard on `src-tauri/src`) | 16 s |
| Python script tests | 67 passed | 3 s |
| The exam, release, real vectors | identical to the "before" | 2 min 32 s |

**What is not done, said plainly.**

- Documents analysed **before** lot A have no end pages until they are read again (a changed file, or a later bump of the structure version): the structure version was not raised on purpose, so that her existing index is not re-read for this alone. The measures above were run on fresh Analyses. A bump costs one extraction pass over every document, vectors reused; it is the owner's call.
- The coverage line was tested as text and as a contract; it was **not seen running** in the application. The human test of lot H shows it.
- No mutation check of the coverage tests was run (only the chunker's end-page code was switched off to prove its tests fail).
- The 8 000-page and 1 007-document libraries were not re-measured: lot A changes nothing in their ranking, and their cost is in the Analyse, which lot A does not touch beyond one extra table write for the passages over a break (a handful per long document).
- `Evidence.matches` is computed and sent but nothing displays it yet (lot F).

## 11. The unified field in the left panel (decision 6, explained)

Today each folder card (Documents, Data) has its own search field above « Documents utilisés », and each searches only its own domain. The unified field would be **one field at the top of the left panel** that searches both domains at once. The finder already returns `domain` on every hit, so the engine needs nothing; what is left is interface: where the results appear (one list with a small badge « document » or « tableur », or two groups), how « add these files to the selection » acts on both folders at once (the mixed documents-and-tables selection already exists), and what the two cards show while a filter is active. It is about one to two hours of client work and one design decision (the layout). It is left for after the human test of the finder, so that the layout is decided on what she has actually used.
