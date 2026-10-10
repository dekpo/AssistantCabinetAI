# Lot 4 - Entities from documents

10 October 2026 · branch `feat/kb-document-ingestion` · based on `kb/integration` at `6e459c0` (tag `kb-after-lot-03` plus the owner's answers, PR #27) · tab "KB 4 - Entities from documents"

Written for the developer or agent of a later lot, and for the owner. English.

## Summary

Analysing a document now teaches the knowledge base the people, organisations and identifiers it names: the names in a file's chunks are read, resolved against what the base already holds and written in the same transaction as the chunks, with the chunk each one was found in. Documents analysed before the knowledge base existed (or while it was off) are brought up to date from the text the index already stores, at the end of an Analyse the user starts, with no request to the gateway; the knowledge step can fail without a document ever failing to be indexed; and an e-mail address, a social security number or an IBAN is stored only as a keyed hash. On screen there is one new line under the Documents card's summary; the names themselves are read with a small script until the management dialog of lot 8.

## What changed

**Rust, new modules (`src-tauri/src/knowledge/`)**

- `extract/text.rs` - the `DeterministicTextExtractor`: titles and names, organisation markers, untitled capitalised runs, gazetteer hits, identifiers by the packs' patterns, words of the file name; all word lists from the packs; caps; poor-OCR rule. 30 unit tests.
- `ingest.rs` - `KnowledgeContext` (one per pass: packs, encoder, gazetteer, key, extractor and resolver ports), the delta builder (resolution order, a lookup layer that shows the resolver the entities the document has just drafted, identifiers and their hash), `store_document`, `learn`, the pass tally and `KnowledgeSummary`. 2 unit tests.
- `backfill.rs` - `refresh_documents` (rounds, stop, progress, distinct counts) and `anything_due`.
- `secret.rs` - `IdentifierKey`: per-workstation key file, `HMAC-SHA256`, fingerprint. 9 unit tests.

**Rust, existing files (additive unless said)**

- `knowledge/mod.rs` - `KnowledgeMode`, `KnowledgeWrite`, `KnowledgeSummary`, `PossibleMatchDraft`; `KnowledgeDelta.possible_matches`; `ApplyOutcome.entities_promoted`; two `Method` variants (`organization_prefix`, `coref_in_document`).
- `knowledge/store.rs` - `promote_candidates`, `apply_possible_matches`, `documents_due`, `entity_id_by_name`, `max_entity_id`, `entity_counts_for_sources`; `apply_delta` runs the first two.
- `knowledge/extract/mod.rs` - `ChunkInput.ocr_confidence`, `ExtractedKnowledge.truncated`; the module `text`.
- `knowledge/gazetteer.rs` - `entry_count`; the rebuild skips identifier entities. `normalize.rs` - `is_title`, `is_weak_title`, `is_stop_word`, `fold_word` crate-visible. `resolve.rs` - `InMemoryLookup::starting_at`.
- `index_store.rs` - `replace_document_with_knowledge` now applies the delta **in a savepoint** and returns `KnowledgeWrite`; `replace_document_plain` (mode off); `apply_knowledge_refresh` (stale guard); `chunk_texts_for_document`; `StoredChunkText`, `RefreshWrite`.
- `indexing.rs` - `run_with_knowledge`, `IndexSummary.knowledge`; `run` unchanged in signature and now delegates. `data_folder.rs` - `knowledge: None`.
- `settings.rs` - `knowledge_mode` (default `suggest`). `commands.rs` - the context built per pass.
- `Cargo.toml`, `Cargo.lock` - `hmac` and `getrandom` become direct dependencies.

**Interface (`apps/desktop/src`)**

- `lib/ipc.ts` (types and the two commands), `lib/knowledgeSummary.ts` (the lines, pure), `components/WorkFolderCard.tsx` (adds them to the pass summary), `locales/{fr-FR,en-US}.json` (`analysis.knowledge.*`, 13 keys each). 8 vitest tests.

**Tests**

- `tests/knowledge_documents.rs` - 43 integration tests (+ 3 ignored: the extraction benchmark, a printer of what a pass learns from the pilot's documents, an index writer for the script). `tests/common/knowledge.rs` - the `Lab` and the snapshot helper. `tests/knowledge_store.rs` - two tests rewritten (see Deviations).

**Documentation** - see below. **Human test** - `human-tests/lot-04-document-ingestion.md` and the read-only script `human-tests/show_kb_entities.py`.

## Decisions taken

All recorded in `docs/DECISIONS.md`, "Settled by KB lot 4". The ones to know:

| Decision | Reason |
| --- | --- |
| A knowledge failure is applied in a savepoint and never costs a document | Owner rule of 9 October; it was this lot's first job. Tested by dropping a `kb_` table under a running pass |
| `knowledge_mode = off` writes no `kb_` row (a context of mode `off` takes `replace_document_plain`); `run` with no context keeps writing the lot 1 source row | The owner asked for "no row written" with the switch off; the existing tests need the legacy behaviour of `run` |
| A surname alone creates nobody; "Dr Martin" links only to a person this document names in full | Owner answer 2 of 10 October. Overrides the lot file's "single surname -> candidate" |
| Whole names first, short forms second, through a lookup layer that includes the drafts of the document | The resolver reads the store, and the entities a document creates are not in the store yet; without the layer "Dr Dupont" would create a second Dupont |
| Weak gazetteer hits (a surname, an initial) lose to an untitled capitalised run | Found by a test: "P. Dupont" was eaten by the known surname "Dupont" and the candidate it had just created was collected by the refresh. "Alice Archer" is a new person even where "Archer" is known |
| A capitalised surname is re-cased for a person only | "SARL" lost its capitals in the first run |
| Identifiers are entities, equal by their reduced value; personal ones by `HMAC-SHA256`; the label is the kind and six digits | Owner answer 3. Matching needs equality only; the label tells the reader what kind of identifier it is without the value |
| The epoch maintains itself (packs changed, key changed, list bigger than the count recorded in `kb_meta`) | Lot 5 and lot 8 add names from outside a pass; with this they need no extra call |
| The summary counts identifiers apart from names | An invoice number is not a "personne ou organisation" |
| ~~A start-up refresh~~ **removed on 11 October 2026** | The owner's rule: no costly silent task at launch. The refresh runs only at the end of an Analyse she starts |
| Named constants: 500 names and 5 000 mentions per source; OCR floor to create 0.80; 4 refresh rounds | One place each, tuned in lot 11 |

## Deviations

- **`run_with_knowledge(.., Option<&mut KnowledgeContext>)`**, not `Option<&KnowledgeContext>` as the lot file wrote: the gazetteer grows during the pass, so the context is mutable.
- **Two files beyond the lot's list:** `ingest.rs` and `secret.rs` (the delta builder and the key); `backfill.rs` was listed. `ChunkInput` gained `ocr_confidence` (lot 3's open question on whether the inputs needed widening: they did, for the poor-OCR rule). `DocumentInput` stayed `{source, chunks}`; the file stem is read from `source.relative_path`.
- **`tests/knowledge_store.rs`: two lot 1 tests rewritten.** `a_failed_write_rolls_back_chunks_and_knowledge_together` and `a_failed_first_write_leaves_neither_chunks_nor_knowledge` encoded the all-or-nothing rule the owner replaced on 9 October; they now assert that the chunks are committed, the answer is `Bypassed`, and no knowledge row is left. The lot file's own atomicity test ("an injected store failure rolls back chunks and knowledge together") is therefore replaced, on the owner's later instruction, by "the document is still indexed and the pass reports the partial failure".
- **The end of `index_data_folder` is not wired.** The lot file lists it among the triggers; the data pass adds no name before lot 5, which calls `backfill::refresh_documents` where it raises the epoch.
- **`should_skip` is untouched.** The owner's note said it "must take [a bypassed document] into account"; the due query does it instead (a document without a source row, or with another hash, is due whether or not the file is skipped).
- **`IndexProgress` gained a field** (`reading_names`) and `Settings` gained `knowledge_packs`; neither was in the lot file.
- The lot file says a titled single surname creates a candidate; the owner's answer says it records nothing. The owner's answer wins.
- No discrepancy found with the master's "what the code looks like today" for the symbols touched (`IndexStore`, `replace_document`, `retain_documents`, `indexing::run`, the catalogues, `Settings`).

## Invariants

| Invariant | Proof |
| --- | --- |
| I3 - nothing a question produces reaches the store | Nothing in this lot reads a question. `the_resolver` of lot 3 still never writes; no command added takes a question |
| I4 - a lookup error means fall back | `a_broken_knowledge_table_does_not_stop_a_document_being_indexed` (the pass completes, the document is indexed, `errors: 1`, then repaired and read on the next pass); a failed read makes the whole document a bypass (`DeltaBuilder::place_all`) |
| I6 - the same relative path in both folders | `the_same_relative_path_in_both_folders_stays_two_sources` (a workbook source with the path of a document is left alone by a second pass) |
| I8 - nothing the knowledge base holds reaches a model or a log | No prompt, log line or gateway message was added; `the_knowledge_tables_hold_names_and_never_a_passage`; personal identifiers: `a_personal_identifier_is_stored_as_a_keyed_hash_and_never_as_itself`, `the_key_lives_beside_the_index_and_never_inside_it`; the summary carries no string (`the_summary_reports_counts_and_no_name`, and the TS test) |
| I9 - model-facing strings stay neutral | No model-facing string added; no guard to add |
| I10 - machine codes, no French in `src-tauri` | `pnpm run test` (the source guard scans the four new `.rs` files); the catalogue parity test; the lines are in the catalogues |
| I11 - nothing destructive on files | Nothing here touches a file in either folder; the key file is the only file written, in the application's own folder |
| I13 - no `cfg`, no Windows path literal | No `cfg` in the new code; no file permission set by the product; tests build paths with `Path::join`. **Not run on macOS** |
| I14 - a second pass writes nothing | `a_second_pass_over_unchanged_files_writes_nothing` (full row snapshot of every `kb_` table, ids and timestamps included, over four files with a promotion in them) |
| I5, I7, I12, I1, I2 | Untouched: no scope, no engine, no query code in this lot |

## Verification

Windows 11, debug profile, `kb/integration` plus this lot. Baseline measured before the first change.

| Command | Baseline | After |
| --- | --- | --- |
| `cargo test` (all targets, from `apps/desktop/src-tauri`) | 894 passed, 0 failed, 7 ignored | **986 passed, 0 failed, 10 ignored** (+92: 44 unit tests, 45 in `tests/knowledge_documents.rs`, 3 in `tests/knowledge_packs.rs`). `tests/chat_idle_timeout.rs` (timing) failed once under load while another build ran and passed on three reruns: flaky, not related |
| of which `--lib` (what CI runs) | 636 | 680 |
| `cargo clippy --lib` | 21 warnings (the figure of lot 3's report, not re-run before the first change) | **21** (two new ones were fixed on the way) |
| `rustfmt --edition 2021 --check` | - | clean on `knowledge/mod.rs` (which formats every module under it), `tests/knowledge_documents.rs`, `tests/common/knowledge.rs`, `tests/knowledge_store.rs`; `commands.rs` (11 hunks) and `index_store.rs` (4) had the same hunks before this lot (measured on `HEAD`); no plain `cargo fmt` was run |
| `pnpm run test` (from `apps/desktop`) | 342 passed (26 files), the figure of lot 3's report, not re-run before the first change | **358 passed** (28 files; tests for the summary lines and for the health-module choice, plus one per new `.rs` file for the source guard) |
| `pnpm run build` | OK | OK |
| `apps/server` `pytest` | not run | not run: nothing there changed |

Not run: the application window, a gateway outside the in-process fake, macOS, a release `tauri build`.

## Measured

Windows 11, the owner's development PC. For orientation.

| What | Number | Method |
| --- | --- | --- |
| Extraction and resolution of 1 000 chunks of about 330 characters, 500 distinct names, against a store holding one document | **127 ms**, release | `cargo test --release --test knowledge_documents extraction_cost -- --ignored --nocapture`; `build` only, not the write. Against 0.7 s to embed **one** chunk on the reference CPU: reading names is about 0.02 % of the embedding cost of the same chunks |
| A pass over the pilot's six fictional documents, names only | 39 ms | `elapsed_ms` of the summary, debug profile (`print_what_a_pass_learns`) |
| The 40 integration tests | about 12 s in all | debug profile, most of it the in-process gateway |

What it found in the pilot's documents (`cargo test --test knowledge_documents print_what_a_pass_learns -- --ignored --nocapture`): with the `health` pack on, as the application now runs: 10 names and 2 identifiers (the quote number and the social security number, the latter as a hash), of which 2 names are wrong (`Conseil de l'Ordre` filed as a person, `Neuf Ans`), both candidates (defect KBD-11). Before the pack was on there were 3 wrong names: the CPAM is now an organisation.

## Added after the owner's replay (11 October 2026)

- **The health pack is on by default**: `Settings.knowledge_packs` (default `["health-fr"]`), the pack `health-ch` (AVS / AHV number, Swiss IBAN and phone, as `health_insurance_number`, `iban`, `phone`), and two choices in « Réglages »: « Base de connaissances » and, as one tick per country, « Santé : France » and « Santé : Suisse ». **Reworked twice on the owner's instruction of 11 October 2026.** First, the packs were renamed `health-fr`, `legal-fr`, `accounting-fr` (an older id is read as its `-fr` name). Then, after the replay of B9, the design was corrected: the packs are separate files but **cumulative** (both countries can be on, for a cross-border worker), and **a module never excludes or transforms an entity type** (the titles and organisation markers of every shipped pack are always read, so a CPAM is an organisation whatever is ticked). My first version made Switzerland *replace* France and let `CPAM du Rhone` become a person; that was wrong and is removed. A pack list is read at the next Analyse only.
- **The start-up refresh is gone**; the Analyse progress bar is labelled while documents analysed earlier are read again (`IndexProgress.reading_names`, a field added to the progress event, sent only when true).
- **Extractor fixes** found while preparing the replay: small words in capitals (`DU`) are particles, a capitalised particle is not a name, an organisation prefix marker may be followed directly by a small word (`CPAM du Rhone`).
- **The human test Part B was rewritten** so that nothing is edited by hand: `human-tests/lot04_helper.py` does every file change.

## Not done

- The Data Folder's names (lot 5), any use of the names at question time (lot 6), any screen that lists them (lots 7 and 8).
- No demotion of a promoted candidate (KBD-12).
- No profile detection that proposes a pack by itself (lot 10); the pack choice is manual.
- macOS.

## Documentation

`docs/DECISIONS.md` ("Settled by KB lot 4"), `docs/PRIVACY-AND-SECURITY.md` (the hashed identifiers, what the hash protects and does not, the lifecycle rows), `docs/ARCHITECTURE.md` (the ingestion path), `docs/CLIENT.md` (the new summary lines), `defects-register.md` (KBD-11, KBD-12, a note on KBD-05), this report, the human test and its script. Not touched: `docs/RETRIEVAL.md` and `docs/SELECTION-AND-MEMORY.md` (lot 6), `docs/LANGUAGE-AND-LOCALE.md`.

## Human test

`docs/test-reports/knowledge-base-pass-1/human-tests/lot-04-document-ingestion.md` - **run and accepted by the owner on 11 October 2026** (Part A, Part B and the smoke test of Part C). Replays found: the unusable wording of Part B (rewritten with a helper script), the start-up refresh (removed), the pack naming and the Swiss option (reworked twice, final design above), and defect KBD-13. Findings fixed on the lot branch in a second commit before the merge. The agent ran the whole scenario as automated tests; the owner ran it in the application.

## Open questions

**Answered by the owner on 11 October 2026** (the first replay of the human test, Part A):

1. **Switch the health pack on for the pilot: yes.** It is the default of the new setting `knowledge_packs`. The owner added a binding note: the product also serves **Swiss health professionals** (AVS / AHV number) and the generic term is **health insurance number**. A second note (replay of 11 October): packs are named **`<domain>-<country>`** (`health-fr`, `health-ch`, later `health-eu`, `legal-ch`...) as separate files, cumulative at run time, and **a module never excludes or transforms an entity type**; knowledge providers are the direction. Written in `AGENTS.md` and `docs/DECISIONS.md`, "Knowledge packs are named domain-country".
2. **OCR floor to create a name (0.80): kept**, calibrated in lot 11 unless something forces it earlier. Nothing in lot 4 does.
3. **The start-up refresh is removed.** No silent background task: the relecture runs only inside an Analyse the user starts (a global rule, now in `AGENTS.md`). The two commands, the interface call and the shared state are gone.

Still open:

4. **Candidates in the future picker** (KBD-11): lots 7 and 9 hide candidates by default; should the extractor also type an untitled capitalised run as `term` rather than `person`?
5. **A promoted candidate stays active** when one of its two sources goes (KBD-12): demote on removal, or carry the number of supporting sources (lot 6)?
6. **Recall of untitled names.** A capital at the start of a sentence is not evidence, so `Jean Dupont a telephone.` at the start of a line records him only if the base already knows the name or a title stands before it. Lot 11's measurement will say what that costs on real letters.
7. **A label with six hexadecimal digits** for a personal identifier (`email 90cd1f`) lets two entries be told apart without showing a value. Fine?
8. **The AVS check digit** is not verified (shape only: 756 prefix, 13 digits).

Defect rows added or changed: **KBD-11** (added, deferred), **KBD-12** (added, deferred), **KBD-05** (note only). No row is `blocking`.

## Next

Lot 6 needs lots 4 **and 5** merged.

- **Lot 5** - `docs/SESSION-KB-LOT-05-Entities-from-tables.md`, tab "KB 5 - Entities from tables", branch `feat/kb-table-semantics`. It can start now, from `kb/integration` after this lot is merged (or beside it; shared files: `knowledge/mod.rs`, `store.rs`, `settings.rs`, `lib/ipc.ts`, the catalogues - additive). What it must know from this lot:
  - Reuse `KnowledgeContext` (packs, encoder, gazetteer, key): `KnowledgeContext::for_index`/`standard`, `learn(connection, &delta)` after a write, `settle_epoch(connection)` and `backfill::refresh_documents` after a data pass (that is the call the lot file asks for at the end of `index_data_folder`). A table extractor returns `ExtractedKnowledge`; the placing of candidates (`DeltaBuilder`, private) is document-shaped - lift what you need into a shared function rather than copying it.
  - A relation, an attribute (`identifier:<scheme>`) and a possible match are already carried by `KnowledgeDelta`; `apply_delta` writes them, promotes candidates and records possible matches. A personal identifier column must go through `IdentifierKey::digest` exactly as `ingest.rs` does (`stored_value`, label `kind + 6 hex digits`), and never be written as itself.
  - The gazetteer epoch maintains itself from `kb_meta.gazetteer_entries`: names you add to the store are noticed by the next context; no extra bump is needed.
  - `IndexStore::replace_document_with_knowledge` and `apply_knowledge_refresh` show the savepoint pattern the owner wants for tables (`put_tabular_inventory_with_knowledge`).
  - Source guard: no French word or non-ASCII letter in `src/` (comments included); accents go in `src-tauri/tests/`.
- **Lot 6** (after 4 and 5): `store::source_ids_for_scope`, `ResolveMode::Query`, `Layered` is ingestion-only. A candidate resolves at most 0.40; an `active` entity can be promoted by two sources and is never demoted (KBD-12). `knowledge_mode` is read from `Settings` (`reads_files()`); in `suggest` nothing reduces a scope.

## Git

First commit (`7bb0311`, pull request #28 on `kb/integration`, CI green on the four jobs): the lot as first delivered. Second commit, on the same branch, with the corrections the owner's replay asked for (cumulative modules that never change a type, one tick per country, the label, the docs):

```text
git status
git add AGENTS.md apps/desktop/src-tauri/resources/README.md apps/desktop/src-tauri/src apps/desktop/src-tauri/tests apps/desktop/src docs/DECISIONS.md docs/test-reports/knowledge-base-pass-1
git commit -m "fix: keep health modules cumulative and never change an entity type"
git push
gh pr edit 28 --body "<updated description, see the final message of the lot>"
gh pr checks 28 --watch
```

After CI is green on the new push:

```text
gh pr merge 28 --merge
git switch kb/integration
git pull
git tag kb-after-lot-04
git push origin kb-after-lot-04
```
