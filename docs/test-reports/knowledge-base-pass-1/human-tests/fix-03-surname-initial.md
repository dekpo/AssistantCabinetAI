# Human test - fix 3, the surname-then-initial form (KBD-15)

Written for the owner, who runs it. English, with the French strings of the interface quoted as data. **Every command is written in full and can be pasted as it is** (owner instruction of 11 October 2026: `.cursor/rules/kb-programme.mdc`, "Exact commands in a human test").

| | |
| --- | --- |
| Branch to be on | `fix/kb-surname-initial` |
| Build | Part A needs no application: `cargo test` from `apps\desktop\src-tauri`. Part B: from `apps\desktop`, `pnpm tauri dev` |
| Services | **None.** The Data Analyse never calls the AI |
| Duration | Part A about 6 minutes (the first build is slower); Part B about 10 minutes; the smoke test about 5 minutes |
| Visible in the application? | **Partly.** Nothing new appears on a card or in a message: the fix changes what the knowledge base stores. You read it with the read-only script `show_kb_entities.py`, which gains one section, « Aliases of people filed as surname + initial ». The question-time behaviour (« did you mean Jean Dupont ? ») has no screen before lot 7: it is proved by automated tests only (Part A) |

**Nothing in this test asks you to edit a file by hand.** Every change to a file or to a setting is made by `lot05_helper.py` (the same helper as lot 5, with one new command, `age-aliases`).

## What this proves, and what it does not

It proves, on the pilot's own fictional workbooks, that `Dupont J` of `rdv-mars-2026.xlsx` is no longer filed with a surname `J` and an initial `D. J`; that it is offered as a **possible match** of `Jean Dupont` of `donnees_publipostage.csv` and **never merged** with it; that `Martin A` is **not** offered as a match of `Sophie Martin` (`A` is not the initial of Sophie); that a base filed by the previous build is corrected by the next **Analyse you start** and by nothing else, with no workbook read again; and that pressing Analyser again changes nothing.

It does not prove what a question does with these names (lot 6 uses them, lot 7 shows them), nor that a given name followed by one letter (`Jean M`) is never taken for a surname and an initial: no file can tell, and the result is only ever a question.

The agent ran the whole scenario as automated tests (`tests\knowledge_tables.rs`: `the_pilots_appointments_and_mail_merge_files_give_a_possible_match`, `a_surname_then_initial_is_a_possible_match_of_the_full_name_in_both_orders`, `a_base_read_before_this_fix_is_corrected_by_the_next_data_analyse_and_only_then`), ran `show_kb_entities.py` and `lot05_helper.py age-aliases` on a temporary copy of an index, and **did not open the application window.**

## The commands you will use (copy them)

Always from the repository root, `C:\Users\elise\Documents\CURSOR\AssistantCabinetAI`.

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py status
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_tables.py
```

`lot05_helper.py` changes files and settings (close the application first, it refuses otherwise). `show_kb_entities.py` prints the **names**, how each was found, the possible matches and the aliases of the surname-and-initial people. `show_kb_tables.py` prints row **counts** only. Where a step says "run `show_kb_entities.py`" it means the second line above, exactly.

## Preparation

1. Close the application (the window and the `pnpm tauri dev` terminal). Then, one line at a time:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py backup
python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py status
python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py prepare-data
```

   `backup` copies your index, settings and identifier key to `C:\Users\elise\kb-backup-lot-05` (it replaces the copy made for lot 5). `prepare-data` makes the folder `kb-lot05` beside your Data folder, copies the three fictional workbooks into it and points the application at it. Your own workbooks are never touched.
2. Start the application:

```text
cd apps\desktop
pnpm tauri dev
```

## Part A - the tests that prove the rules (no application)

From `apps\desktop\src-tauri`:

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| A1 | `cargo test --lib` | `test result: ok. 697 passed; 0 failed; 2 ignored` | A failure |
| A2 | `cargo test --test knowledge_resolver` | `test result: ok. 53 passed; 0 failed` | A failure |
| A3 | `cargo test --test knowledge_tables` | `test result: ok. 50 passed; 0 failed; 3 ignored` | A failure |
| A4 | `cargo test --test knowledge_resolver surname` | The tests named `a_surname_then_initial_is_a_possible_match_of_the_full_name_in_both_orders`, `a_surname_then_initial_never_picks_between_two_full_names` and `the_store_offers_a_surname_then_initial_and_the_full_name_to_each_other` are `ok` | A failure |

## Part B - in the application

The buttons are the **Data card's** « Voir », « Changer », « Reset » and « Analyser ». After each Analyse, read the lines under the Data card.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| B1 | On the Data card click « Reset », confirm. Then run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_tables.py` | The three files are orange. `kb_entities 0` (if you analysed your Documents folder before, the entities of the documents stay and are not 0) | Rows of the Data folder left after a Reset |
| B2 | Click « Analyser » on the Data card. Then run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py` | Under the card, as in lot 5: `3 tableurs lus sur cet ordinateur, sans l'IA : les tableurs s'analysent en un instant.`, `15 personnes et organisations repérées, dont 0 déjà connues.`, `6 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).`, `7 colonnes de tableur reconnues (personnes, organisations, identifiants...) dans 3 tableurs lus.` In the script, section « Possible matches (never merged) »: **`Jean Dupont  ~  Dupont J   [initial, open]`** (the two names may come in the other order), and no pair for `Martin A` or `Sophie Martin`. Both `Dupont J` and `Jean Dupont` are in the list as `person  active`, each with its own file (`rdv-mars-2026.xlsx` and `donnees_publipostage.csv`): they are **two** entities | The two names merged into one; a pair for `Martin A` / `Sophie Martin`; no pair at all |
| B3 | In the output of B2, read the section « Aliases of people filed as surname + initial » | `9` people. `Dupont J: 'dupont' (partial_surname), 'dupont j' (canonical_variant), 'j dupont' (initial)`. `Martin A: 'a martin' (initial), 'martin' (partial_surname), 'martin a' (canonical_variant)`. The same three aliases for each of `Bernard L`, `Fontaine R`, `Girard P`, `Lambert T`, `Petit M`, `Roux S`, `Simon V` | An alias that is the single letter by itself (`'j' (partial_surname)`), or `'d j'` |
| B4 | Click « Analyser » on the Data card **again**. Then run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_tables.py` and compare with the same command after B2 | No knowledge line under the card (nothing was read again). Every count is the same, `kb_possible_matches` included | A knowledge line, a count that moved, a second pair |
| B5 | **A base filed by the previous build.** Close the application. Run `python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py age-aliases`. Then run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py` | The helper prints `The aliases of 'Dupont J' are the old ones again (a surname 'J', an initial 'D. J'), no possible match is recorded, and the alias generation is forgotten.` The script now shows `Dupont J: 'd j' (initial), 'dupont j' (canonical_variant), 'j' (partial_surname), 'j dupont' (reordered)`, « Possible matches (never merged): 0 », and no `alias_generation` line in `kb_meta`. **This is the state the previous build left**; the script is a read-only look and changes nothing | The helper refusing for another reason than the application running |
| B6 | Start the application (`pnpm tauri dev` from `apps\desktop`). Click « Analyser » on the Data card (no file changed). Then run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py` | **No line under the card** (no workbook was read: the correction does not read anything). In the script: `Dupont J` is back to `'dupont' (partial_surname), 'dupont j' (canonical_variant), 'j dupont' (initial)`, « Possible matches (never merged): 1 » with `Jean Dupont ~ Dupont J [initial, open]`, and `alias_generation  '2'` in `kb_meta`. The correction ran inside the Analyse you started | The old aliases still there; the correction done before you pressed Analyser (it must not) |
| B7 | Click « Analyser » on the Data card once more. Run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_tables.py` | Nothing changes (the correction is done once) | A pair added twice |

When you are done: close the application and run `python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py restore` (your settings, with your own Data folder, and your index come back); the `kb-lot05` folder can be deleted.

## Part C - smoke test, the product as before

These questions are answered by the engine, with no AI (every answer ends with « Réponse calculée depuis votre dossier des données, sans l'IA. »). Selection: tick the file named in the step in the Data card's list. The answers are those of lot 5, which you confirmed on 11 October 2026; this fix does not touch the engine.

| # | Do | Expected |
| --- | --- | --- |
| S0 | Start the application with the `kb-lot05` folder as Data folder. Click « Analyser » on the Data card | The four lines of B2, and the card reads `3 fichiers, 3 analysés` |
| S1 | Tick only `factures-fournisseurs-2026.xlsx`. Ask `Quel est le maximum de montant ?` | `Plus grande valeur de montant : **620**.` `Calculé sur 8 lignes.` |
| S2 | Same selection. Ask `Quel est le total de montant par fournisseur ?` | MedSupply : 1 450, Fournitures Dupont : 550, Papeterie Lefevre : 215, `Calculé sur 8 lignes.` |
| S3 | Tick only `rdv-mars-2026.xlsx`. Ask `Quelle est la moyenne de duree_min ?` | `Moyenne de duree_min : **24,5**.` `Calculé sur 10 lignes.` |
| S4 | Same selection. Ask `Combien de rendez-vous par salle ?` | Salle 1 : 5, Salle 2 : 3, Salle 3 : 2 |

## Results table

| Step | Expected | Observed | OK? | Screenshot |
| --- | --- | --- | --- | --- |
| A1 | | | | |
| A2 | | | | |
| A3 | | | | |
| A4 | | | | |
| B1 | | | | |
| B2 | | | | |
| B3 | | | | |
| B4 | | | | |
| B5 | | | | |
| B6 | | | | |
| B7 | | | | |
| S0 to S4 | | | | |

## Findings

| # | What was seen | Bug, edit or idea | Fixed in |
| --- | --- | --- | --- |
| | | | |
