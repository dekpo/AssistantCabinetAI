# 01 - The validation protocol of the Knowledge Base (lot 11)

Written for the owner, who plays the manual part, and for the developer or agent who has to read its results. English;
the French sentences are quoted as what is typed or read on screen.

The protocol answers one question with figures and with real use: **may the intelligent targeting (« Ciblage intelligent »)
become `Automatique` by default?** The owner's own expectation is that it should stay on `Suggestions seulement`; the
protocol is built so that either answer is the result of a measure, not of an impression. It adds no screen and no
function to the application.

## 1. Four kinds of evidence

| Kind | Who | What it proves | Where |
| --- | --- | --- | --- |
| **Automatic, on a library with a ground truth** | the agent | what the base finds, what it merges, what it keeps and drops, never reading outside the selection, time per stage | `apps/desktop/src-tauri/tests/knowledge_validation*.rs`, `fixtures/knowledge-sandbox` (`README.md`) |
| **Real stack** | the agent, on the owner's PC | the same on real embeddings and the real OCR, with the passages the search really cites | `tests/knowledge_validation_real.rs` |
| **The manual script** | the owner | the window: the line under an answer, the button that widens, the buttons and the dialog, the Resets, a folder that grows, files added, changed, deleted, and the answers themselves read by a person | `human-tests/lot-11-validation.md`, run with `human-tests/lot11_helper.py` |
| **Daily use** | the owner | the recall on **her** library: `kb_diagnostics` fills while she works in `Suggestions seulement` | `lot11_helper.py report` |

Nothing the agent ran is part of the script (`AGENTS.md`, "Human tests"); the script holds only what needs a window
and a person.

## 2. What is measured, and how it is read

**For every question, in each of the three modes** (`Désactivé` = `off`, `Suggestions seulement` = `suggest`,
`Automatique` = `auto`): the files the base kept, the passages the search cited, the time of each stage.

- **A relevant file missed** (a false negative): a file that holds the answer and was not searched. This is what the
  release gate protects. A file counts as holding the answer when the generator knows it does (the sandbox) or when the
  owner says so (her library).
- **A useless file added** (a false positive): a file searched or cited that does not hold the answer.
- **The base-off search always cites six passages** (the evidence cap), so on a library of a few dozen files most of what
  it cites for a named person is **filler**. A reduced set drops it on purpose. The raw figure « of the files cited with
  the base off, how many does the reduced set keep » is therefore low by construction and is reported **beside** the
  figure that matters: of the files that hold the answer, how many are kept (defect KBD-34; the gate is read on the
  second, once the owner has accepted that amendment).
- **A wrong confident entity**: the base reduced the search on the wrong person. Target 0.
- **A leak**: a file outside the selection (not ticked, or in the other folder) was read. Target 0.
- **A false merge**: two different people filed as one. Target 0; the sandbox holds pairs that are designed to provoke it.
- **Time**: at question time, the knowledge lookup, the embedding of the question, the search, the first word, the whole
  answer. The generation of the answer is the model's and is the same in the three modes; the gain is read honestly (a
  gain of a few milliseconds on a wait of tens of seconds is no gain).

## 3. The cases (the table of the lot, with where each is proved)

| Group | Case | Proved automatically by | Needs the owner |
| --- | --- | --- | --- |
| Scale | A 10 files, B 20, C **30** (the library that motivated the programme), D 50 | quality and recall at tiers 10 to 50 (`report_quality_and_recall`, `ACAI_TIERS`), the real run at 30 | part A (30 files) |
| Scale | E 100 and more: selected against candidates, search time, size of the context, quality | tiers 100, 200, 500, 1 000 (`knowledge_validation_bench.rs`), the real run at 100 and 200, `report_what_the_product_does_with_each_format` | parts D and Z |
| Entities | the Dupont family resolves correctly; Dr Martin / Dr Pierre Martin never confidently wrong; aliases across documents | `person`, `homonym`, `alias`, `practitioner` questions; the false-merge count; `the_invariants_hold_for_every_sandbox_question_on_thirty_documents` | part A, part B |
| Entities | a data entity (the `Patient` / `Médecin traitant` workbook is recognised and the engine's filtering stays exact) | `report_the_data_questions` (the engine's answer against what the sheet holds); lot 5 tests | part B |
| Entities | mixed: 30 and more documents and several workbooks | `mixed` questions; defects KBD-21 to KBD-24 | part B (one question) |
| Safety | a name written only `M. Dupont` or `Jean-Dupont`; many `Martin` and few `Dr Pierre Martin`; **explicit selection A, B, C with D relevant: D never retrieved**; folder A against folder B: B never used; the widen button restores the old answer; automatic widening on a rare name | `leak` and `small_scope` questions (`leaked == 0` asserted); lot 6 isolation tests | part B |
| Lifecycle | new file; analyse; re-analyse; modify (stale associations gone); delete (the association goes, the others stay); each Reset then re-analyse; folder switch; selection change; **ask several questions then look in the Knowledge dialog: nothing new** | `the_lifecycle_of_the_protocol_leaves_the_base_clean` (every step, `integrity_check` clean), I3 and I14 tests | part C |
| Reliability | embedding timeout (no half-written base); malformed document; malformed workbook; empty file; zero candidates; a failing lookup | lot 1, 4 and 6 tests (`a_knowledge_lookup_failure_falls_back`, `zero_candidates_fall_back_to_the_full_selection`) | not repeated by hand |
| Small scope | 1 to 5 files: same answers, same citations, no visible overhead | `small_scope` questions (never reduced) | part B |
| Conversation | a follow-up after an entity question; documents stay selected; memory unchanged; nothing in the dialog came from the conversation | `follow_up` questions, `the_history_of_a_conversation_does_not_change_the_knowledge_base` | part B |

## 4. How a result is recorded

`02-results.md`, one table per part. Per question: the **test id** (`Q032`, the id of `questions.json`, so the ground
truth is one lookup away), the mode, whether the answer is correct, whether the right sources are cited, whether a
relevant file was missed, whether a useless file was added, the time, a note. The figures of the release gate come from
`python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py report`, which adds up what the application
recorded (numbers and codes, never a question or a name).

## 5. The decision

`03-release-gate.md`: the grid, each condition with its threshold, how it is measured, what was measured and who decides.
`Automatique` becomes the default only if **all** hold; otherwise it stays optional and the report says so.
