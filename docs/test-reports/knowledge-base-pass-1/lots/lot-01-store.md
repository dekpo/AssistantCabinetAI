# Lot 1 - Store and migrations

9 October 2026 · branch `feat/kb-store` · based on `kb/integration` at `d215939` (tag `kb-after-lot-00`) · tab "KB 1 - Store and migrations"

Written for the developer or agent of a later lot, and for the owner. English.

## Summary

The `kb_*` tables now exist in `index.sqlite3`, created additively by the index's own migration, and the index's lifecycle (a document analysed or replaced, a document gone from the folder, both Resets) keeps them in step inside the transactions it already opens. A document's chunks and its knowledge are written in one transaction, a delta that cannot be applied rolls the chunks back, and applying the same delta twice changes no row. Nothing is visible in the product: no extractor writes an entity yet (lot 4), no command reads the store (lot 6), so the only things the owner can observe are that her existing index migrates without loss and that the new tables are there and empty of names.

## What changed

**Store (Rust)**

- `src-tauri/src/knowledge/store.rs` - new. The schema (master section 6, 18 tables and views), `ensure_schema`, sources and their lifecycle (`upsert_source`, `remove_sources`, `clear_domain`, `clear_all_automatic`, `clear_everything`, `move_source`), `apply_delta` (a diff, see Decisions), `gc_orphans`, the manual primitives (`create_entity`, `add_alias`, `remove_alias`, `set_attribute`, `remove_attribute`, `add_mention`), `merge_entities` / `unmerge_entity` / `delete_entity` / `restore_entity`, the reads later lots build on (`find_entities_by_normalized`, `find_entities_by_phonetic`, `is_tombstoned`, `prefix_search`, `source_ids_for_scope`, `files_for_entities`, `entity_with_aliases`, `list_entities`, `effective_entity`, `get_entity`, `sources`), `integrity_check` (11 counters) and `table_counts`. 41 unit tests beside it.
- `src-tauri/src/knowledge/mod.rs` - the types the store speaks: `Domain`, `SourceRef`, `Origin`, `EntityStatus`, `AliasKind`, `MentionLocator`, `EntityRef`, the drafts, `KnowledgeDelta` (+ `KnowledgeDelta::empty`), `ApplyOutcome`, `Entity`, `Alias`, `EntityRecord`, `IntegrityReport`, the filters, and the constants `CURRENT_KB_VERSION = 1`, `CURRENT_SCHEMA_VERSION = 1`, `NOT_EXTRACTED = 0`. `pub mod store;` added beside `pub mod diagnostics;`.
- `src-tauri/src/index_store.rs` - `configure_connection` (foreign keys on, 3-second busy timeout, no WAL) called by `open_at`; `migrate` calls `ensure_schema`; `pub(crate)` `connection()` and `transaction()`; `replace_document_with_knowledge` does the real work and `replace_document` delegates with an empty delta; `retain_documents`, `clear`, `retain_tabular_inventories`, `clear_tabular_inventories` remove the matching domain's sources in their transaction (`clear_tabular_inventories` was a single statement; it is a transaction now, so the inventories and the workbooks' knowledge go together); `clear_knowledge(include_manual)`. 3 unit tests.
- `src-tauri/src/error.rs`, `src/locales/{fr-FR,en-US}.json` - `knowledge_unavailable`, `knowledge_entity_not_found`, `knowledge_merge_refused`, with a sentence in both languages (the guard that every machine code has one passes).
- `src-tauri/tests/knowledge_store.rs` - new, 19 integration tests (migration from the old DDL, the lifecycle through a real `IndexStore`, atomicity, the two Resets, a real pass with the fake gateway, locks).

**Human check**

- `docs/test-reports/knowledge-base-pass-1/human-tests/lot-01-store.md` and `show_kb_tables.py` (read-only; counts only; Python standard library).

**Documentation**

- `docs/DECISIONS.md` ("Settled by KB lot 1"), `docs/PRIVACY-AND-SECURITY.md` (the directory-of-names caveat for the DPIA draft, a row in the "who stores what" table, the reset semantics), `docs/OPERATIONS.md` (the `kb_*` tables are in `index.sqlite3`, no `-wal` file).

`lib.rs` and `commands.rs` are **unchanged**: `pub mod knowledge;` already existed, and `reset_index` / `reset_data_index` call `IndexStore::clear` / `clear_tabular_inventories`, which now do the knowledge side themselves in the same transaction.

## Decisions taken

| Decision | Reason | Where recorded |
| --- | --- | --- |
| `replace_document` records the source with `kb_version = 0` (`NOT_EXTRACTED`); an extractor that ran and found nothing records `CURRENT_KB_VERSION` | If the legacy path wrote version 1, a document analysed before lot 4 would look already extracted and the backfill would skip it for ever | `DECISIONS.md`, `KnowledgeDelta::empty` doc |
| Applying a delta is a diff (unchanged rows are not written, differing ones updated in place, absent ones removed), not delete-and-insert | The lot asks that an identical delta change no row; with delete-and-insert the ids and timestamps would differ. Stable ids also matter to the split/undo log of lot 8 | `DECISIONS.md` |
| A mention on a merged entity is matched through the effective entity | Otherwise one merge makes the next pass rewrite every mention of the victim | `store.rs`, test `a_merge_is_a_redirect_and_reads_follow_it` + `applying_the_same_delta_twice_changes_no_row` |
| Merge resolves both ids to their current survivor first; refuses the same entity, different types and tombstones | Resolving first makes a cycle impossible by construction (the "cycle refusal" test is the proof), and keeps chains one hop long | `DECISIONS.md` |
| `delete_entity` keeps aliases and attributes, removes mentions and relations; a merged entity cannot be deleted | A restore must lose nothing the user wrote; deleting a redirect target is ambiguous | `DECISIONS.md` |
| A schema that cannot be completed is `index_unavailable`, not `knowledge_unavailable` | The file is then in a state nobody tested; "the knowledge base is unavailable, your documents are fine" would be untrue | `index_store::migrate` comment |
| The merge/unmerge/delete/restore and manual-write primitives are in lot 1 | Step 5 lists fewer, but the lot's own tests (merge round trips, tombstone, manual survival) cannot be written without them, and lot 8's dialog is then only a wrapper | this report |
| Functions that write several statements take a `&Transaction` | The compiler then refuses a caller that would leave half an operation behind | `store.rs` module doc |

## Deviations

- **Foreign keys were already on.** The master (section 6, step 1) says the index has "no `PRAGMA foreign_keys`" and asks to check that no test depends on keys being off. The code fact is true (no pragma), but `rusqlite`'s bundled SQLite is compiled with foreign keys on by default, so the pragma changed nothing in behaviour on this build and no existing test could break. It is kept, explicit, because the index must not depend on a build flag. The `busy_timeout` is genuinely new (it was 0). The whole existing suite was run with only the two pragmas added, before any other change: 523 + 147 passed as before.
- **`kb_attributes` uniqueness** is `(entity_id, key, value, COALESCE(source_id, 0))`, not the master's `(entity_id, key, value)`. With the master's key, two files stating the same email would share one row owned by the first, and removing that file would silently remove the statement the second still makes.
- **Extra schema, all additive:** indexes on the foreign keys and on `normalized` / `phonetic_key`; `NOT NULL created_at/updated_at`; the two views `kb_effective_entity` and `kb_effective_mentions` (the master said "views or queries"); three triggers that keep `kb_names_fts` in step with `kb_aliases` with `rowid = alias_id` (so a row is found and removed by key, and a cascade delete cleans the name index too).
- **Types that lot 3 will add** (`EntityTypeId`, `RoleId`, `Confidence`, `Method`) are not here: the drafts use `String` for type, role and method and `f32` for confidence, which lot 3 can wrap without a migration.
- The launcher's `git add` list names `commands.rs` and `lib.rs`; neither changed (see above).
- No deviation from the code facts of the master otherwise: `replace_document`'s transaction boundary, `retain_documents`, `clear`, `clear_tabular_inventories`, `clear_tabular_workbooks` and `reset_*` were verified in the code before changing them.

## Invariants

| Invariant | Proof |
| --- | --- |
| I3 - nothing a question produces reaches the store | Structural in this lot: the only writers are the index lifecycle and the manual primitives; no question, answer or history code calls the store. The row-hash tests of I3 belong to lots 4 and 6, when something can be asked |
| I4 - a failure means fall back, never "nothing found" | The store returns machine codes and never panics (`a_locked_database_gives_a_clean_code_and_never_a_panic`, `a_reference_to_nothing_rolls_the_whole_delta_back`); the fallback itself is lot 6 |
| I6 - the same relative path in both folders stays two sources | `knowledge::store::tests::the_same_relative_path_in_both_folders_stays_two_sources`, `tests/knowledge_store.rs::the_same_relative_path_in_both_folders_stays_two_sources` |
| I8 - nothing text-like is stored or logged | The tables hold names, locators, counts and codes; `kb_diagnostics` is counts only and unused until lot 6; no log line was added |
| I9, I10 - no model-facing string, machine codes only | No model-facing string added. `src/guards/sources.test.ts` (ASCII, no French marker word) passes on `store.rs` and `mod.rs`, test modules included; three new codes are localised |
| I11 - no destructive action on user files | `tests/knowledge_store.rs::deleting_an_entity_is_a_tombstone_and_touches_no_file_row` (chunk text and `documents`, `chunks`, `chunks_fts`, `tabular_*` counts identical before and after), `clearing_the_cell_cache_is_not_a_knowledge_event`, `the_knowledge_base_can_be_reset_with_or_without_the_users_own_rows` |
| I13 - no `cfg`, no platform crate, no Windows path literal | None added; fixtures use `/` and temp directories. Nothing was run on macOS |
| I14 - idempotence | Precondition: `knowledge::store::tests::applying_the_same_delta_twice_changes_no_row` (row-by-row snapshot of nine tables, timestamps aged first so a refresh would show). End to end: `tests/knowledge_store.rs::a_real_pass_registers_each_document_and_a_second_pass_changes_nothing`. The invariant's own named test (`a_second_pass_over_unchanged_files_writes_nothing`) is lot 4's, when there is something to write |
| Atomicity (master section 10) | `a_failed_write_rolls_back_chunks_and_knowledge_together`, `a_failed_first_write_leaves_neither_chunks_nor_knowledge`, `an_embedding_failure_leaves_no_knowledge_rows_for_that_file` |

## Verification

Windows 11, debug profile. Baseline measured on `d215939` before the first change.

| Command | Baseline | After |
| --- | --- | --- |
| `cargo test` (all targets), `--lib` | 523 passed, 2 ignored | **567 passed**, 2 ignored (+41 `knowledge::store`, +3 `index_store`) |
| `cargo test`, the integration files (19 before, 20 after) | 147 passed, 2 ignored | **166 passed**, 2 ignored (+19 `knowledge_store`). Total 733 passed, 0 failed, 4 ignored |
| `cargo clippy` | 20 warnings in the lib | the same 20; none from the new code or tests |
| `rustfmt --edition 2021 --check` | `index_store.rs` 4 diffs, `error.rs` 0 | the same 4 (all pre-existing: `clear`, `retain_documents`, `retain_tabular_inventories`, one assert); `error.rs`, `knowledge/mod.rs` (which formats `store.rs` and `diagnostics.rs`) and `tests/knowledge_store.rs` clean. No plain `cargo fmt` was run |
| `pnpm run build` | OK | OK |
| `pnpm run test` | 333 passed, 26 files | **334 passed**, 26 files (+1: the language guard on `store.rs`) |
| `apps/server` `pytest` | not run | not run (nothing there changed) |

Not run: the application window (see Human test), a gateway outside the in-process fake, macOS.

The check script was run on a throwaway index built by the real code (not on the owner's index): 18 `kb_*` objects, `kb_entity_types 6`, `kb_source_domains 2`, `kb_meta 5`, `kb_sources 1` with `kb_version=0`, journal mode `delete`, no file beside it, no foreign-key violation.

## Measured

Nothing was timed: the lot adds no per-question work, and per-document work of one source upsert, four small selects (the source's existing mentions, relations, attributes and signals, all empty today) and one garbage-collection query, inside the transaction the document already had. The cost of the knowledge write during Analyse will be visible in the `write_ms` of `analysis-timings.jsonl` once the owner runs the lot 0 baseline; lot 4 is where it can grow.

## Not done

- Nothing reads the store from a command, a setting or a screen (lot 6 onwards). `IndexStore::clear_knowledge` is not wired to a command (lot 8).
- No write path for the **data** domain through the index: `put_tabular_inventory` does not register a source yet. The store functions work for `Domain::Data` (tested through a second connection); lot 5 adds the call beside the inventory write.
- Documents analysed before this build have **no** `kb_sources` row until they change or lot 4's backfill reads them: unchanged files are skipped by `should_skip`. This is by design and is what the human test step B1 shows.
- `split` (homonyms) and the operation-log-driven undo beyond unmerge/restore: lot 8.
- Nothing on macOS. No code is platform-specific.

## Documentation

`docs/DECISIONS.md`, `docs/PRIVACY-AND-SECURITY.md`, `docs/OPERATIONS.md`, this report, the human test and its script. `docs/ARCHITECTURE.md` (ports) and `docs/RETRIEVAL.md` (targeting step) are lots 3 and 6.

## Human test

`docs/test-reports/knowledge-base-pass-1/human-tests/lot-01-store.md` - **proposed, not run**. Visible: **partly** (almost nothing on screen). It migrates the owner's real index after a backup, checks with `show_kb_tables.py` that the four old tables keep their counts and the 18 `kb_*` objects exist empty of names, follows a source through add / edit / delete / Reset on both cards, and ends with the seven-question smoke test. The agent did not open the window and did not touch the owner's index.

## Open questions

1. **Should a knowledge failure fail the document?** Today a database error while applying the delta rolls the document's chunks back (atomic, as the lot demands), so a broken knowledge base could stop Analyse for that file. With an empty delta this is practically unreachable. Lot 4 brings extraction, whose failures (not database failures) should probably skip the knowledge and still index the document. Recommendation: keep it atomic for database errors, make extraction errors non-fatal in lot 4.
2. The folder cards swallow an index that fails to open (`open_index(...).ok()` in `work_folder_inventory` and the data-folder report), so a failed migration would show as "nothing analysed" rather than as an error. Outside this programme's scope; noted in the human test so it is recognised. Worth a one-line fix someday.

## Next

`docs/SESSION-KB-LOT-02-Names-accents-and-sounds.md`, tab "KB 2 - Names, accents and sounds" (branch `feat/kb-normalize-phonetic`), **if lot 2 is not merged yet**; it is independent of this lot. Lot 3 (`SESSION-KB-LOT-03-Ports-packs-resolver.md`, branch `feat/kb-ports-and-packs`) needs lots 1 and 2 both merged. What lot 3 must know: the types live in `knowledge/mod.rs` and it extends them; `knowledge/mod.rs` now has `pub mod store;`, so a branch that added `pub mod normalize;` beside it will conflict on those two adjacent lines (keep both); a `PhoneticEncoder` change recomputes `kb_aliases.phonetic_key` with `store::add_alias` (it refreshes the key of an automatic alias); the resolver's `EntityLookup` can be implemented over `find_entities_by_normalized`, `find_entities_by_phonetic`, `is_tombstoned` and `prefix_search`, which already follow merges and hide tombstones; `kb_meta` holds `phonetic_version`, `packs_hash`, `extractor_version` (empty) and `gazetteer_epoch` (`'0'`), read and written with `store::meta` / `store::set_meta`.

What lot 4 must know: set `KnowledgeDelta::kb_version` (zero means "no extractor ran"); build the delta from resolved entities and call `IndexStore::replace_document_with_knowledge` (it refuses a delta for another file); a draft whose name is a tombstone is dropped with everything that references it and counted in `ApplyOutcome::entities_suppressed`; an entity with no mention is collected at the end of `apply_delta`, so give every draft at least one mention; `should_skip` must also consider `kb_sources` (absent row or `kb_version < CURRENT_KB_VERSION`) for the backfill; side connections need `index_store::configure_connection`.

## Git

```text
git status
git add apps/desktop/src-tauri/src/index_store.rs apps/desktop/src-tauri/src/error.rs apps/desktop/src-tauri/src/knowledge apps/desktop/src-tauri/tests/knowledge_store.rs apps/desktop/src/locales/fr-FR.json apps/desktop/src/locales/en-US.json docs/DECISIONS.md docs/OPERATIONS.md docs/PRIVACY-AND-SECURITY.md docs/test-reports/knowledge-base-pass-1
git commit -m "feat: add the knowledge store schema and lifecycle hooks"
git push -u origin feat/kb-store
gh pr create --title "feat: add the knowledge store schema and lifecycle hooks" --body "Lot 1 of the Knowledge Base programme. Adds the kb_* tables to the existing index.sqlite3 (additive, IF NOT EXISTS, a pre-KB database opens and keeps every row), a knowledge::store module (sources, delta application as a diff so an unchanged file writes nothing, merge as a redirect, deletion as a tombstone, garbage collection, integrity check) and the hooks that keep it in step with the index: a document's chunks and its knowledge are written in one transaction, a vanished document or a Reset removes the matching folder's knowledge in the same transaction. Every index connection now sets foreign keys on and a 3-second busy timeout; no WAL. Three machine codes with French and English sentences. Nothing reads the store yet and no extractor writes to it, so nothing changes on screen. Adds the human test, a read-only script that prints table counts, and the reset semantics in PRIVACY-AND-SECURITY.md." --base kb/integration
```

After the owner has accepted the human test and CI is green:

```text
gh pr merge feat/kb-store --merge
git switch kb/integration
git pull
git tag kb-after-lot-01
git push origin kb-after-lot-01
```
