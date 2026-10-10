# Lot 3 - Ports, packs and resolver

10 October 2026 · branch `feat/kb-ports-and-packs` · based on `kb/integration` at `7018a33` (tag `kb-after-lot-02-bis` plus the gateway fix, PR #25) · tab "KB 3 - Ports, packs, resolver"

Written for the developer or agent of a later lot, and for the owner. English.

## Summary

The knowledge base now has its three replaceable seams and the data they read. `knowledge::packs` loads and validates the lexicon packs (a neutral base plus `health`, `legal` and `accounting`, each in French and English), `knowledge::extract` defines the extraction port with a composite that merges several extractors, and `knowledge::resolve` holds the deterministic resolver that implements the table of master section 7, the aliases an entity is born with and two lookups (one over the real store, one in memory for tests). Nothing reads a document or a table yet and nothing is visible in the application: lot 4 is the first caller.

## What changed

**Code (Rust, `src-tauri/src/knowledge/`)**

- `packs.rs` - new. The pack format, strict loader, `PackSet` (merged titles, particles, stop-words, organisation markers, identifier schemes with compiled patterns, column rules, relations, role labels, detection terms), locale fallback, fingerprint. 18 unit tests.
- `resolve.rs` - new. `EntityResolver` / `DeterministicResolver`, `EntityLookup`, `StoreLookup` (over `store`), `InMemoryLookup` (fake for tests and dry runs), `SurfaceInput`, `ResolveContext`, `Resolution`, `aliases_for`, `alias_phonetic_key`, the named thresholds. 11 unit tests.
- `extract/mod.rs` - new. `EntityExtractor`, `CompositeExtractor`, the inputs (`DocumentInput`, `WorkbookInput`), `ExtractContext`, `Candidate`, `ExtractedKnowledge`. 9 unit tests.
- `gazetteer.rs` - new. The in-memory list of names, leftmost-longest matching up to five words, incremental `insert`, `epoch` mirrored in `kb_meta`. 7 unit tests.
- `maintenance.rs` - new. `ensure_phonetic_keys` (recompute every alias key in one transaction when the encoder's signature changed) and `record_packs`. 7 unit tests.
- `mod.rs` - the five `pub mod` lines; `EntityTypeId`, `RoleId`, `Confidence`, `Method`; `AliasKind::is_ambiguous_by_nature`.
- `store.rs` - additive reads the resolver needs: `find_entities_by_alias`, `find_aliases_by_phonetic`, `find_aliases_with_word`, `find_entities_by_identifier`, `name_list_rows`, `alias_phonetic_rows` (kind included), `set_alias_phonetic_key`. Existing functions untouched.
- `src/error.rs` - `KnowledgePackInvalid { pack, path }`, code `knowledge_pack_invalid`.

**Resources and dependency**

- `resources/knowledge/base/{fr-FR,en-US}.json` and `resources/knowledge/packs/{health,legal,accounting}/{fr-FR,en-US}.json` - 8 files. `resources/README.md` documents the format.
- `Cargo.toml` - `regex` as a direct dependency (see Decisions); `Cargo.lock` gains one line.
- `src/locales/{fr-FR,en-US}.json` - the sentence for `knowledge_pack_invalid` and the role labels `knowledge.roles.*` (12 keys per language). No screen shows them yet.

**Tests and fixtures (`src-tauri/tests/`)**

- `knowledge_packs.rs` - new, 18 tests: every pack in every locale alone and together, files match the registry, both languages keep one shape, words-and-phrases-only, role label keys exist in both catalogues, the base is neutral, a domain pack only adds, identifier samples (invented), a social security number split by a line break, the non-ASCII guard for the knowledge sources, a printed summary.
- `knowledge_resolver.rs` - new, 46 tests: one per row of the master table on the in-memory lookup, the same rules through the real store, and `the_resolver_cases_hold`, which runs the 28 rows of the owner-editable `fixtures/knowledge/resolver-cases.json` and prints the table.

**Documentation**

- `docs/DECISIONS.md` ("Settled by KB lot 3"), `docs/ARCHITECTURE.md` ("The knowledge layer and its ports"), `docs/LANGUAGE-AND-LOCALE.md` (consumer 4, lexicon packs), `resources/README.md`, this report, `human-tests/lot-03-ports-and-packs.md`.

`lib.rs`, `commands.rs`, `settings.rs`, `index_store.rs` and `indexing.rs` are **unchanged**.

## Decisions taken

All recorded in `docs/DECISIONS.md`, "Settled by KB lot 3". The ones to know:

| Decision | Reason |
| --- | --- |
| **Minimum word length for "same sound and one edit": 4**, one named constant (`resolve::FUZZY_MIN_TOKEN_LEN`), **not the 6 of master section 7** | Measured and reviewed in lot 2 bis; lots 6 and 9 use 4. The owner can overrule by changing the constant |
| `regex` added as a direct dependency (`default-features = false`, `std`, `perf`, `unicode-perl`, `unicode-case`) | Already in the tree (1.13.1, through `tauri-utils`), so no new code in the build; linear-time, no ReDoS; pure Rust. Size limit on each compiled pattern, and a pattern matching the empty string is refused |
| `weak_titles` and `particles` are fields of the pack format | Lot 2 asked for them (open question 2); without them "Me", "M" and "van" would be Rust literals |
| A domain pack cannot change a type | No field for it; a header mapped to two types by two active packs is refused when the set is assembled |
| Token-set equality is a `PossibleMatch`, never a link | It is signal 6 in the master. The two-word case is covered by the `reordered` alias, which is exact |
| In ingestion a surname or an initial links only to an entity **of the same source**, and only when nothing else in the source answers to it; the caller passes the entities seen so far | "Unique among entities of that source" cannot be decided by the resolver alone. Without the list it declines (an initial becomes a separate candidate, a surname stays unresolved) |
| A candidate entity resolves in query mode with a confidence capped at 0.40 | Under the 0.80 reduction threshold: a guess can rank, never narrow (master section 6, rule 5) |
| `EntityLookup::failed()` (a provided method) | The spec's trait has no error channel; "could not look" must never read as "nothing there". The resolver answers `Unresolved(lookup_failed)` (invariant I4) |
| The phonetic key of an alias is computed by `alias_phonetic_key(display, kind, ...)`: ambiguous titles are switched off for the aliases built from the words of a name | Found by a test: the initial alias `M. Dupont` of Marie Dupont was read as "Monsieur Dupont" and sounded like the bare surname. Creation and recomputation share the function |
| The form without an elided article is an alias of its own | Found by a test: "Hopital Central" did not find "l'Hopital Central" |
| Keys and the packs fingerprint are refreshed by the pass, not on open | The locale belongs to the client; the index does not know it. Lot 4 calls `maintenance::ensure_phonetic_keys` and `record_packs` once before the first file |
| The gazetteer is not persisted, only its epoch | It is cheap to rebuild once per pass; the epoch tells lot 4 which sources were read against an older list |

## Deviations

- **Two files beyond the lot's list:** `gazetteer.rs` and `maintenance.rs`, because the spec's gazetteer and phonetic-maintenance items need a home and `resolve.rs` is already large. The master's architecture sketch has neither file; lots 4 to 6 import them by these names.
- **The master's minimum length (6) is replaced by 4**, as the launcher instructed, and said here and in the final message.
- **`src/error.rs` and the two catalogues changed**, which the launcher's `git add` list does not name. `knowledge_pack_invalid` is the code the spec asks for, and the repository's guard requires a sentence for every code in both languages. The role labels are there because a test proves every pack's `label_key` resolves.
- **"On open" became "once per pass"** for the phonetic recomputation (see Decisions).
- The master says the gazetteer is built from `kb_entities` / `kb_aliases`; it is, through `store::name_list_rows`, which also drops the merged entities' names onto their survivor.
- No discrepancy with the master's "what the code looks like today" for the symbols this lot touches (`IndexStore`, `knowledge::store`, `tabular::question`'s pack loader pattern, the catalogues).

## Invariants

| Invariant | Proof |
| --- | --- |
| I3 - nothing a question produces reaches the store | The resolver never writes: `knowledge_resolver::resolving_changes_nothing_in_the_store` (row counts and integrity before and after, both modes) |
| I4 - a lookup failure falls back | `a_lookup_error_means_unresolved_never_new` (a store whose views are gone), `resolve::tests::a_lookup_that_failed_means_unresolved_not_new` |
| I7 - nothing outside the selection is revealed | `an_ambiguous_answer_never_names_an_entity_outside_the_selection`, `a_surname_is_judged_inside_the_selection_first`, `an_entity_outside_the_selection_is_not_resolved_by_its_full_name`, and the same through the store (`the_store_judges_a_surname_inside_the_selection`). The final named test (`ambiguity_chips_only_offer_entities_present_in_the_selection`) is lot 7's, when chips exist |
| I8, I9 - nothing sent to a model, no model-facing string | No prompt, command, log line or constant read by a model was added. Packs and aliases stay local. No neutrality guard to add |
| I10 - machine codes, no French in `src-tauri` | `pnpm run test` (the Rust source guard scans the five new files); `the_knowledge_sources_hold_no_non_ascii_letter_outside_their_tests`; `a_pack_holds_words_and_phrases_never_a_sentence` (raw pack files); `every_role_label_key_exists_in_both_catalogues`; the catalogue parity test |
| I13 - no `cfg`, no Windows path literal | No `cfg` in the knowledge code; tests build paths from `CARGO_MANIFEST_DIR` with `Path::join`; no platform crate. **Not run on macOS** |
| I1, I2, I5, I6, I11, I14 | Untouched: no scope, no named-file, no engine, no source, no delete or idempotence code in this lot |
| Alias phonetic keys | No test asserts a literal key. Keys are only compared with each other, or with a recomputation of the same function |

## Verification

Windows 11, debug profile, from `apps/desktop/src-tauri` unless stated. Baseline measured on `7018a33` before the first change.

| Command | Baseline | After |
| --- | --- | --- |
| `cargo test` (all targets) | 778 passed, 0 failed, 7 ignored | **894 passed, 0 failed, 7 ignored** (+116: 52 unit tests, 64 in `tests/`) |
| of which `--lib` (CI's command) | 584 | 636 (+52) |
| `cargo test --test knowledge_packs`, `--test knowledge_resolver` | new | 18 and 46 passed |
| `cargo clippy --lib` | 21 warnings | **21**, none in a file of this lot (two were fixed on the way) |
| `rustfmt --edition 2021 --check` on `knowledge/mod.rs` (which formats every module below it), `error.rs`, `tests/knowledge_packs.rs`, `tests/knowledge_resolver.rs` | clean | clean. No plain `cargo fmt` was run |
| `pnpm run test` (from `apps/desktop`) | 336 passed (26 files) | **342 passed** (26 files; +6, the source guard adds one test per new `.rs` file) |
| `pnpm run build` | OK | OK |
| `apps/server` `pytest` | not run | not run: nothing there changed |

Not run: the application window, a gateway outside the in-process fake, macOS.

## Measured

Windows 11, the owner's development PC, debug profile, for orientation only.

| What | Number |
| --- | --- |
| Load and validate the base and the three packs (compiles the 9 patterns), French | about 43 ms per load, average of 20 (`print_the_summary_of_every_pack`). A release build is expected to be several times faster; not measured |
| The 46 resolver tests (11 of them run through a real in-memory database) | under 1.1 s in all |
| The 28-row resolver contract | 0.4 s |

Consequence for lot 6: a question should not load the packs. Build the `PackSet` once (per locale and set of active packs) and keep it in the client's state; only the pass (lot 4) and a settings change need a new one.

## Not done

- No extractor that reads a document or a table (lots 4 and 5): the port, the composite and the inputs are there, and the first implementation will tell whether `DocumentInput` and `WorkbookInput` need widening (the workbook input is text per column; lot 5 may add the typed cells).
- No command, setting, screen or `kb_possible_matches` writer. The resolver returns `PossibleMatch { create_separate }`; the caller creates the row.
- Which packs are active is the caller's parameter (`PackSet::load(locale, &["health"])`). The setting and the activity-profile suggestion are lot 10.
- No organisation-suffix alias ("Dupont SARL" also answering to "Dupont"), no match on the compact form ("du pont" / "dupont"): neither is in master section 7.
- The resolver does not use a title to rank a man against a woman (`Rene` / `Reine`): the question is still open (lot 2 bis, open question 3). They are a possible match, never a link.
- `title_spoken` is loaded and queryable (`PackSet::title_spoken`) but nothing serves it: lot 9's surface API.
- Nothing on macOS.

## Documentation

`docs/DECISIONS.md`, `docs/ARCHITECTURE.md`, `docs/LANGUAGE-AND-LOCALE.md`, `apps/desktop/src-tauri/resources/README.md`, this report, the human test. Not touched: `docs/PRIVACY-AND-SECURITY.md` (see open question 6, which lot 4 must settle first), `docs/RETRIEVAL.md` (lot 6).

## Human test

`docs/test-reports/knowledge-base-pass-1/human-tests/lot-03-ports-and-packs.md` - **proposed, not yet run.** Visible in the application: **no.** Part A (about 15 minutes, no application): the printed summary of the packs, breaking a pack on purpose and seeing the test name it, putting a sentence in a list, running the 28-row resolver contract and adding a row of her own, and five rows to judge. Part B (about 10 minutes): the usual smoke test (Analyse, document and data questions, Reset), because the lot adds a dependency, an error code and catalogue entries to a product that must behave as before. The agent ran every command of Part A; it did not open the window.

## Open questions

1. **Confirm 4, not 6,** as the common minimum word length of the fuzzy rule (one constant: `FUZZY_MIN_TOKEN_LEN`).
2. **Ingestion and a surname alone.** `Dr Martin` in a document where no Martin appears records nothing when Martins are known elsewhere. That is the safe choice (no wrong link) and costs recall: a document that only says `Dr Martin` will not be found when she asks about Pierre Martin. The alternative is a low-confidence link when exactly one Martin exists in the whole store. Lot 11's shadow measurement can decide; until then the safe choice stands.
3. **The starting vocabulary** is a first draft by an engineer: the stop-words ("oui", "non", "bon"), the organisation markers, the header words, the French wording of the role labels (`Prestataire` for the neutral `provider`, `Partie adverse`, `Expert-comptable`). She can edit any of it as one line; the test says if she broke something.
4. **A title does not rank sex.** `Rene` / `Reine` stay a possible match. A `Mme` or `M.` in the surface could rank, which is a resolver rule to decide.
5. **`known_shared` pairs and the resolver** (lot 2 bis, open question 3): a man's and a woman's first name that sound alike are offered as "did you mean" in a question. Acceptable as a hint; not changed.
6. **Personal identifiers in `kb_attributes` (for lot 4, before it stores anything).** The master stores `identifier:<scheme>` attributes. For a scheme with `personal_key: true` (an e-mail address, a social security number, an IBAN) that means a directory of such numbers in the local index, which the DPIA draft would have to name. Matching only needs equality, so a keyed hash of the reduced value would do. Recommendation: lot 4 stores a hash for `personal_key` schemes and the value only for the others; decide before the first identifier is written.
7. Defects: **no row added or changed.** Rows re-read: KBD-01 (the lesson is applied: `PackSet::column_rule_for` compares the whole header before splitting it, with a test, for lot 5), KBD-05 (the social security pattern uses `\s` and a test reads a number split by a line break). No row is `blocking`.

## Next

Lots 4 and 5 can start together once this lot is merged; **recommend lot 4 first** if only one chat is open.

- **Lot 4** - `docs/SESSION-KB-LOT-04-Entities-from-documents.md`, tab "KB 4 - Entities from documents", branch `feat/kb-document-ingestion`. What it must know from this lot:
  - Call `maintenance::ensure_phonetic_keys(connection, &packs.title_set(), encoder.as_ref())` and `maintenance::record_packs(connection, &packs)` once, before the first file; build the `Gazetteer` once with `rebuild_from_store`; `bump_epoch` when the pass added names.
  - Per candidate: `DeterministicResolver.resolve(&SurfaceInput, &ResolveContext)` in `ResolveMode::Ingestion`, with `source_entities` = the entities already seen in the same file. Act on the outcome: `Existing` links the mention; `NewEntity` creates the entity with `aliases_for(...)` (give every draft at least one mention); `PossibleMatch { create_separate: true }` creates the entity and a `kb_possible_matches` row with `PossibleReason::as_code()`; `Unresolved` records nothing.
  - `aliases_for` is the only function that should make aliases; it computes the phonetic key with `alias_phonetic_key`.
  - `IdentifierScheme::scan` returns the matches of a pattern with their reduced value; the field `personal_key` is open question 6.
  - The resolver never writes and never blocks: a `LookupFailed` answer means skip the candidate, never fail the document (owner rule of 9 October).
  - Lot 1's note still applies: remove the one remaining rollback with a savepoint, first.
- **Lot 5** - `docs/SESSION-KB-LOT-05-Entities-from-tables.md`, tab "KB 5 - Entities from tables", branch `feat/kb-table-semantics`. `PackSet::column_rule_for(header)` gives the type, subtype, role and confidence of a header; `PackSet::relations()` the role pairs; `Method::ColumnValue` the confidence of a cell. Watch KBD-01.
- Lots 6 and 9 read `resolve::FUZZY_MIN_TOKEN_LEN` (4) rather than writing their own, and `Resolution` / `SourceSet` for the query side.

## Git

```text
git status
git add apps/desktop/src-tauri/src/knowledge apps/desktop/src-tauri/src/error.rs apps/desktop/src-tauri/resources/knowledge apps/desktop/src-tauri/resources/README.md apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.lock apps/desktop/src-tauri/tests apps/desktop/src/locales/fr-FR.json apps/desktop/src/locales/en-US.json docs/DECISIONS.md docs/ARCHITECTURE.md docs/LANGUAGE-AND-LOCALE.md docs/test-reports/knowledge-base-pass-1
git commit -m "feat: add entity extractor and resolver ports with lexicon packs"
git push -u origin feat/kb-ports-and-packs
gh pr create --title "feat: add entity extractor and resolver ports with lexicon packs" --body "Lot 3 of the Knowledge Base programme. Adds the replaceable seams of the knowledge layer and the data they read: lexicon packs (a neutral base plus health, legal and accounting, in French and English, loaded and validated by knowledge::packs, words and patterns only), the extraction port with a composite that merges extractors, the deterministic resolver implementing the resolution table (identifier, canonical name, alias, safe fuzzy as a possible match only, tombstones, ambiguity judged inside the selection first), the aliases an entity is born with, an in-memory gazetteer with an epoch, and the maintenance that recomputes stored phonetic keys when the encoder changes. The minimum word length of the fuzzy rule is one named constant set to 4, from the lot 2 bis measurement. regex becomes a direct dependency (already in the tree). Nothing reads a document or a table yet and nothing changes on screen; one error code and the role labels are added to both catalogues. The resolver contract is an editable JSON file." --base kb/integration
```

After the owner has accepted the human test and CI is green:

```text
gh pr merge feat/kb-ports-and-packs --merge
git switch kb/integration
git pull
git tag kb-after-lot-03
git push origin kb-after-lot-03
```
