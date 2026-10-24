# TABULAR step 12: wave 1 closure (24 October 2026)

Branch `chore/tabular-wave1-closure`, from the tip of `feat/tabular-integration` (tag `tabular-after-11`). Documents, plus one owner-ordered fix (S12-1) after her run of the regression script: `tabular_answer::advisory_target` (one condition) and one test in `tests/named_file_wins.rs`.

## What changed

- `docs/DECISIONS.md`: one consolidating section, "Settled by the TABULAR series, wave 1" (rows W-1 to W-10, open items O-1 to O-6), with the names and constants checked against the merged code; rows of earlier step sections made untrue by later steps were corrected in place (step 02 row 9, step 03 row 2 and known limit (a), step 03b rows 1, 13 and 15, step 04 row 14, step 10 known limit on follow-ups); three findings of the walk (S12-1 to S12-3).
- `docs/ARCHITECTURE.md` ("The mixed tier") and `docs/SELECTION-AND-MEMORY.md` ("Documents and tables together"): the doors, the blocks of the turn as built, and the "model never receives a cell value or a row" absolute rewritten as scoped to the data-only paths.
- `docs/PRIVACY-AND-SECURITY.md`: the DPIA paragraph for rows chosen by the engine, the drafted template and the follow-up state; the setting has three values.
- `docs/test-reports/large-test/HUMAN-TEST-results.md`: status lines under H-1 and H-2.
- Not versioned: the regression script `docs/SESSION-TABULAR/reports/wave-1-regression-test-humain.md` (French), plan section 9 (status for waves 2 and 3) and a "Corrected by step 12" block in prompts 13 to 26.

## Findings of the walk (not fixed)

- S12-1 (**fixed after the owner's run, 24 October 2026**): `tabular_answer::advisory_target` returned `None` for a formula or advisory question that writes `Test/factures-fournisseurs-2026.xlsx` in full when `kb-lot05/factures-fournisseurs-2026.xlsx` is also ticked (the bare-name group stays in `named.ambiguous`); door 4 then answers the yes/no `HasFormulas` with no writer. Fix: count an ambiguous group only when none of its files is in `named.exact`, as `locate_fill_source` does. Test `a_workbook_written_in_full_is_advisory_even_when_a_same_named_one_is_ticked`; `rustfmt --check` blocks of `tabular_answer.rs` 47 -> 47.
- S12-2: "La feuille contient-elle des formules ?" gives the sheet names (the word *feuille* wins over *formules*).
- S12-3: `03/2026` and `2026-03` are read as the year; "le 11 mars 2026" as March.

## Walk of the script

Run once on copies (her index and both folders copied to the scratchpad, a throwaway test file under `tests/`, deleted afterwards): tables-only dialogue and follow-ups (2 215, 660, 380, 1, three suppliers, 8, 925, 450, 120, unticked workbook dropped), door 1 (plan from the named workbook, ambiguous name, two names, drafted template), rows chosen (MedSupply 3 of 3, total 1 450; March 3 of 3, total 660; 2024 of `revenue_sub_agency.csv` 16 of 15 958, first row 19898, last 35855), identifier lookup, named document targeted, trace (`14` workbooks not used with 15 ticked), two spellings of a day equal. The steps that need the model (texts, formula notes, explanation, analysis) were checked up to the turn sent to the gateway only.

## Final pass (24 October 2026, once, after the last edit)

- `cargo test -j 2 -- --test-threads=1`: 2 304 passed, 0 failed, 59 ignored (run again after the S12-1 fix).
- `cargo clippy -j 2 --all-targets`: exit 0, 104 warnings, no error; same 104 warnings as before the fix, none in the lines added (pre-existing, not investigated).
- `pnpm run build`: ok. `pnpm run test`: 2 956 passed, 0 failed.

## Prompts 13 to 26

Corrected in place (plan section 9 and a block per prompt): 13 (readers of the typed workbook), 14 (where a glossary meaning is read, carry-over keeps real names), 15 (four more refusal sites), 16 (shares are in characters, setting wins), 17, 18 (carry-over decision per operation), 19 (a profile already exists), 20 (pure modules), 21, 22 (`without_output_folder`), 23, 24, 25 (the replay harness is stale), 26. Read, otherwise nothing to change.

## Owner's run (24 October 2026)

Parts 0 to 5B conform; part R restored settings, journal and index (five fingerprints identical, `Generated` removed). New findings: S12-4 (excerpts unrelated to a writing request: owner decided a step of its own), S12-5 (an invented total passed unflagged once, not investigated), S12-6 (cosmetic). The final pass below was run again after the fix.
