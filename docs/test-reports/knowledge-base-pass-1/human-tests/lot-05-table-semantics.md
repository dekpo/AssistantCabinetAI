# Human test - lot 5, Entities from tables

Written for the owner, who runs it. English, with the French strings of the interface quoted as data. **Every command is written in full and can be pasted as it is** (owner instruction of 11 October 2026: `.cursor/rules/kb-programme.mdc`, "Exact commands in a human test").

| | |
| --- | --- |
| Branch to be on | `feat/kb-table-semantics` |
| Build | Part A needs no application: `cargo test` from `apps\desktop\src-tauri`. Part B: from `apps\desktop`, `pnpm tauri dev` |
| Services | **None for the Data Analyse itself** (it never calls the AI). The AI machine is touched only by step B8, to show that the analysis does not need it |
| Duration | Part A about 5 minutes; Part B about 25 minutes (B7 to B11 optional, 10 minutes together); the smoke test about 8 minutes |
| Visible in the application? | **Partly.** On screen: the new lines under the **Data card's** analysis summary and its progress label while documents analysed earlier are read again. The columns, the names and where they were found are not shown anywhere in the interface yet (lots 7 and 8); you read them with the read-only script `show_kb_entities.py` |

**Nothing in this test asks you to edit a file by hand.** Every change to a file or to a setting is made by `lot05_helper.py`, except the settings you change in « Réglages ».

## Status

**Run by the owner on 11 October 2026 (first version): every step B1 to B11 and the smoke test matched**, with two findings, both about this document and not about the product: the commands were abbreviated (`stop-ai`, "in the script"), and the expectation "no error banner" in B8 was wrong (see B8). The product was changed afterwards on the owner's answers (row numbers as in the spreadsheet program, headers read in both languages, columns that were not read are recorded with their reason, order numbers read as orders), so **this version is the one to run again**, at least B2, B5, B12 and B13. The counts in the summary lines did not change; the row numbers did (+1: the header is row 1).

## What this proves, and what it does not

It proves that **pressing Analyser on the Data card** (and only that: nothing runs at start-up or in the background) teaches the knowledge base what the columns of your spreadsheets mean and which people, organisations and identifiers their values name; that a `montant` or a date is never read as a name; that a first name and a family name in two columns are one person; that an e-mail address is never written as itself; that pressing Analyser again changes nothing; that a changed, a removed or a reset workbook takes its knowledge with it and nothing else; that **a module only adds meaning and never changes a type**; that the columns that were **not** read are recorded with the reason; that rows are the rows of the file; and that the spreadsheets' answers are the same as before (the engine is untouched).

It does not prove that nothing is missed (a header is read by the words of the packs: one more word is one line in a pack), nor that `Dupont J` (the pilot's own `rdv-mars-2026.xlsx` writes patients like that) is linked to `Jean Dupont`: that is defect KBD-15, fixed in its own branch before lot 6. Nothing uses the columns to answer a question yet (lot 6).

The agent ran the whole scenario as automated tests (`tests\knowledge_tables.rs`, in particular `the_numbers_the_human_test_quotes_hold`) and every command of the helper script on temporary folders; **it did not open the application window.**

## The commands you will use (copy them)

Always from the repository root, `C:\Users\elise\Documents\CURSOR\AssistantCabinetAI`.

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py status
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_tables.py
```

`lot05_helper.py` changes files and settings (close the application first, it refuses otherwise). `show_kb_entities.py` prints the **names**, the columns, how each column was read, and the links. `show_kb_tables.py` prints row **counts** only. Where a step says "run `show_kb_entities.py`" it means the second line above, exactly.

## Preparation

1. Close the application (the window and the `pnpm tauri dev` terminal). Then, one line at a time:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py backup
python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py status
python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py prepare-data
```

   `backup` copies your index, settings and identifier key to `C:\Users\elise\kb-backup-lot-05`. `prepare-data` makes the folder `kb-lot05` beside your Data folder, copies the three fictional workbooks into it and points the application at it (a Data folder must already be chosen in the application, as it is). Your own workbooks are never touched.
2. Start the application:

```text
cd apps\desktop
pnpm tauri dev
```

## Part A - what a pass learns from the pilot's workbooks (no application)

From `apps\desktop\src-tauri`:

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| A1 | `cargo test --test knowledge_tables print_what_a_pass_learns -- --ignored --nocapture` | A `summary:` line with `entities_detected: 15, matched_existing: 0, created_new: 15, candidates: 0, identifiers: 6, relations: 0, semantic_columns: 7, workbooks_analysed: 3, errors: 0`, then the 7 columns read, then 26 entities (12 people, 3 organisations, 3 order numbers, 3 e-mail labels, 5 articles) | A different count |
| A2 | `cargo test --test knowledge_tables` | `test result: ok. 47 passed; 0 failed; 3 ignored` | A failure |

## Part B - in the application

The buttons are the **Data card's** « Voir », « Changer », « Reset » and « Analyser », and « Réglages » at the top right. After each Analyse, read the lines under the Data card.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| B1 | On the Data card click « Reset », confirm. Then run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_tables.py` | The three files are orange. In the output `kb_column_semantics 0`, `kb_entities 0` and, under « Sources by folder and extractor version », `(none)` | Rows left after a Reset |
| B2 | Click « Analyser » on the Data card. Then run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py` | Under the card: `3 tableurs lus sur cet ordinateur, sans l'IA : les tableurs s'analysent en un instant.`, `15 personnes et organisations repérées, dont 0 déjà connues.`, `6 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).`, `7 colonnes de tableur reconnues (personnes, organisations, identifiants...) dans 3 tableurs lus.` (and `N documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.` only if your Documents folder has been analysed). In the script: « Columns read as something: 7 » (`Article` item, `Email` identifier/email, `No_Commande` identifier/order 0.90 `header_exact`, `Nom` person/family_name, `Prenom` person/given_name, `fournisseur` organization role supplier, `patient` person role client), then « Columns NOT read, with the reason: 10 » all `no_header_match` (`Civilite`, `ID_Client`, `Prix_Unitaire`, `Quantite`, `Total_Ligne`, `date` twice, `montant`, `duree_min`, `salle`), then 26 entities. **Rows are the rows of the file**: `MedSupply ... first row 2, rows [2,4,7], role supplier, column_value, x3`, `Fournitures Dupont ... rows [3,6,9]`, `Jean Dupont ... donnees_publipostage / Nom, first row 2, rows [2,3], column_value, x2`, `Dupont J ... first row 2, rows [2,6]` | A different count; a `montant`, a date or `salle` read as a name |
| B3 | Run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py --look-for jean.dupont@email.com` | `knowledge base tables (kb_*): not found` and `stored document text (chunks): found in 0 chunk(s)`. The three e-mail entities read `email xxxxxx` in B2's list | `FOUND` for the knowledge base tables |
| B4 | Click « Analyser » on the Data card **again**. Then run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_tables.py` and compare with the same command after B2 | No knowledge line under the card (nothing was read again). Every count and every `kb_meta` value is the same, `gazetteer_epoch` included | A knowledge line, a count that moved |
| B5 | « Réglages » -> « Module santé » -> **untick « Santé : France »**, close « Réglages », click « Analyser » on the Data card. Then run **`python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py`** (this one, not `show_kb_tables.py`) | Under the card: `6 personnes et organisations repérées, dont 6 déjà connues.`, `6 colonnes de tableur reconnues ... dans 3 tableurs lus.` In the script, `patient` is **no longer** in « Columns read as something » and **is** in « Columns NOT read » as `patient  module_off  (a module that is off would read it as person/client)`; the nine `Dupont J`-style people are gone; `MedSupply`, `Fournitures Dupont` and `Papeterie Lefevre` are **still organization** and `Jean Dupont` **still person** | An organisation listed as a person, or a patient listed as anything |
| B6 | « Réglages » -> « Module santé » -> **tick « Santé : France »** again, close, click « Analyser » on the Data card | `15 personnes et organisations repérées, dont 6 déjà connues.`, `7 colonnes ... dans 3 tableurs lus.` The nine patients are back | The patients missing |
| B7 *(optional)* | Close the application. Run `python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py edit-workbook` (replaces `Simon V` by `Garnier C` in `rdv-mars-2026.xlsx`, keeps the original). Start the application (`pnpm tauri dev` from `apps\desktop`), click « Analyser » on the Data card, run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py`. To undo: close the application, `python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py restore-workbook`, start it, « Analyser » | `3 tableurs lus ...`, `9 personnes et organisations repérées, dont 8 déjà connues.`, `1 colonne de tableur reconnue (...) dans 1 tableur lu.` In the script `Simon V` is gone, `Garnier C` is there (`first row 11`), the suppliers and the mail-merge names untouched. After the undo `Simon V` is back and `Garnier C` is gone | `Simon V` still listed |
| B8 *(optional)* | **The AI machine stopped.** Run `docker compose stop` (from the repository root). On the Data card click « Reset », confirm, click « Analyser ». Put it back with `docker compose up -d` | The same four lines as B2, in the card. **The application's own red banner « Cela n'a pas fonctionné - Aucune réponse de l'IA à l'adresse http://127.0.0.1:8080... » appears while the AI machine is stopped, whatever you do**: it is the application's connection check, not the analysis. What matters is that the Data Analyse worked and its lines are shown | The four lines missing, or « Analyser » grey |
| B9 *(optional)* | Close the application. Run `python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py hide-workbook`. Start the application, click « Analyser » on the Data card, run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py`. To undo: close the application, `python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py restore-hidden`, start it, « Analyser » | The card says `1 tableur retiré du dossier, oublié de l'analyse`. In the script `MedSupply`, `Fournitures Dupont`, `Papeterie Lefevre` and the `fournisseur` column are gone, 2 sources left, 23 entities; the patients and the mail-merge names are still there. After the undo 26 entities again | A patient lost with the invoices |
| B10 *(optional)* | Click « Reset » on the **Data** card, confirm, run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py` | `Sources the knowledge base has read: 0`, `Columns read as something: 0`, `Entities: 0`. (To also see that the documents' knowledge is left alone, analyse your Documents folder first: its `documents` sources and entities stay.) Then « Analyser » gives B2's lines again | A table name left |
| B11 *(optional)* | Close the application. Run `python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py mode off`. Start the application, click « Reset » on the Data card, confirm, click « Analyser ». Then run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_tables.py`. To undo: close the application, `python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py mode suggest`, start it | The three files are green, **no knowledge line** under the card; `tabular_inventories 3` and `kb_sources`, `kb_mentions`, `kb_column_semantics` at 0 | A knowledge line or a `kb_` row with the switch off |
| B12 *(new)* | After B2, run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py` and read « Columns NOT read, with the reason » | Ten columns, each with `no_header_match`: nothing in any pack means `salle`, `montant`, a date or `Quantite`. After B5 (`Santé : France` unticked) `patient` is added with `module_off`. This is what the management dialog of lot 8 will show to explain "why was this column not read" | A column read as something that is a date or an amount |
| B13 *(new)* | Order numbers: in B2's entity list find `CMD-2026-001` | `identifier/order   active   CMD-2026-001   donnees_publipostage.csv [cells donnees_publipostage / No_Commande, first row 2, rows [2,3], column_value, x2]` (an **order**, not an invoice) | `identifier/invoice` |

When you are done: close the application and run `python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py restore` (your settings, with your own Data folder, and your index come back); the `kb-lot05` folder can be deleted. Make sure « Base de connaissances » reads « Activée » and only « Santé : France » is ticked under « Module santé » (the default).

## Part C - smoke test, the product as before

These questions are answered by the engine, with no AI (every answer ends with « Réponse calculée depuis votre dossier des données, sans l'IA. »). Selection: tick the file named in the step in the Data card's list.

| # | Do | Expected |
| --- | --- | --- |
| S0 | Start the application with the `kb-lot05` folder as Data folder. Click « Analyser » on the Data card | The four lines of B2, and the card reads `3 fichiers, 3 analysés` |
| S1 | Tick only `factures-fournisseurs-2026.xlsx`. Ask `Quel est le maximum de montant ?` | `Plus grande valeur de montant : **620**.` `Calculé sur 8 lignes.` |
| S2 | Same selection. Ask `Quel est le total de montant par fournisseur ?` | MedSupply : 1 450, Fournitures Dupont : 550, Papeterie Lefevre : 215, `Calculé sur 8 lignes.` |
| S3 | Same selection. Ask `Combien de factures pour le fournisseur MedSupply ?` | `Nombre de lignes : **3**.` `Compris comme : fournisseur = MedSupply.` |
| S4 | Tick only `rdv-mars-2026.xlsx`. Ask `Quelle est la moyenne de duree_min ?` | `Moyenne de duree_min : **24,5**.` `Calculé sur 10 lignes.` |
| S5 | Same selection. Ask `Combien de rendez-vous par salle ?` | Salle 1 : 5, Salle 2 : 3, Salle 3 : 2 |
| S6 | Same selection. Ask `Combien de rendez-vous entre le 09/03/2026 et le 15/03/2026 ?` | `Nombre de lignes : **4**.` `Compris comme : date entre 2026-03-09 et 2026-03-15.` |
| S7 | Click « Reset » on the Data card, confirm, click « Analyser », select only `factures-fournisseurs-2026.xlsx` and repeat S2 | The same answer |

**Run by the owner on 11 October 2026: S1 to S7 matched exactly.**

## Results table

| Step | Expected | Observed | OK? | Screenshot |
| --- | --- | --- | --- | --- |
| A1 | | | | |
| A2 | | | | |
| B1 | | | | |
| B2 | | | | |
| B3 | | | | |
| B4 | | | | |
| B5 | | | | |
| B6 | | | | |
| B7 | | | | |
| B8 | | | | |
| B9 | | | | |
| B10 | | | | |
| B11 | | | | |
| B12 | | | | |
| B13 | | | | |
| S0 to S7 | | | | |

## Findings

| # | What was seen | Bug, edit or idea | Fixed in |
| --- | --- | --- | --- |
| 1 | The first version of this document abbreviated commands (`stop-ai`, "in the script") and the owner typed `stop-ia` and ran the wrong script at B5 | Edit: every command is now written in full; rule added to `.cursor/rules/kb-programme.mdc`, `docs/SESSION-KB-WORKFLOW.md` and `human-tests/README.md` | this version |
| 2 | B8 promised "no error banner" with the AI machine stopped; the application shows its red connection banner whatever the analysis does | Edit: expectation corrected (the Data Analyse still ran and its lines were shown) | this version |
| 3 | The entity list printed `RDV/ patient` for the appointments workbook while the column list printed `RDV / patient` | Not reproduced by the code (one format string); read as a paste artefact | - |
| 4 | Owner's answers to the open questions: rows as in the spreadsheet program, headers in both languages, keep the columns that were not read, an order scheme, accept the limit on company names in a `Nom` column, keep the display of upper-case names | Changes made in this lot (see `lots/lot-05-table-semantics.md`) | `feat/kb-table-semantics` |
