# Human test - lot 1, Store and migrations

Written for the owner, who runs it. English, with the French strings of the interface quoted as data.

| | |
| --- | --- |
| Branch to be on | `feat/kb-store` |
| Build | `cd apps/desktop` then `pnpm tauri dev` |
| Services | `docker compose up -d` from the repository root (gateway, Ollama) with the chat and embedding models loaded, as for any pass |
| Needs on the PC | Python 3 (the check script below uses only its standard library) |
| Duration | About 20 minutes |
| Visible in the application? | **Partly: almost nothing.** The knowledge base is empty and no screen reads it. What you can see is that the application starts on your **existing** index, that Analyse, a question and Reset behave exactly as before, and, with the small script below, that the new `kb_*` tables are there, empty of names, and that the index grew nothing else |

## What this proves, and what it does not

It proves the one risky thing in this lot: **the migration of a real `index.sqlite3`** (your own, with its chunks, its
workbook inventories and its cell cache) opens, loses nothing, and keeps working through Analyse, a question and both
Resets, with the new foreign-key and lock-waiting settings on. The agent proved it with automated tests on a database
built from the old table layout and on a fresh one; **it did not open the application window and did not touch your
real index**, so every step below is yours to confirm.

It does not prove anything about names, targeting or speed: nothing writes an entity yet (lot 4 does). If a step says
"nothing new on screen", that is the expected result, not a gap.

## Preparation

1. Close the application.
2. Back up what the test may alter (PowerShell):

```text
$backup = "$env:USERPROFILE\kb-backup-lot-01"
New-Item -ItemType Directory -Force $backup
Copy-Item "$env:LOCALAPPDATA\com.assistantcabinetai.desktop\index.sqlite3" $backup
Copy-Item "$env:APPDATA\com.assistantcabinetai.desktop\settings.json" $backup
```

   To go back to the state before this test: close the application and copy both files back. The build from `main` ignores the
   `kb_*` tables, so a restored index also works with it.
3. The fictional fixtures of the first human pass, as in lot 0: `docs/test-reports/human-acceptance-pass-1/fixtures/documents`
   in `C:\Users\<you>\AssistantCabinetAI\Docs\Test` and `.../fixtures/data` in `C:\Users\<you>\AssistantCabinetAI\Data\Test`
   (6 documents, 2 workbooks). If your usual index describes other folders, that is fine for steps A and B; the smoke
   test needs the fixtures. Do not use real files.
4. The check script, used four times below (PowerShell, from the repository root):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_tables.py
```

   It opens the index **read-only** and prints table names and row counts only: no file name, no passage, no name.
5. **Before** starting the new build, with the application closed, run the script on the backup (your index as it is now):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_tables.py "$env:USERPROFILE\kb-backup-lot-01\index.sqlite3"
```

   Expected: `No knowledge base table: this index was never opened by a build that has one.` Write down the four counts
   under "The index's own tables" (`documents`, `chunks`, `tabular_inventories`, `tabular_workbooks`): call them
   **D0, C0, I0, W0**.
6. `docker compose up -d`, then `pnpm tauri dev`.

## Part A - the migration

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| A1 | The application window opens. Look at the Documents card and the Data card (they list your analysed files) | Exactly as before: the same files, the same states. **Nothing new on screen** | An error banner (`index_unavailable` would read "L'index local n'a pas pu être ouvert"). **Also a bug: every file suddenly shown as not analysed.** The folder cards swallow an index that fails to open and show "nothing analysed yet", so a failed migration looks like an empty index, not like an error |
| A2 | Close the application. Run the check script (no argument) | `Journal mode: delete; files beside the index: none`. Under "The index's own tables": **D0, C0, I0, W0 unchanged**. Under "The knowledge base": 18 lines `kb_*`; `kb_entity_types 6`, `kb_source_domains 2`, `kb_meta 5`; **every other line 0**. `kb_meta` shows `schema_version '1'`. `Foreign key violations: 0` | A different count for the four index tables (a loss), a `-wal` or `-shm` file, a non-zero line other than those three, fewer than 18 `kb_*` lines. If the script says `No knowledge base table`, the index was simply not opened yet (no folder chosen, or nothing asked): choose the folders, press **Analyser** on the Documents card, close, and run it again. If it still says so, that is a bug |
| A3 | Start the application again (`pnpm tauri dev`), then close it, then run the script again | The same numbers as A2: opening twice changes nothing | Any change |

If A1 shows an error, stop and send the agent the exact text and the script output. Your backup restores everything.

## Part B - the lifecycle, with a visible effect on the counts

Interface in French. Settings: Documents folder `...\Docs\Test`, Data folder `...\Data\Test`. The `kb_sources` line of the script
is what you watch; every row has `kb_version=0` because no extractor exists yet.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| B1 | Start the application. Press **Analyser** on the Documents card | The summary reads as before (files unchanged are skipped; nothing unreadable). Then close and run the script: **`kb_sources` is still 0** for files analysed before this build | An error, or a file listed as unreadable that was fine before |
| B2 | Add one new file to the Documents folder: create `kb-test-note.txt` containing `Note de test fictive. Rendez-vous avec Camille Exemple le 12 mars.` Start the application, press **Analyser** | The summary counts one new file analysed. Close and run the script: `kb_sources` is **1**, `documents kb_version=0  1`; `kb_entities`, `kb_mentions`, `kb_aliases` stay 0; `documents` is D0+1 | `kb_sources` 0 (the new file is not registered) or more than 1; any name stored (`kb_entities` above 0) |
| B3 | Edit `kb-test-note.txt` (change a word), start, **Analyser**, close, run the script | `kb_sources` still **1**, `chunks` changed accordingly, nothing else new | `kb_sources` 2 (a duplicate source for one path) |
| B4 | Delete `kb-test-note.txt` from the folder. Start, **Analyser**. The summary says "document retiré du dossier, oublié de l'analyse" (one). Close, run the script | `kb_sources` back to **0**, `documents` back to D0 | `kb_sources` stays 1 (a vanished file keeps its source) |
| B5 | Start. On the Documents card press **Réinitialiser** and confirm. Close, run the script | `documents` 0, `chunks` 0, `kb_sources` 0, **`tabular_inventories` still I0 and `tabular_workbooks` still W0** (the Data side is untouched); the 18 `kb_*` objects are still there | A `kb_*` object missing, the Data counts changed |
| B6 | Start. **Analyser** on the Documents card again | All the files are analysed again, as in a first pass. Close, run the script: `kb_sources` equals the number of files analysed (D0 if you had the 6 fixtures, plus any other file in the folder), all `kb_version=0` | `kb_sources` different from `documents` |
| B7 | Start. On the Data card press **Réinitialiser** and confirm. Close, run the script | `tabular_inventories` 0, `tabular_workbooks` 0; **`documents`, `chunks` and `kb_sources` unchanged from B6** | The Documents side moved |
| B8 | Start. **Analyser** on the Data card | The 2 workbooks are analysed again. Script: `tabular_inventories` 2. `kb_sources` is **unchanged** (workbooks register in lot 5, not now) | A data source appears, or an error |

## Smoke test - the product as before

Same fixtures, same expected answers as `docs/test-reports/human-acceptance-pass-1/01-protocol.md`. Select the documents
named in each line.

| # | Do | Expected |
| --- | --- | --- |
| S1 | All documents selected. Ask `Combien de fichiers au total ?` | 6, answered "sans l'IA" |
| S2 | All documents selected. Ask `Que dit le courrier de la CPAM concernant la radiation ?` | Hugo Exemple struck off the general scheme from 1 February 2026; the sources list `courrier-cpam-radiation.pdf`, page 1 |
| S3 | All documents selected. Ask `Quelle est la durée du bail du cabinet d'après le contrat ?` | Nine years, 1 April 2024 to 31 March 2033 |
| S4 | Only the invoices workbook selected, no document. Ask `Quel est le maximum de montant ?` | 620 |
| S5 | The quote `devis-imprimante-medsupply.pdf` and the invoices selected. Ask `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans le devis signé ?` | 1 450 (table) against 1 200,00 HT (quote), both sides cited |
| S6 | Only `bail-cabinet-2024.pdf` selected. Ask `Que dit bail-cabinet-2024.pdf ?` | An answer from that file only |
| S7 | Nothing selected. Ask `Rédige un e-mail de relance pour un fournisseur dont la facture n'est pas réglée.` | An answer labelled "Réponse sans vos documents" |

If any answer differs from a run on `main`, that is a regression of this lot: note the question and the answer. Do not
guess at the cause.

## Results table

| Step | Expected | Observed | OK? | Screenshot |
| --- | --- | --- | --- | --- |
| Prep 5 (D0, C0, I0, W0) | counts written down | | | |
| A1 | | | | |
| A2 | | | | |
| A3 | | | | |
| B1 | | | | |
| B2 | | | | |
| B3 | | | | |
| B4 | | | | |
| B5 | | | | |
| B6 | | | | |
| B7 | | | | |
| B8 | | | | |
| S1 | | | | |
| S2 | | | | |
| S3 | | | | |
| S4 | | | | |
| S5 | | | | |
| S6 | | | | |
| S7 | | | | |

## Findings

A bug found here is fixed on `feat/kb-store` (a new commit, the pull request updated) before the merge block is used. An
idea goes to the open questions of `lots/lot-01-store.md`.

| # | What was seen | Bug or idea | Fixed in |
| --- | --- | --- | --- |
| | | | |
