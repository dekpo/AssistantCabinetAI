# The large reproducible test

16 October 2026. Written in English for the people and the agents who run it; the library, the questions and the
application's answers are in French. Every name, number and file is invented; no real document is involved.

What it is: a generated library of **1 251 documents** (1 153 readable text documents, 94 scans and pictures, 4 files no
reader opens) and **510 spreadsheets**, a **book of 300 questions** (150 on the documents, 100 on the spreadsheets, 50
that need both), a **runner** that puts every question through the application's own code on any model the AI machine
serves, and a **scorer** that decides by itself everything that can be decided. The same library, the same questions
and the same scoring serve every model; one result folder per model.

It shows the application as it stands on the restart state of 16 October 2026 (commit `fe15e03` and the eight readers
built on it). `FEATURES.md` lists every feature, the questions that demonstrate it and how each is judged.

## 1. What you need

| Need | How |
| --- | --- |
| Python 3.11 or later, with Pillow | `pip install pillow` (the scans and pictures are drawn with it) |
| Rust, for the runner | `cargo` in `apps/desktop/src-tauri` (release build, a few minutes the first time) |
| The AI machine | the gateway of the stack; `ACAI_GATEWAY` or `--gateway`. `GET /health` lists the model aliases it serves |
| The text recognition engine | staged in `apps/desktop/src-tauri/binaries` and `resources` (`scripts/fetch-ocr-resources.ps1`); without it the 94 scans are empty and their questions cannot pass |
| Disk | about 21 MB for the library, 10 MB to 40 MB for the index; both under `data/large-test/`, never versioned |

The runner never touches the application's own data: the index is copied to a temporary folder for each run and the
library is only read.

## 2. The commands

Every step is one command, run from the repository root. `python scripts/large_test/cli.py <step>` wraps the
`cargo test` lines shown in `apps/desktop/src-tauri/tests/large_test.rs` and writes the description of the run
(commit, model, embedding alias, machine, date, hash of the book) into the result folder.

| Step | Command | What it writes |
| --- | --- | --- |
| 1. Generate | `python scripts/large_test/cli.py generate` | `data/large-test/library/` : `documents/`, `data/`, `documents.json`, `workbooks.json`, `entities.json`, `questions.json`, `book.csv`, `book.md`, `manifest.json` |
| 2. Analyse | `python scripts/large_test/cli.py analyse --embed-alias assistant-embed` | the index in `data/large-test/index/<embedding alias>/` and `data/large-test/analyse/<alias>/analyse.json` (summary lines, counts, time) |
| 3. Roads | `python scripts/large_test/cli.py routes` | `routes.jsonl`: the road each question takes and the passages found, with no answering model; `routes-scores.jsonl`, `routes-report.md` |
| 4. Run a model | `python scripts/large_test/cli.py run --model ministral-3:3b` | `data/large-test/<date>-<model>/` : `answers.jsonl`, `scores.jsonl`, `summary.json`, `report.md`, `reading-sheet.csv`, `run.json` |
| 5. Compare | `python scripts/large_test/cli.py compare data/large-test/<a> data/large-test/<b>` | the models side by side, the same rows for each |
| Score again | `python scripts/large_test/cli.py score --results data/large-test/<folder>` | rewrites the score of a run already made |

Options: `--only D-001,D-002` (a few questions), `--limit 40`, `--library <folder>` (the versioned slice, below),
`--fake` (the tests' fake gateway: checks the plumbing, judges nothing).

The versioned slice, for a quick check and for the tests: `fixtures/large-test-slice/` (about a hundred documents, forty
spreadsheets, thirty-one questions). `python scripts/large_test/cli.py generate --slice --library fixtures/large-test-slice`
rewrites it; `python -m unittest scripts.test_large_test scripts.test_gen_knowledge_sandbox` proves the generator, the
book and the scorer (add `ACAI_SKIP_SLOW=1` to skip the full library, about twenty seconds).

## 3. What makes a run reproducible

- The library is generated from a fixed seed (`20261016`): the same bytes every time, on Windows and on macOS (the digests
  in `scripts/test_gen_knowledge_sandbox.py` guard the older sandbox the same way). The first N documents of the library
  are the N documents of a smaller one, so a result at 200 compares with the same question at 1 000.
- The index is built once per embedding model and kept; only the answering model changes between runs. A run copies the
  index to a temporary folder.
- The run records: the commit, whether the tree was clean, the model alias, the embedding alias, the gateway, the machine,
  the date, the hash of the book, the temperature (0.1 for an answer from material, as in the application), and the time
  of every question.
- **A model's answer is not deterministic.** Each question is asked once, at the application's own temperature, and a second
  run can differ. A figure that depends on the model is labelled with the model. The engines that answer without a model
  (the folder, the word search, the spreadsheets) give the same answer every time: the report counts them apart.

## 4. What is scored by the program, and what is not

Decided by the program, from the data the files were written from (never from the files, never from the application):

| Check | Used for |
| --- | --- |
| a number (to the cent), a date (in words or in figures), words, a pattern | a fact in a document, a total, a count |
| the files cited, the page cited | where an answer comes from; a slide is a page |
| a complete list and its total, no wrong file | the word search (« Quels documents parlent de X ? »), in documents and in tables |
| a count, a rank | the folder's own answers |
| a refusal, and no invented date or amount | an answer the documents do not hold |
| a forbidden fact (a look-alike's date, a file not ticked) | isolation and look-alike names: counted as a leak |
| the road taken | the feature a question is about (section 5) |
| the engine's reason | a spreadsheet question the engine must refuse (formula, ambiguous sheet, value not found...) |

Not decided: whether a prose answer is well written, whether it is complete beyond the facts listed, whether a scanned page
was read correctly. Those questions (`needs a reading` in the book: the scans and the organisation letters) are scored on what
can be checked and appear in `reading-sheet.csv`, a short sheet a person fills in; never all 300. The scorer is lenient on
extra numbers in a prose answer (any number equal to the expected one passes) and strict on forbidden ones.

## 5. Every expectation comes from the real path

The expected answers come from the generated data. The expected **road** of each question (which of the application's
engines should take it) is the author's reading of the feature; `routes` records the road really taken, with the
router, the selection, the knowledge base and the retrieval of the application, for all 300 questions and no answering
model. A question whose road differs from the book is **reviewed, not silently adjusted**: the report lists each with the
reason (a defect of the application, a defect of the question, or a limit worth knowing). Run `routes` after every
change of the library or of the book.

## 6. Files of a result

```text
data/large-test/<date>-<model>/
  run.json            the description of the run
  answers.jsonl       one line per question: road, answer, sources, structured answer, times
  answers.jsonl.meta.json
  scores.jsonl        one line per question: every check, pass or fail
  summary.json        counts per block, feature, format; refusals, leaks, wrong files, times
  report.md           the same, readable, with the list of failures
  reading-sheet.csv   the questions that need a person
```

## 7. The script the owner follows

`HUMAN-TEST.md` : one backup at the start, one restore at the very end, every command in full, every click by its exact
label; about thirty questions asked in the interface, one or more per feature, then the runner on the model she chooses.
The automated suites are never part of it.

## 8. Limits, stated

- A document in RTF is read as text: a table in it comes out as tab-separated cells. A message in EML is read without its
  attachments. A file protected by a password, or damaged, is named in the Analyse summary as not read. The subtitle files
  are read as one page; the presentations one page per slide, notes left out; charts and diagrams in a presentation are not
  read. UTF-16 text is not read.
- The folder questions know the words pdf, texte, markdown, word, png and jpeg: « Combien de fichiers vtt ? » is not
  understood as a question about extensions and returns the total of analysed documents. The book does not ask it; the
  report lists it as a finding.
- The scans are drawn by the generator and read by the local engine: what it misreads is the engine's, and the six scan
  questions are marked for a reading.
- The knowledge-base chips (the « @ » picker, « Élargir à ma sélection ») need the live entities of an index and are
  checked by hand, not by a question.
- A model that is not served by the AI machine is not run; no model is pulled for this test.
