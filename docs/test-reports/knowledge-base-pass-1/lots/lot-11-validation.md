# Lot 11 - Prove it: validation, measurement, release gate, documentation

12 October 2026 · branch `feat/kb-validation` · based on `kb/integration` at `5d18c55` (tag `kb-after-lot-09`, lot 10 withdrawn) · tab "KB 11 - Prove it"

Written for the developer or agent who was not in the session, and for the owner. English.

## Summary

Lot 11 adds **no screen, no setting, no command and no line of product code**: it builds a library with a ground truth, the tools that measure the knowledge base against it (names found and merged, files kept and dropped, files never read, time by stage, on fake vectors and on the real
stack with the real text recognition), the script the owner follows in the real application, and the documentation of the whole programme, and it records every defect it found without fixing one. The measure answers the question the programme was built for with an honest « not yet »:
`Automatique` narrows 56 % of the questions of a 200-document library by a median 98 % of the files, never reads outside the selection, but keeps 91.5 to 95.9 % of the files that hold the answer (gate: 98 %), chooses a wrong entity on 3 of 205 questions, and **changes the wait by milliseconds out of seconds**;
the release gate as written cannot be met by any reduction (KBD-34). **`Automatique` stays optional and `Suggestions seulement` stays the default**, as the owner expected, until she has played the script and decided.

## What changed

**The library (versioned) - `fixtures/knowledge-sandbox/` (214 files, 3.6 MB) and its generator**

- `scripts/gen-knowledge-sandbox.py` and `scripts/knowledge_sandbox/{pools,cast,writers,legacy,docs,plan,tables,questions,build}.py` - deterministic generator: a cast of 488 invented people and 51 organisations of France, French-speaking Switzerland and abroad; confusables (the Dupont and Martin families, swapped given name and surname,
  particles, apostrophes, `Dre`); identifiers with valid keys (social security, AVS, IBAN FR and CH, invoice, contract, policy); 16 formats (pdf, docx, txt, md, jpg, png, scanned pdf, and seven the product does not read); scans at three degrees of badness; 115-page and 290-page documents with planted sentences; eight workbooks;
  205 questions with their relevant files, expected entities and risk. Tiers 30, 100, 200, 500, 1 000 are prefixes of one stream.
- `fixtures/knowledge-sandbox/{documents,data}/` plus `{entities,documents,workbooks,questions,manifest}.json` and a `README.md`: the 200-document tier, with the ground truth. `data/knowledge-sandbox-1000` is generated on demand and never versioned.

**The tools (tests, no product code)**

- `apps/desktop/src-tauri/tests/common/validation.rs` (+ one line in `common/mod.rs`) - `Truth`, `Library`, `Stack`, `Real`, `Replay` (the part of `sourced_answer` before the model: inventory, selection, router, knowledge step), formatting helpers.
- `tests/knowledge_validation.rs` - 3 asserted tests (the sandbox matches its manifest; the invariants hold for every question at 30 documents in the three modes; the workbook formats of the sandbox are real files) and 5 reports (`report_quality_and_recall`, `print_what_the_base_holds`, `explain_one_question`, `report_what_the_product_does_with_each_format`).
- `tests/knowledge_validation_bench.rs` - cost and gain from 30 to 1 000 files, fake vectors.
- `tests/knowledge_validation_real.rs` - the real stack: embeddings through the gateway, the bundled Tesseract and pdfium, a few real answers; `real_run`, `ocr_on_every_scan`.
- `tests/knowledge_validation_walk.rs` - the lifecycle of the owner's protocol (asserted: `the_lifecycle_of_the_protocol_leaves_the_base_clean`) and the probes (extractor, folder router, Swiss number and date formats, data questions, Analyse summaries, the lines of the window).

**The owner's script**

- `human-tests/lot-11-validation.md` and `human-tests/lot11_helper.py` (backup, restore, prepare, tier, mode, lifecycle, names, report, clear-log, questions) - Parts 0, A to E, Z, R.

**Documentation**

- `docs/KNOWLEDGE-BASE.md` (new, the canonical spec), `docs/OPERATIONS.md` (« Running the knowledge-base validation »), `docs/DECISIONS.md` (« Settled by KB lot 11 »), pointers in `docs/ARCHITECTURE.md`, `docs/RETRIEVAL.md`, `docs/SELECTION-AND-MEMORY.md`, `docs/PRIVACY-AND-SECURITY.md`, `docs/LANGUAGE-AND-LOCALE.md`, `docs/CLIENT.md`, `docs/ROADMAP.md`.
- `docs/test-reports/knowledge-base-pass-1/`: `01-protocol.md`, `02-results.md` (template, owner), `03-release-gate.md`, `04-final-report.md`, `measurements/` (the raw reports), the register (KBD-25 to KBD-38 and « Notes added by lot 11 »), this report.

## Decisions taken

All in `docs/DECISIONS.md`, « Settled by KB lot 11 ». The ones to know:

| Decision | Reason |
| --- | --- |
| `auto` is **not** made the default; the gate is not met | `03-release-gate.md` |
| The gate is read on the cited files that hold the answer, with the raw figure beside it (**proposed amendment of 11.7 item 2, for the owner**) | the base-off search always cites six passages, so the raw figure is low by construction (KBD-34) |
| Thresholds are not assertions in the tests; the tests assert the invariants only | the owner decides the numbers |
| A fixture is a fictional file; `fixtures/knowledge-sandbox/real/` does not exist | `AGENTS.md`, no real patient data |
| The protocol's Resets come before any file is changed, so the rebuilt list can be compared name for name | found by the walk: after a changed letter the old name stays the canonical one until a rebuild, which is correct but not comparable |
| No fix in this lot: every defect is a register row and a proposal of the post-KB battery | the owner's rule of 9 October |

## Deviations

- The lot file asks for ≥ 98 % recall on the owner's real library over 100 questions: **not done here**, it needs her use (Part E). The sandbox and the real stack give the nearest figures, with the same definition.
- The confusable cases are not hand-made one by one: the generator makes them by the hundred with their ground truth, so a result is a lookup and not a judgement.
- The platform requirement (Windows and macOS): the lot adds no `cfg`; the macOS run is **not done** (no machine). CI does not run the integration targets (proposal P-4).

## Invariants

I1 to I14 are proved by the existing named tests (lots 1 to 9), all green; lot 11 adds the **replay** of I1 (nothing outside the selection searched: 0 in 2 198 replays of the sandbox's questions), of I3 (a question creates nothing: the snapshot of every `kb_` table is equal after questions in the three modes, asserted by `the_lifecycle_of_the_protocol_leaves_the_base_clean`),
of I4 (any doubt is the old path: `no_entity`, `ambiguous`, `named_file`, `small_scope` counted in « Why a question was not reduced ») and of I5 (the table engine's answers are unchanged: `report_the_data_questions`).

## Verification

Windows 11, Rust 1.98.1, 12 October 2026.

| Command | Result |
| --- | --- |
| `cargo test` (every target) | **1 248 passed, 0 failed, 35 ignored**, 41 targets |
| `cargo clippy --all-targets` | nothing from this lot's files |
| `rustfmt --edition 2021 --check` on the validation files | clean |
| `pnpm run test` | **622 passed**, 39 files (includes the language guard on `src-tauri/src`) |
| `pnpm run build` | built |
| The ignored runs | all executed once and their reports kept in `measurements/` |

## Measured

The tables are in `04-final-report.md` (sections 6 and 7) and `03-release-gate.md`; the headline: recall of the files that hold the answer 91.6 / 95.9 / 91.5 % (real stack, 30 / 100 / 200 documents); raw 31.3 / 38.0 / 41.6 %; median reduction 96 to 99 %; lookup 3 ms median, 9 to 24 ms p95 (150 ms allowed at 1 000 files);
the base's share of an Analyse 0.07 to 1.4 %; the answer 10.7 s against 11.1 s; persons found 88.5 to 96.2 %, organisations 60.9 to 66.7 %, 2 false merges.

## Not done

- The owner's parts of the protocol (A to E, Z): they need her window and her days. Everything the agent could observe was run on copies and the expected values are in the script.
- A second embedding model, a second chat model, a macOS run, the 500 and 1 000-document tiers on the real stack (hours of embedding): the 500 and 1 000 tiers were run with fake vectors.
- No fix (by rule). `fixtures/knowledge-sandbox/real/` (the owner's own books) does not exist and will not.

## Documentation

See « What changed ». `docs/KNOWLEDGE-BASE.md` is new and canonical; the other documents carry one pointer each.

## Human test

`docs/test-reports/knowledge-base-pass-1/human-tests/lot-11-validation.md` · status: **proposed**, walked by the agent on temporary copies (helper commands, the real Rust code for the counts, lists and lines, the real text recognition); the window was not seen. The owner's results go in `02-results.md`.

## Open questions

**Answered by the owner on 12 October 2026** (`docs/DECISIONS.md`, « Settled after KB lot 11 »): 1 verdict accepted and gate item 2 rewritten; 2 the battery is applied now, Switzerland first, on new branches (`docs/SESSION-KB-FIX-04-battery.md`); 3 CI runs `cargo test --tests` on Windows and macOS; 4 **`kb/integration` is not merged into `main`**, all branches and tags are kept (`05-branches-and-stages.md`); 5 the Swiss rows come first. The list below is kept as asked.

For the owner:

1. **Accept the verdict** (`auto` stays optional)? And **amend 11.7 item 2** as proposed (KBD-34)?
2. **Order the fix battery** (`04-final-report.md`, section 9, 24 fixes in 5 groups) and cut it into branches from `main`.
3. **CI**: add `cargo test --tests` on both runners (today `cargo test --lib`); 11 minutes on the development PC.
4. **After the merge**: `kb/integration` into `main`? (Her decision; this was the last lot.)
5. The Swiss targets in `AGENTS.md` make KBD-26 (`Dre`), KBD-35 (AVS number read as a file name), KBD-38 (`CHF 1'450.00`) matter more than their S3: say if they come first.

Rows added: **KBD-25 to KBD-38** (14). Rows changed (notes only): KBD-04, KBD-11, KBD-12, KBD-16, KBD-17, KBD-19, KBD-26, KBD-27, KBD-29, KBD-34. **No row is `blocking`.**

## Next

This was the last lot. After the merge and the tag `kb-after-lot-11`, the fix battery is applied in batches on new branches (`docs/SESSION-KB-FIX-04-battery.md`), then the big human test is rewritten. `kb/integration` is **not** merged into `main`.

## Git

```text
git status
git add fixtures/knowledge-sandbox scripts/gen-knowledge-sandbox.py scripts/knowledge_sandbox apps/desktop/src-tauri/tests docs/KNOWLEDGE-BASE.md docs/ARCHITECTURE.md docs/RETRIEVAL.md docs/SELECTION-AND-MEMORY.md docs/PRIVACY-AND-SECURITY.md docs/LANGUAGE-AND-LOCALE.md docs/CLIENT.md docs/OPERATIONS.md docs/DECISIONS.md docs/ROADMAP.md docs/test-reports/knowledge-base-pass-1
git commit -m "test: validate the knowledge base and document it"
git push -u origin feat/kb-validation
gh pr create --title "test: validate the knowledge base and document it" --body "Lot 11: a generated library with ground truth, the measures, the release gate and the final report. No product code." --base kb/integration
```

After the owner has accepted the human test and CI is green:

```text
gh pr merge <number> --merge
git switch kb/integration
git pull
git tag kb-after-lot-11
git push origin kb-after-lot-11
```
