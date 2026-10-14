# Retrieval lab: the baseline and the Phase-0 design note

13 October 2026 · branch `feat/retrieval-lab` · Phase 0 of `docs/SESSION-INDEXING-RETRIEVAL-MASTER.md`. **No product code was changed.** Everything below was measured by measurement code only: `apps/desktop/src-tauri/tests/retrieval_lab.rs` (the replay) and `scripts/retrieval_lab_score.py` (the scoring, tested by `scripts/test_retrieval_lab_score.py`). Written for the owner and for the agent who builds Phases 1 to 3. English; the interface strings stay French.

## 1. What the lab is

The replay builds a temporary analysed copy of the knowledge sandbox, embeds it through the tower's gateway, then replays the 205 questions of `fixtures/knowledge-sandbox/questions.json` **without the generation model** and records, for each question and each channel, the ranked passages (file, page, whether the passage carries the planted fact) and the time. The scorer reads that record and the ground truth.

| Channel | What it is |
| --- | --- |
| `pipeline` | What the product sends today for an ordinary question: the folder router, the knowledge step with the base off, the search with the baseline limits. Six passages at most. |
| `hybrid30` | The same ranking (cosine plus the flat +0.2 lexical bonus) read to a depth of 30, with no routing. |
| `vector30` | Cosine alone, depth 30. |
| `lexical30` | The application's own full-text query (an OR of every word of four letters or more), depth 30. |
| `term_all` | A plain phrase search of `chunks_fts`, every matching passage, for the six set sentences only (the offline check of `07-record-study-fixes-and-freeze.md`, section 5). |

**Metric definitions.** *Recall@k* is the share of the question's relevant files (`relevant_readable`) that appear among the first k **passages**. *MRR* is the reciprocal rank of the first passage of a relevant file. *nDCG@10* uses one binary gain per relevant file, at its first passage. *fact@k* is 1 when one of the first k passages carries every string of the question's `expect.contains` (accents, case and punctuation ignored), which measures the **passage**, not only the file. *Wrong files* counts the distinct files returned that are not relevant: it is meaningful for the pipeline (six passages) and for the set sentences; at depth 30 it only says that 30 passages come from many files, so it is not a quality figure there.

**How to run it** (once, at the end of a change; the real run takes about 15 minutes and calls the tower's embedding model with fictional fixtures only):

```powershell
cd C:\Users\elise\Documents\CURSOR\AssistantCabinetAI\apps\desktop\src-tauri
$env:ACAI_GATEWAY = "http://192.168.1.83:8080"; $env:ACAI_EMBED_ALIAS = "assistant-embed"; $env:ACAI_TIER = "200"; $env:ACAI_LAB_LABEL = "baseline"
cargo test --release --test retrieval_lab -- --ignored --nocapture
cd ..\..\..
python scripts/retrieval_lab_score.py data/retrieval-lab/baseline --categories
```

`ACAI_FAKE=1` checks the plumbing with fake vectors (the vector channels then mean nothing). To compare a later run: `python scripts/retrieval_lab_score.py data/retrieval-lab/<new> --against data/retrieval-lab/baseline`. The output lives in `data/` (not in git).

## 2. The baseline (the current pipeline, before any change)

Run of 13 October 2026, `kb/integration` code (`7d1cd0c`) plus the catalogue commit, library of 200 documents (tier 200), perfect text recognition for the scans (the fixtures' own text, so scans do not lose words here), embedding `assistant-embed` = `nomic-embed-text`, 768 dimensions, on the LAN tower. One run; not repeated (the figures below are single measurements).

### 2.1 The Analyse

| | |
| --- | --- |
| Documents the product reads | 176 of 200 (the other 24 are formats it does not read) |
| Passages (chunks) | 965, 882 651 characters |
| Analyse, whole | **744 s** (12 min 24 s) |
| of which embedding | **739.8 s (99.4 %)**, 224 batches, about **0.77 s a passage** |
| of which extraction / chunking / writing | 0.1 s / 0 s / 2.8 s |
| Index file | 7.66 MB |
| Question embedding, median | **846 ms** (a cost added to every answer, before any search) |

### 2.2 Retrieval, all questions

194 questions were replayed (the folder question is answered without the documents, the 10 workbook questions are out of scope). 183 have a relevant readable file and are scored; 9 have none (the documents cannot answer them); 2 have only an unreadable relevant file.

| Channel | R@5 | R@10 | R@30 | MRR | nDCG@10 | fact@5 | fact@10 | fact@30 | Files found / expected | Median / p95 time |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `pipeline` (6 passages) | 0.705 | 0.790 | 0.790 | 0.756 | 0.720 | 0.714 | 0.714 | 0.714 | 407 / 844 | 15 / 28 ms |
| `hybrid30` | 0.617 | 0.704 | **0.938** | 0.709 | 0.671 | 0.786 | 0.786 | 1.000 | 729 / 844 | 10 / 21 ms |
| `vector30` | 0.483 | 0.581 | 0.731 | 0.613 | 0.547 | 0.357 | 0.571 | 0.643 | 460 / 844 | 1 / 3 ms |
| `lexical30` | **0.783** | **0.844** | 0.931 | **0.858** | **0.853** | 0.714 | 0.714 | 1.000 | 721 / 844 | 0.7 / 3 ms |

`fact@k` covers the 14 questions that carry a planted fact, and counts a fact only in a passage of the question's relevant file (corrected on 14 October 2026: a figure repeated in a neighbouring document used to count; the sample-set figures of section 3.2 are unchanged by it). For `pipeline`, R@5 = R@10 = R@30 because it never returns more than six passages.

What it says, with the reading marked:

- **The word search beats the product's own ranking at depth 10 on this exam** (R@10 0.844 against 0.704 for the hybrid and 0.790 for the pipeline; MRR 0.858 against 0.709). *Measured.* The reason is *inferred*: the vector half, with this embedding model on French text, is the weakest channel (R@30 0.731, `term` R@30 0.420), and the flat +0.2 bonus lets it push lexical hits down. `hybrid30` is best only at depth 30, where the two halves complement each other (0.938).
- **Identifiers are the weakest family**: `identifier` pipeline R@10 0.515, MRR 0.263. The four invoice-number questions (Q131 to Q134, `FAC-2026-014`, `-114`, `-116`, `-117`) have their planted fact **in none of the six passages the pipeline sends**; `lexical30` finds it only at ranks 13 to 29, `hybrid30` at ranks 1 to 13. *Measured.* Consistent with the research note (no identifier normalisation, an OR over every word of the sentence).
- **A question the documents cannot answer still gets six passages**: the 9 such questions return 6.0 passages on average through the pipeline (30 through `hybrid30`). Retrieval never abstains; the refusal is left to the model. *Measured.*
- **The long-document needles are easy as built**: the eight questions find the right file at rank 1 or 2 and the fact within the first four passages in the pipeline (Q169 at 4, the others at 1 or 2), because each names its entity in the question. They do not yet test a fact without a distinctive word (section 3.1).
- Per-family figures for the pipeline: `person` R@10 0.969, `named_file` 1.000, `long_document` 1.000, `ocr` 1.000; weak: `follow_up` 0.357, `homonym` 0.271, `multi_entity` 0.500, `term` 0.551, `identifier` 0.515, `alias` 0.652, `practitioner` 0.689.

### 2.3 The six set sentences (E2 of the final test)

| Sentence | Files holding the answer | Pipeline (6 passages) | `hybrid30` | Plain word search | Wrong files (word search) |
| --- | --- | --- | --- | --- | --- |
| `LAMal` | 13 | 6 | 13 | 13 | 0 |
| `franchise annuelle` | 13 | 6 | 13 | 13 | 0 |
| `Genève` | 33 | 6 | 30 | 33 | 0 |
| `Lausanne` | 20 | 6 | 20 | 20 | 0 |
| `Nyon` | 8 | 6 | 8 | 8 | 0 |
| `Annecy` | 24 | 6 | 24 | 24 | 0 |
| **Total** | **111** | **36** | **108** | **111** | **0** |

The pipeline's **36 of 111** is exactly the figure of the owner's final test, which confirms the lab replays the real path. The word search finds **111 of 111** here, against 105 of 109 in the owner's live index: the difference is the file the lifecycle step deleted and the scan whose text recognition lost the word, neither of which exists in this clean copy. The acceptance target of the master file (105 of 111, zero wrong, no model call) is therefore reachable from a plain phrase search; this baseline says nothing about synonyms.

### 2.4 What the baseline does not cover (honest limits)

- **Passage-level truth is thin**: 14 questions carry a planted fact, the 115-page guide is the only long PDF and a 1-page-per-file DOCX is the only long DOCX (extraction makes every DOCX "page 1"). No fact is hidden in a table split over a page break, a list or a heading; no fact is worded without a distinctive keyword; there is no book of 500 or 1 000 pages. Section 3.1 proposes the missing truth.
- **The refusal side has 9 questions** and only the retrieval half is measured (passages returned), not whether the model refuses.
- **French is measured only through the exam's own vocabulary**; the embedding model is the only one tried. Section 3.2.
- **One run, one machine, one library of 200.** The scale (5 000 to 100 000 passages) is not measured here; the fake-vector bench `knowledge_validation_bench.rs` already scales the library to 1 000 documents and is the tool for the Phase-1 scale tests.

## 3. The sample set and the embedding comparison (the owner's decisions of 13 October 2026)

**Decisions taken.** (1) Passage-level truth is a **separate, small set**: 30 to 50 files at most, so that a run stays short. (2) No 1 000-page book for now; real French textbooks are used as long documents, as **extracts of 30 pages**, never in git (their rights are not stated in the files). (3) `bge-m3` and `qwen3-embedding:0.6b` are served by the tower's gateway under their own names (checked: dimension 1 024 for both, 768 for `assistant-embed`). (4) The metric definitions of section 1 are accepted. (5) **Phase 2 changes order: the lexical improvements (question frame, identifier normalisation, French stemming, field weights) are measured before the rank fusion**, because the plain word search is the stronger channel on the baseline exam.

### 3.1 What was built

`scripts/gen-retrieval-lab.py` (nine tests in `scripts/test_gen_retrieval_lab.py`) writes `fixtures/retrieval-lab/` (in git, 160 KB, deterministic) in the same shape as the knowledge sandbox, so the replay reads it through `ACAI_SANDBOX_DIR`. With `--books-dir` it also cuts two 30-page extracts into `data/retrieval-lab-full/` (git ignores `data/`): pages 101 to 130 of the anatomy manual of Marcel Pirard (circulation) and pages 50 to 79 of the medical microbiology course of Dr Ayad Wissem (urine examination). Every planted fact is checked against the text of its page at generation time. The Pressbooks epidemiology book is in English and the MSF drug guide has 678 pages: neither is used.

| | Synthetic set | With the two extracts |
| --- | --- | --- |
| Files / pages | 29 / 30 | **31 / 90** (126 passages) |
| Questions | 39 | **51** (42 with a planted fact, 9 whose answer is absent) |

The questions are worded so that the distinctive word of the answer is often absent from them (`paraphrase`: « taux de sucre dans le sang » for « glycémie »). The documents include near-duplicates (three insurer letters with the same template, two cholesterol results for two people named Dubois, two invoices a number apart, two quotes), identifiers written with and without separators (invoice, AVS number, IBAN, health insurance number, claim reference), a **table whose header is the last line of page 1 and whose rows start page 2** (three questions, one of which needs the column name and the value in the same passage), a list item, a heading that carries the answer, and nine absent facts.

### 3.2 The embedding comparison on this set (one run each, on the tower)

| Model | Dimension | Analyse (126 passages) | Per passage | Question embedding, median | fact@6 (pipeline) | fact@10 (hybrid30) | Median top cosine: absent / answerable |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `nomic-embed-text` | 768 | 97 s | 0.75 s | 832 ms | 0.929 (39/42) | 0.976 | 0.685 / 0.744 |
| `bge-m3` | 1 024 | 228 s | **1.79 s (2.4x)** | **1 765 ms** | 0.952 (40/42) | 0.976 | 0.512 / 0.618 |
| `qwen3-embedding:0.6b` | 1 024 | 191 s | 1.50 s (2.0x) | 1 391 ms | 0.976 (41/42) | 0.952 | 0.477 / 0.634 |

**How to read it, and why it does not decide.** *Measured:* file-level recall is 1.000 for every channel and every model (31 files are too few to confuse), so this set cannot rank the models on finding the file; the fact-in-passage figures differ by one or two questions out of 42 (one question is 2.4 points), which is inside the noise of a single run. The direction (the two multilingual models are at or above `nomic`, at twice the Analyse time and twice the question latency) is what the sandbox exam of section 2 must confirm.

*Measured, and usable now:* the **top cosine of an absent question is above the lowest one of an answerable question for every model** (absent maximum 0.778 against answerable minimum 0.607 for `nomic`; 0.636 against 0.495 for `bge-m3`; 0.684 against 0.490 for `qwen3`), so **no similarity threshold can turn retrieval into a refusal**, whatever the model. `qwen3-embedding:0.6b` separates the medians best (gap 0.157, against 0.106 for `bge-m3` and 0.059 for `nomic`).

**What the set does prove: the table split.** Question L028 (the column name *and* the value, in one passage) is **missed by every channel and every model**: the header sits in the passage of page 1, the row in that of page 2. This is the failure that the structure-aware chunking of Phase 1 (a table never cut, its header repeated) must fix, and it is the first measured goal of that phase.

### 3.3 What decides the embedding model, and what it costs

The sandbox exam (205 questions over 200 documents) is the one with enough files to separate the models (the `identifier`, `alias`, `homonym` and `term` families). At the rates above, 965 passages cost about **29 minutes of embedding for `bge-m3` and 24 minutes for `qwen3-embedding:0.6b`**, plus the 194 question embeddings (about 5 minutes each), against 12 minutes for `nomic`: **about an hour of tower time for both**. Not run yet: it is the owner's call (section 5).

### 3.4 The big set and the sandbox exam, run overnight (14 October 2026)

**The big set** (`python scripts/gen-retrieval-lab.py --books-dir <Downloads> --documents 500 --out data/retrieval-lab-big`; not in git): **500 documents, 1 369 pages, 1 746 passages, 236 questions** (224 with a relevant file, 12 whose answer is absent). 470 documents come from ten templates (insurer letters, glucose and creatinine results, invoices, meeting minutes, sick-leave certificates, appointment letters, transport invoices, AVS files, vaccination notes), each about a different person or number, so hundreds of near-identical files must be told apart. Seven textbook extracts of 56 to 200 pages each (Pirard, Ayad, Ferri, Kisangani, Lambert, Balaka, Marroun) bring 33 questions with a planted fact, a known page and a known file. Run on the tower with the three embedding models; `scripts/retrieval_lab_overnight.sh` is the driver.

| Model | Analyse (1 746 passages) | Per passage | Question embedding (median) | Application path: R@10 / MRR / fact in the cited passages | `hybrid30`: R@10 / R@30 | `vector30`: R@10 / R@30 |
| --- | --- | --- | --- | --- | --- | --- |
| `nomic-embed-text` | 30.2 min | 1.03 s | 1.17 s | 0.929 / 0.863 / 0.884 | 0.915 / 0.969 | 0.884 / 0.942 |
| `bge-m3` | 54.3 min | 1.86 s (1.8x) | 1.85 s (1.6x) | 0.933 / 0.898 / 0.911 | 0.929 / 0.978 | 0.942 / 0.973 |
| `qwen3-embedding:0.6b` | 45.2 min | 1.54 s (1.5x) | 1.43 s (1.2x) | **0.942** / 0.889 / **0.924** | **0.951** / **0.982** | 0.942 / 0.978 |
| word search alone (`lexical30`, any model) | 0 | - | - | 0.938 / **0.898** / 0.906 | - | - |

- *Measured.* At 500 documents the three models are within 1.3 points of each other on the application path (R@10) and within 4 points on the fact in the cited passages (about 9 questions of 224). The multilingual models help the vector channel more than the application path, because the word search already carries it. The plain word search alone (R@10 0.938, MRR 0.898, fact 0.906) is **at or above the application path for `nomic`** (0.929, 0.863, 0.884), as on the baseline exam.
- *Measured.* The question families that name one person (`keyword`, `paraphrase`, 148 questions) are found at 1.00 by every model and channel, so **this set tests scale and near-duplicates, not the difficulty of a paraphrase**: every templated question carries a unique name. Do not read 1.00 as "paraphrase is solved".
- *Measured, the model-independent weakness.* The `identifier` family (38 questions) is 0.58 / 0.61 / 0.66 for `nomic` / `bge-m3` / `qwen3`. It is **one kind of identifier**: an **AVS number written without dots in the question** (`7561234567897`) while the document holds `756.1234.5678.97`: the planted fact is in the cited passages for **1 of 17** questions with `nomic`, 2 of 17 with `bge-m3`, 4 of 17 with `qwen3`, and 3 of 17 for the word search. Every invoice number (full `FAC-2026-nnnn` or partial `2026-nnnn`, 18 of 18), the IBAN, the health insurance number and the claim reference are found by all of them. This is exactly identifier normalisation (C5): the first Phase-2 item to build, and no embedding model replaces it.
- *Measured.* Books: fact in the cited passages 0.76 / 0.88 / 0.91 for `nomic` / `bge-m3` / `qwen3` (33 questions): this is where the multilingual models show. The table split: 0.33 / 0.67 / 0.67 (L028, the column name and the value in one passage, fails for all three).
- *Measured.* Search time on the application path: median 22 ms (p95 43 ms) at 1 746 passages, against 15 ms at 965 (the full scan of Phase 1, item 10, grows linearly).

**The sandbox exam with `bge-m3`** (200 documents, 205 questions; `nomic` is the baseline of section 2): Analyse 29.4 min (1.83 s a passage) against 12.4 min. Application path R@10 0.795 against 0.790, MRR 0.812 against 0.756; **`vector30` R@10 0.729 against 0.581, R@30 0.804 against 0.731; `hybrid30` R@10 0.776 against 0.704, and the planted fact in the first ten passages 1.000 against 0.786**. On French text `bge-m3` is clearly the better vector model; the application path hides most of it because the word search carries the ranking.

**`qwen3-embedding:0.6b` on the sandbox exam** (redone on 14 October after the tower's Windows update; the first attempt was cut by the restart and discarded): application path R@10 0.792 (`nomic` 0.790, `bge-m3` 0.795), MRR 0.801; `vector30` R@10 0.655 (0.581 / 0.729); `hybrid30` R@10 0.766 (0.704 / 0.776). So on the exam `bge-m3` > `qwen3` > `nomic` on the vector side, the reverse order of the big set by a small margin: **the two multilingual models are equivalent within the noise, both clearly above `nomic` on the vector channel.**

**The tower changed speed between the runs, so the times above are not comparable with this one.** After the update the same models run several times faster (`qwen3` on the exam: 965 passages in 8.1 minutes, 0.50 s a passage, question embedding 152 ms). Re-timed in the new state on the 126-passage sample set (one run each):

| Model | Analyse (126 passages) | Per passage | Question embedding (median) |
| --- | --- | --- | --- |
| `nomic-embed-text` | 15 s | 0.11 s | 96 ms |
| `bge-m3` | 33 s | 0.26 s (2.4x) | 117 ms |
| `qwen3-embedding:0.6b` | 64 s | 0.50 s (4.5x) | 137 ms |

*Measured* in the new state: `qwen3-embedding:0.6b` is the **slowest** of the three (the order of the cost reversed, it was cheaper than `bge-m3` before), and every question embedding is now under 140 ms, so the question-latency argument against the larger models has gone. *Extrapolated, not measured:* the 1 746-passage big set would cost about 3 minutes with `nomic`, 7.5 with `bge-m3` and 14.5 with `qwen3`. The figures of the first three tables of this section describe the tower before the update.

**What this settles, and what it does not.** *Settled:* the weak points that no model fixes are the AVS-number format and the table split; the word search is as good as the whole application path and is the channel to improve first (decision 5); `bge-m3` and `qwen3-embedding:0.6b` are both better than `nomic` on the vector channel and on books, and equivalent to each other. *Leaning, not decided:* `bge-m3`, because with the tower's new speed it costs 2.4 times `nomic` in Analyse time (about 7.5 minutes for the big set) against 4.5 times for `qwen3`, for the same quality, and it was the better of the two on the French exam. *Not settled:* the gain on the application path is small today (the word search carries the ranking), and the tower's speed may change again (the Mac mini is expected). The honest procedure stays: build the lexical improvements first, then re-run the models once on the same two sets and decide on the figures that remain.

**Per-question reports for the owner's test in the application** (not in git: they quote facts from books whose rights are not stated): `data/retrieval-lab/report-big.md` (236 questions, three models) and `data/retrieval-lab/report-exam.md` (205 questions, the three models). Each question gives what it must find (file, fact, page), and for each model whether the application's own path (the six passages it would cite) finds it. The list of the 26 questions the application path misses with `nomic` is at the top of the big report. The documents of the big set to point the application at: `data/retrieval-lab-big/documents` (500 files; indexing them changes the application's real index, so the usual backup comes first).

## 4. Phase-2 order, recorded

The master file's Phase 2 begins with contracts, then the question frame, then candidate generation fused by RRF. By decision 5 the order becomes: (a) contracts; (b) the **lexical channel** made precise and measured alone on the baseline exam: question frame, French analysis and stemming, field weights, phrase and proximity, identifier normalisation, the domain thesaurus; (c) only then the fusion (RRF), the rerank and the assembly. A method is kept only if recall@10 rises and no family regresses.

## 5. Open points for the owner

1. **The embedding model**: `bge-m3` is the leaning choice (section 3.4); the decision waits for the re-run after the lexical improvements. Nothing to do now.
2. **Identifier normalisation first.** The measured model-independent failure is the AVS number written without dots (1 of 17 found on the application path with the current model). Confirm it as the first item of Phase 2 (decision 5 already puts the lexical channel first).
3. **Refusal.** No similarity threshold separates an absent answer from a present one (section 3.2); the big set confirms it, since the 12 absent questions still get six passages. Record in the Phase-2 plan that the refusal comes from a check of the question's content terms and entities against the passages, plus the model.
4. **A model switch forces a full re-Analyse** (the index must record the model and the dimension, Phase 1): about 3 minutes with `nomic`, 7.5 with `bge-m3` for the big set at the tower's present speed; the figures were 30 and 54 minutes before its update.

## 6. Files of Phase 0

| File | Role |
| --- | --- |
| `apps/desktop/src-tauri/tests/retrieval_lab.rs` | the replay (ignored test; reuses `tests/common/validation.rs`) |
| `scripts/retrieval_lab_score.py`, `scripts/test_retrieval_lab_score.py` | the scoring tool and its tests |
| `scripts/gen-retrieval-lab.py`, `scripts/test_gen_retrieval_lab.py` | the sample set generator and its tests |
| `scripts/retrieval_lab_bulk.py`, `scripts/retrieval_lab_report.py`, `scripts/test_retrieval_lab_report.py`, `scripts/retrieval_lab_overnight.sh` | the templated documents and extra books of the big set, the per-question report, the long-run driver |
| `fixtures/retrieval-lab/` | the versioned synthetic set (29 files, 39 questions) |
| `docs/RETRIEVAL-LAB.md` | this file |
| `data/retrieval-lab/` | the runs: `baseline`, `lab-*`, `big-nomic`, `big-bge-m3`, `big-qwen3-0.6b`, `exam-bge-m3`, `exam-qwen3-0.6b`, `retime-*` (not in git) |
| `data/retrieval-lab-full/` | the set with the two book extracts (31 files, not in git) |
| `data/retrieval-lab-big/` | the big set: 500 files, 236 questions (not in git) |
| `data/retrieval-lab/report-big.md`, `report-exam.md` | the per-question reports (not in git) |

Run the sample set: `python scripts/gen-retrieval-lab.py --books-dir "C:\Users\elise\Downloads" --out data/retrieval-lab-full`, then the replay with `ACAI_SANDBOX_DIR` set to the absolute path of that folder, `ACAI_TIER=50`, and `ACAI_EMBED_ALIAS` set to `assistant-embed`, `bge-m3` or `qwen3-embedding:0.6b`. Score it with `python scripts/retrieval_lab_score.py data/retrieval-lab/<label> --questions data/retrieval-lab-full/questions.json`.

Final pass done on 14 October 2026 for what this phase added: `rustfmt --check` on `retrieval_lab.rs` alone (clean; never plain `cargo fmt`), `cargo clippy --test retrieval_lab` (no warning in the new file), the 30 script tests (`python -m unittest test_retrieval_lab_score test_gen_retrieval_lab test_retrieval_lab_report` from `scripts/`: scorer 10, generator 14, report 6; all pass). No product code changed, so the product suites were not rerun.

## 7. Decisions recorded on 14 October 2026

1. **The tower's window is `ministral-3:3b=16384`** (owner's decision, after reading `docs/CONTEXT-BUDGET.md`; confirmed by `/health` after `docker compose up -d server`). It gives 14 passages and 14 000 characters to a question; the lab's `pipeline` channel keeps the baseline limits (6 passages), so its figures do not depend on this value.
2. **Tabular files are protected from the text re-indexing (binding rule for Phase 1).** The workbooks' data lives in the **same SQLite file** as the text index (`tabular_inventories`, `tabular_workbooks`, and `kb_column_semantics` for the base's reading of the columns). The master file lets the index be rewritten but does not name them, so this rule is added: a schema change touches only `documents`, `chunks`, `chunks_fts` and the new tables (sections, fields); it never drops, clears or rewrites a `tabular_*` table or `kb_column_semantics`; an automatic test migrates an index that holds workbook data and checks that those tables are identical afterwards, and the `data` questions of the 205-question exam are replayed before and after every Phase-1 step. Nothing in Phase 0 touched them: no tracked file was modified (only new files were added), the lab runs on temporary copies, and the workbook questions are outside the replay.
3. **The lexical channel comes before the fusion** (decision 5 of 13 October, section 4).
4. **Long-document measures are those of `docs/SESSION-LONG-DOCUMENTS.md` lots 1 to 9, now in Phase 1 and 3 of the master file**; their open decisions (the « long » threshold, the display « vectorisé à X % », summaries stored in the workstation's index, the duration estimate, whether a per-passage reduction leaves shadow mode) are put to the owner in the Phase-1 design note, as planned.

