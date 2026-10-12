# 04 - Final developer report of the Knowledge Base programme (lots 0 to 11)

Written for the developer or agent who was not in the sessions, and for the owner who decides what happens to the branch `kb/integration`. English; the French sentences are what is typed or read on screen.
12 October 2026 · closes the programme at lot 11 (lot 10 was withdrawn) · branch `feat/kb-validation`, based on `kb/integration` at `5d18c55`.

The spec of the thing is `docs/KNOWLEDGE-BASE.md`; the rules it keeps are in `docs/DECISIONS.md` (« Settled by KB lot N »); this file is the **evidence**: what was built, what was measured, what was found, what to fix next.
The release decision is `03-release-gate.md`; every raw run is in `measurements/`; the script the owner plays is `human-tests/lot-11-validation.md`.

## 1. Summary

The knowledge base reads the names, organisations, places and identifiers of the documents and workbooks the user analysed, keeps them on the workstation, and, in the mode « Automatique », searches only the files that talk about the names of a question.
Lot 11 added nothing to the product. It built a **library with a ground truth** (200 documents versioned, 1 000 generated, 28 scans, long documents, seven formats the product does not read, workbooks) and measured the base against it, then wrote the script the owner plays.

| Question asked | Answer measured |
| --- | --- |
| May « Automatique » become the default? | **No.** The gate is not met (`03-release-gate.md`). The owner's inclination, « Suggestions seulement », is what the evidence supports |
| Does it keep the files that hold the answer? | 91.6 %, 95.9 % and 91.5 % of the files that hold the answer and that the base-off search cites, at 30, 100 and 200 documents on the real stack. 98 to 100 % for a person written in full; 66 to 75 % for a practitioner with a Swiss title |
| Does it ever read outside the selection, or learn from a question? | **No**: 0 files outside the selection in 2 198 replays; the base is identical before and after questions in every walk |
| Does it make mistakes of identity? | 2 false merges (two people with swapped given name and surname) in a library of 291; 3 questions out of 205 at 200 documents narrowed on another entity than the one named (8 of 225 at 1 000) |
| Does it make the user wait less? | **No measurable gain.** Search 18.9 ms to 9.1 ms (median, 200 documents); the answer 10.7 s against 11.1 s (7 warm questions). Its own cost: 3 ms per question; 1 to 3 % of an Analyse |
| What did it find that is not the base's fault? | 38 rows in the register in all; 14 new in this lot (KBD-25 to KBD-38), among them: the Analyse lists seven document formats it never reads (10 % of a Swiss library), the folder router answers « no matching file » to an AVS number, Swiss amounts `CHF 1'450.00` are not numbers to the table engine, a typed workbook name that holds a year answers `0` |

What is true of the programme as a whole: the design held (every invariant I1 to I14 passes), the cost is small, and **recall is limited by what the extractor can read, not by the reduction rule**. The first
lever is therefore the fix battery of section 9, group 2, not another lot of the base.

**Decisions of the owner after reading this report (12 October 2026, `docs/DECISIONS.md`):** verdict accepted; gate item 2 rewritten; the battery is applied now, Switzerland first, before the big human test; CI runs the integration tests on Windows and macOS; `kb/integration` is never merged into `main`; every branch and tag is kept (`05-branches-and-stages.md`).


## 2. Architecture: what was built, and where

```text
apps/desktop/src-tauri/src/knowledge/        25 700 lines (tests included), no platform `cfg`, no platform crate
  normalize.rs  phonetic.rs  packs.rs        words: accents, titles, particles, sound; lexicon packs (JSON data)
  store.rs  maintenance.rs  secret.rs        the `kb_*` tables in index.sqlite3, migrations, the keyed hash for personal identifiers
  extract/{text,table}.rs  gazetteer.rs      what a document or a workbook names (candidates), the list of names already known
  resolve.rs  ingest.rs  backfill.rs         a name to an entity (never a fuzzy merge); the pass at the end of an Analyse
  query.rs  reduce.rs  targeting.rs          at question time: names of the question, the files of the selection, the reduced set,
                                             the search and the single widening
  diagnostics.rs                             timings and counts, numbers and codes only
  manage.rs  picker.rs  suggest.rs  sheet.rs  the dialog, `@`, suggestions, the sheet of a name
  surface.rs  repair.rs  guard.rs            read-only contracts for speech, transcript repair, an outbound-text guard
apps/desktop/src/                            the targeting line, the widen button, chips, `@`, the dialog (TypeScript only)
```

Four rules hold the design together, and lot 11 checked each by measurement rather than by reading code:

1. **The user's selection is the authority.** A reduced set is built by filtering the selection; a final check drops any path that is not in it
   (I1). Measured: **0 files outside the selection or left out on purpose were searched, in 2 198 replays** of the sandbox's questions
   in three modes (`leak` and `small_scope` questions assert it; every other question counts it).
2. **Any doubt is the old path.** No entity, a candidate, ambiguity, a named file, a small selection, a lookup that fails: the search on the
   whole selection runs, exactly as with the base off (I4).
3. **Nothing a question says is learned.** The snapshot of every `kb_` table is identical before and after questions (I3), also in the walk.
4. **Nothing the base holds leaves the workstation.** Counts, codes and milliseconds are all the logs hold; the model never sees a name
   because of the base (I8, I9).

## 3. The knowledge model

Types are neutral (`person`, `organization`, `location`, `identifier`, `item`, `term`); a role is an attribute; an entity has a status
(`candidate`, `active`, `merged`, `deleted`) and an origin (`automatic`, `manual`). Names are stored with their other spellings
(`canonical_variant`, `reordered`, `initial`, `partial_surname`, `title_form`, `filename_form`, `manual`) and the sound of each. A mention says where:
a chunk of a document, the cells of a table (sheet, column, rows), a file name. Relations exist only where a table states them. Personal
identifiers (social security, AVS, IBAN, e-mail) are stored as a keyed hash. Merge is a redirect; delete is a tombstone; every user action is
reversible except removing a name, editing and setting a detail. Full description: `docs/KNOWLEDGE-BASE.md`.

## 4. Retrieval: how the base meets the user's selection

Between the router and the question's single embedding call: read the names of the question (runs of one to five words, one query), decide by
the rules of `reduce` (at least 8 files selected, an entity at 0.80 or more that is not a candidate, in no more than 60 % of the selection, nothing
ambiguous, at least 30 % dropped, at most 15 documents kept), add the safety net (files the base never read, open possible matches, files whose text
holds another form of the name), search, widen once if the reduced search is empty or far from the question in meaning. `suggest` records the
shadow and never narrows; a chip the user picked narrows in any mode; the widen button is the old path. Documents and workbooks are decided
independently and the base only shortens the list of workbooks the engine is given.

## 5. DATA: the tables

Headers are read as a whole against the packs before any word of them is looked at (the lesson of KBD-01); a column is read as people, organisations,
places or identifiers with a confidence, and a column that is not read is recorded with the reason (a sheet of free notes, a column called `Nom`
that holds zone codes: both rejected in the sandbox). A table's entities are active at once. The engine is untouched (I5): the base never writes a
sheet restriction and never changes a number. Measured by the sandbox: in the library of 200, 164 of the 165 persons written in the workbooks are found (99.4 %), their files tied with a recall of 94.8 % and a precision of 99.0 %; the 10 organisations of the tables are all found; 21 columns of 7 workbooks were recognised in the first 30 documents (people, organisations, identifiers), 102 relations between columns were read, and a sheet of free notes and a column called `Nom` that holds zone codes were both rejected, with the reason recorded; a person written `Dupont J` is a possible match of `Jean Dupont` (KBD-15, fixed). What the base does not do with a table: it never writes a sheet restriction and never changes a number.

## 6. Performance (baseline against reduced, honest)

Machine: the owner's development PC (Windows 11, CPU only for the models), the gateway and Ollama of the local Docker stack (`ministral-3:3b`, `nomic-embed-text`), the bundled Tesseract and pdfium. Libraries: `fixtures/knowledge-sandbox` (tiers 30, 100, 200)
and `data/knowledge-sandbox-1000` generated with `--extra-rows 40` (tiers 500, 1 000). Method: `tests/knowledge_validation_bench.rs` (fake vectors, to isolate the base) and `tests/knowledge_validation_real.rs` (real embeddings, real OCR).

### 6.1 What an Analyse costs, and what the base adds

Real stack, documents folder, one pass per step (`measurements/real-tier-*.md`):

| Step | Files read | Total | Embedding | Text recognition | Reading names (base) |
| --- | --- | --- | --- | --- | --- |
| first 30 documents | 28 | 24.6 s | 19.8 s | 3.9 s (3 scans) | 0.35 s (documents 97 ms, tables 251 ms): **1.4 %** |
| 30 to 100 | 61 new | 640 s | 629 s, **585 s of it for two long documents** (a 115-page guide and a manual: 460 and 331 passages, 341 s and 244 s) | 7.5 s (7 scans) | 0.43 s: **0.07 %** |
| 100 to 200 | 87 new | 64 s | 48.5 s | 11.7 s (18 scans) | 0.75 s: **1.2 %** |

With fake vectors, the base alone (`measurements/bench.md`, Analyse with the base off against on, documents and tables): +0.3 s at 30 documents, +1.6 s at 100, +1.4 s at 200, +5.2 s at 500, **+11.1 s at 1 000** (878 files read; index 16.5 MB, 1 289 entities, 2 445 aliases, 4 135 mentions).
It is a share of an Analyse that is dominated by the embedding of the passages: **one hundred-page document costs as much as sixty short ones**.

### 6.2 What a question costs

Median / p95, in milliseconds, real stack, 200 documents (176 read), questions that search the documents (`measurements/real-tier-200.md`):

| Mode | Embedding of the question | Knowledge lookup | Search | Passages scored | Files searched |
| --- | --- | --- | --- | --- | --- |
| Désactivé | 292 / 370 | 0 | 18.9 / 38.8 | 965 | 176 |
| Suggestions seulement | 292 / 370 | 7 / 16 | 18.3 / 26.8 | 965 | 176 |
| Automatique | 292 / 370 | 6 / 13 | 9.1 / 25.7 | **15** | **7** |

Fake vectors, lookup median / p95 at 30, 100, 200, 500, 1 000 documents: 3 / 10, 3 / 7, 4 / 7, 5 / 15, 9 / 24 ms (`Automatique`). The search over « tous » is linear (0.4 ms at 28 files, 16 ms at 878); a search over **all but one ticked file** is not
(1.3 ms, 12.9, 18.1, 80.5, **320 ms** median at 28, 89, 176, 447, 878 files with the base off; KBD-16, still there; `Automatique` brings it back to 17.8 ms by searching 15 files).

**The wait is the model's.** The prompt holds at most 6 passages in every mode. In `Automatique` the median number of passages in the prompt falls from 6 to 4 or 5, and in a sample of 8 questions at 30 documents the prompt fell from 5 677 to 3 098 characters (median).
The generation times of that sample: with the base off 107 to 166 s (the chat model had to be loaded again after the embedding model on this PC: **an artefact of the machine, the same prompts took 3 to 27 s in the next mode**), « Suggestions seulement » 10.7 s,
« Automatique » 11.1 s (median of the 7 warm questions; per question -3 s to +12 s). Conclusion: **no gain that a user would notice**, in the one place where a shorter prompt could have paid. A machine where a long prompt costs more than a shorter one might show one; this one does not
(`measurements/real-tier-30-with-generation.md`).

### 6.3 The formats that cost nothing because they are never read

In the library of 200: 21 files (10.5 %) of kinds the product lists and never reads (`measurements/formats.md`): `.odt` 7, `.rtf` 5, `.eml` 4, `.vtt` 2, `.odp` 1, `.ppt` 1, `.pptx` 1, plus the table `.ods` (`Failed`); `.xls` **is** read. At 1 000: 122 of 1 000 (12 %).

## 7. Accuracy

### 7.1 Names found, merged and tied to their files (`measurements/quality-tier-*.md`)

| Documents (read) | Persons found | Organisations found | False merges |
| --- | --- | --- | --- |
| 30 (28) | 25 of 26 (96.2 %) | 12 of 19 (63.2 %) | 2 |
| 50 (41) | 31 of 33 (93.9 %) | 14 of 23 (60.9 %) | 2 |
| 100 (89) | 72 of 78 (92.3 %) | 22 of 34 (64.7 %) | 2 |
| 200 (176) | 92 of 104 (88.5 %) | 33 of 50 (66.0 %) | 2 |
| 500 (447) | 172 of 189 (91.0 %) | 34 of 51 (66.7 %) | 2 |
| 1 000 (878) | 263 of 291 (90.4 %) | 33 of 51 (64.7 %) | 2 |

At 200 documents, files tied to the right name: persons in documents **90.2 %** recall and 98.0 % precision; organisations in documents **71.5 %** recall and 100 % precision; persons in tables 164 of 165 found, 94.8 % recall, 99.0 % precision; organisations in tables 10 of 10.
By the shape the name is written in: `title_full` (`Dr Anne Marchand`) 92.7 %, `full` 88.1 %, `reordered` 100 %, `no_accent` 100 %, `initial_surname` 80 %, `title_surname` (`Dr Dupont`) 0 % (never tied on its own, by design), an organisation as an `acronym` 0 %, a name `split by a line break` 0 % (KBD-32).
By format: txt and md 100 %, docx 91.7 %, pdf 93.1 %, scan-pdf 91.7 % for persons; organisations 61.5 % (scan-pdf) to 100 % (txt). Not found at 200 (29 names): the `Dre` doctors, seven recipients alone on a line, and the organisations without a marker word (KBD-26, KBD-27, KBD-31).

**What the real text recognition costs** (`measurements/ocr.md`, 28 scans of the library, by how badly each page was scanned):

| Scan quality | Scans | Words kept | Names and numbers kept exactly | Seconds per scan |
| --- | --- | --- | --- | --- |
| clean (0) | 7 | 99.1 % | 100.0 % (23 of 23) | 0.4 |
| average (1) | 9 | 92.4 % | 91.1 % (41 of 45) | 0.7 |
| poor (2) | 12 | 75.8 % | 66.1 % (37 of 56) | 0.9 |

A name read wrongly cannot be tied to its person. Of the 8 sources lost by the real run at 30 documents, 6 are one letter that writes its patient only as `M. Dupont` (KBD-37) and 2 are one photograph whose name the text recognition misread; at 200 documents the 15 `not_linked_to_the_name` include three more photographs.

### 7.2 Which files are kept, question by question

Replay of the part of `sourced_answer` that comes before the model, in `Automatique` (`quality-tier-*.md`), fake vectors:

| Documents (read) | Questions | Narrowed | Recall of the narrowed set, every relevant file | Median reduction | Wrong entity | Outside the selection searched | Lookup median / p95 (ms) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 30 (28) | 127 | 63 | 90.2 % (92 of 102) | 96 % | 1 | 0 | 2 / 6 |
| 50 (41) | 127 | 60 | 91.9 % (124 of 135) | 95 % | 1 | 0 | 2 / 6 |
| 100 (89) | 195 | 109 | 82.3 % (265 of 322) | 98 % | 3 | 0 | 3 / 6 |
| 200 (176) | 205 | 115 | 85.9 % (353 of 411) | 98 % | 3 | 0 | 3 / 9 |
| 500 (447) | 220 | 128 | 53.3 % (561 of 1 053) | 99 % | 8 | 0 | 9 / 23 |
| 1 000 (878) | 225 | 127 | 44.2 % (822 of 1 858) | 99 % | 8 | 0 | 9 / 22 |

This table counts **every** relevant file, including the ones the base-off search would not cite (it cites six). The fall at 500 and 1 000 documents is almost all `capped` (418 of 492 lost files at 500, 892 of 1 036 at 1 000):
a question such as « Quels documents viennent de Genève ? » has 34 relevant files and the cap keeps 15. The base-off search would not have cited more than six. The real stack counts only the files the base-off search cites, which is the question that matters (7.3).
Why a question was not reduced, at 200 documents: 39 name nobody (`no_entity`), 15 name a file (`named_file`), 13 are ambiguous (the user is asked), 11 are answered by the folder router, 8 had no candidate, 2 had no discriminating entity, 2 are on a small selection.

### 7.3 Recall on the real stack (embeddings of `nomic-embed-text`, Tesseract), files that hold the answer and that the base-off search cites

| Documents (read) | Questions the base would narrow | Recall | Lost, by reason | Raw (every cited file, filler included) |
| --- | --- | --- | --- | --- |
| 30 (28) | 58 | **91.6 %** (87 of 95) | `not_linked_to_the_name` 8 | 31.3 % |
| 100 (89) | 103 | **95.9 %** (186 of 194) | `capped` 5, `not_linked_to_the_name` 3 | 38.0 % |
| 200 (176) | 112 | **91.5 %** (237 of 259) | `capped` 7, `not_linked_to_the_name` 15 | 41.6 % |

By category at 200 documents: alias 100 %, homonym 100 %, identifier 100 %, term 100 %, multi-entity 100 %, person 98.0 % (99 of 101), organization 94.7 %, mixed 90.9 %, ocr 87.5 %, **leak 72.2 %**, **practitioner 65.7 %**. The `leak` category is the questions
built to provoke a loss of recall (the Dr Gaillard who is also a commune, the Dre Vuagniaux); it is where the base is weakest and it is on purpose.

Answers and citations compared with the base off, 200 documents, questions with a known answer (174): at least one right source off 160, `Automatique` 165; the cited files that really hold the answer 43.0 % against **59.9 %**; the same first source in 86 of the 112 narrowed questions.
In 31 of the 112 the search was widened by itself because the narrow set was too far from the question in meaning (16 of 58 at 30 documents, 37 of 103 at 100).

## 8. Real-world results (the manual protocol)

The protocol `human-tests/lot-11-validation.md` has 9 parts (0, A to E, Z, R). **The agent walked every step on a copy** with the real helper (`lot11_helper.py`) and the real Rust code for the lines, the counts and the lists; the owner's results go into `02-results.md`.
What the walk established (and the protocol says so, step by step):

- The lifecycle holds: the base is identical after a Reset of each card and of the whole base followed by an Analyse (559 names, same list); a new letter adds its person, a renamed file keeps its associations (name only changed, as the exception of 25 September 2026 says), a changed letter replaces the old surname, a deleted letter takes its associations away
  and leaves the others; growing from 30 to 100 and back keeps `integrity_check` clean (`the_lifecycle_of_the_protocol_leaves_the_base_clean`). One thing stays stale on purpose: a name that two files agreed on is not demoted when one of them leaves (KBD-12).
- Nothing a question says becomes knowledge: four questions about invented names (`Madame Zénobie Quarante`, `Hyacinthe Vaudroz`, `Dr Ernest Blondeau`, `Léopold Tschumi`) in each of the three modes change no row.
- A name written only as `Dre` is not a title, a hospital without a marker word is not found, a recipient alone on a line is not read, an AVS number is answered « no matching file » by the folder router. Each is a step of the protocol with the expected consequence, so that she sees it in her window.

The owner's own parts (A to E) are **pending**. They are the only evidence about her library, her models and her window; `03-release-gate.md` says what is still missing for each gate item.

## 9. The post-KB fix battery (owner instruction of 9 October 2026)

Every row of the defects register that is `deferred` has a proposal here; none is fixed in this lot. Cheapest and most
harmful first, rows that share code grouped. **Size**: S under a day, M two to three days, L four to six (agent plus
review). The owner accepts the battery, orders it and cuts it into branches; each group below is one branch
(`fix/kb-<topic>`, base `main` after the release decision) with its own tests and its own short human test.

### Group 1 - one-line or data changes that remove a confident wrong answer or a silent loss (do first)

| Fix | Rows | What | Files | Smallest fix | Risk | Test that proves it | Human step | Size |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **F-01** | KBD-16 | A search over many explicitly selected files is quadratic: 1 000 files, 4 530 ms | `index_store.rs` (migration) | `CREATE INDEX IF NOT EXISTS chunks_by_document ON chunks (relative_path)` | none for answers; a few MB of index | `knowledge_validation_bench.rs` (the all-but-one selection at 1 000 files falls from a median 320 ms to the 16 ms of « Tous les documents »); `knowledge_targeting_bench.rs` | untick one file of a 1 000-file library and ask: the answer starts within the same second | S |
| **F-02** | KBD-26 | `Dre`, `Dresse` are not titles: a Swiss woman doctor is filed with her title in her name | `resources/knowledge/base/{fr-FR,en-US}.json` (data) | add `Dre`, `Dresse`, `Dr.`, `Pre`, `Docteure`, `Professeure` to the person titles | none (a list) | the sandbox's four `Dre` doctors are found under their own name, one entity each (`report_quality_and_recall`, and probe 5 and 6) | `Que dit la Dre Vuagniaux ?` and `Quels courriers a écrit la Dre Anne-Sophie Vuagniaux ?` give the same files | S |
| **F-03** | KBD-29 | A commune written in a table steers the documents of a topic question, and the cap drops half of what the lexical net found | `knowledge/reduce.rs` | a `location` entity never drives a reduction of the documents side (it may only order); or the cap does not apply to a candidate set made only of the lexical net | low: only `auto` changes | `explain_one_question` Q011, Q012, Q102 are not reduced; `reduced_scope_is_always_a_subset_of_the_selection` unchanged | `Quels documents viennent de Genève ?` in `auto` shows no « Recherche ciblée » line | S |
| **F-04** | KBD-20 | A restored name has no table mention until its workbook is read again | `knowledge/manage.rs`, `knowledge/backfill.rs` | a deletion and a restore mark the entity's workbook sources due | low | delete then restore a name found in a column, analyse the Data card with no file change: the cells mention is back | the same, in the dialog | S |
| **F-05** | KBD-24, KBD-01 (watch point) | none of the two is a one-liner; see group 3 | | | | | | |

### Group 2 - the extractor reads fewer names than a real library holds (the biggest gain for `auto`, and for every KB surface)

| Fix | Rows | What | Files | Smallest fix | Risk | Test that proves it | Human step | Size |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **F-06** | KBD-27 | Organisations without a marker word, or with lowercase words after one, are not found (HUG, CHUV, HCL, SUVA, URSSAF...); some are filed as persons | `knowledge/extract/text.rs`, `resources/knowledge/packs/*` | a list of well-known organisations and acronyms per country in the packs, matched exactly; an acronym of 3 to 6 capitals is an organisation candidate; read lowercase words of a marker's name (`Hôpitaux universitaires de Genève`) | medium: a new class of candidates; run the type-stability test (`a_module_never_changes_the_type_of_a_name`) | the sandbox's organisations never found are found (`report_quality_and_recall`), none is a person (probe 7 to 11) | `Quels courriers viennent du CHUV ?` in `auto` shows « Recherche ciblée : N sur M documents — CHUV » | M |
| **F-07** | KBD-28, KBD-11 | Organisation names cut or mis-typed at the edges (`Entre Bureautique Léman SA`, `Fils SARL`, `l'Étude Chappuis`, `Dupont Immobilier` as a person); an address fragment promoted to a person; an untitled run filed as a person | `knowledge/extract/text.rs`, `store.rs` (promotion) | drop a sentence-initial stop-word before a suffix; `&` joins capitalised words; a run ending with a legal suffix is an organisation whatever its first word; do not promote a fragment seen only with an address marker; type an untitled run as `term` | medium (changes types of existing candidates: raise `CURRENT_KB_VERSION` so documents are read again at the next Analyse) | probes 12 to 15; the sandbox's unmapped entities list is empty of persons that are organisations | the Knowledge dialog lists no `Entre ...` and no `l'Hôtel de Ville` as a person | M |
| **F-08** | KBD-31, KBD-32, KBD-33 | A name alone on a line, a name split by a line break, and a name with a particle are not read | `knowledge/extract/text.rs` | read a line that is only two or three capitalised words as a person candidate; join the lines of a paragraph before reading names (as identifiers already are); let particles join two capitalised words | medium: more candidates, so more junk unless F-07 is done with it (candidates never narrow) | probes 0 to 4 and 19; the seven insurer recipients of the sandbox are found | the new letter of the human test that starts with `Madame Sidonie Quenneville` alone on a line is found | M |
| **F-09** | KBD-25, KBD-36 | Two people whose given name and surname are swapped (`Lucas Roy`/`Roy Lucas`) are one entity; a surname with an apostrophe (`D'Amico`, `N'Diaye`) is split into several entities and a question for the full name resolves to another person's surname alias | `knowledge/resolve.rs` | a reordered alias is an alias of the entity **and never merges two entities that both have an exact full-name form in sources of the same domain**; file them separately with a possible match | medium: touches the identity key; the surname-first administrative form `DUPONT Jean` must still join `Jean Dupont` | the sandbox pairs give 0 false merges, `DUPONT Jean` still joins `Jean Dupont`, and `Giuseppe D'Amico`, `Aïcha N'Diaye`, `Sean O'Brien` each give one entity | `Que dit Roy Lucas ?` offers « Vouliez-vous dire : Lucas Roy » and does not search both people | M |
| **F-10** | KBD-12, KBD-13 | A promoted candidate is never demoted when its second source goes; a gazetteer hit overwrites the real method | `knowledge/store.rs`, `knowledge/ingest.rs` | carry the number of supporting sources and demote below two; keep the strongest method of the same place across rounds | low to medium | shrink the library 100 to 30: the names promoted by the removed files are candidates again (`the_lifecycle_of_the_protocol_leaves_the_base_clean`, C5, asserts equal counts) | the dialog shows the real method (« titre et nom ») for a name found by a title | S |

### Group 3 - the product answers confidently and wrongly (tabular tier and folder router; not the KB's code, found by it)

| Fix | Rows | What | Files | Smallest fix | Risk | Test that proves it | Human step | Size |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **F-11** | KBD-01 | A column named `Total_Ligne` is answered as a row count | `tabular/question.rs` | test the whole column name against the question before splitting it into words | low to medium: the classifier; the whole `tabular_reference` suite is the guard | on the publipostage file, `Quelle est la somme de Total_Ligne ?` is 1 592,98, the maximum 899, the mean 318,596 | the same questions | S |
| **F-12** | KBD-24, KBD-22 | A title shared by several values of a column answers `0`; a month or a year on a date column the file does not settle answers `0` | `tabular_answer.rs` (`detect_filters`) | do not filter on a residual word that belongs to several distinct values of its column (or filter with `In`); let the words consumed by a longer value count together; when the only date-like column is `ambiguous_date` and the question holds a month, a year or a weekday, answer with the nudge | medium: filters | `Combien de lignes pour Dr Jean Dupont ?` equals `... pour Jean Dupont ?` on a column of three doctors written `Dr ...`; the four sentences of KBD-22 give a nudge naming the column | `Combien de rendez-vous pour Dr Paul Martin ?` against `rendez-vous-2026.xlsx` | M |
| **F-13** | KBD-21, KBD-23 | With documents and two workbooks selected, a plain question about the documents is answered « Quel classeur ? »; « combien de documents » is a row count | `tabular_answer.rs`, `commands.rs` (mixed tier) | ask « which workbook » only when the classifier recognised a data operation or a column; a question whose subject word is « documents » or « fichiers » is a folder question | medium: the mixed tier decision of session 16 | `Que disent mes documents à propos de Jean Dupont ?` reaches `mixed_tier`, `Combien de lignes ?` still asks which workbook | the same, with documents and two workbooks ticked | M |
| **F-14** | KBD-04 | The file name appended by the « which workbook » button, written in a suggestion **or typed by hand** (`Combien de lignes dans factures-fournisseurs-2026.xlsx ?` answers 0) is read as date words | `choices.ts`, `tabular_answer.rs` | pass the workbook as a structured argument, or strip the path and extension from the text the interpreter reads | low | a workbook named `ventes-2025.xlsx` holding rows of 2024 and 2025: `Combien de lignes ?` then the button gives the file's rows, with no `date en 2025` clause | the same | S |
| **F-15** | KBD-14 | A `;` CSV with a title row and no comma is read as one column | `tabular/csv_adapter.rs` | score the delimiters on the lines that have at least two fields | low | a `;` file with a title row, a header and ten rows reads as four columns | drop such a CSV in the Data folder | S |
| **F-16** | KBD-18, KBD-35 | A person's name, an acronym or an **AVS number** can be read as the name of a file: `À qui appartient le numéro AVS 756.3052.5778.43 ?` is answered « no matching file » and `Quels documents concernent HUG ?` lists files | `file_reference.rs`, `folder_questions.rs` | a reference made only of name words with no extension and no separator is a file reference only if a file of that stem exists; a token of digits and dots is not a file name | medium: the router is the first thing a question meets | the sandbox's `identifier` and `organization` questions route to the documents | `Montre la facture FAC-2026-014` and the AVS question reach the documents | M |
| **F-24** | KBD-38 | Swiss amounts (`CHF 1'450.00`) are not numbers to the table engine: a sum or a maximum falls to the model | `tabular/inventory.rs` | add `CHF`, `Fr.`, `SFr.` to the unit symbols and `'` and `’` to the thousands groupings, with the existing convention logic | low to medium: the number reader is shared by every table question | the probe's sum and maximum on `patients-suisse.csv` are computed by the engine (8 941,54 and 2 310,25); the French file unchanged; `tabular_reference` suite green | `Quelle est la somme des montants dans patients-suisse.csv ?` | S |

### Group 4 - the retrieval's own limits (decide with measured recall)

| Fix | Rows | What | Files | Smallest fix | Risk | Test that proves it | Human step | Size |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **F-17** | KBD-19 | A short question that names nobody is narrowed to the previous answer's person | `conversation.rs`, `knowledge/query.rs` | carry names only when the question holds a pronoun or an elision from the pack vocabulary, or when the new question's own retrieval is close to the previous answer's passages | medium: the follow-up rule is shared with retrieval | `Que contiennent les notes internes ?` after a question about Sophie Rochat is not narrowed; `Et son numéro AVS ?` still is | the same two questions in `auto` | M |
| **F-18** | KBD-17, KBD-37 | No absolute score tells a reduced set that holds the answer from one that names the right person and lacks it; a file that names a person only as `M. Dupont` is dropped by a reduction on `Jean Dupont` | `knowledge/targeting.rs` | a relative rule: compare the reduced evidence with a sample of the full selection, or a term-coverage check of the reduced evidence against the question's other words | medium | `lot-06-widen-calibration.py` extended: the three families separate | the question « Le Dr Dupont a-t-il parlé de la photocopieuse ? » widens by itself | M |
| **F-19** | KBD-34 | The release gate measure counts filler as a miss | `knowledge/diagnostics.rs`, the gate text | split `miss_no_mention` into filler and extraction gap by a lexical check of the cited passage against the question's forms; amend the gate text (owner) | low | the diagnostics of the sandbox run: `miss_no_mention` falls to the real gaps | `lot11_helper.py report` prints both | S |

### Group 5 - product decisions first

| Fix | Rows | What | Files | Smallest fix | Risk | Test that proves it | Human step | Size |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **F-20** | KBD-30 | ODT, RTF, EML, VTT, ODP, PPTX, PPT documents and ODS tables are listed and never read; the Analyse summary does not say so | `discovery.rs`, `extraction.rs`, `tabular/`, the interface | one extractor per format as plain text (zip and XML for ODT/ODP/PPTX, headers kept for EML, cues for VTT), ODS by `calamine`, and a count of files not read with the reason in the Analyse summary | medium to high: new parsers, the language contract | the sandbox's 21 files of 200 are indexed and a question about a sentence they hold cites them | add an `.odt` and ask | L (one lot per format family; the summary line first, S) |
| **F-21** | KBD-06 | The model's sentence contradicts the engine block on 7 of 14 answers (small models) | `commands.rs` (mixed tier), `prompts/` | show the engine block first and a line saying the computed figure prevails; choose the model that will ship | medium: model-facing text (profession-neutral guard) | the mixed answers of `small-model-comparison-1` re-run with the new layout | the invoices-against-quote question | M |
| **F-22** | KBD-05 | A figure split by a line break draws a false « le chiffre ... ne figure dans aucun des extraits » | `number_check.rs` | join a space-grouped run across a line break before comparing | low | `1 85 03 69` + newline + `123 456 78` reads as one number | ask for a number the document writes on two lines | S |
| **F-23** | KBD-02 | `mars/neurologie.pdf` (a folder and a file) is not a reference | `file_reference.rs` | accept a trailing folder suffix when it is unique | low | the three spellings of the same file answer alike | `Que dit mars/neurologie.pdf ?` | S |

Rows closed on the way: KBD-07 and KBD-09 (CI green, PR #23), KBD-08 (thinking switch and empty answer), KBD-10 (empty keys), KBD-15
(surname then initial, PR #30). KBD-03 is not a defect. **Not fixed and not proposed: KBD-13 is inside F-10.** New rows from
the owner's run of this lot's human test are added to the register as she reports them and join this battery.

Suggested order: **F-01, F-02, F-03, F-11, F-16, F-24, F-04, F-10, F-22, F-23, F-14, F-15** (cheap and harmful), then **F-06 with F-07 and F-08**
(the gain for the base itself: this is where recall comes from), **F-09, F-12, F-13, F-16**, then **F-17, F-18, F-19**, and **F-20, F-21**
after the owner's decisions.

## 10. Limitations

**False negatives** (a file that holds the answer is not searched in `Automatique`), by cause, measured at 200 documents (58 lost files in 13 questions of 115 narrowed):

| Cause | Lost files | Row |
| --- | --- | --- |
| The cap of 15 candidates cuts a list-type question (a place, an organisation, a subject) | 33 | KBD-29, F-03 |
| A short question carries the previous answer's person | 7 | KBD-19, F-17 |
| The file never mentions the name the extractor reads (`Dre`, a hospital with no marker) | 11 | KBD-26, KBD-27 |
| The file names the person only as `M. Dupont` | 7 | KBD-37, F-18 |
| A scan whose name the text recognition misread | `not_linked_to_the_name`, 15 on the real stack | OCR, `measurements/ocr.md` |

**False positives** (a useless file searched): filler is the rule, not the exception (the base-off search always cites six passages), and the base keeps a few more than it needs (22 to 40 filler files kept against 231 to 365 dropped, real stack).
A name that two people share is asked about, never guessed (13 ambiguous questions of 205 at 200 documents).

**Identity**: two people with swapped given name and surname are one entity (KBD-25); a surname with an apostrophe becomes three entities (KBD-36); `CSS Assurance` is filed as a person (KBD-27), `Dupont Immobilier` too (KBD-28).
In the measures no identity error made a narrowed search lose the right person's files through the entity alone: the cases keep them through the lexical net. They are wrong in the picker and in the dialog.

**Performance**: the lookup is milliseconds; the Analyse is the embedding, not the base; a search over hundreds of explicitly ticked files is quadratic (KBD-16, 320 ms at 878 files); a hundred-page document costs minutes to embed.

**What the base cannot do by design**: read a name in a language or a script no pack knows; know a person from a surname alone; learn from a question; decide a number (the table engine is the authority, I5); help when the user names a file (the router answers).

**What the product does around it, found by this lot** (not the base's, recorded because the user meets them): seven document formats listed and never read (KBD-30); an AVS number read as a file name (KBD-35); Swiss amounts not read as numbers (KBD-38); a typed workbook name with a year answers `0` (KBD-04);
a title shared by several cells answers `0` (KBD-24).

## 11. Future improvements, only as options the data supports

1. **Read the names the extractor misses** (fix battery group 2). The measured gain is the one that matters: the recall that stops at 91.5 % comes from names the extractor does not read, not from the reduction rule. Data supports it; it is data and a few rules, not a model.
2. **A local named-entity model behind `EntityExtractor`** (spaCy, a small multilingual NER). Not supported yet: the missed names are patterns and lists (a title, a marker word, a line that is only a name), cheaper to add than a model. Revisit when group 2 is done and the owner's library shows a residue.
3. **A relative widening rule** (KBD-17): compare the reduced evidence with a sample of the full selection, or check that the question's other words occur in it. Supported by the measure (31 of 112 widened by themselves, 7 relevant files lost to the carry-over).
4. **Entity embeddings**: not supported. Nothing measured fails for want of a semantic match between names.
5. **Richer relations** (who writes to whom, which policy covers whom): not measured; the 102 relations read from the tables are used by nothing in the product yet.
6. **A dated timeline on the entity sheet**: a product wish, not a measure.
7. **User-initiated export and import of manual knowledge between workstations**: supported by the multi-cabinet vision (`docs/DEPLOYMENT.md`), not by this lot.

## 12. Verification and reproduction

Run by the agent on 12 October 2026, Windows 11, Rust 1.98.1; **nothing in this list is part of the owner's script**.

| Check | Result |
| --- | --- |
| `cargo test` (every target, debug) | **1 248 passed, 0 failed, 35 ignored**, 41 targets |
| `cargo clippy --all-targets` | no warning from the lot's files; the 21 warnings of `src/` and the existing tests were there before |
| `rustfmt --edition 2021 --check` on the four validation files and `tests/common/validation.rs` | clean (never a plain `cargo fmt`: `main` is not format-clean) |
| `pnpm run test` (vitest, with the language guard on `src-tauri/src`) | **622 passed**, 39 files |
| `pnpm run build` | built |
| Determinism of the generator | two runs give the same bytes; the first N documents of a larger library are the N documents of a smaller one; the only difference with the library walked in the first half of the lot is one question added at the end (Q205) |
| macOS | **not run** (no macOS machine); the lot adds no `cfg` and no platform crate, and its tests write only to temporary folders |

To reproduce: `python scripts/gen-knowledge-sandbox.py` (200 documents, versioned) or `--documents 1000 --extra-rows 40 --out data/knowledge-sandbox-1000`;
`cargo test --release --test knowledge_validation report_quality_and_recall -- --ignored --nocapture` with `ACAI_TIERS` and `ACAI_SANDBOX_DIR`;
`cargo test --release --test knowledge_validation_bench -- --ignored --nocapture`; the real stack (`docker compose up -d`):
`ACAI_TIERS=30,100,200 cargo test --release --test knowledge_validation_real real_run -- --ignored --nocapture` (about 17 minutes, mostly the embedding of two long documents), `ACAI_GENERATE=8` for the generated answers;
`ocr_on_every_scan` for the text recognition. `docs/OPERATIONS.md`, « Running the knowledge-base validation », lists the variables and the files each run writes.
