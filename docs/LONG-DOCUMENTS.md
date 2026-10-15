# Long documents: design note, corpus and baseline (tab 2, lot 0)

Written 14 October 2026 for the owner, who decides the six points of section 7, and then for the developer of lots 1 to 9. English, like all of `docs/`. **No product code has been written.** Lot 0 produced a corpus, a ground truth, measuring tools, a golden test and the baseline below; the product source (`apps/desktop/src`, `apps/server`, `src-tauri/src`) is unchanged, and so are the spreadsheet tables and `src/tabular/*`.

The aim, binding (owner, 14 October 2026): indexing must be perfect for a library of **30 documents**, for one of **1 000 documents**, and for **one document of 1 000 pages or more**. Both scales are measured, not assumed. This tab builds and measures ingestion and the structures search needs; the retrieval methods (fusion, rerank, stemming, refusals, summaries by the model) are tab 3.

## 1. Which figure is real

Everything below says which one it is.

| Figure | Real or faked |
| --- | --- |
| Corpus, page and chunk counts, characters | real |
| Hash, extraction, chunking, write (store + full-text + knowledge-base ingest), profile pass, finder latency, index size without vectors | **real** (this machine, release build) |
| Embedding time | **faked** in the big runs (two-number vectors, instant). The real cost is **projected** as chunks x 0.106 s, the seconds a passage **measured for real** on the LAN tower in the scale-0.1 run (section 4.4). The 0.74 s of 7 October is stale (older machine) |
| Memory peak | real, but vectors are two numbers wide: add `chunks x 768 x 4` bytes for the real vectors (stated in each row) |
| Retrieval of a planted fact through `lexical30` (the application's own full-text query) | **real** |
| `pipeline`, `hybrid30`, `vector30` with fake vectors | **meaningless**: not tabulated. They are real only in the scale-0.1 run |

## 2. The corpus and the ground truth (built, tested, kept out of git under `data/`)

Generator: `scripts/gen-long-documents.py` (tests: `scripts/test_gen_long_documents.py`, 17 tests, seen failing before the generator existed). Deterministic (the same seed gives the same bytes, tested). Fictional content only.

```text
python scripts/gen-long-documents.py --out data/long-documents                       # 7 long documents, 8 000 pages
python scripts/gen-long-documents.py --base data/knowledge-sandbox-1000 --out data/long-library-1000
python scripts/gen-long-documents.py --scale 0.1 --out data/long-documents-tenth   # a tenth of every size
```

| Document | Format | Pages | File | Structure it carries |
| --- | --- | --- | --- | --- |
| `longs/guide-qualite.pdf` | PDF, text layer | 500 | 2.2 MB | outline (2 levels), running header, numbered footer |
| `longs/recueil-procedures.pdf` | PDF, text layer | 1 000 | 4.4 MB | same |
| `longs/encyclopedie-pratique.pdf` | PDF, text layer | 2 000 | 8.8 MB | same |
| `longs/manuel-formation.docx` | DOCX | 500 logical | 0.13 MB | `Heading1` / `Heading2` styles |
| `longs/livre-blanc.md` | Markdown | 1 000 logical | 3.8 MB | `#` / `##` headings |
| `longs/rapport-annuel.txt` | text | 1 000 logical | 3.8 MB | numbered headings (`3.2 Title`), no markup |
| `longs/archives-numerisees.pdf` | scanned PDF, no text layer | 2 000 | 0.3 MB | the text of each page is in `documents.json`; the laboratory's fake OCR answers it |

A *logical page* of a DOCX, MD or TXT file is 44 body lines; the product reads such a file as one single page. `sections.json` holds each document's real sections (label, level, first and last page): the truth the structure work is scored against.

**Planted facts** (50 questions, 7 documents, plus 4 absent): at known pages, in a known section, each with a unique key so a question has exactly one right file. `contains` is what one passage must carry to count; a question never contains its own answer (tested).

| Kind | What is planted | Why it is there |
| --- | --- | --- |
| `prose` (14) | one sentence with a code | the plain case |
| `heading` (6) | the key word is only in a section heading; the sentence under it names the answer | section path / contextual prefix |
| `list` (6) | a titled list inside one page; `contains` = title **and** item | lists |
| `list_split` (6) | the list title is the last line of a page, the items start the next page | lot 3: a list is never cut |
| `table_split` (6) | the header row is the last line of a page, the rows start the next page; `contains` = header **and** row | lot 3 (inventory 1.4, "L028") |
| `hyphen_line` (6) | the key word is cut at the end of a line (`pharmaco-` / `vigilance`) | lot 1 |
| `hyphen_page` (6) | the key word is cut across a page break | lot 1 |
| `absent` (4) | a topic the corpus covers (`franchise`, `tarif`...), a fact that is nowhere | the refusal tests (tab 3) |

Every PDF page has the same running header and a numbered footer (`Page 12 / 500`): the dirt lot 1 has to remove. Questions carry `fact_pages` (PDF and scan), `fact_logical_pages` (DOCX, MD, TXT) and `fact_sections` (the section label, for every document).

**Two libraries, always both.**

- **30 documents**: the first 30 of `fixtures/knowledge-sandbox` (28 are read by the product).
- **1 000 documents with long documents inside**: `data/long-library-1000` = the 1 000-document sandbox (which already holds PDFs of 115, 198 and 291 pages and three long DOCX) **plus** the seven documents above, appended after document 1 000 (tier 1 007; nothing of the base changes, tested). The product reads 885 of its 1 007 documents.

**Tools** (all under `scripts/` or `tests/`, none is product code): `retrieval_lab_score.py` now also scores `fact@k` / `page@k` / `section@10` per kind of planted fact (`--planted`; tests added first, seen failing); `peak_memory.py` runs a command and reports its peak working set (Windows and macOS/Linux, tested); `retrieval_lab.rs` also writes the profile time and the finder's latency (`finder.json`) and can stop after the Analyse (`ACAI_LAB_ANALYSE_ONLY=1`); **`tests/chunking_golden.rs`** records a hash of the chunks (id, page, ordinal, text, order) of the **146** short text-layer documents of the sandbox in `tests/golden/short-chunks.golden`: it was written first and seen failing (no golden file), then recorded, then shown to catch a one-character change.

## 3. What the code does today (read, not assumed)

- `chunks.embedding` is `BLOB NOT NULL`; `chunks.section` is the chunk's **ordinal inside its page**, not a section of the document; `documents` has no version column, so `should_skip` knows only the file hash and the OCR engine.
- `indexing.rs`: one file at a time, in order: hash, **synchronous** extraction (no `spawn_blocking`), chunk, **embed every chunk of the file**, then one `write_document` transaction (chunks + full-text + knowledge-base mentions). A file whose embedding fails leaves no trace and the next pass redoes it all; **an extraction error aborts the whole pass** (`ExtractionFailed`), only an embedding failure is tolerated (three in a row).
- The profile pass and the knowledge-base backfill run at the end, from stored text. `all_chunks` loads every chunk's text and vector for every question.
- A PDF is read whole into memory (`std::fs::read` then `pdf-extract`'s per-page text, all pages at once): about 4 MB of text for 1 000 pages, small. The heavy item is never the text, it is the chunks with their vectors and, for a scan, the page images.

## 4. The baseline

Release build, this workstation (Windows 11), temporary copies, the knowledge base on (`health-fr`, `health-ch`), fake OCR. Wall times in seconds.

### 4.1 The Analyse, split (vectors faked)

| Run | Documents read | Chunks | Characters | Total | Hash | Extract | Chunk | Write (a) | Embed (fake) | Profile | Other |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 30 documents | 28 | 28 | 17 k | 0.40 | 0 | 0 | 0 | 0.16 | 0.00 | 0.09 | 0.15 |
| 7 long documents, 8 000 pages | 7 | 24 105 | 24.6 M | 14.0 | 0.02 | 6.4 | 0.04 | 6.2 | 0.9 | 0.34 | 0.05 |
| 1 000 + 7 long documents | 885 | 28 711 | 28.7 M | 79.5 | 0.03 | 7.1 | 0.05 | 37.8 | 0.9 | 6.5 | 27.1 |

(a) *write* = chunks + full-text + the knowledge-base ingest of the file, **not split today** (lot 4 must split it). On the 1 000-document library it is 43 ms a file against 0.9 s for a 2 000-page document: the cost per file, not per page, dominates there. "Other" = the knowledge-base backfill and the part of the pass the timer does not name; the profile pass is 7 ms a file.

Run twice: the second time (Analyse only) gave 11.8 s, 85.0 s and 1.4 s; the spread is 8 to 18 % on the big runs, and the 30-document run is dominated by loading the vocabularies once (profile 0.09 s then 1.09 s). The figures above are the first run.

**Projected real embedding** at 0.106 s a passage (measured, 4.4): 24 105 passages = **43 min** for the 7 long documents; 28 711 passages = **51 min** for the 1 000 + 7 library; the 1 000-page PDF alone, 3 982 passages = **7 min**; the 2 000-page text PDF, 7 960 passages = **14 min**. At the 0.74 s of 7 October the same would be 4 h 57, 5 h 54, 49 min and 1 h 38: the machine matters more than the code, and the practice's Mac mini will have its own figure. Either way the embedding is **the Analyse**: 99.3 % of the scale-0.1 run (251 s of 253 s), and every other step together is under a minute for 8 000 pages.

### 4.2 Index size and memory peak

| Run | Index file (fake 2-number vectors) | Real vectors add (768 floats) | Peak memory, whole lab run (b) | Peak, Analyse only |
| --- | --- | --- | --- | --- |
| 30 documents | 1.4 MB | +0.1 MB | 25 MB | **23 MB** |
| 7 long documents | 76 MB | +74 MB = **150 MB** | 166 MB (+74 MB with real vectors) | **94 MB** (+74 MB) |
| 1 000 + 7 long documents | 99 MB | +88 MB = **187 MB** | 211 MB (+88 MB) | **117 MB** (+88 MB) |

The Analyse alone peaks at **94 MB for 8 000 pages** (the whole text of the largest file, its chunks, one file's rows) and 117 MB for 885 files; a real vector adds 3 KB a chunk. The scale-0.1 run with real vectors peaked at 49 MB (2 378 chunks, whole lab).

(b) the whole lab process: the Analyse, then loading **every** chunk (as `all_chunks` does for each question) and the questions. An upper bound for the Analyse.

### 4.3 Questions, finder, profile

| Run | `pipeline` question latency, median / p95 (fake 2-number vectors: a lower bound) | Finder, one word, median of 5 | Finder, all files | Profile pass in the Analyse |
| --- | --- | --- | --- | --- |
| 30 documents | 0.4 ms / 0.6 ms (28 chunks) | 0.7 to 1.6 ms | 0.6 ms | 90 ms |
| 7 long documents | 164 ms / 347 ms | 0.2 to 6.2 ms | 0.7 ms | 341 ms |
| 1 000 + 7 long documents | 139 ms / 292 ms | **9 to 19 ms** over 885 files | 6.4 ms | 6.5 s (7 ms a file) |

The finder is inside its target (under 50 ms at 5 000 files). The question latency of the two big rows is for 24 to 29 thousand chunks with 2-number vectors; with real 768-number vectors `all_chunks` also moves 74 to 88 MB of blobs per question. Measured for real at 2 378 chunks: **44 ms median, 78 ms p95** for the `pipeline` channel, about 18 µs a chunk, which extrapolates to about **0.5 s at 28 711 chunks and 1.8 s at 100 000** (an extrapolation: lot 7 measures it) and is the case for the lean ranking. Embedding the question costs 178 ms (median of 54) on the tower.

### 4.4 The planted facts

**Word search (`lexical30`, real).** Share of questions whose fact is in the first k passages (the passage must carry **all** of `contains`), 7 documents, 50 questions, then the 1 000 + 7 library:

| Kind | 7 documents fact@10 | 1 007 documents fact@10 | Comment |
| --- | --- | --- | --- |
| `prose` | 1.00 | 1.00 | |
| `heading` | 1.00 | 1.00 | the heading and the next line share a chunk |
| `list` | 1.00 | 1.00 | |
| `list_split` | **0.50** | **0.50** | never above 0.5 at any depth: title and item are in two chunks |
| `table_split` | **0.50** | **0.50** | same: header and row are in two chunks (inventory "L028") |
| `hyphen_line` | 0.83 | 0.83 | found through the common words, not the cut word |
| `hyphen_page` | **0.33** (0.50 at 30) | **0.33** | the cut word and its page break |
| `absent` | 30 passages returned for 4 questions that have no answer | 30 | what the refusal rule must catch |

`page@10` is 1.00 for `prose`, `heading`, `list`, `hyphen_line` and **0.00** for `list_split`, `table_split`, `hyphen_page`: the passage returned sits on the page of the header, not on the page of the fact. `section@10` cannot be scored today: the product reports no section label (lot 2 adds it).

**Real vectors, scale 0.1 (7 documents, 800 pages, 2 378 passages, the `assistant-embed` model on the LAN tower, 768 numbers).** The Analyse took **253 s, of which 251 s embedding** (152 batches of 16, 1.65 s a batch: **0.106 s a passage**); the index is 16 MB; the peak 49 MB. Share of questions whose fact is in the first 10 passages, 50 questions, by kind and channel (`pipeline` is what the product sends today: six passages at most; the others read 30 deep):

| Kind | `pipeline` | `hybrid30` | `vector30` | `lexical30` |
| --- | --- | --- | --- | --- |
| `prose` (14) | 1.00 | 0.93 | 0.86 | 1.00 |
| `heading` (6) | 0.83 | 0.33 | 0.17 | 1.00 |
| `list` (6) | 1.00 | 1.00 | 0.50 | 1.00 |
| `list_split` (6) | **0.33** | **0.33** | **0.33** | **0.33** |
| `table_split` (6) | **0.50** | **0.33** | **0.17** | **0.50** |
| `hyphen_line` (6) | 0.50 | 0.83 | 0.67 | 1.00 |
| `hyphen_page` (6) | **0.33** | **0.50** | **0.50** | **0.50** |
| all 50 | 0.70 | 0.66 | 0.52 | 0.80 |
| `absent` (4): passages returned | 5.8 | 30 | 30 | 30 |

Files found among the first 10 passages (recall@10): 0.90, 0.94, 0.84, 1.00. Three kinds fail on **every** channel, with real vectors too: the cause is the chunk boundary, not the model. The vector channel is weakest where the answer hangs on a code or a heading word, which is the lexical channel's strength (tab 3 fuses them). The product's `pipeline` returns 5.8 passages for a question that has no answer: nothing in the retrieval says "no".

### 4.5 What the baseline says

1. **Four of the eight kinds are already right** through the word search; the three that fail (`list_split`, `table_split`, `hyphen_page`) fail **by construction**, because a chunk never crosses a page: that is lot 3 (table and list never cut) and lot 1 (hyphenation across pages).
2. **The Analyse is the embedding.** With the model faked, 8 000 pages take 14 seconds; with the real model on today's tower they take about 43 minutes (about 5 hours at the 7 October speed). Anything that makes the embedding resumable (lot 4) or makes fewer or smaller passages (lot 3 size, lot 1 cleaning) is worth minutes to hours; anything else is worth seconds.
3. **A small file costs more than a page.** The 1 000-document library spends 43 ms a file in "write" (store + full-text + knowledge-base ingest) and 27 s outside the named steps; that is where a 1 000-document Analyse can be made faster without touching the vectors, and lot 4 must time it.
4. **Memory is not a problem on the Analyse** (94 MB for 8 000 pages, 117 MB for 885 files) **but the ranking is**: at 768 numbers the vectors are 74 to 88 MB held and re-read for every question (`all_chunks`, about 18 µs a chunk, a measured 44 ms at 2 378 chunks). Lot 7's lean ranking is where the memory and the latency are won.
5. **An extraction error aborts the whole pass** today (section 3). On a 1 000-page file that means losing hours; lot 4 turns it into "this file failed, the others went on".

## 5. The design (lots 1 to 9)

Written before the build: the contract each lot was tested against. Where the build differs (the prefix is off, the vectors sit in a nullable column of `chunks` rather than a `chunk_vectors` table, the cards are their own table rather than a `kind = 'card'` chunk, the vector-scoring restriction and int8 were not applied), sections 8 bis to 12 say so and win.

### 5.1 The layout, and how an old index is handled (decision 2)

One new layout version, `index_layout = 2`, stored in `file_profile_meta` (the table that already carries `profile_schema.<domain>`), read by one query at launch, reported by the existing `index_status` (`needsAnalyse`) with the **existing French line**, no second mechanism.

```text
documents   + chunker_version, ingest_version, ingest_state ('reading' | 'text' | 'done' | 'failed'), pages, chars
chunks      chunk_id, relative_path, page_number, ordinal (was "section"), section_id NULL, kind ('body'|'table'|'list'|'card'),
            text (what is cited), index_text NULL (the prefixed text that is embedded and indexed, when it differs),
            origin, confidence, vector_state ('pending'|'done')            -- NO embedding column
chunk_vectors  chunk_id PRIMARY KEY, vector BLOB NOT NULL                    -- a pending chunk simply has no row
chunks_fts  unchanged shape (chunk_id UNINDEXED, text); text = index_text when there is one
doc_sections  document, section_id, parent_id, level, ordinal, title, label, first_page, last_page, source ('outline'|'style'|'markdown'|'heuristic'|'slice')
file_profile_sections  (exists, empty) filled; per-section entities and dates in two new profile tables
file_profile_meta  index_layout, embedding_model, embedding_dimension, chunker_version
```

Vectors in their own table are what lets "text first, vectors later" be a clean schema (no empty blob) **and** what lets the ranking score the vectors without loading a single passage (lot 7).

**Old index (recommended policy).** At launch: one query, `needsAnalyse` if `index_layout < 2`; the old tables are **not touched** (`migrate` must not alter an old `chunks` when it opens it); the search and the answer say the index must be redone instead of reading an incompatible layout. When she presses Analyser, the layout is rebuilt **inside the pass**: the old chunks and their vectors are **copied forward** (`INSERT ... SELECT`, seconds), documents whose hash and `chunker_version` are unchanged are not read again, and only a chunk whose **text changed** is embedded again. A layout change therefore costs minutes, not the 5 hours of a rebuild from nothing. An automatic test proves the old index is detected and left byte-identical until the pass.

### 5.2 Ingestion in two speeds (lot 4)

- **Phase 1, per file, cancellable, inside `spawn_blocking`**: hash, extract **page by page** (the OCR path rasterises and recognises one page at a time and checks the cancellation between pages), clean (lot 1), structure (lot 2), chunk (lot 3), then write the chunks (`vector_state = 'pending'`), the full-text rows, the sections and the knowledge-base mentions, **committed every N pages** for a long file (`ingest_state = 'reading'` until the last page) so a cancelled pass keeps what was read; the next pass resumes at the first uncommitted page. The profile pass runs once phase 1 of all files is done, so the finder and the entity base work on everything before a single vector exists.
- **Phase 2**: the pending chunks in batches (16 inputs, 20 000 characters, the current constants), **each batch one transaction** (vectors + `vector_state`), so a stop loses at most one batch. The state of a file is *derived* (`done / total` chunks), never stored twice. A file that keeps failing is marked `failed`, **does not stop the pass**, stays searchable by words, and the card says to analyse again (« degrade, never block indexing »). An extraction error becomes the same state.
- **Order**: phase 1 of every file, then phase 2. The progress text (catalogue, both languages) shows both phases and the share vectorised; the search returns a machine code `partial_vectors` with the share when it ran on a partly vectorised index (its sentence is tab 3's).
- **Memory bound**: text of one file at a time (about 4 MB per 1 000 pages), chunks of one file at a time, one batch of vectors; the profile's identifier pass reads chunks row by row (the cap of 500 stays).

### 5.3 Clean text (lot 1)

Applied to the text that is **indexed and embedded**; the cited text keeps the document's own wording except where the repair is certain. Repeated headers and footers: lines that recur on most pages at the same position once digits are normalised (the running header and `Page n / N` of the corpus), kept in a per-document list so nothing is lost silently. Hyphenation: a line ending in `-` followed by a lowercase letter is joined across lines **and across pages**; the compounds a French text writes with a hyphen at a line end (`peut-être`) are protected by a short list of prefixes. Unicode normalised (NFC), ligatures expanded, spaces inside numbers (`12 345`) kept as one token, empty and boilerplate chunks dropped, OCR chunks below the confidence floor flagged. **By default this applies only above the long threshold** (decision 1), so the golden test of the 146 short documents stays green; applying it to short documents too is a separate, measured decision.

### 5.4 Structure (lot 2)

`doc_sections` is filled from, in order: the PDF outline, DOCX heading styles (`w:pStyle`), Markdown `#`, heading heuristics (numbered headings, short capitals, a line followed by body text), and last the slices of N pages. **PDF outline: `lopdf` (decision 5).** The locator becomes `{ page, ordinal, section: { id, label } }` end to end: `chunks.section` is renamed `ordinal`, `Evidence`, the finder's `Locator` (which carries the ordinal in `section` today) and « Sources » follow, in Rust and in `ipc.ts`. Tables and lists are marked as blocks. Per-section entities and dates go to the profile (design 5.2). The `fact_sections` of the corpus score it.

### 5.5 Chunker v2 above the threshold (lot 3)

Blocks first (paragraph, list, table), then sizes. A table or list is never cut and a split one repeats its header row or title; the **deterministic prefix** (title, type, author, date, section path) is added to `index_text`, never to `text`; `chunker_version` joins `should_skip`, so a change of chunker re-reads only the long documents. Chunk size and overlap are **measured** (decision 4). Below the threshold the chunker is today's, byte for byte (golden test).

### 5.6 Document cards (lot 5)

One deterministic digest per file with no model: title, outline, the first sentence of each section, counts of pages, sections and tables, the profile's type, date and roles. Stored as a `kind = 'card'` chunk, indexed and embedded in phase 2, **never cited as a passage of the document**. It is tab 3's input for "plan" and "résume ce document".

### 5.7 The knowledge base on a book (lot 6)

`MAX_ENTITIES_PER_SOURCE` (500) and `MAX_MENTIONS_PER_SOURCE` (5 000) scale with the number of passages; a `truncated` code is stored and shown in the entity detail window; the profile's `identifiers_truncated` and `cells_truncated` stay and feed the finder's `Coverage.partial`.

### 5.8 Vector store and ranking at scale (lot 7)

The model id and the dimension are recorded; a **mismatch is detected and stated, never searched silently**. The ranking scores `chunk_vectors` for the scope, keeps the top candidates in a heap, and fetches their text afterwards (`SELECT ... WHERE chunk_id IN`); when the scope is large the vector scoring is restricted to the other channels' candidates. **Parity test**: on small libraries the returned passages equal today's, in the same order. Measured at 5 000, 20 000 and 100 000 synthetic chunks and on the 1 000-document library. int8 only if measured worth it.

### 5.9 The finder and the profile on long documents (lot 8)

`FileHit` carries a list of locators (the pages or sections that matched, with a total) and a long document appears **once**; the profile reads the first page for the header facts and is bounded where it reads more; `index_status` and the Analyse summary report the new layout; the search fields and the six set sentences are re-measured on the 1 000 + 7 library (111 of 111, no wrong file, under 50 ms at 5 000 files).

### 5.10 Safety, and the spreadsheets (lot 9)

Tests: an old index is detected and untouched; `tabular_snapshot` (`tabular_inventories`, `tabular_workbooks`, `kb_column_semantics`) identical around a rebuild; the `data` questions of the 205-question exam give the same answers after a rebuilt index; nothing heavy runs at launch or after a pass. `src/tabular/*` is not edited by any lot.

## 6. The plan, with times

Working-time estimates, to be replaced by the measured ones (the earlier steps of this chain took 10 to 20 % of what was estimated). Each lot is tests first, seen failing; the full suite, `clippy`, `rustfmt --check`, `pnpm run build` and `pnpm run test` run **once, at the end**.

| Lot | What | Estimate |
| --- | --- | --- |
| 0 | Corpus, ground truth, golden test, baseline, this note | **done: about 28 minutes, see section 8** |
| 1 | Clean text | 40 to 60 min |
| 2 | Structure, sections, locator contract | 1 h 30 to 2 h 30 |
| 3 | Chunker v2 above the threshold, calibration | 1 h to 1 h 30 |
| 4 | Resumable two-speed ingestion, `spawn_blocking`, degrade-never-block | 2 h to 3 h |
| 5 | Document cards | 30 to 45 min |
| 6 | Knowledge base on a book | 30 min |
| 7 | Vector store and lean ranking, measures at 5 000 / 20 000 / 100 000 | 1 h 30 to 2 h |
| 8 | Finder and profile on long documents | 1 h |
| 9 | Schema safety and spreadsheets | 1 h |
| End | One final pass, one report, `docs/DECISIONS.md` and register once, git commands | 45 min |

## 7. The six decisions

**Decided by the owner on 14 October 2026: all six as recommended** (threshold 60 000 characters; copy forward the unchanged chunks and vectors; two progress lines and a card line under 100 %; chunk size measured in lot 3 and decided on the figures; `lopdf` with our own outline walk; the postponed readers stay postponed). The text below is the reasoning that was put to her. `docs/DECISIONS.md` is updated once, at the end of the tab.

1. **The "long" threshold.** The fixtures cannot calibrate it: the 146 short documents hold at most 965 characters (one page) and the long ones are 115 pages and 334 000 characters; nothing lies between. **Recommended: long = more than 60 000 characters of extracted text (about 15 printed pages), whatever the format** (a DOCX or a TXT has no pages, so characters are the only common unit), confirmed in lot 3 on documents of 5 to 100 pages that I generate for the purpose. Below it the chunker and the cleaning are today's, byte for byte.
2. **The schema rebuild policy.** **Recommended** as in 5.1: detected by one query at launch, old tables untouched, the search says the index must be redone, and « Analyser » rebuilds inside the pass by copying forward the chunks and vectors that did not change (minutes, not hours). Alternative: rebuild from nothing (simpler, 5 hours of embedding on a big library).
3. **How "vectorisé à X %" is shown.** **Recommended:** during the pass, two lines (« Lecture des documents : 340 sur 885 », then « Recherche par sens : 62 % »); afterwards on the folder card only when below 100 % (« Recherche par sens : 62 % »), nothing at 100 %; the search says it ran on a partial index through a machine code. Alternative: a state word only, with no percentage.
4. **Chunk size and overlap above the threshold.** Today 1 200 characters, no overlap. The embedding being 99 % of an Analyse, the size is a time decision: 800 characters = about 50 % more passages (a 1 000-page PDF: about 10.5 min instead of 7 on today's tower), 1 600 = about 25 % fewer (about 5 min); at the 7 October speed these would be 74 and 37 minutes. **Recommended:** measure 800 / 1 200 / 1 600 with 0 / 12 / 20 % overlap on the planted facts with real vectors (lot 3) and keep **1 200 characters with 12 % overlap** unless a cell clearly wins; you decide on the figures.
5. **PDF outline reader.** **Recommended: `lopdf`** with our own walk of the outline tree. A throwaway test (not in the tree) read the outline of the 500- and the 2 000-page PDFs in 6.6 ms and 24 ms to load, with no library to ship, on both platforms, and it is what `pdf-extract` already uses. Two traps found: `Document::get_toc` keys entries by title (duplicate titles are lost) and decodes titles as UTF-8, which garbles the accents of a PDF-doc-encoded title, so the walk must be ours, with PDFDocEncoding and UTF-16 decoding. `pdfium-render` reads bookmarks well but needs its dynamic library per platform at extraction time.
6. **The readers postponed on 13 October** (ODT, RTF, EML, VTT, ODP, PPTX, ODS). **Recommended: they stay postponed.** The pilot's long files are PDF and DOCX, which this tab covers; each reader is its own test surface, and the folder card already says how many files of an unread format were not read (KBD-30). Revisit when a real big ODT or RTF shows up.

Inside decision 1: should the cleaning of lot 1 (headers, hyphenation) also apply to **short** documents? **Decided by the owner on 14 October 2026: no, not for now** (it would change the 146 golden documents). A later, measured proposal with the diff shown may come back to it.

## 8. Real elapsed time

Lot 0, wall clock, from the clock stamps of the session (17:53:19 to about 18:21): **about 28 minutes in all**, against the 2 h 30 I announced.

| Step | Wall clock |
| --- | --- |
| Branch check, reading the session file, the sandbox generator, the scorer, the laboratory and the design notes the session names | 17:53 to 17:57 (about 4 min) |
| Generator test written and seen failing; generator written; 17 tests green | 17:57 to 18:01 (about 4 min) |
| Both corpora generated (7 documents 16 s; 1 000 + 7 documents 22 s) | 18:01 to 18:02 |
| Peak-memory tool and per-kind scoring: tests written and seen failing, then green | 18:02 to 18:04 (about 2 min) |
| Golden test: seen failing, recorded (146 documents), shown to catch a change | 18:04 (20 s of tests, 7 s of build) |
| Fake-vector baselines: 7 documents 38.6 s, 30 documents 2.1 s, 1 000 + 7 documents 218.6 s (of which the Analyse 79.5 s) | 18:05 to 18:10 |
| Real-vector baseline, scale 0.1: 269.7 s (I had planned 30 minutes from the 0.74 s of 7 October) | 18:10 to 18:15 |
| Outline test with `lopdf` (45 s of build), Analyse-only memory peaks (about 2 min) | 18:13 to 18:19 |
| This note | written between the runs, about 10 min |

What I got wrong in the plan: the real embedding speed (0.106 s a passage, not 0.74), which made the real-vector run seven times shorter and moved the projected Analyse of the whole corpus from 5 hours to 43 minutes.

## 8 bis. Lot 1 - clean text: result (14 October 2026, about 12 minutes, 18:45 to 18:57)

**Built.** `src/cleaning.rs` (`clean`), called in `indexing.rs` between extraction and chunking; `chunking::is_long` and `LONG_DOCUMENT_CHARS = 60 000` (decision 1). Above the threshold only: ligatures expanded, no-break / narrow / thin spaces turned into a plain space, soft hyphens dropped, NFC; lines with no letter or digit dropped; headers and footers removed (a short line, at most 3 numbers, among the first or last two lines of at least half of the pages, digits compared as one symbol); a word cut at the end of a line joined, inside a page and across a page break (the word stays on the page where it starts), with a short list of French prefixes protected and nothing touched before a capital letter or a digit. A short document is returned as it came.

**Tests first, seen failing:** `tests/text_cleaning.rs`, 9 tests (5 failed against the empty stub, the 4 "keep it" tests passed by construction, then all 9 passed). `tests/chunking_golden.rs` (146 short documents) stays green.

**Measured against the baseline (fake vectors; word search is real).**

| | Baseline | After lot 1 |
| --- | --- | --- |
| 7 long documents: chunks / characters | 24 105 / 24.56 M | 23 680 / 24.17 M (**-425 passages, -1.7 %**, about 45 s of real embedding) |
| 7 long documents: chunking + cleaning | 42 ms | 412 ms |
| 1 000 + 7 documents: chunks / Analyse (fake) | 28 711 / 79.5 s | 28 286 / 76.0 s; cleaning 0.6 s |
| `hyphen_line` fact@5 / fact@10 (word search) | 0.17 / 0.83 | **1.00 / 1.00** |
| `hyphen_page` fact@10 | 0.33 | 0.50 |
| every other kind | unchanged | unchanged |
| 1 007 documents, word search R@10 / MRR / fact@10 | 0.797 / 0.804 / 0.771 | 0.797 / **0.842** / **0.800** |

`hyphen_page` improves only partly on purpose: the cut word is now whole, but its sentence still ends on the next page, in another chunk. That is the chunk boundary of lot 3, not the cleaning.

**Not done in this lot, and why.** (1) *Flag low-confidence OCR chunks*: a chunk already carries its OCR `confidence` (stored in `chunks.confidence`); a separate flag has no consumer before the card and the search codes of lots 4 and 5, so none was added. (2) *Boilerplate chunks*: only lines without a letter or digit and repeated headers/footers are dropped; a general "repeated chunk" rule needs the counts of lot 4's phase 1. (3) *A cleaning version in `should_skip`*: an already-analysed long document is not re-read for this change (no version column yet); it joins the `chunker_version` of the new layout (5.1), so the next rebuild picks it up.

## 8 ter. Lot 2 - structure and the locator: result (14 October 2026, about 45 minutes, 19:00 to 19:45)

**Built.**

- `src/structure.rs`: the sections of a document and the section of each passage. Title sources in order: the **PDF outline** (`lopdf`, our own walk of the tree with `decode_text_string`: accents kept, repeated titles kept, 8 levels, 20 000 entries at most), **DOCX heading styles** (`Heading1`, `Titre2`, `Titel3`, `Rubrik1`, or an explicit outline level), **Markdown** `#` (not inside a code fence), **heuristics** (`3.2 Title`, `Chapitre 3`, `Annexe B`, a short capital line followed by a real paragraph; a list item, a sentence and a lone capital line are not headings), and for a **long document with no heading** slices of 20 pages (or of 20 000 characters for a document read as one page). A short document with fewer than two headings has no structure. A passage belongs to the section in effect **where it starts**.
- Storage, additive tables only: `doc_structure` (version per document), `doc_sections`, `chunk_sections`; deleted with their document (replace, removal, reset). `STRUCTURE_VERSION = 1` joins `should_skip`: a document stored without it, or under an older version, is **read again, and its vectors are reused** (a passage whose id and text are unchanged keeps its stored vector, in `indexing.rs`): this is the copy-forward of decision 2, done at passage level.
- The locator end to end. `Evidence.section: u32` is now `Evidence.ordinal` (the rank inside the page) and `Evidence.section` is `Option<EvidenceSection { title, level, first_page, last_page }>`; the finder's `Locator.section` is now `ordinal` plus `section_label`; `format_evidence` tells the model `page 12, section "3.2 Title"`; « Sources » names the sections (`chat.sourceSections`, both catalogues) and so does the copied text; `ipc.ts` follows.
- The profile: `file_profile_sections` is filled (it was empty), with two new tables, `file_profile_section_entities` (from the knowledge base's mentions) and `file_profile_section_dates` (a row-by-row scan of each section's passages). `SCHEMA_VERSION` and `BUILD_VERSION` of the profile are 2: an index from before is reported by the existing `index_status` and the existing French line, and left byte-identical until Analyse (tested).

**Tests.** `tests/structure.rs` 13 (10 failed against the empty module, 3 passed by construction), `tests/structure_store.rs` 9 (all 6 first ones failed before the wiring; the two locator tests failed to compile before the types changed; the profile test and the TypeScript tests for sections were written after their code and pass, they would fail on the old code), plus the 9 of lot 1 and the golden test of the 146 short documents, all green. Nearby integration tests (profile, finder, repeated indexing, embedding batches and failures, timings, knowledge documents, end to end retrieval) green.

**Measured (fake vectors; the word search is real; same corpus as the baseline).**

| | After lot 1 | After lot 2 |
| --- | --- | --- |
| 7 long documents: Analyse / profile pass / index size | 10.2 s / 0.3 s / 76 MB | 15.6 s / 2.5 s / 78 MB |
| 1 000 + 7 documents: Analyse / profile pass / index size | 76.0 s / 7.0 s / 97 MB | 75.9 s / 9.2 s / 102 MB |
| 1 000 + 7 documents: finder, one word | 8 to 22 ms | 6 to 15 ms |
| word search R@10 / MRR / fact@10 on 193 questions | 0.797 / 0.842 / 0.800 | 0.797 / 0.842 / 0.800 (identical) |
| `section@10`: prose / list / hyphen_line | not scorable | **1.00 / 1.00 / 1.00** |
| `section@10`: list_split / table_split / hyphen_page | not scorable | 0.50 / 0.50 / 0.50 (the same questions whose fact is found at all) |
| `section@10`: heading | not scorable | **0.00** |

The Analyse of a library of 1 000 documents does not slow down; the 7 long documents cost about 2 s more (outline reading, assignment, the section scan of the profile). Reading again a document whose structure is missing costs its extraction and no embedding (tested: zero embedding request).

**Where it is not good yet, and why.** `heading` scores 0 on `section@10` although the fact is found every time: the planted heading sits inside a chunk that **starts in the previous section** (a chunk is cut by size, not at a heading), so the passage is labelled with the earlier section. The rule "the section in effect where the passage starts" is correct and cannot be improved without moving the chunk boundary: that is lot 3 (cut at headings above the threshold). `list_split`, `table_split` and `hyphen_page` stay at their baseline for the same reason (lot 3).

**Not done in this lot, and why.** (1) The finder returns a long document **once with the list of the pages or sections that matched**: that is lot 8 (`FileHit` keeps its first passage today; only its locator changed meaning). (2) A section label for `LocatorView` of the entity window (`passage {section}` there is the page ordinal and says « passage »): untouched on purpose. (3) Tables and lists marked as blocks: they are recognised by the chunker of lot 3, which is where they are used. (4) The `index_layout` / `chunk_vectors` layout of 5.1 is lot 4's; this lot's tables are additive and need no layout version of their own.

## 8 quater. Lot 3 - the chunker for long documents: result (14 October 2026, about 2 h 15 including 90 minutes of real-vector grid, 19:55 to 22:00)

**Built.**

- `src/chunking_long.rs`, used for a document above 60 000 characters (decided threshold), decided on the text as extracted, before cleaning. It works on the document as one stream of lines (a passage may run over a page break; its page is the page of its first line). A **heading starts a passage** and nothing is carried over it, but only a heading whose line was **found** in the text (`Section.located`). A **table** (rows with `|`, tabs or columns of spaces) and a **list** (bullets or numbered items, with the line ending in `:` before it) **are never cut**: they go in whole up to twice the size; a bigger one is cut between rows and every part repeats its header row. A full passage is closed at the **last sentence end** if that keeps 60 % of it, never inside a table or list. Optional overlap, optional context prefix.
- Plumbing: `Chunk.index_text` (the text embedded and put in the full-text index when it differs from the cited text), table `chunk_index_text` (the prefix only, not a copy of the text), `doc_structure.chunker` (1 or 2), `chunker_version` in `should_skip` (a document stored with more than 60 000 characters but cut by chunker 1 is read again; a short one never is), vector reuse that also compares the prefix, `chunk_ids` unchanged in form (`path#p<page>#s<n>`, n counted per starting page).
- Two defects found by the measure and fixed with a test each: (1) an outline entry whose title the page does not show was anchored to the first line of its page, which cut the table that starts there (the title's dash had been garbled by the extraction): titles are now matched up to two edits and an unlocated section is a boundary, not a cut; (2) the sentence-end rule could cut a list between its title and its items.
- The options are `LongOptions { max_chars, overlap_percent, prefix }`; the laboratory sets them for measurement (`ACAI_CHUNK_MAX`, `ACAI_CHUNK_OVERLAP`, `ACAI_CHUNK_PREFIX`), the product never does.

**Tests.** `tests/chunker_long.rs` 10 and `tests/chunker_long_store.rs` 5 (all 9 and 4 of them seen failing before the code existed); 2 more in `tests/structure.rs`; the golden test of the 146 short documents stays green (a short document is cut by the old chunker, untouched).

**The threshold (decision 1), checked.** The planted failures do not depend on length: with the old chunker they appear in the 500-page documents and would in a 30-page one, because a table can straddle a page at any length. The generator cannot build a document below about 30 pages with the eight kinds of fact, so 12-page documents were not measured. A corpus of 30 to 120 pages (480 pages, 7 documents) with the new chunker scores 1.00 on every kind. So the threshold is a compatibility choice (what stays byte-identical), not a quality one; 60 000 characters stays, as decided.

**Measured with the word search, 8 000 pages (fake vectors; the word search is real).** Every cell of the grid below scores `fact@10` 1.00 on every kind and `section@10` 1.00 (the `heading` kind, 0.00 after lot 2, is now 1.00), against 0.82 and 0.70 before. Word search cannot choose a size.

**Measured with the real model** (the tenth-scale corpus, 7 documents, 800 pages; 50 planted facts; `pipeline` is the product's own path, six passages at most; `hybrid30` and `vector30` read 30 deep; the figure is the share of facts in the first 10 passages). Baseline before lot 1 for reference: `pipeline` 0.70, `hybrid30` 0.66, `vector30` 0.52.

| Size / overlap / prefix | Passages | Embedded chars | Analyse | `pipeline` | `hybrid30` | `vector30` | `section@10` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1200 / 0 % / none | 2 549 | 2.39 M | 261 s | **1.00** | **0.98** | **0.92** | 0.96 |
| 1200 / 12 % / none | 2 735 | 2.63 M | 282 s | 1.00 | 0.96 | 0.90 | 0.96 |
| 1200 / 20 % / none | 2 993 | 2.91 M | 311 s | 1.00 | 0.92 | 0.88 | 0.96 |
| 800 / 12 % / none | 4 018 | 2.64 M | 308 s | 1.00 | 0.96 | 0.90 | 0.98 |
| 1600 / 12 % / none | 2 065 | 2.66 M | 276 s | 0.92 | 0.96 | 0.82 | 0.90 |
| 1200 / 12 % / **prefix** | 2 735 | 2.81 M | 292 s | 1.00 | 0.86 | 0.74 | 0.96 |
| 800 / 12 % / prefix | 4 018 | 2.91 M | 337 s | 1.00 | 0.92 | 0.78 | 0.98 |
| 1600 / 12 % / prefix | 2 065 | 2.79 M | 292 s | 0.88 | 0.82 | 0.64 | 0.84 |
| 1200 / 0 % / prefix | 2 549 | 2.55 M | 272 s | 1.00 | 0.86 | 0.76 | 0.96 |
| 1200 / 20 % / prefix | 2 993 | 3.11 M | 359 s | 1.00 | 0.78 | 0.70 | 0.96 |

(Each cell is one run; 50 questions, so a difference of 0.04 is two questions.)

**What the grid says.**

1. **The context prefix is not kept.** With it, `hybrid30` and `vector30` fall by 0.10 and 0.16 at 1200 / 12 %, it costs 7 % more embedded characters, and it changes nothing for the product's path or the word search. It is **off by default** (`LongOptions::default()`), the code stays behind its switch (the master plan keeps a method only if the figures say so). Probable cause, to check on real documents: the section titles repeat across documents and make passages of different documents look alike.
2. **Overlap does not pay.** With no prefix, 0 % is at least as good as 12 % and 20 % on both semantic channels and is the cheapest; 20 % is the worst and the slowest.
3. **1600 characters is too big** (`pipeline` 0.92, `vector30` 0.82). **800 characters is not better than 1200** (0.96 / 0.90 against 0.98 / 0.92) and costs **58 % more passages** (the ranking reads every passage) and 18 % more Analyse time.
4. Passages: the new chunker makes 7 % to 69 % more passages than the old one on the same text (2 378 before); at 1200 / 0 % it is 7 %.

**Decided by the owner (14 October 2026), as recommended: 1200 characters, 0 % overlap, no prefix.** It is `LongOptions::default()`.

**On the 1 007-document library and the 8 000 pages** (fake vectors, quiet machine, the prefix on in this run): Analyse 13.8 s and 82.8 s (lot 2: 15.6 s and 75.9 s), 26 974 and 31 613 passages (lot 2: 23 680 and 28 286), index 99 MB and 124 MB (lot 2: 78 and 102), peak memory 185 MB and 231 MB, finder 6 to 16 ms. The exam is unchanged or better: word search `R@10` 0.797 to 0.802, MRR 0.842 to 0.858, `fact@10` 0.800 to 0.943 (the three kinds of fact that failed now pass). `page@10` stays at 0.00 for the split kinds **by construction**: the page of a passage is the page where it starts; the fact is on the next one. `section@10` is the right measure.

**Not done in this lot.** (1) The prefix keeps the title from the file name only: type, author and date come from the profile, which is built after the passages (the pass order changes in lot 4); given point 1 it was not worth reordering. (2) The index size with the prefix off was not re-measured on the big libraries. (3) The sentence-end rule and the overlap code stay, measured and unused by default.

**A finding outside the plan.** Two `grep` processes, started on 8 October and never ended, were using two processor cores. I stopped them. Every timing of this tab before 20:55 was taken with them running; they do not change a ratio, and the quiet-machine figures above are the ones to read.

## 8 quinquies. Lot 4 - ingestion in two speeds, stoppable and resumable: result (14 October 2026, about 1 h 40, 22:10 to 23:05)

**Built.**

- **A passage may be stored without its vector.** In an index made by this version `chunks.embedding` is `BLOB` without `NOT NULL`: an absent vector is `NULL`, never an empty blob. An index from before has the column `NOT NULL` and is **left byte-identical when opened** (tested, with its layout, vectors and word index); the Analyse the user starts upgrades it in one transaction (`upgrade_layout`), copying every passage and vector as it is. A partial index (`chunks_pending`) finds the passages that wait.
- **The pass has two speeds** (`indexing::run_cancellable`, `run_with_knowledge` is the same with no stop). **Phase 1**, file by file: read, clean, cut, then the text, the word index, the structure and the names are written in one transaction, with the stored vector for a passage whose id and text are unchanged and none for the others. When every file is done, the names of documents analysed earlier are read and the profiles are built: the word search, the finder and the entity base work on everything. **Phase 2**, batch by batch (16 passages, 20 000 characters, in reading order): each batch is stored in its own transaction. Nothing is embedded before every file is written (tested with the gateway looking at the index at its first request).
- **Stoppable at any moment** between two files, two batches or two scanned pages (`cancelled`), and **resumable**: a stopped pass keeps everything; the next one reads no file again and computes only the missing vectors (tested: after a stop at the third batch, every passage is embedded exactly once across the two passes, none twice). `cancel_analysis` (Tauri command) raises the stop; the folder card has a « Arrêter » button while a pass runs.
- **Degrade, never block.** A file that cannot be read is named (`extraction_failed`) and the pass goes on (it used to abort the whole pass). When a batch fails, each of its files is tried alone and only the file the server keeps refusing is set aside: its text stays, searchable by words, and the next pass retries its vectors (the old rule that a failed file "leaves no trace" is gone on purpose; the tests that said so were rewritten, see below). Three files refused in a row, or a server that is gone, still stop the pass with the gateway's own error, everything kept.
- **A long scan is read page by page and kept page by page** (`src/ocr_resume.rs`): each recognised page goes to `ocr_page_cache` (keyed by the file and its hash, written ten at a time), a stop is honoured before the next page is rendered, and the next pass takes the kept pages instead of rendering and recognising them again; the kept pages are dropped when the document is stored or the file changes. Tested with a six-page scan stopped after three pages: three more recognitions, in order, each page once.
- **Progress and partial search.** `IndexProgress` carries `vectorsDone` / `vectorsTotal` (phase 2); the bar follows the files, then the vectors; the label reads « Lecture des documents » then « Recherche par sens : x % »; after a stopped or partial pass the summary says so; the card shows « Recherche par sens : x % » only while below 100 % (`vector_share`, one count). `SearchOutcome.unvectorised_chunks` is the machine code `partial_vectors` (tab 3 writes its sentence); a passage without a vector is found by its words only (tested). Catalogues in French and English.
- Smaller: the identifier scan of the profile stops at its cap instead of reading the rest of a book; the dead embedding helpers were replaced by one tested batch rule.

**Tests.** Written first and seen failing: `two_speed_store` (4, compile errors), `ocr_resume` (3), the TypeScript progress tests (3). Written together with or after the code, passing: `two_speed_pass` (7; they would not compile on the old code, which had no `run_cancellable`). **Rewritten on purpose** because they described the old contract: `embedding_failures` (7 of 15: a failed file left no trace, one request per file, per-file progress), `analysis_timings` (1), `knowledge_documents` (1: a file whose vector the server refuses now keeps its names).

**Measured.** Real model, tenth-scale corpus (800 pages, 2 549 passages) at the chosen chunker defaults:

| | Before lot 1 | Now |
| --- | --- | --- |
| Analyse, real model | 253 s | 259 s |
| **Word search, finder and names usable after** | 253 s (everything at the end) | **2.3 s** |
| Planted facts in the first 10 passages: product path / mixed / vector | 0.70 / 0.66 / 0.52 | **1.00 / 0.98 / 0.92** |
| Peak memory, whole run | 49 MB | 51 MB |

8 000 pages, fake vectors: Analyse 22.7 s (lot 3: 13.8 s; the rest is the phase-2 bookkeeping, 0.3 ms a passage, invisible next to 106 ms a passage of real embedding), Analyse-only peak memory 95 MB (lot 1 baseline: 94 MB), 24 941 passages; 1 007 documents: Analyse 92 s (83 s), peak 214 MB, finder 6 to 18 ms. Embedding batches go from one-or-more per file to full batches across files (1 559 instead of 1 690 for 8 000 pages).

**What the stop and the memory do and do not cover.** A stop is honoured between files, batches and scanned pages, not inside the extraction of one text file (seconds). A text PDF is still extracted whole, page texts included, then dropped once its passages are written (about 4 MB of text per 1 000 pages: the measured peak, 95 MB for 8 000 pages, is the bound). Only a scan is page-by-page. The extraction runs through `block_in_place` on the application's multi-threaded runtime (borrowed ports cannot move into `spawn_blocking`) and inline on a current-thread test runtime. The interface was type-checked and unit-tested but **not seen running**.

**Left for later.** A per-file « vectorisé à x % » (the count exists, `vector_counts_of`; only the global share is shown); `slowestFiles[].embedMs` is 0 (the vectors are not computed per file any more); the Data Folder pass is unchanged.

## 9. Reproducing the baseline

```text
# once
python scripts/gen-long-documents.py --out data/long-documents
python scripts/gen-long-documents.py --base data/knowledge-sandbox-1000 --out data/long-library-1000
cd apps/desktop/src-tauri && cargo test --release --test retrieval_lab --no-run
# one run (fake vectors; add ACAI_GATEWAY=... and drop ACAI_FAKE for the real model), with the peak memory
ACAI_SANDBOX_DIR=<abs path to the root> ACAI_FAKE=1 ACAI_TIER=<documents> ACAI_LAB_LABEL=<name> \
  python ../../../scripts/peak_memory.py -- <abs path of target/release/deps/retrieval_lab-*.exe> --ignored --nocapture
python scripts/retrieval_lab_score.py data/retrieval-lab/<name> --questions <root>/questions.json --planted
```

## 10. Lots 5 to 9: results (14 and 15 October 2026)

Built in one run while the owner slept; the single final pass is section 11. Elapsed times are wall clock from the session's stamps: lot 5 about 20 minutes (23:10 to 23:30), lot 6 about 10 (23:30 to 23:40), lot 7 about 20 (23:30 to 23:50, overlapping lot 6), lot 8 about 10 (23:40 to 23:50), lot 9 about 6 (23:44 to 23:50), against 4 h 30 to 6 h of estimates in section 6.

### 10.1 Lot 5 - document cards

- **What it is.** One digest per document, built with no model after the profiles (`cards.rs`, `CARD_VERSION = 1`). Fixed English labels (`Title`, `Type`, `Date`, `Roles`, `Size`, `Start`, `Outline`); the type, the date and the roles come from the profile, the outline from the sections, the start from the first sentences. It lives in its **own table** (`document_cards`, with its own word index `document_cards_fts` and its own vector): it is **never a passage**, so no retrieval path can cite it as one (`a_card_is_never_a_passage`).
- **Order.** Cards are built after the knowledge-base backfill and the profiles, and embedded **first** in phase 2 (`pending_chunks` returns them before the passages), so « plan » and « résume ce document » work as soon as the first vectors exist. They count in `vector_counts` (a 7-document library has 7 more units than passages).
- **A card follows its text, not its re-reading.** A document written again keeps its card until the card text changes (`documents_needing_a_card`), and `replace_card` keeps the stored vector when the new text equals the old one: a document read again for another reason is not embedded again. A document with no text has no card.
- **Tests.** `document_cards` (6): title, outline and first sentences; what the profile knows; a document without sections; never a passage; searchable by words and embedded with the rest; follows its document and is not rebuilt for nothing. Seen failing before the code. `embedding_batches` now expects the card (CHUNKS + 1 inputs).

### 10.2 Lot 6 - the knowledge base on a book

- **Caps grow with the passages** (`caps_for` in `knowledge/extract/text.rs`): half as many names and five times as many mentions as passages, floors 500 names and 5 000 mentions (today's constants, so a short document is unchanged), ceilings 20 000 and 100 000. A source over its cap is stored in `kb_source_truncated`, `SourceMentions.truncated` reaches the entity sheet, which says so in both catalogues. The profile's identifier scan stops at its cap and feeds the finder's `Coverage.partial`.
- **Tests.** `knowledge_book` (3): the caps grow and never fall below the old floor; a source over its cap says so in the sheet; the same names in a book of many passages are all kept. One lib test was **rewritten on purpose** (`a_source_with_many_mentions_in_few_passages_is_capped_by_mentions_not_entities`): the cap now follows the passages, so a single repeated name on one page no longer hits it.

### 10.3 Lot 7 - vector store and lean ranking

- **Ranking.** `search_scoped_with_hits` streams the vectors of the scope one row at a time (`for_each_vector`), keeps the best, and fetches the text of the few kept passages afterwards (`passages_by_id`). Same scores, same order as before: `ranking_parity` compares 360 results of the lean ranking with a copy of the old algorithm over every scope (and was **mutation-checked**: a changed weight fails it). A passage without a vector is ranked by its words alone, as before.
- **Measured** (release, synthetic chunks of 768 numbers, `ranking_scale_bench`, run at lot 7; the ranking code has not changed since):

| Passages | Before | After |
| --- | --- | --- |
| 5 000 | 52 ms, 32 MB | **36 ms, 8 MB** |
| 20 000 | 190 ms, 96 MB | **144 ms, 9 MB** |
| 100 000 | 970 ms, 445 MB | **755 ms, 9 MB** |

- **The model is recorded** with its dimension (`file_profile_meta`: `embedding_model`, `embedding_dimension`). A pass with another model recomputes every vector (cards included) in the pass the user starts, once; vectors of another length under the same model name are detected the same way. A search whose question has another length than the stored vectors returns `mismatched_chunks` and does not compare them; the word half still answers. `SearchOutcome` also carries `unvectorised_chunks` (the partial index).
- **Not applied, on purpose.** The restriction of vector scoring to other channels' candidates and int8 vectors: not needed at 100 000 passages (755 ms, 9 MB), and each would change the ranking. Both stay available for tab 3.
- **Tests.** `ranking_parity` (2), `vector_store` (3): the model and dimension are recorded; another model recomputes everything once; a question of another dimension is reported.

### 10.4 Lot 8 - the finder and the profile on long documents

- A long document is **one hit** that carries its places: `FileHit.locators` (at most `MAX_LOCATORS = 20`, each `{ page, ordinal, sectionLabel }`) and `locatorsTotal`; `DocumentListing.pages` and `matchesTotal` for the set answer, which lists the pages (« pages 41, 42, 57 ») instead of repeating the file. The profile schema is version 2: per-section entities and dates in two tables, `dates_in_text` for the header facts, `index_status` reports the new layout.
- **Tests.** `finder_long` (2): one hit with every place where the word matched; the places are capped and the total still counts them all.
- **Measured** (1 007 documents, 885 read, fake vectors): the finder answers one word in **8 to 30 ms** (baseline 9 to 19 ms: the places cost a few ms on a word found in 84 files, still inside the 50 ms target) and « all files » in 7.8 ms (6.4). `file_finder_bench`: at 5 000 files the worst probe is 33 ms (target 50), at 20 000 files 130 ms (target 200).

### 10.5 Lot 9 - schema safety and the spreadsheets

- **Tests.** `schema_safety` (4): an old index is recognised by one lookup and every table of it is left alone; the spreadsheet tables (`tabular_inventories`, `tabular_workbooks`, `kb_column_semantics`) are identical after every kind of rebuild; the `data` questions of the exam are answered the same by a rebuilt index; nothing heavy runs when the index is opened or after a pass. `chunking_golden` (1): the chunks of the 146 short documents are byte-identical to the recorded ones. `src/tabular/*` was not edited.
- **Not done.** A mutation check of the spreadsheet protection (change a spreadsheet table on purpose and see the test fail) was not run; the tests assert identity, which a mutation would break, but this was not shown.

## 11. The final pass (15 October 2026)

Run once, after the last edit; wall clock from the session's stamps.

| Check | Result | Time |
| --- | --- | --- |
| `cargo test --no-fail-fast` (debug) | **1 537 passed, 0 failed, 49 ignored** (the measures) | 3 min 52 s |
| `cargo clippy --release --all-targets` | no warning in code written in this tab; the others sit in files not touched here | 32 s |
| `pnpm run build` (type check and bundle) | passes (the usual chunk-size notice) | 6 s |
| `pnpm run test` | **694 passed**, 41 files (includes the language guard on `src-tauri/src`) | 3 s |
| Release build of the three measures | | 1 min 45 s |

Five tests failed at the first full run. Three described the old contract and were updated on purpose (the lib test on the mention cap, `embedding_batches` now counting the card, one `knowledge_store` test about a file the server refuses). Two exposed real defects, both fixed: a pass without a knowledge context did not register the source of a stored document, and a card was deleted when its document was written again. `rustfmt` was applied to the files this tab wrote; `index_store.rs` and `commands.rs` were formatted whole (their diff is larger than the change); `chunking.rs`, `manage.rs`, `mixed_answer.rs` and `ocr/mod.rs` were not clean before this tab and were left as they were.

**Measures, final** (release, temporary copies, knowledge base on, fake OCR):

| Run | Analyse | Peak memory (Analyse) | Passages | Index | Finder, one word | Baseline (section 4) |
| --- | --- | --- | --- | --- | --- | --- |
| 7 long documents, 8 000 pages, fake vectors | **28.5 s** | **97 MB** | 24 941 | 82 MB | 0.2 to 10.9 ms | 14.0 s, 94 MB, 24 105 passages |
| 1 000 + 7 documents, fake vectors | **90.1 s** | **121 MB** | 29 508 | 106 MB | 11 to 30 ms | 79.5 s, 117 MB, 28 711 |
| 800 pages, **real model** (`assistant-embed`, LAN tower) | **278 s**, of which 276 s embedding | 37 MB (whole lab) | 2 549 | 18.7 MB | n/a | 253 s, 49 MB |

The Analyse with a faked model is slower than the baseline (28.5 s against 14.0 s for 8 000 pages) because it now also cleans, finds the structure, cuts at blocks, builds the cards and bookkeeps two phases; the first run of lot 4 gave 22.7 s and the spread of these runs is 8 to 25 %. With the real model, 161 batches of 16 at 1.7 s: **0.108 s a passage**, 99 % of the Analyse, so the extra work is invisible. The projection of section 4.1 stands: about **45 minutes for the 8 000 pages** on today's tower.

**Planted facts, real vectors, 800 pages, 50 questions** (share of questions whose fact is in the first 10 passages; baseline in brackets):

| Channel | fact@10 |
| --- | --- |
| `pipeline` (what the product sends, six passages at most) | **1.00** (0.70) |
| `hybrid30` | **0.98** (0.66) |
| `vector30` | **0.92** (0.52) |
| `lexical30` | **1.00** (0.80) |

`list_split`, `table_split` and `hyphen_page` are now 1.00 on the product path (0.33 to 0.50 before): a table or a list is never cut, and a word cut across a page is joined. The recall of the right file at 10 is 1.00 on `pipeline` and `lexical30`, 0.98 and 0.96 on the other two. `section@10` is **1.00** on all kinds except `prose` (0.86). The 4 questions the documents cannot answer still receive **6.0 passages** on `pipeline` (5.8 before): nothing in the retrieval says « no », which is tab 3's lot E.

## 12. What tab 3 can rely on (and what it must not assume)

**Can rely on**

1. **The locator.** `Evidence.ordinal` (rank of the passage in its page) and `Evidence.section: Option<EvidenceSection>` (title and pages), mapped to « Sources » in `ipc.ts`; the finder's `Locator { page, ordinal, sectionLabel }` with `locators` (at most 20) and `locatorsTotal`. A passage belongs to the section in effect where it **starts**.
2. **Sections.** `doc_structure`, `doc_sections`, `chunk_sections` (store methods in `index_store.rs`, producers in `structure.rs`: PDF outline, DOCX styles, Markdown, heuristics, slices). `Section.located` says whether a title was found on its page; an unlocated section is a boundary, never a cut.
3. **Chunks above the threshold.** 1 200 characters, no overlap, tables and lists never cut (header repeated), cut at headings and at sentence ends. `LongOptions { max_chars, overlap_percent, prefix }` is the switch struct; `chunk_index_text` holds the prefixed text when `prefix` is on. **The prefix exists and is off** (KBD-60: it lowered the meaning-based channels on the planted facts); do not assume `index_text` differs from `text`.
4. **Cards.** `document_cards` and `document_cards_fts`, embedded; stage T0 of the summaries. Never cite one as a passage.
5. **Vector states.** `chunks.embedding` is nullable (NULL = pending). `vector_counts()` gives `{ done, total }` including cards, `vector_counts_of(path)` per file, `SearchOutcome.unvectorised_chunks` and `mismatched_chunks`. `IndexProgress` carries `vectors_done` and `vectors_total`; the front end has `useVectorShare` and the Tauri commands `cancel_analysis` and `vector_share`.
6. **The lean ranking.** `for_each_vector` and `passages_by_id` are the primitives; the parity test is the guard for any change of weights. A fusion (RRF) or a rerank slots in after the candidate list.
7. **Caps.** `RetrievalLimits` is untouched (six passages); the knowledge-base caps are proportional and report `truncated`.
8. **Fixtures and baseline.** `scripts/gen-long-documents.py` (corpus and ground truth, `--scale`, `--base`), `scripts/peak_memory.py`, `scripts/retrieval_lab_score.py --planted`, the 8 000-page corpus, the 1 007-document library, and the figures of sections 4 and 11.

**Must not assume**

- `Evidence.matches`, `partial` on the chat's coverage and a « used / total » count: **not built** (lot A).
- The passage's `page` is the page where the passage starts: for a fact that sits after a page break inside a split table, list or hyphenated word, the product reports the page of the header (`page@10` is 0.00 for those three kinds, `section@10` is 1.00). A page range on the locator would fix it.
- A refusal: none exists (6.0 passages for an absent fact).
- Per-file vector share, `slowestFiles[].embedMs` (always 0), and a stop inside the extraction of one text file (seconds) are not available (KBD-64, KBD-65).
- The stop button, the two progress lines and the card line « Recherche par sens » were type-checked and unit-tested but **never seen running** (KBD-62).
- The six set sentences were not re-scored by the scorer on the 1 007 library after lot 8; the `set_search_lab` and `set_search` tests pass in the full run.
- A text PDF is extracted whole in memory (about 4 MB per 1 000 pages): the measured peak is the bound.
- Short documents are not cleaned (owner decision); headers, footers and cut words in documents under 60 000 characters stay as they are (KBD-63).
