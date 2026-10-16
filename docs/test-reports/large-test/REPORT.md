# Report: the eight readers and the large reproducible test

16 October 2026, branch `feat/new-readers` (from `61c30a5`), nothing committed. Written for the owner and for the agent who
continues. English; the library and the questions are French.

## 1. Part A: the readers

### The map

Everything that decides what a file is and what is read, on this state:

| Place | What it decides | Change |
| --- | --- | --- |
| `extraction::extract` | which reader opens an extension | falls through to `readers::read` for anything it does not handle itself |
| `discovery::SUPPORTED_EXTENSIONS` | what the document pass discovers | `vtt srt rtf eml odt odp pptx` added; a test requires every extension of `readers::EXTENSIONS` to be there |
| `file_record::FileKind::from_extension`, `mime_type_for` | the kind of a file, the folder card's unread count (`FileKind::Unsupported`) | `vtt srt rtf eml` = `document_text`; `odt odp pptx` = `document_office`; `ods` = `tabular_candidate` |
| `tabular/mod.rs` (two matches), `tabular/inventory.rs` (`TabularFormat`), `data_folder::format_of` | the workbook formats | `ods` through the XLSX adapter; `TabularFormat::Ods` |
| `indexing::unread_formats` | the line « n fichiers n'ont pas été lu » | unchanged; it keys on `FileKind::Unsupported`, so the new formats stop being counted by themselves |
| `finder` (`Domain::Data`), `data_folder` (card state) | which files are workbooks | unchanged; they key on `FileKind::TabularCandidate`, so `.ods` is a workbook |
| `filename_sanitizer` | the clean-name rule | unchanged; it renames the files of the new formats like any document |
| `resources/work-folder-questions/*.json` `extensions` | the nouns of « Combien de PDF ? » | **not changed** (see section 5, finding 1) |

A new file of each format is `analysed` on the folder card (checked by `tests/readers.rs`: indexed, not unreadable, not unread, the
card counts it).

### The readers

`src/readers/{mod,cues,vtt,srt,rtf,eml,odf,pptx}.rs`. VTT and SRT share one cue parser (`cues`). Pages as in the brief: one page,
except ODP and PPTX (one page per slide; PPTX in the order of `presentation.xml`, numbered order as the fallback). Rebuilt from the
reference version of `d3e8eec`, read line by line; added over it: RTF `\bin` binary data, surrogate pairs and multi-byte `\'hh`
runs; EML "no header field = empty", numeric and common named HTML entities; ODT comments and tracked deletions left out; PPTX
`mc:Fallback` not read twice; a 64 MiB cap per zip part. **Dependencies added: none** (`zip`, `quick-xml`, `encoding_rs`,
`calamine` were already there).

### Tests

- Per reader, unit tests in the module (cues 4, vtt 4, srt 5, rtf 9, eml 9, odf 7, pptx 5, mod 3): **seen failing first** against
  a stub for every reader except the cases `a_file_with_no_cue_is_empty` (vtt, srt, passed with the stub) and `an_xml_reference_gives_its_character`
  (written with its function).
- `tests/readers.rs` (10): a real Analyse per format (indexed, searchable by a word of its text, the pages as in the table, not unread, the card
  says analysed, the file byte for byte as it was), a broken file named and not stopping the others, an empty file reported empty, ODS through the
  Data pass. **Seen failing first:** rtf, eml, odt, odp, pptx, ods, broken, empty. **Not seen failing:** vtt and srt (the registration
  was wired before their integration test).
- Existing tests that named these extensions (all adapted by replacing the example with `.pages .wps .msg .key`, not weakened):
  `filename_sanitizer` (lib), `tests/file_finder.rs` (2), `tests/knowledge_fix_swiss_first.rs` (F-20), `apps/desktop/src/lib/analysis.test.ts`,
  `unreadFormats.test.ts`.
- **Sandbox truth, before and after.** The generated files of `fixtures/knowledge-sandbox` are unchanged (their `readable` flags describe the
  older generator); `tests/common/validation.rs::Truth::load` now marks the files of the new formats readable. On the 200-document sandbox: unread
  files **21 → 1** (the old binary `.ppt`); tier 200 scans 196 files, 168 indexed. The six set sentences: expected **111 → 117**, found **117 of 117**
  (before: at least 105 of 111), wrong files **0 → 0**. Lot 11's published figures are history and were not rewritten.

### What the folder card now says

On the library of the large test: « 1 fichier n'a pas été lu : son format n'est pas pris en charge (1 .ppt) » and nothing else unread.

## 2. Part B: the large test

Protocol: `README.md` of this folder. Features: `FEATURES.md` (generated from the book; every one of the 50 registered features has questions (three of them, the file formats of the spreadsheets, through the format of the questions);
no feature of the set-aside branches is listed). Owner's script: `HUMAN-TEST.md` and `HUMAN-TEST-results.md` (39 questions, every expected value copied from data).

### The library (`python scripts/large_test/cli.py generate`, 15 s, 21 MB, seed 20261016)

| | Count |
| --- | --- |
| Documents | **1 251**: 1 153 readable text documents; 94 scans and pictures (scan-pdf 53, jpg 27, png 14); 4 files no reader opens (1 `.ppt`, 3 spreadsheets left among the documents) |
| By format | pdf 374, docx 196, eml 104, txt 97, odt 78, rtf 65, vtt 53, pptx 53, srt 52, scan-pdf 53, md 41, odp 40, jpg 27, png 14, csv 1, xlsx 2, ppt 1 (every new format ≥ 40; floor was 30) |
| Spreadsheets | **510**: xlsx 204, csv 153 (UTF-8 and Windows-1252, French and Swiss number styles), ods 153; 11 families |
| Traps | title on row 3: 73 · merged title: 30 · SUM formula: 31 · empty column: 46 · free-text second sheet: 51 · dates as text: 193 · Swiss numbers: 51 |
| Slice (versioned) | `fixtures/large-test-slice/`: 101 documents, 41 workbooks, 31 questions, 1.9 MB |

### The book (300 questions)

150 on documents, 100 on spreadsheets, 50 mixed. New formats: vtt 6, srt 6, rtf 7, eml 8, odt 12 (incl. person questions), odp 6, pptx 6 (a question on slide 2 or later in
every deck family); ODS: 36 spreadsheet questions (floor 15). Every question carries its feature, files, expected answer, expected road and what is scored.
`book.csv` and `book.md` are the readable renderings (in `data/large-test/library/`).

### Measured times

| Step | Time |
| --- | --- |
| Generate the library | 15 s |
| Analyse of the whole library on the tower (release build, `assistant-embed`) | **377 s** in all: documents 345 s (text recognition 76 s, embeddings 202 s, writing 20 s), spreadsheets 32 s. 8 139 passages, 9 383 vectors (cards included) |
| Record the roads of 300 questions (no answering model) | 133 s |
| Run the book on `ministral-3:3b` | about 11 minutes of questions (median 1.2 s, 95th percentile 2.7 s) |
| Run the book on `gemma4:e4b` | median 3.9 s, 95th percentile 8.8 s |

Analyse summary lines: documents 1 247 scanned, 1 244 analysed, 3 empty (scans the engine could not read), 91 read by text recognition, 2 too faint to index, 0 failed, 0
renamed, `.ppt` ×1 unread; spreadsheets 510 scanned, 499 usable, **11 unusable** (see finding 8).

### The roads checked (section 5 of the README)

`routes` recorded the road and the passages of all 300 questions on the full library: **273 of 300 match the book and its deterministic checks**; the 27 that do not
are all findings of the application, none an error of the book: 2 (a spaced social-security number is not read as an identifier), 9 (the deck slide that holds the fact is not among the passages),
7 (an invoice number, an order code or a phrase did not bring its file), 9 (the letter of another person was brought: follow-ups, look-alikes, one scan); the list is in `results/routes-report.md`. Nothing was adjusted to fit; four defects of the first draft of the book were corrected before this count
(see `scripts/large_test/book*.py`: the identifier sentence shape, counts with a range because the scans may not be read, the workbook half of the mixed list, unusable workbooks).

### Results by model (same library, same book, same index, one asking each)

| | ministral-3:3b | gemma4:e4b |
| --- | --- | --- |
| Passed | **248 / 300** | **253 / 300** |
| Documents / spreadsheets / mixed | 120 / 97 / 31 | 119 / 100 / 34 |
| Refusals correct | 6 of 8 | 8 of 8 |
| Leaks (a forbidden fact in an answer) | 3 | 2 |
| Wrong files in a listing | 0 | 0 |
| Median / 95th percentile per question | 1.2 s / 2.7 s | 3.9 s / 8.8 s |
| Answers with a figure in no source | 5 | 1 |

Embedding alias `assistant-embed` and temperature 0.1 for both; commit `61c30a5` with a dirty tree; machine `eliseberthier`; the AI machine at `192.168.1.83:8080`.
Features, formats and failures are in `results/compare.md` and the two `report.md`. Answers are not deterministic: each question was asked once, a second run can differ;
the figures are labelled with the model. 149 questions are answered with no answering model (the folder, the word search, the engines, the plans); 134 pass with ministral and 137 with gemma: the difference is the model-proposed plans (T05, T21).

**What cannot be scored automatically:** the prose of an answer, the reading of a scan, the organisation letters (`needs a reading`, 6 questions, `reading-sheet.csv`).
The scorer passes any number equal to the expected one even when the answer prints others.

## 3. Defects of the application found by the test (none fixed: out of scope)

Model-independent (the same in every column), in the order of their weight:

1. **The mixed tier does not compare a document with a table** (M02: 1 of 13 for both models): asked whether the invoice total in the document is the same as in the workbook,
   the answer lists the workbook's columns and never the row. The cause was not investigated (the sentence « dans le document » is a document reference that may tie to no entity of a row).
2. **A deck's fact is often not among the passages**: for the questions whose answer is on slide 3 or 4, the passages cited are slides 1 and 2 (D21 1 of 6, D22 2 of 6). The module code
   is on slides 1 and 2, the fact on 3 or 4, and the per-file passage cap leaves it out.
3. **An invoice number is not found among 90 invoices** (FAC-2026-118, -119: the pinned-first identifier rule is not built); a transcript's order code once (D-033).
4. **Letters of people who share a surname** are mixed up by the retrieval (D02: 5 of 10 and 4 of 10), and the small model gives a look-alike's date to two refusal questions (the larger one does not).
5. **The follow-up question** brings the wrong person's letter 4 times of 6.
6. **A number typed with spaces** (`1 04 11 38 296 167 42`) is not read as an identifier by the set sentence; the compact form is.
7. **A date range written in words** (« du 2 janvier au 16 janvier ») is read as the whole month, silently; written `02/01/2026` or `2026-01-02` it is exact.
8. **A table ending with a SUM formula among fewer than 19 rows is unusable** (formulas over 5 % of the rows, the documented rule): 11 of the 31 formula files are red. The 20-row ones are refused column by column as designed.
9. **The router does not know the new extensions** (finding 1 of section 5).
10. **« Combien de documents ai-je ? »** answers 1 244 where 1 247 are readable: the three scans the engine could not read are not counted (correct, but the book accepts a range).

Infrastructure: the AI machine's Docker stack stopped answering twice during the work (health returned nothing, `docker ps` over SSH hung); the runner now resumes (`--resume`) keeping the answers
given and dropping the ones that timed out. I did not restart the stack (agreed scope: warn the owner first).

## 4. What was not walked

The window cannot be driven from here. Walked on temporary copies: every command-line step (`generate`, `analyse`, `routes`, `run`, `score`, `compare`, and the `robocopy` forms of the backup,
the copy and the restore, which return 1 = copied and bring the original settings back). **Not walked:** the clicks of Parts A, B, C, D and F of `HUMAN-TEST.md` (they name the labels of the
French catalogue, taken from `fr-FR.json`). The expected values of the questions are the ones the runner measured; the label of a button you cannot find is a defect of the script, say so. I used PowerShell once, for that
`robocopy` walk, against the repo rule; the commands given to you are `cmd`.

## 5. Findings for the owner (decisions)

1. **The folder questions do not know the seven extensions** (`resources/work-folder-questions/{fr-FR,en-US}.json`, `extensions`): « Combien de fichiers vtt ? » returns the total of analysed documents.
   One data line per extension fixes it; I did not make it because the brief forbids changing the router without reporting first. Say yes and it is a five-minute change with a test.
2. `Truth::load` recomputes the sandbox truth in the tests instead of regenerating the versioned sandbox (which would move published figures).
3. `tabular::question::{has_writing_intent, has_distributive_word}` are now `pub` (the runner replays the mixed tier's dispatch from outside the crate).

## 6. Limits

An RTF table is read as tab-separated cells; an e-mail attachment is not read; a password-protected or damaged archive is a parse failure named in the summary; UTF-16 subtitles are not read; charts and
diagrams of a presentation are not read; model answers vary between runs; the scans are drawn by the generator and read by the local engine. The knowledge-base chips need a live index and are checked by hand.

## 7. The single final pass (after the last edit)

| Check | Result |
| --- | --- |
| `cargo test --no-fail-fast` | 79 targets, **1 594 passed, 0 failed**, 52 ignored |
| `cargo clippy --all-targets` | no warning in a file I created; the warnings listed are in existing files |
| `rustfmt --check` on the files I created | clean |
| `pnpm run build` | built |
| `pnpm run test` | 41 files, **702 passed** |
| Python | 54 tests (the generators, the guard of the old sandbox's bytes, the book, the scorer) passed; the 23 of the guard and the book re-run after the last edit |
| Generate the full library, Analyse on the tower, roads of 300, two scored runs, compare | done once each, section 2 |
| The owner's script | generated from the book; command lines walked, clicks not (section 4) |

## 8. Git

The branch exists. Suggested, in this order (docs are versioned; `docs/SESSION-*.md` stays local):

```text
git add apps/desktop/src-tauri/src apps/desktop/src-tauri/tests/readers.rs apps/desktop/src-tauri/tests/file_finder.rs apps/desktop/src-tauri/tests/knowledge_fix_swiss_first.rs apps/desktop/src-tauri/tests/set_search.rs apps/desktop/src-tauri/tests/common/validation.rs apps/desktop/src/lib/analysis.ts apps/desktop/src/lib/analysis.test.ts apps/desktop/src/lib/unreadFormats.test.ts
git commit -m "feat: read VTT, SRT, RTF, EML, ODT, ODP, PPTX and ODS files"
git add scripts fixtures/large-test-slice apps/desktop/src-tauri/tests/large_test.rs
git commit -m "test: add the large reproducible test with its generator, book, runner and scorer"
git add docs/DECISIONS.md docs/INDEXING-RETRIEVAL-INVENTORY.md docs/test-reports/large-test
git commit -m "docs: record the readers, the large test, its protocol, its results and its findings"
```
