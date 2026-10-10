# Lot 2 bis - Real names calibration

9-10 October 2026 · branch `feat/kb-names-calibration` · based on `kb/integration` at `3eeba5c` (lot 2 merged, CI green) · tab "KB 2 bis - Real names calibration"

Written for the developer or agent of a later lot, and for the owner. English.

## Summary

The French key and the English Soundex of lot 2 were confronted with the complete public lists of real names: Insee surnames (218 982) and first names (42 942), US Census surnames (162 253), US SSA first names (105 966). The English Soundex was far too coarse (800 first names under one key, 1 343 masculine/feminine pairs sharing a key) and was replaced by an own rule set, `en-rules` version 1; the French rules got five targeted fixes (`fr-rules` version 2), 75 empty keys were removed, and pairs of opposite-sex first names sharing a key went from 62 to 19 in French and from 1 343 to 9 in English, all of which are real homophones now listed in the contract files. **The owner reviewed the 88 pre-filled rows on 10 October 2026 and agreed with every proposal** (no exception); the rules that her answers contradicted were then fixed (see "Owner's review") and the minimum-length recommendation of 4 stands.

## What changed

**Code (Rust, `src-tauri/src`)**

- `knowledge/phonetic.rs` - `FrenchPhonetic` version 2 (final `ss`, `as`, `id`, `im`/`iam`/`yam`/`ham` sounded, wider feminine marker, final `ay`/`ey`/`oy`, `oel`); `EnglishPhonetic` is now `en-rules` version 1, an own rule set (no Soundex any more, no dependency); `encode_soundex` is gone.
- `knowledge/normalize.rs` - two changes for the empty key (KBD-10): only a one-letter word before an apostrophe is an elided article; a name with nothing else to encode keeps its particles, then its titles in `sounds_like`.

**Tests and fixtures (`src-tauri/tests/`, not scanned by the language guard)**

- `knowledge_names_calibration.rs` - new. The measuring harness: health, gate calibration (pairs and a 2 000-selection simulation), sex rule, misses and hits, names from elsewhere. `committed_samples_hold` always runs on the samples; `full_lists_calibration` runs on the complete lists when present (`data/names/` or `ACAI_NAMES_DIR`) and otherwise prints a notice and passes. Three `#[ignore]` helpers: `write_samples_from_full_lists`, `print_sex_pair_rows`, `print_review_candidates`.
- `knowledge_names.rs` - the contract test now covers both languages (`the_phonetic_contract_holds`, `the_english_phonetic_contract_holds`) and fails when a `known_shared` pair no longer shares a key; new tests for the empty-key guarantee, the endings that tell two people apart, and the English silent letters. `the_english_key_is_a_soundex` became `the_english_key_is_selected_by_the_locale` (it encoded the behaviour this lot deliberately replaces).
- `fixtures/knowledge/phonetic-fr.json` (43 collide groups, 44 differ pairs, 19 known-shared pairs, 4 false friends), new `phonetic-en.json` (37 collide groups, 40 differ pairs, 9 known-shared pairs, 1 false friend); the rows of the owner's review are all in them, and four samples `names-sample-{fr-surnames,fr-first-names,en-surnames,en-first-names}.tsv` (300 names, counts, men and women).

**Scripts and documentation**

- `scripts/fetch_name_lists.py` - downloads the four archives into `data/names/` (git-ignored), safe extraction, prints SHA-256. Python standard library, same on Windows and macOS.
- `docs/DECISIONS.md` ("Settled by KB lot 2 bis"), `docs/DATA-SOURCES.md` (status, encoding finding, hashes), `README.md` ("Sources"), `docs/LANGUAGE-AND-LOCALE.md` (consumer 4), `defects-register.md` (KBD-10), `human-tests/lot-02-bis-names-review.md`.

## Decisions taken

| Decision | Reason | Where recorded |
| --- | --- | --- |
| Replace the English Soundex by `en-rules:1` | Measured: mean 31.7 surnames and 27.5 first names per key, largest group 622 and 829, 1 343 masculine/feminine pairs sharing a key. The launcher asked to replace it if too coarse | `DECISIONS.md` |
| Vowels kept, in classes; `z` = `s`; `d` and `t` kept apart; a leading `h` dropped | Telling "John" from "Joan" and "Jane" matters more than catching every spelling; `d`/`t` merged would link "Tim" and "Dim"; dropping `h` is the French choice the owner already accepted ("Hanna"/"Ana") | `phonetic.rs` header |
| The sex rule is a hard rule with a visible exception list | The launcher says pairs it cannot separate are reported, not hidden. `known_shared` in the contract file lists them, a test fails on an unlisted pair and on a listed pair that no longer collides | `DECISIONS.md`, contract files |
| Sex-rule thresholds: 2 000 bearers (Insee), 20 000 (SSA), 90 % of one sex | Keeps real, frequent, clearly gendered names; 943 + 1 203 French and 735 + 966 English names | `DECISIONS.md` |
| `Anas`/`Elias`, `Hamid`, `Karim`, `Mariam`... rules | The French rule "drop the final consonant" collapsed them onto feminine names (`Ana`, `Elia`, `Amy`, `Karin`, `Marian`); the rules are narrow (an exception list by ending) and each has a test | `phonetic.rs`, `DECISIONS.md` |
| A feminine marker on `-ique`, `-ele`, `-ole`, `-ale`, `-ule`, `-ile`, `-elle`, `-olle`... | `Michel`/`Michele`, `Paul`/`Paule`, `Pascal`/`Pascale`, `Daniel`/`Daniele`, `Frederic`/`Frederique` are the largest violations of the sex rule; `Nicole`/`Nicolle` stay one name | `DECISIONS.md` |
| No empty key: fix in `normalize.rs` | 75 surnames had none; the fix is two lines of the lot 2 file, found by this lot's measures. Listed under deviations | KBD-10 |
| Gate recommendation 4, in the three places | Table below. Confirmed by the owner's review | `DECISIONS.md`, this report |
| No second encoder for names of other origins | Keys are deterministic, ASCII and follow the French reading; usable next to the edit distance | `DECISIONS.md` |
| Samples of 300 names per list are versioned | Smallest set that keeps the always-run test meaningful (health and sex rule on real names) and the citations required by the licences | `DATA-SOURCES.md`, `README.md` |
| Python (standard library) for the download script | The machine has Python 3.13 and no `uv`; the script has no dependency and works on macOS | this report |
| The harness is its own test file, `knowledge_names_calibration.rs` | `knowledge_names.rs` is already the owner's reading table; the launcher's command `cargo test --test knowledge_names -- --nocapture` still prints the phonetic table, and the calibration is `cargo test --release --test knowledge_names_calibration -- --nocapture` | this report |

## Deviations

- **A change in `knowledge/normalize.rs`**, lot 2's file. The launcher scopes the lot to the encoders' rules and fixtures, but the empty key came from `normalize_name` (elision and titles), not from an encoder. The change is small, additive in effect (only names that had no key are affected, plus any word of two or more letters before an apostrophe, which is no longer dropped), and tested.
- **The proxy "likely different people" was dropped** from the usage table. A first version counted a suggestion as wrong when both names had comparable frequencies; reading the pairs it counted (Smith/Smyth, Lefebvre/Lefevre, Gauthier/Gautier) showed it flagged ordinary spelling variants. The number of false suggestions comes from the owner's review instead, and the review is stratified by length to measure exactly that.
- **The usage simulation measures two things**, not the single "suggestions per selection" of the launcher: pairs of the selection that the gate would link to each other ("inside", a resolver ambiguity), and suggestions drawn by 20 mentions of other frequent names per selection ("outside", the false "did you mean" the gate exists to avoid).
- **The review has 88 rows**, not 100, in six tables (the four lists, hits, misses), and the question differs by table (the introduction of the review file says which).
- **Section 4 of the launcher**: the English Soundex was replaced rather than kept, which the launcher allows when the group sizes show it is too coarse.
- **The first rule changes were made before the owner's marks**, on the automatic measures (sex rule, empty keys, group sizes), as the launcher orders; her marks may bump the versions again.

## Invariants

| Invariant | Proof |
| --- | --- |
| I8, I9 - nothing stored or logged, no model-facing string | Pure functions and a test harness; no product code prints a name. The harness prints names, in tests only. No new model-facing constant, so no neutrality guard to add |
| I10 - no non-ASCII letter and no French marker word in `src-tauri/src` | `pnpm run test`: 336 passed. A comment of this lot tripped the guard once (a marker word in a comment) and was reworded |
| I13 - no `cfg`, no Windows path literal | The data folder comes from `ACAI_NAMES_DIR` or `CARGO_MANIFEST_DIR` with `Path::join`; the script uses `pathlib` |
| No real person, no patient data | The lists are public aggregates; the samples are the 300 most frequent names with counts |
| The lot 2 contract keeps passing | Every row of the lot 2 `collide` and `differ` lists still holds; one lot 2 behaviour is deliberately changed (English Soundex) and its test replaced |

## Verification

Run from `apps/desktop/src-tauri` unless stated, on the owner's Windows PC.

| Command | Result | Before the lot |
| --- | --- | --- |
| `cargo test` (all targets, debug) | **777 passed, 0 failed, 7 ignored** (583 lib + 194 in `tests/`), run again in full after the owner's review and the six rule fixes | 771 passed (583 lib + 188), 4 ignored |
| `cargo test --release --test knowledge_names --test knowledge_names_calibration` | 26 + 2 passed (3 helpers ignored) | 22 |
| `cargo test --lib` | 583 passed (CI's command) | 583 |
| `cargo clippy --lib` | 21 warnings, none in `normalize.rs`; one in `phonetic.rs`, `explicit_counter_loop` in `starts_with_at`, code of lot 2 left as it was (the two `chars().last()` comparisons the newer clippy reports, one of them mine, were fixed) | 22 |
| `rustfmt --edition 2021 --check` on the four touched Rust files | clean | |
| `pnpm run test` (from `apps/desktop`) | 336 passed (26 files) | 336 |
| `pnpm run build`, `apps/server` `pytest` | **not run**: no TypeScript and no server file was touched | |

The full-lists test takes 8 seconds in a release build and 46 seconds in a debug build, which is what plain `cargo test` runs; with no lists in `data/names/` it passes at once.

## Measured

Machine: the owner's Windows PC. Lists as of 9 October 2026, hashes in `docs/DATA-SOURCES.md`. Surnames and first names are folded (accents removed, case dropped), so two spellings of one name are one entry; Insee first names are summed over all periods and counts are rounded to 5 by Insee; SSA names are summed over every year 1880-2025. "Top names" are the 5 000 most frequent of a list.

### 1. Health, before and after

| List | Names | Keys before | Keys after | Mean names per key before / after | Largest group before / after | Empty keys before / after |
| --- | --- | --- | --- | --- | --- | --- |
| French surnames | 218 982 | 150 590 | 150 219 | 1.45 / 1.46 | 75 (the empty key) / 43 | 75 / 0 |
| French first names | 42 942 | 25 094 | 25 380 | 1.71 / 1.69 | 43 / 41 | 5 / 0 |
| English surnames | 162 253 | 5 122 | 124 176 | 31.68 / 1.31 | 622 / 22 | 0 / 0 |
| English first names | 105 966 | 3 856 | 58 469 | 27.48 / 1.81 | 829 / 55 | 0 / 0 |

No key has a non-ASCII character; a second run gives the same keys (checked on every name of every list).

Group-size distribution after (keys / names in a group of that size):

| List | 1 | 2 | 3-5 | 6-10 | 11-50 | 51-200 | > 200 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| French surnames | 116 266 | 20 724 | 10 542 | 2 126 | 561 | 0 | 0 |
| French first names | 17 882 | 3 995 | 2 650 | 685 | 168 | 0 | 0 |
| English surnames | 101 228 | 15 574 | 6 361 | 898 | 115 | 0 | 0 |
| English first names | 39 578 | 9 974 | 6 477 | 1 835 | 603 | 2 (109 names) | 0 |

Before, the English lists had 92 keys (26 784 surnames) and 107 keys (34 604 first names) above 200 names. The largest groups after are spelling families, for example French `ALIA` (Alya, Alia, Aaliyah...), `IZAK` (Isaac, Isaak, Izak...), English `ALIA` (Aaliyah, Aliyah, Alia...), `KRISTIN` (Christine, Kristin, Kristine...). The twenty largest groups of each list are printed by the harness.

### 2. Gate calibration - "same key and edit distance 1" by minimum length

Pairs of names at edit distance 1 in the whole list: 676 167 (French surnames), 149 268 (French first names), 569 328 (English surnames), 560 238 (English first names). Of those, the pairs that pass the gate (same key, both words of at least L letters):

| L | French surnames, whole list / top names | French first names | English surnames | English first names |
| --- | --- | --- | --- | --- |
| 3 | 61 451 / 496 | 20 625 / 1 203 | 35 817 / 301 | 58 482 / 1 206 |
| 4 | 59 087 / 476 | 20 005 / 1 143 | 33 665 / 273 | 57 376 / 1 161 |
| 5 | 51 492 / 426 | 16 794 / 907 | 26 571 / 205 | 51 480 / 950 |
| 6 | 37 875 / 296 | 10 693 / 547 | 17 154 / 124 | 37 236 / 591 |

Usage - 2 000 random selections of 30 names drawn by frequency from the 5 000 most frequent (same selections at every L). "Inside" = pairs of the selection that the gate would link; "outside" = suggestions drawn by mentions of frequent names that are not selected (20 mentions per selection), per 100 mentions:

| L | FR surnames inside / outside | FR first names | EN surnames | EN first names |
| --- | --- | --- | --- | --- |
| 3 | 0.043 / 0.28 | 0.043 / 0.24 | 0.024 / 0.17 | 0.053 / 0.36 |
| 4 | 0.043 / 0.28 | 0.042 / 0.23 | 0.022 / 0.16 | 0.044 / 0.32 |
| 5 | 0.041 / 0.26 | 0.030 / 0.20 | 0.021 / 0.15 | 0.028 / 0.26 |
| 6 | 0.035 / 0.21 | 0.026 / 0.17 | 0.011 / 0.09 | 0.018 / 0.16 |

Reading: whatever the length, the gate links about 4 pairs in 100 selections and answers about 1 mention in 350 of a name that is not selected. Going from 4 to 6 letters removes 38 % (French surnames) to 52 % (French first names) of the pairs that exist, and the examples that go with them are real spelling variants: French first names at 4-5 letters `Marc`/`Mark`, `Henri`/`Henry`, `Alain`/`Allain`, `Annie`/`Anie`; English `Cook`/`Cooke`, `Reed`/`Read`, `Sara`/`Sarah`, `Eric`/`Erik`. The most frequent pairs that pass at 4 letters are `Martin`/`Martins`, `Lefebvre`/`Lefevre`, `Bernard`/`Bernhard`, `Thomas`/`Tomas`, `Durand`/`Durant` (French surnames) and `Smith`/`Smyth`, `Brown`/`Browne`, `Davis`/`Davies` (English).

**Recommendation (confirmed by the review, which agreed with the proposals below):** **4 letters in the three places** - master section 7 signal 6 (resolver, now 6), `SESSION-KB-03-targeting.md` (lot 6 fallback, 4) and `SESSION-KB-04-surfaces.md` section 9.2 (lot 9 repair, 4). The cost of going from 6 to 4 is one more "did you mean" in about 100 selections of 30 names (0.035 to 0.043 pairs per selection for French surnames); the benefit is the 40 to 50 % of real variants that 6 would miss. Never 3: a one-letter edit then changes a third of the name and the pairs keep growing (`Lee`/`Le`). The review measured the precision of what the gate suggests on 48 pairs of the most frequent names: 41 were right (85 %), 7 were not, all suffixed or foreign forms of another name (`Georges`/`Georget`, `Nicolas`/`Nicola`, `Martin`/`Martins`, `David`/`Davide`, `Chen`/`Shen`, `Daniel`/`Daniele`, `Rene`/`Reine`). By length, the wrong ones are 2 of 16 at 4 letters, 1 of 16 at 5 and 4 of 16 at 6 or more: a longer minimum does not make a suggestion safer, so it is not a reason to lose the true variants. Of the seven, `Daniel`/`Daniele` was separated by a rule; the other six are listed as `known_shared` or `false_friends` in the contract files.

### 3. Sex rule

Names with at least 2 000 bearers (Insee) or 20 000 (SSA) and at least 90 % of one sex:

| List | Masculine | Feminine | Opposite-sex pairs covered | Sharing a key before | Sharing a key after | In `known_shared` |
| --- | --- | --- | --- | --- | --- | --- |
| French first names | 943 | 1 203 | 1 134 429 | 62 | 19 | 19 |
| English first names | 735 | 966 | 710 010 | 1 343 | 9 | 9 |

What the French fixes separated: `Michel`/`Michele`, `Paul`/`Paule`, `Pascal`/`Pascale`, `Daniel`/`Daniele`, `Raphael`/`Raphaele`, `Noel`/`Noele`, `Adel`/`Adele`, `Frederic`/`Frederique`, `Anas`/`Ana`, `Elias`/`Elia`, `Gildas`/`Gilda`, `Hamid`/`Amy`, `Khalid`/`Calie`, `Ilan`/`Ilham`, `Karim`/`Karin`, `Marian`/`Mariam`. What stays, all real homophones: `Rene`/`Reine` (the accent that tells them apart is folded away on purpose, as file names carry none), `Eddy`/`Heidi`, `Billy`/`Billie`, `Johann`/`Johanne`, `Loris`/`Laurie`, `Anis`/`Annie`, `Laurent`/`Lauren`, `Loup`/`Lou`, `Elie`/`Ellie`, `Mahdi`/`Maddy`, `Neil`/`Nell`... in English `Randy`/`Randi`, `Tony`/`Toni`, `Gene`/`Jean`, `Don`/`Dawn`, `Eli`/`Ellie`. The generated `differ` rows added to the contract files are the pairs that now differ; the pairs that cannot are the `known_shared` rows.

### 4. Misses and hits (top 5 000 names of each list)

| List | Pairs at distance 1 with another key | of which a usual spelling variation (candidate misses) | Same key, distance above 1 (candidate hits) |
| --- | --- | --- | --- |
| French surnames | 4 487 | 123 | 472 |
| French first names | 6 262 | 249 | 869 |
| English surnames | 3 528 | 64 | 147 |
| English first names | 6 896 | 135 | 940 |

Most pairs at distance 1 with another key are simply different names (`Martin`/`Marin`). Selected for the review: one pair per kind of spelling edit among the variants, and the hits by combined frequency (88 rows in all). The two rows where I think a rule is wrong: `Michelle`/`Michele` and `Diane`/`Dianne` (English, both feminine spellings of one name).

### 5. Names from elsewhere (French encoder, Insee list)

Thirty frequent surnames chosen by hand: Benali `BENALI`, Haddad `ADA`, Nguyen `NGIa`, Tran `TRa`, Martins `MARTe`, Ferreira `FERERA`, Pereira `PERERA`, Goncalves `GoKALVE`, Rodrigues `RODRIGE`, Kowalski `KOVALSKI`, Nowak `NOVAK`, Yilmaz `ILMAZ`, Demir `DEMIR`, Kaya `KAIA`, Ozturk `OZTYRK`, Bensaid `BaSED`, Mebarki `MEBARKI`, Khelifi `KELIFI`, Saidi `SEDI`, Diallo `DIALO`, Traore `TRAOR`, Camara `KAMARA`, Toure `TUR`, Sylla `SILA`, Boukhari `BUKARI`, Kone `KON`, and for reference Lefebvre `LEFEVR`, Garcia `GARSIA`, Fernandez `FERNaDE`, Rossi `ROSI`. Usable: deterministic, ASCII, never empty, and the same spelling always gets the same key. Limits: the key follows the French reading (nasal vowels for `an`/`en`, a silent final `d`, `t` or `s`: `Haddad` loses its last `d`, `Tran` becomes a nasal) and a cedilla-less `Goncalves` is read with a `K` where Portuguese says `S`. Good enough as a hint next to the edit distance; no second encoder.

## Owner's review

Run on 10 October 2026: every proposal of the 88 rows accepted, no exception. Her answer made the proposals the reference, so each row became a contract row (a `collide` group for a "yes", a `differ` pair for a "no", `false_friends` for a "no" that the key cannot separate) and the six rows where the encoder disagreed with a proposal were fixed:

| Row | Proposal | What was wrong | Fix |
| --- | --- | --- | --- |
| M16 `Michelle`/`Michele` | same sound | `Michele` had a long `e`, `Michelle` a short one | in English a final `elle` has the same vowel as a final `ele`, and both take the feminine marker |
| M18 `Diane`/`Dianne` | same sound | `Diane` was read with a long `a` | an `a` after another vowel (`Diane`, `Julianne`) is short |
| EF10 `Daniel`/`Daniele` | not the same | they shared a key | the feminine marker also covers `ele` |
| H10 `Henri`/`Emrys` | not the same | `Emrys` was read with a French nasal | an `m` followed by `r` or `l` is never nasal (`Emrys`, `Amrani`, `Imran`) |
| H12 `Mitchell`/`Michel` | not the same | `tch` and `ch` gave the same key | `tch` has its own letter (`C`) in the English key |
| H15 `Tran`/`Trahan` | not the same | the `h` between two vowels was dropped | in English an `h` between two vowels is kept |

The other 82 rows already held. The pairs she judged different that no rule separates (`Martin`/`Martins`, `David`/`Davide`, `Georges`/`Georget`, `Nicolas`/`Nicola`, `Chen`/`Shen`) are in `false_friends`, with a test that fails when a rule finally separates one. `Micheal`/`Michele` left `known_shared` because the new rule separates them. The versions did not change again: no key is stored anywhere yet (lot 3 wires the store), so `fr-rules:2` and `en-rules:1` are the versions the first stored keys will carry.

## Not done

- The constants of the minimum length: recommendation only (4); they stay with lots 3, 6 and 9 and were not changed.
- No "About" page (does not exist; noted in `DATA-SOURCES.md`).
- `pnpm run build` and `pytest` (no TypeScript or server file touched).
- Macroscopic properties of the English rules on a real dictation (what a speech engine writes): the rules were checked on the spelling of names, not on transcripts. That is the speech re-baseline's job.

## Documentation

`docs/DECISIONS.md` ("Settled by KB lot 2 bis"), `docs/DATA-SOURCES.md`, `README.md` ("Sources"), `docs/LANGUAGE-AND-LOCALE.md` (consumer 4), `defects-register.md` (KBD-10, fixed). Local: `docs/SESSION-KB-STATUS.md`.

## Human test

`human-tests/lot-02-bis-names-review.md` - **run**. There is no screen: the review of 88 pre-filled pairs is the human test, plus the command the owner can run to see every table again (`cargo test --release --test knowledge_names_calibration -- --nocapture`; `cargo test --test knowledge_names -- --nocapture` still prints the collision table of the French contract). Status: **run by the owner on 10 October 2026: she agrees with every proposal, no exception.**

## Open questions

1. None left from the review. Questions below are for lots 3, 6, 9.
2. Lots 3, 6 and 9: confirm **4** as the common minimum length once the review is in (the owners of the three constants are those lots).
3. `Rene`/`Reine`, `Billy`/`Billie`... stay as known homophones: is a "did you mean" between a man's and a woman's first name acceptable when the documents give the sex? Today it is a hint, never a merge; the resolver (lot 3) could use a title (`Mme`, `M.`) to rank, which is its decision.
4. Defects: KBD-10 added and fixed in this lot. No `blocking` row.

## Next

- **Lot 3** - `SESSION-KB-LOT-03-Ports-packs-resolver.md`, tab "KB 3 - Ports, packs, resolver", branch `feat/kb-ports-and-packs`, from `kb/integration` after this lot is merged (or beside it, with the rule below). What it must know:
  - The keys changed: `fr-rules:2` and `en-rules:1` are what `kb_meta.phonetic_version` stores; the store recomputes when the signature changes. **No lot 3 test may assert a literal key**; compare two names' keys with each other.
  - `encoder_for_locale` returns the English rule set for `en-*`; `EnglishPhonetic` no longer produces Soundex digits.
  - Minimum length: see the recommendation above; the constant stays lot 3's.
  - `known_shared` pairs (contract files) are the first-name pairs that will always share a key.
- **Speech re-baseline** (`KB-PREREQUISITES.md` P-3, P-7): English repair is possible with `en-rules:1`, same port, same gate, ASCII keys. Candidate pairs for lot 9's `repair-fr.json` (reviewed, to be confirmed by her marks): `Moreau`/`Moro`, `Laurent`/`Lorand`, `Morel`/`Maurel`, `Faure`/`Fort`, `Morin`/`Maurin`, `Marie`/`Mary`, `Jeanne`/`Jane`, `Philippe`/`Filipe`, `Paul`/`Pol`, `Thomas`/`Tomas`, `Leroy`/`Leroi`, `Gauthier`/`Gautier`, `Lefebvre`/`Lefevre`; pairs to be left apart: `Richard`/`Ricard`, `Vidal`/`Vital`, `Henri`/`Emrys`, `Jean`/`Jehan`, `Gay`/`Jay`.

## Git

```text
git status
git add apps/desktop/src-tauri/src/knowledge apps/desktop/src-tauri/tests scripts/fetch_name_lists.py README.md docs
git commit -m "feat: calibrate name keys on public French and English name lists"
git push -u origin feat/kb-names-calibration
gh pr create --title "feat: calibrate name keys on public French and English name lists" --body "Measures the French and English phonetic keys on the complete Insee, US Census and SSA lists of names (harness in tests/knowledge_names_calibration.rs, lists fetched by scripts/fetch_name_lists.py into the git-ignored data/names). Replaces the English Soundex by an own rule set (en-rules:1) and brings fr-rules to version 2 (sounded final s, d and m, wider feminine marker). Removes 75 empty keys (normalize_name no longer drops a long word before an apostrophe and keeps particles and titles when nothing else is left). Opposite-sex first names sharing a key go from 62 to 19 in French and from 1343 to 10 in English, the remainder listed as known homophones in the contract files. Adds small derived samples with their citations and a recommendation for the minimum token length of the fuzzy gate. No schema, no setting, no user-visible change." --base kb/integration
```

After the owner has accepted the review and CI is green:

```text
gh pr merge <number> --merge
git switch kb/integration
git pull
git tag kb-after-lot-02-bis
git push origin kb-after-lot-02-bis
```
