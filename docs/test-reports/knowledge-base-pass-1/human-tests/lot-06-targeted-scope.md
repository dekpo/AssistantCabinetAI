# Human test - lot 6, Targeted scope

Written for the owner, who runs it. English, with the French strings of the interface quoted as data. **Every command is written in full and can be pasted as it is** (`.cursor/rules/kb-programme.mdc`, "Exact commands in a human test" and "A human test is a script, not a list").

| | |
| --- | --- |
| Branch to be on | `feat/kb-targeted-scope` |
| Build | From `apps\desktop`, `pnpm tauri dev` |
| Services | **The AI machine must be up** (these are questions about documents): `docker compose up -d` from the repository root, if it is not running |
| Duration | Preparation 5 minutes; Part B 8; Part C 8; Part E 6; Part D (optional, spreadsheets) 10; final restore 2 |
| Visible in the application? | **Partly, and on purpose.** In `suggest` (the default) nothing visible changes: that is the acceptance criterion. You see the **« Sources »** under an answer, the order of the spreadsheets offered by « Plusieurs tableurs sont sélectionnés », and, with the read-only script `show_kb_diagnostics.py`, what the knowledge base measured for each question. **The line "this answer used 4 files of 30", the « élargir » button and the chips arrive in lot 7.** |

## Status

**First run by the owner on 11 October 2026**, B1 to D5 and S1 to S3. What matched: B2 to B5, C1, C2 (with 3 files), C4 to C7, D2, D3, D4. What did **not** match, and why (all of it is in the protocol below, corrected):

- **The agent's expectations were wrong in four places**, not the product: P3 (« dont 1 déjà connue »: your index already held `Jean Dupont` from the mail-merge file), C3 (`Que dit le Docteur Pierre Martin ?` is read by the folder's own router as naming the file `pierre-martin.txt`, so it is a named-file question: `named_file`, which is correct and was not predicted), the line « off records nothing » (the table gets no row, but the timing log gets a line with `mode=off`), and the order of the parts (see below).
- **One product defect found, fixed in this branch:** B1 and C2 kept **3** files and not 4. `rappel-m-dupont.txt`, which only says « M. Dupont », was not reached because `Dupont J`, a patient of a workbook you had **not** selected, made the surname look shared. A surname now counts as shared only when the other person is mentioned **inside the selection** (`a_surname_shared_only_with_a_workbook_outside_the_selection_is_still_searched`). Re-run B1 and C2.
- **The order of the protocol was wrong:** Part D ended with `lot06_helper.py restore`, which put your original settings back, so Part E ran on your own folder (six documents) and its expectations were false. The parts are reordered below, each part states the state it starts from and the state it leaves, and the only `restore` of this test is the last step.

C6 gave the data lot 11 needs: with `auto`, `Le Docteur Dupont a-t-il parlé de la photocopieuse ?` was answered « Non, ... ne mentionne pas la photocopieuse » although `note-photocopieuse.txt` does (`widened False`): defect KBD-17, the known limit of `auto`.

**Second run by the owner on 11 October 2026 (re-run of B1, C2, C3 and Part E, protocol reordered): every step matched, no finding.** B1: `4` kept, `cut 86`, `applied 0`; C2: « Sources » only the four files including `rappel-m-dupont.txt`, `applied 1`; C3: only `martin-controle.txt` and `martin-ordonnance.txt`, `kept 2`, `cut 93`; Part E: 30 documents, the Dupont schedule answered, the final restore gave back her own Documents folder and `writeTimingLog False`. **Lot 6 is accepted.** The AI server for the second run was a Docker Compose stack on another machine of the local network (a Windows tower, 16 GB of RAM, a much stronger graphics card, `http://192.168.1.83:8080` set as the server address): `ministral-3:3b` answered in 23 to 26 s instead of 70 to 90 s. **Nothing in the knowledge base or the server code changes with it**: the knowledge base is local to the client, only the address in the settings differs.

## What this proves, and what it does not

It proves, in the running application, that **a question that names a person is read in milliseconds against the knowledge base, inside the files you selected**; that in `suggest` the answer rests on exactly the files it rested on before while the knowledge base records what it *would* have done; that in `auto` the search covers only the files that name that person (and those whose text holds the name in another form), and says so in numbers; that a name which fits several people (`Dr Martin`) is not guessed; that a question with no name, or a question that names a file, is searched as before; that a spreadsheet is ordered or chosen from the person a question names; and that a question changes nothing the knowledge base knows.

It does not prove that `auto` finds everything: **it can miss a document that does not name the person** (C6). It shows nothing new on screen for the reduction itself: that is lot 7.

The agent ran the whole scenario as automated tests and every helper command on temporary folders. **The agent did not open the application window.** The wording of the model's sentence is not judged here: judge the « Sources » and the numbers.

## How to read this protocol

- **One line = one command**, typed in the order given, from `C:\Users\elise\Documents\CURSOR\AssistantCabinetAI`.
- Each part begins with **« You start with »** (the state the application and the settings must be in) and ends with **« You leave »**. If your state is not the one written, stop and say so: do not guess.
- Nothing is undone before the very last step, **Final restore**.
- The expected numbers assume the index you already have (your Data folder analysed in earlier tests, so `Jean Dupont` is already known). Where that changes a number, the step says so.

## The commands you will use (copy them)

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py status
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_diagnostics.py
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py
```

`lot06_helper.py` changes files and settings (close the application first, it refuses otherwise). `show_kb_diagnostics.py` prints, per question that went through the knowledge base, one line of **counts, codes and milliseconds** (never a question, a name or a file name) and the last lines of `retrieval-timings.jsonl`. `show_kb_entities.py` prints the names the knowledge base holds. (If columns of the table look glued together after you paste it into a message, that is the paste, not the script.)

## The thirty documents

`lot06_helper.py prepare` writes them in a folder `kb-lot06` beside your Documents folder (invented, French):

- **Dr Jean Dupont**: `dupont-bilan-2026.txt` (cholesterol 2,1 grammes par litre), `dupont-lettre-confrere.txt`, `dupont-horaires.txt`. And `rappel-m-dupont.txt`, which only says « M. Dupont ».
- **Mrs Claire Martin**: `martin-controle.txt`, `martin-ordonnance.txt`. **Dr Paul Martin**: `paul-martin.txt`. **Dr Pierre Martin**: `pierre-martin.txt`.
- Twenty-two notes: `note-01.txt` to `note-21.txt` and `note-photocopieuse.txt`.

## Preparation

**You start with:** the application closed, your own settings (your Documents folder and, if you have one, your Data folder), the AI machine up.

1. One line at a time:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py backup
python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py status
python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py prepare
python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py clear-log
```

   `backup` copies your index, settings and identifier key to `C:\Users\elise\kb-backup-lot-06`. `prepare` makes `kb-lot06`, writes the thirty documents in it, points the Documents folder at it, sets the mode to `suggest` and switches the timing log on. Your own documents are never touched.
2. Start the application:

```text
cd apps\desktop
pnpm tauri dev
```

3. On the **Documents** card click « Analyser ». **Expected:** thirty documents analysed and a line `6 personnes et organisations repérées, dont 0 déjà connues.` **or `dont 1 déjà connue`** if your Data folder was analysed before (`Jean Dupont` is also in the mail-merge file). Then run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py`: among the entities, four people **active** (`Claire Martin`, `Jean Dupont`, `Paul Martin`, `Pierre Martin`) and two organisations only **candidates** (`cabinet des Lilas`, `cabinet du Parc`). The other lines are your Data folder's.
4. In the Documents card's list **tick** the box « Tous les documents ». **Expected:** « Documents utilisés : tous ».
5. Run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_diagnostics.py`. **Expected:** `Questions recorded: 0`.

**You leave with:** the application running on `kb-lot06`, mode `suggest`, « Tous les documents » ticked, no question recorded.

## Part B - `suggest`, the default: nothing changes, everything is measured

**You start with:** what Preparation left. Ask the questions in the order given; the interface does not send the previous answer's names to the knowledge base yet (lot 7), so each question is read on its own. After each answer open « Sources » and run `show_kb_diagnostics.py`.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| B1 | Ask `Que dit le Docteur Jean Dupont du bilan sanguin ?`. Write down the file names in « Sources », in order (list L1) | An answer as before. One line: `1 suggest documents entity_files -`, `30` documents, **`4` kept** (the three Dupont files and `rappel-m-dupont.txt`), `0 0` workbooks, 6 files cited, 3 or 4 of them in the reduced set, `1` name, `widened 0`, `applied 0`, `would 1`, **`cut 86`**. `applied 0` and `would 1`: the base *would* have searched 4 files out of 30 and did not | `applied 1`; `kept 3` (the defect fixed since the first run: re-build with `pnpm tauri dev` first) |
| B2 | Ask `Que dit le Dr Martin ?` | A second line: `none ambiguous`, `would 0`, `names 0` (three Martins fit: the base asks and never picks) | `entity_files` on that line |
| B3 | Ask `Que contiennent les notes internes ?` | A third line: `none no_entity`, `would 0` | A reduction |
| B4 | Ask `Que dit dupont-bilan-2026.txt ?` | The answer rests on that one file. A fourth line: `none named_file`, `would 0` | A reduction |
| B5 | Close the application. Run `python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py mode off`. Start it again (`cd apps\desktop`, `pnpm tauri dev`), keep « Tous les documents » ticked, ask B1's question | **« Sources » identical to L1** (same files, same order). `show_kb_diagnostics.py`: **still four lines in the table** (the off mode writes no row), and the timing log shows a fifth line with `mode=off outcome=skipped skipped=off` | A different list; a fifth row in the table |

**You leave with:** mode `off`, the application running, four rows in the table. Close the application before Part C.

## Part C - `auto`: the search covers the files that name the person

**You start with:** the application closed, mode `off`, `kb-lot06` still the Documents folder.

1. One line at a time, then start the application and tick « Tous les documents » if it is not ticked:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py mode auto
python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py clear-log
cd apps\desktop
pnpm tauri dev
```

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| C1 | Open « Réglages » | The knowledge-base choice reads « Automatique (recherche seulement dans les fichiers cités par la question) » | Another label |
| C2 | Ask `Que dit le Docteur Jean Dupont du bilan sanguin ?`. Open « Sources ». Run `show_kb_diagnostics.py` | **Every file of « Sources » is one of these four**: `dupont-bilan-2026.txt`, `dupont-horaires.txt`, `dupont-lettre-confrere.txt`, `rappel-m-dupont.txt`. One line: `auto documents entity_files -`, `30 4`, `applied 1`, `would 1`, `cut 86`, `cited` equal to `in set`. (`widened 1` means the closest passage was far from your question and the whole selection was searched: tell the agent.) | A file outside the four; `applied 0`; `kept 3` |
| C3 | Ask `Que dit Madame Claire Martin du contrôle ?` | « Sources » lists only `martin-controle.txt` and `martin-ordonnance.txt`. A line with `kept 2`, `applied 1`, `cut 93` | Another file in « Sources » |
| C4 | Ask `Que dit le Dr Martin ?` | `none ambiguous`, `applied 0`. « Sources » is whatever the whole selection gives | `applied 1` |
| C5 | Ask `Que contiennent les notes internes ?` | `none no_entity`, `applied 0` | A reduction |
| C6 *(a known limit, not a bug)* | Ask `Le Docteur Dupont a-t-il parlé de la photocopieuse ?` | The reduced set is the same four files. **Either** `widened 1` and `note-photocopieuse.txt` may be in « Sources », **or** `widened 0` and the answer says the documents do not mention it although `note-photocopieuse.txt` does. Note which one | Nothing to file; tell the agent which |
| C7 | Ask `Que dit le Docteur Pierre Martin ?` | **A named-file question**: the folder's router reads « Pierre Martin » as the name of `pierre-martin.txt`, so the line says `none named_file` and « Sources » is that one file. This is the rule that a named file is never reduced | `applied 1` |
| C8 | Run `show_kb_diagnostics.py` and read the end of the output | « Timing log » lines such as `path=documents mode=auto outcome=applied skipped=None candidates=4 cut=86% widened=False lookupMs=... entities=1`. **No question, no name, no file name** anywhere | A name or a file name |

**You leave with:** mode `auto`, the application running, « Tous les documents » ticked. Close the application before Part E.

## Part E - smoke test, the product as before, on the same thirty documents

**You start with:** the application closed, mode `auto`, `kb-lot06` the Documents folder. (This part runs **before** the spreadsheets and before the restore, so it is still your thirty documents.)

1. Put the default mode back, then start:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py mode suggest
cd apps\desktop
pnpm tauri dev
```

| # | Do | Expected |
| --- | --- | --- |
| S1 | On the Documents card click « Analyser » | A pass that reads nothing new; no error |
| S2 | Tick « Tous les documents ». Ask `Combien de documents y a-t-il dans le dossier ?` | Answered from the folder's inventory, without reading any document: **30** |
| S3 | Ask `Quand le Docteur Dupont reçoit-il ?` | An answer from `dupont-horaires.txt` (lundi et jeudi, de 9 h à 12 h), cited in « Sources » |
| S4 | Click « Reset » on the Documents card, confirm, click « Analyser », ask S3 again | The same answer. `show_kb_diagnostics.py` still lists the questions asked since `clear-log` (a Reset empties the documents, not the record of questions) |

**You leave with:** mode `suggest`, the application running on the thirty documents. Close it.

## Part D (optional) - two spreadsheets

**You start with:** the application closed, `kb-lot06` the Documents folder, mode `suggest`. This part changes your Data folder and one setting; **its own last step undoes exactly those two and nothing else**.

1. One line at a time, then start the application:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py backup
python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py prepare-data
python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py min-scope 2
python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py mode suggest
cd apps\desktop
pnpm tauri dev
```

2. Data card: « Analyser ». The three files are green. **Untick** « Tous les documents » on the Documents card (« Documents utilisés : aucun »: the questions below are about the data only). Tick **only** `rdv-mars-2026.xlsx` and `factures-fournisseurs-2026.xlsx` in the Data card's list.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| D2 | Ask `Combien de lignes pour Dupont J ?` | « Plusieurs tableurs sont sélectionnés. Lequel voulez-vous ? Nommez-le dans la question : » with **`rdv-mars-2026.xlsx` first** (without the knowledge base the invoices come first, alphabetically). The choice is still yours | The invoices first |
| D3 | Close the application. Run `python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py mode auto`. Start it (`cd apps\desktop`, `pnpm tauri dev`), make sure « Documents utilisés : aucun » and the same two spreadsheets are ticked, ask `Combien de lignes pour Dupont J ?` again | **No question about which spreadsheet**: `Nombre de lignes : 2.` and `Compris comme : patient = Dupont J.` | The which-spreadsheet question again, or another number |
| D4 | Ask `Combien de lignes dans factures-fournisseurs-2026.xlsx pour Dupont J ?` | The invoices answer (a spreadsheet you name is never replaced by the base's choice). The engine reads « Dupont » as the supplier `Fournitures Dupont`: that is the spreadsheet engine's own reading, not the knowledge base's | An answer from `rdv-mars-2026.xlsx` |

3. **Undo this part, and only this part** (close the application first), in this order:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py min-scope default
python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py restore
```

   `lot05_helper.py restore` puts back your Data folder and the settings as they were at the start of this part (still the `kb-lot06` Documents folder).

**You leave with:** the application closed, `kb-lot06` the Documents folder, your own Data folder, the default minimum scope.

## Final restore (the only one)

**You start with:** the application closed.

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py restore
python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py status
```

**Expected:** « restored index.sqlite3 », « restored settings.json », and `status` showing **your own Documents folder** (not `kb-lot06`) and `writeTimingLog : False`. The `kb-lot06` folder can be deleted.

## Results table

| Step | Expected | Observed | OK? | Screenshot |
| --- | --- | --- | --- | --- |
| P3 | | | | |
| B1 | | | | |
| B2 | | | | |
| B3 | | | | |
| B4 | | | | |
| B5 | | | | |
| C1 | | | | |
| C2 | | | | |
| C3 | | | | |
| C4 | | | | |
| C5 | | | | |
| C6 | | | | |
| C7 | | | | |
| C8 | | | | |
| S1-S4 | | | | |
| D2 | | | | |
| D3 | | | | |
| D4 | | | | |
| Final restore | | | | |

## Findings

Bugs: fix in the same lot branch before the merge. Ideas, and the answer to C6: the lot report's open questions.
