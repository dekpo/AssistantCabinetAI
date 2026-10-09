# Lot 2 - Names, accents and sounds

9 October 2026 · branch `feat/kb-normalize-phonetic` · based on `kb/integration` at `8af79cc` (lot 1 merged) · tab "KB 2 - Names, accents and sounds"

Written for the developer or agent of a later lot, and for the owner. English.

## Summary

The client now has one place that turns text into comparable forms (`knowledge::normalize`) and a replaceable French phonetic key (`knowledge::phonetic`), both pure functions with no database and no dependency added. `file_reference::fold_text` keeps its path and delegates to the new `fold`; a test proves the answer is unchanged on every file and folder name of the fixture directories. Nothing is visible in the product: lot 3 (resolver, packs) and lot 4 (extraction) are the first callers, so the human test is a developer-level one, the table of names the encoder treats as the same sound, which the owner can edit in a JSON file.

## What changed

**Code (Rust)**

- `src-tauri/src/knowledge/normalize.rs` - new. `fold`; `sound_form` (the word as a phonetic encoder reads it, a cedilla turned into `s`); `TitleSet` (titles, ambiguous titles, particles, stop-words, all passed in as data); `normalize_name` / `normalize_name_with(WeakTitles)` returning `NormalizedName { display, tokens, titles, joined, compact, without_elision, token_set_key, initials, sounds_like }`; `normalize_identifier`; `filename_name_candidates`. 7 unit tests.
- `src-tauri/src/knowledge/phonetic.rs` - new. The `PhoneticEncoder` port (`id`, `version`, `encode_token`, provided `encode_name` and `signature`), `FrenchPhonetic` (`fr-rules` v1), `EnglishPhonetic` (Soundex, `en-soundex` v1), `encoder_for_locale`, `bounded_damerau_levenshtein`. 7 unit tests.
- `src-tauri/src/knowledge/mod.rs` - two lines: `pub mod normalize;` and `pub mod phonetic;` beside the existing `pub mod` lines.
- `src-tauri/src/file_reference.rs` - `fold_text` is now a one-line delegate to `knowledge::normalize::fold`; one new unit test, `fold_text_gives_the_same_output_on_the_names_of_every_fixture_folder`, keeps the old body verbatim and compares.

**Tests and fixtures**

- `src-tauri/tests/knowledge_names.rs` - new, 22 integration tests (normalisation tables, titles and the "Me" trap, particles, initials, identifiers, file-name candidates, the phonetic contract, the printed collision table, the edit distance, idempotence and determinism over 2 000 generated texts). It lives in `tests/`, not in `src/`, because the language guard of the Rust sources forbids a non-ASCII character and the words "le/la/les/des..." anywhere under `src/`, and a test about French accents and particles cannot be written without them.
- `src-tauri/tests/fixtures/knowledge/phonetic-fr.json` - the contract: groups that must share a key (`collide`), pairs that must not (`differ`), names that are only printed (`observe`). Read at run time, so a row added by the owner is tested without a rebuild.
- `src-tauri/tests/fixtures/knowledge/name-words-fr.json` - stands in for a lexicon pack (titles, ambiguous titles, particles, stop-words) until lot 3 ships the real packs.

**Documentation**

- `docs/DECISIONS.md` ("Settled by KB lot 2"), `docs/LANGUAGE-AND-LOCALE.md` ("Consumer 4 - comparing names"), `human-tests/lot-02-normalize-phonetic.md`.

`lib.rs`, `commands.rs`, `settings.rs`, the locale catalogues and `Cargo.toml` are unchanged.

## Decisions taken

| Decision | Reason | Where recorded |
| --- | --- | --- |
| `TitleSet` also carries particles and stop-words (and a list of ambiguous titles) | `normalize_name` needs titles and particles, `filename_name_candidates` needs stop-words; one object keeps both signatures at the two arguments the lot file gives | `DECISIONS.md` |
| "Me" and "M" are titles only at the start of a name, with a capital, followed by a capitalised word; `WeakTitles::{Auto, Always, Never}` lets the extractor overrule | The lot file says the extractor decides and the normaliser takes a flag; `Auto` is the safe default for a caller that cannot see the sentence | `DECISIONS.md` |
| Every non-alphanumeric character separates words, not only the listed ones | "Drop other punctuation" would glue "Dupont,Jean" into one word | `DECISIONS.md` |
| The phonetic alphabet is plain ASCII (upper-case consonants and vowels, lower-case nasal vowels) | The source guard forbids a non-ASCII character in `src-tauri/src`, and an ASCII key is easy to store and index anywhere; the lot file's phonetic symbols (a tilde vowel, a long s) are replaced one for one | `phonetic.rs` header, `DECISIONS.md` |
| `Durand` and `Durant` collide | They are homophones; a collision is only ever a "did you mean?", never a merge. The lot file left the choice open ("decide and record") | `phonetic-fr.json`, `DECISIONS.md` |
| `encode_name` keeps the written order of the words | The lot file says "encode each non-particle token, join with `-`". Order-free comparison is `token_set_key`; lot 3's `reordered` alias gets its own key | `DECISIONS.md` |
| A mute final `e` is dropped and remembered, and keeps the consonant before it pronounced; only the `p` of "Philippe" is the exception | Without it "Martine"/"Simone" would fall onto "Martin"/"Simon", which the lot forbids; "Philippe"/"Filip", which the lot requires, needs the `p` gone | `DECISIONS.md`, comment in `encode_french` |
| "Francois" without a cedilla sounds like "Francois" with one (a rule on `nco` + `oi`) | Every sanitised file name is ASCII, so the name in a document and the name in a file name would otherwise never meet phonetically | `phonetic-fr.json` (`François`/`Francois`), `DECISIONS.md` |
| Final `x` is silent only after `u`, final `z` only through the `ez` rule | "Max" and "Alex" are pronounced; the lot file's list `t d s x z p` was applied where French really drops them | `DECISIONS.md` |
| "Keep a leading letter signal for stability" is read as: a silent final consonant is never stripped below two letters, so the first sound of a key is the first sound of the name | The sentence is ambiguous; this reading guarantees a non-empty key and a stable first character | this report |

## Deviations

- **`TitleSet` is wider than the lot file's `TitleSet`** (see Decisions). Lot 3 should fill all four lists from the packs; the pack format of the lot 3 section already has `titles`, `stop_words` and a place for particles to be added (it lists none: add a `particles` array and a `weak_titles` array to the format, or lot 3 will have to invent them).
- **`NormalizedName` has two fields the lot file does not list**: `without_elision` (the lot file asks for the variant, not where it lives) and `sounds_like` (the words in `sound_form`, which is what the encoder reads so that a cedilla is not lost). `core_tokens` was not added.
- **Phonetic tests are mostly integration tests** in `tests/knowledge_names.rs` rather than unit tests beside the code, because of the source guard (see What changed). Only ASCII, marker-free tests are in `src/`.
- **Master section 5 says `fold_text` "lives" in `normalize.rs` and the old path "re-exports"**: it is a one-line delegating function, not a `pub use`, because `fold_text` is `pub(crate)` and the lot file wants the output proved equal; same effect for every caller.
- **The launcher's `git add` list omits `tests/knowledge_names.rs` and the docs**; the block at the end names them.
- No discrepancy with the master's "what the code looks like today" for the symbols this lot touches (`file_reference::fold_text`, `unicode-normalization` already a dependency).
- Surprise worth remembering: U+02BC (modifier letter apostrophe) is `char::is_alphanumeric()` in Rust, so an apostrophe test must run before the letter test or "d" + U+02BC + "Aubigne" becomes one word. The test `every_kind_of_apostrophe_is_a_separator_and_the_elision_is_kept_apart` guards it.

## Invariants

| Invariant | Proof |
| --- | --- |
| I8, I9 - no model-facing string added, nothing stored or logged | Pure functions; no prompt, log line, command or store call was added. No guard to add: there is no new model-facing constant |
| I10 - no non-ASCII letter and no French marker word in the Rust sources | `pnpm run test`: `src/guards/sources.test.ts` scans the two new files and passes (336 tests) |
| I13 - no `cfg`, no Windows path literal | No `cfg` in the new code; the integration test builds the fixture path from `CARGO_MANIFEST_DIR` with `Path::join`; the `fold_text` test reads fixture directories relative to it |
| Existing behaviour of file-name matching unchanged | `file_reference::tests::fold_text_gives_the_same_output_on_the_names_of_every_fixture_folder` plus every existing `file_reference`, `tabular_*` and `mixed_*` test unchanged and green |

## Verification

Run from `apps/desktop/src-tauri` unless stated. The baseline was taken on the lot branch before any change.

| Command | Result | Before the lot |
| --- | --- | --- |
| `cargo test` (all targets) | **771 passed, 0 failed, 4 ignored** (583 lib + 188 in `tests/`) | 733 passed, 0 failed, 4 ignored (567 lib + 166) |
| `cargo test --lib` (what CI runs) | 583 passed, 0 failed, 2 ignored | 567 passed |
| `cargo test --test knowledge_names` | 22 passed | new |
| `cargo clippy --all-targets` | 22 warnings in the lib, all in files this lot did not touch (`tabular`, `tabular_answer`, ...); **none in `normalize.rs`, `phonetic.rs` or `knowledge_names.rs`**. The four `file_reference.rs` findings are at lines 285-317 (old code). Clippy was not run before the change, so "no new finding" rests on the locations, not on a before/after count | not measured |
| `rustfmt --edition 2021 --check` | `normalize.rs`, `phonetic.rs`, `knowledge_names.rs` formatted; `knowledge/mod.rs` clean; `file_reference.rs` has four formatting differences at lines 791-826 that **were there before** and are not in my edits (not reformatted: `main` is not fmt-clean, so only touched files are formatted and this one is left as is) | |
| `pnpm run test` (from `apps/desktop`) | 336 passed (26 files) | 334 (the source guard adds one test per new `.rs` file) |
| `pnpm run build` | green (the usual chunk-size notice) | |
| `apps/server` `pytest` | not run: not touched | |

Not run: any test of the application window; the dev build (`pnpm tauri dev`) was not started.

## Measured

Nothing measured and no benchmark: the functions are not on any path yet. For orientation only, the 2 000-text property test (normalise, encode, check idempotence) runs in about 0.1 s in a debug build on the owner's Windows PC.

## Not done

- No pack loading, no alias generation, no `kb_meta.phonetic_version` write or recomputation: lot 3. The encoder's `signature()` (`fr-rules:1`) is the string it will store.
- No `title_spoken` (spoken forms for text-to-speech): the pack format of lot 3 owns it.
- No extractor calls these functions yet: lot 4.
- `EnglishPhonetic` is a conventional Soundex checked on the textbook cases (Robert/Rupert, Ashcraft, Tymczak) and one pair; English names were not put through the same contract as the French ones.
- The French rules were checked against the contract table in `phonetic-fr.json` and the 'observe' names the owner can read in the printed table; they were **not** measured against a real list of surnames. Expect corrections once the owner reads the table: that is what the JSON is for.

## Documentation

`docs/DECISIONS.md` (Settled by KB lot 2; Defects found during the Knowledge Base programme), `docs/LANGUAGE-AND-LOCALE.md` (Consumer 4), `docs/ROADMAP.md` (After K-E: the fix battery), `.cursor/rules/kb-programme.mdc` (duty 5, defects), `docs/test-reports/knowledge-base-pass-1/defects-register.md` (new). Local, not versioned: `SESSION-KB-WORKFLOW.md` section 2.4, the master, the launchers of lots 3 to 11, `SESSION-KB-05-validation.md` section 11.10, `SESSION-KB-STATUS.md`. Master section 11 also lists `docs/ARCHITECTURE.md`: not edited here, because the knowledge layer's description belongs to the lot that makes it do something (lot 3).

## Human test

**Run by the owner on 9 October 2026: Part B, the 26 starred steps, reported; Part A still to be confirmed in one line. No regression attributable to the lot; four defects found that exist independently of it (KBD-04 to KBD-07). Results: `human-tests/lot-02-results.md`.** A by-product, the comparison of eight small models on five shared questions, is in `docs/test-reports/small-model-comparison-1/` (best: `ministral-3:3b`).

`human-tests/lot-02-normalize-phonetic.md` - the protocol as proposed. Part A (10 minutes): `cargo test --test knowledge_names -- --nocapture` prints the phonetic collision table; the owner reads it, adds and removes rows of `phonetic-fr.json`, and sees a wrong row fail by name. Part B (60 to 75 minutes, about 30 for the starred steps): a non-regression battery of 41 questions on the owner's two real test folders (8 documents, 5 data files), each with its expected answer. Blocks: D (folder questions, no AI, including the file-name matching that goes through `fold_text`), C (content questions through the model), T (tables, no AI), M (documents and tables together), N (nothing selected). Every deterministic expectation was verified by running the product's own engine and folder router on a copy of her folders (two throwaway integration tests, deleted); the model answers are judged on facts and sources. Results are recorded in `human-tests/lot-02-results.md` from the transcripts she sends. Nothing of lot 2 is visible in the interface.

## Open questions

1. Should `Durand`/`Durant`, `Dubois`/`Duboit`, `Michel`/`Michelle` and `Henri`/`Henry` really be offered as the same sound? They collide today. They are hints only, but every row of `phonetic-fr.json` is the owner's to move to `differ`.
2. Lot 3's pack format needs two arrays the lot file does not list: `particles` and `weak_titles`. Confirm they go in the packs (recommended) rather than in Rust.
3. Names from outside the French rules (Arabic, Vietnamese, Portuguese surnames are common in a French practice) go through the French encoder today and will often get an odd key; the printed table includes a few (`Nguyen`, `Benali`, `Haddad`, `Garcia`). Whether that is acceptable for the pilot, or whether to add a second rule set later, is a product question. **Settled on 9 October 2026 (owner): treated by the inserted lot 2 bis** ("Real names calibration", `docs/DECISIONS.md`, "Names calibrated on real data"), which measures the French and English keys on public lists of real names, includes names of other origins, and may bump the encoders' version.
   The minimum token length for "same key and edit distance <= 1" is **6 characters in master section 7 (signal 6, lot 3) but 4 in lots 6 and 9**; lot 2 bis measures the suggestions per realistic selection and recommends, the constant stays with the lot that owns it. This lot's encoder carries no length filter.
4. **Defects found while preparing the human test and during the owner's replay** (the replay added **KBD-04** the file name appended by the "which workbook?" button read as filter words, **KBD-05** a false number warning on a figure split by a line break, **KBD-06** model sentences that contradict the engine block, **KBD-07** the continuous integration has never been green; all in the register with their effect on the lots to come; KBD-07 is `open` and proposed for a fix branch before lot 3).
   Those found while preparing the human test are recorded in `defects-register.md` (new in this lot, with the owner's rule of 9 October 2026): **KBD-01** (a column whose name contains a "rows" word, `Total_Ligne`, is answered as a row count; `Prix_Unitaire` and `Quantite` of the same file work; confirmed by running the engine; no effect on lots 3 to 5, watch point for lots 5, 6 and 9; `deferred`), **KBD-02** (a partial path `mars/neurologie.pdf` matches no file; by design; no effect; `deferred`, to decide), **KBD-03** (not a defect). None blocks the KB. Lot 11 must propose the fix battery for the `deferred` rows.

## Next

**Lot 2 bis (inserted 9 October 2026, before or beside lot 3)** - `SESSION-KB-LOT-02-bis-Real-names-calibration.md`, tab "KB 2 bis - Real names calibration", branch `feat/kb-names-calibration`. It needs lot 2 merged. It confronts this lot's French key and English Soundex with public lists of real names, measures the length threshold, applies an automatic sex rule and one short owner review, and may bump `signature()` (`fr-rules:2`). Lot 3 may run beside it provided **no lot 3 test asserts a literal phonetic key**.

Lot 3 - `SESSION-KB-LOT-03-Ports-packs-resolver.md`, tab "KB 3 - Ports, packs, resolver", branch `feat/kb-ports-and-packs`. It needs lots 1 **and** 2 merged into `kb/integration`. What it must know from this lot:

- Build the `TitleSet` from the packs with `with_titles`, `with_weak_titles`, `with_particles` and `with_stop_words`; the starting words are in `tests/fixtures/knowledge/name-words-fr.json`.
- `encode_name(&normalize_name(text, &titles))` is the key to store in `kb_aliases.phonetic_key`; `encoder.signature()` is the string for `kb_meta.phonetic_version`, and `encoder_for_locale(locale)` picks the encoder.
- A phonetic match is a `PossibleMatch`, never `Existing` (master section 7, signal 6); the edit-distance helper is `bounded_damerau_levenshtein`.
- `knowledge/mod.rs` now declares `pub mod normalize;` and `pub mod phonetic;` right under `pub mod diagnostics;`.

## Git

```text
git status
git add apps/desktop/src-tauri/src/knowledge apps/desktop/src-tauri/src/file_reference.rs apps/desktop/src-tauri/tests/fixtures/knowledge apps/desktop/src-tauri/tests/knowledge_names.rs .cursor/rules/kb-programme.mdc docs/DECISIONS.md docs/LANGUAGE-AND-LOCALE.md docs/ROADMAP.md docs/test-reports/knowledge-base-pass-1
git commit -m "feat: add name normalisation and a French phonetic key"
git push -u origin feat/kb-normalize-phonetic
gh pr create --title "feat: add name normalisation and a French phonetic key" --body "Adds knowledge::normalize (one place for folding text, normalising names and identifiers, file-name candidates) and knowledge::phonetic (a replaceable phonetic encoder port with a French rule-based key and an English Soundex, plus a bounded edit distance). file_reference::fold_text now delegates to the new fold with unchanged output. Pure functions, no database, no new dependency, nothing user-visible yet. The expected sounds live in tests/fixtures/knowledge/phonetic-fr.json so they can be extended without touching Rust. Lot report and human test under docs/test-reports/knowledge-base-pass-1." --base kb/integration
```

After the owner has accepted the human test and CI is green:

```text
gh pr merge feat/kb-normalize-phonetic --merge
git switch kb/integration
git pull
git tag kb-after-lot-02
git push origin kb-after-lot-02
```
