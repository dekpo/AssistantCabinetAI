# TABULAR step 08 - Named file wins (23 October 2026)

Branch `fix/named-file-wins`, from the tip of `feat/tabular-integration`. Decisions: `docs/DECISIONS.md`, "Taken while building TABULAR step 08". Plan: cases H and I of section 1.3 (both marked fixed), sections 4.2 and 4.3.

## What changed

The workbook a question names now decides where a letter is filled from, a merge the program cannot prepare is asked about instead of being written by a model, the sheet a plan was built from is read again by the preview and the generation, and a document named in a mixed selection is searched alone.

| File | Change |
| --- | --- |
| `src-tauri/src/tabular_answer.rs` | `FillLocation::{Found, NoSource, SeveralSources, SheetToChoose}` and `FillSheet`; `locate_fill_source` takes `&NamedWorkbooks`; `open_workbook_for_fill` / `open_for_fill(.., requested)` split ("which sheets" and "open this sheet"); `fill_source_of`, `names_an_identifier`, `sheet_named_in`; `load_fill_table(.., sheet, ..)`; three `TabularAnswer` variants (`FillNeedsTemplate`, `FillNeedsSheet`, `FillSeveralSources { candidates, query }`); `NamedWorkbooks::covers` is public |
| `src-tauri/src/fill_plan.rs` | `FillRequest.sheet`; `decide` and `FillDecision::{Plan, Ask, Editorial}`, the whole outcome of door 1 in one Tauri-free function |
| `src-tauri/src/commands.rs` | `fill_tier` computes `selection_facts` once, locates, decides and traces; `fill_inputs` passes the sheet; `mixed_tier` targets a named document (`DocumentPlan::OneFile`) and sends its folder facts |
| `src-tauri/src/mixed_answer.rs` | `named_document`, `folder_facts_block`, `FOLDER_FACTS_INSTRUCTION`, `MixedContext.folder_facts`, `build_turn_for(.., folder_facts, ..)` |
| `src-tauri/src/workbook_use.rs` | `UseReason::SheetToChoose`; the three outcomes in the `Engine` branch |
| `src/lib/{ipc,tabularAnswer,choices,fill}.ts`, `src/locales/{fr-FR,en-US}.json` | the three union members and `FillSheet`, their sentences, buttons only for one ambiguous name, `sheet` in `FillRequest`, `sheet_to_choose`, the extension is not repeated when a typed stem is replaced by a path |
| `tests/named_file_wins.rs` (new) | 10 tests; the latent sheet bug first (it failed before the fix), the named workbook against every other workbook (one load), two names, several sheets, no template, not ticked, a file written in full, case L, case I |
| existing tests | `locate_fill_source` callers (`fill_plan.rs`, `workbook_use_trace.rs`, `large_test.rs`, `mixed_routing_baseline.rs`); case H flipped; `folder_facts: ""` in the nine `MixedContext` literals |

## The latent sheet bug

Reproduced before the fix by `a_workbook_narrowed_to_one_sheet_is_read_again_from_that_sheet`: a workbook of two sheets narrowed to one by the selection got a plan, then `load_fill_table` (called with no sheet) found two reachable sheets and returned nothing, so the preview and the generation failed with `FillSourceUnavailable`. The plan's `sheet` now travels back in `FillRequest.sheet` and is looked up in the workbook, never trusted.

## Walked on a copy of her index (23 October 2026)

Settings read: Documents `C:\Users\elise\AssistantCabinetAI-test\Docs`, Tables `C:\Users\elise\AssistantCabinetAI\Data` (not the `-test` one), server `http://192.168.1.83:8080`, model `qwen3:1.7b`, `fr-FR`. Her index was copied into the scratchpad and the real code path run on the copy with a throwaway test, deleted afterwards (her folders and her index were only read).

| Question (fr-FR, everything ticked) | Result |
| --- | --- |
| "... pour chaque facture du fichier Test/factures-fournisseurs-2026.xlsx" | plan: `modeles/modele-relance-facture.txt` + `Test/factures-fournisseurs-2026.xlsx`, sheet `Factures`, 8 letters, first date 2026-01-10 (the first version asked "which of two files" because the path is also read as the name two files carry: fixed, item 3 of the DECISIONS) |
| "... du fichier factures-fournisseurs-2026.xlsx" | `FillSeveralSources`, the two files, `query` set |
| "... du fichier factures-test.csv" | `FillNeedsTemplate` (no template fits its columns) |
| "... de Test/factures-fournisseurs-2026.xlsx et de factures-test.csv" | `FillSeveralSources`, the two files, no query |
| "... de revenue_sub_agency.xlsx" | `FillNeedsTemplate` |
| "Ecris un mail a Alpha ... dans factures-test.csv" | `Editorial` (case L) |

Case I, lexical search only (the AI machine was not used): "Que dit le fichier 2026-01-11_devis-pharma-depot.pdf du montant total ?" gave 6 excerpts from at least five different quotes over the whole folder and 1 excerpt, from the named file, with `OneFile`. The `Folder facts` block holds that one file's name, kind and readability and the counts of the folder, no line of its text.

Not walked: any gateway call (the tower was not used). The decisions above make none. Her folders hold no workbook of several sheets: that outcome is proved by `a_named_workbook_with_several_sheets_asks_which_one_and_lists_them_all`.

## Corrections made to later prompts and to the plan

- Prompt 09: a section "What step 08 changed for this step" (`FillLocation`, `decide` and `FillDecision`, where `NeedsDraft` goes, `FillRequest.sheet`, the trace), the read-first list, item 3 (`FillStart` and `decide`), and the real state of her folders in the human test.
- Prompt 10: the merge-request line of the tests (door 1 answers first through `decide`; `Editorial` is the path that reaches the writer).
- Plan section 1.3: cases H and I marked fixed by step 08.
- Prompts 11 and 12 mention step 08 only in passing: read, nothing to change.

## Owner's test of step 08 (23 October 2026), her index, `qwen3:1.7b`

All parts as expected. Part 1: card with `modeles/modele-relance-facture.txt`, data `Test/factures-fournisseurs-2026.xlsx`, 8 letters, date 2026-01-10, other fields left as written, trace « plan prepared », « 14 ticked workbooks were not used ». Part 2: the no-template sentence, no card, no letters. Part 3: the two files with buttons; the button gave the part 1 card with one extension. Part 4: two files listed. Part 5 (3 s): answer from `2026-01-11_devis-pharma-depot.pdf` page 1, the only source. Part 6: 8 then 16 files (`-1.txt`, `-1-2.txt`), `md5sum` of the template and the workbook identical; index and log restored, `Generated` removed by hand.

Script defect (mine): it said nothing is ticked by default and that "everything is used". Nothing is ticked by default; with no tables ticked the question goes to the documents tier (« Aucun fichier nommé ... dans votre dossier de documents »), which is the existing behaviour. The script now says to click « Select. auto » on « Tout » first (step 0.5).
