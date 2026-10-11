# Human test - lot 7, Targeting in the interface

Written for the owner, who runs it. English, with the French strings of the interface quoted as data. **Every command is written in full and can be pasted as it is** (`.cursor/rules/kb-programme.mdc`, "Exact commands in a human test" and "A human test is a script, not a list").

| | |
| --- | --- |
| Branch to be on | `feat/kb-targeting-ui` |
| Build | From `apps\desktop`, `pnpm tauri dev` |
| Services | **The AI machine must be up** (these are questions about documents): the address in « Réglages » must answer. On the local machine, `docker compose up -d` from the repository root; with the tower of the local network, nothing to do |
| Duration | Preparation 5 minutes; Part A 6; Part B 8; Part C 8; Part D 6; Part E 4; final restore 2; Part F (optional, spreadsheets) 8. About 40 minutes without Part F |
| Visible in the application? | **Yes, this is the lot that makes the knowledge base visible.** The line under an answer, its two buttons, the names it asks about, the `@` list, the chips, the settings group and the developer panel |

## Status

**First run by the owner, 11 October 2026** (her message was cut at B6 by the size limit). Matched: Preparation, A1 to A7, B1 to B5, and on screenshots C1, C2's list, C3, C5 and C6. What did **not** match, and which side was wrong:

- **The protocol was wrong:** her own Data folder was analysed and holds names (`Dupont J`, `Fournitures Dupont`, `Sophie Martin`, `Martin A`, `Clavier Mécanique`), so the `@` list showed extra « figure dans 1 fichier non sélectionné » lines, `Jean Dupont` outside the selection counted 4 files (three documents and `donnees_publipostage.csv`), and « Ajouter à ma sélection » ticked that spreadsheet, which switched later questions to the documents-and-tables tier. `prepare` now points the Data folder at an empty folder and `restore` puts it back. Re-run Preparation (not the backup) and Parts C to F.
- **The agent's expectation of B6 was wrong** (see B6): the widen button cannot repair a carried short question.

**Waived by the owner, 11 October 2026:** the rest of her results was lost to the chat's size limit and cannot be recovered, so the test is treated as validated. B7, C4, C7, D, E and F were not observed by anyone.

## What this proves, and what it does not

It proves, in the running application, that **an answer whose search was narrowed says so, in numbers and in names, and that one click searches everything again**; that a name which fits several people is asked about *after* the answer, never guessed; that the `@` list offers only names of the files you selected, with their number of files; that a name found only in files you did not tick is offered apart, labelled, and only your own click adds those files; that the settings and the developer panel work; and that with the knowledge base off the screen is as it was.

It does not prove that `auto` finds everything: **it can miss a document that does not name the person** (defect KBD-17). Step B7 is there so that you see the remedy, the button **Élargir à ma sélection**, in front of you.

**What the agent ran, and what only you can see.** The agent ran every automated suite (numbers in the lot report), every helper command of this protocol in order on temporary copies of the settings and the index, and every question of the steps below through the same Rust code the application uses, on the same thirty invented documents: the expected figures and sentences come from there. **The agent did not open the application window and clicked nothing**: everything you click here is unverified until you run it. Two steps depend on the real embedding model and could not be walked: B7 (what the wider search finds) and, if you run it, F1. They say so.

## How to read this protocol

- **One line = one command**, typed in the order given, from `C:\Users\elise\Documents\CURSOR\AssistantCabinetAI`.
- Each part begins with **« You start with »** and ends with **« You leave with »**. If your state is not the one written, stop and say so: do not guess.
- Nothing is undone before the very last step, **Final restore**. Part F is optional, comes last, and brings its own undo.
- Where a figure depends on the model's choice of words, the step judges the **line under the answer** and the **« Sources »**, not the sentence of the answer.
- The `@` list counts the files that **mention** a name (3 for Jean Dupont); the line under an answer counts the files that were **searched** (4), which also include a document that only says « M. Dupont » and so cannot be linked to the person. That difference is expected.

## The commands you will use (copy them)

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot07_helper.py status
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_diagnostics.py
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py
```

`lot07_helper.py` changes files and settings (close the application first, it refuses otherwise). The mode and the thresholds are changed **in the application** this time (« Réglages »), not by a script. `show_kb_diagnostics.py` prints, per question that went through the knowledge base, one line of counts, codes and milliseconds (never a question, a name or a file name). `show_kb_entities.py` prints the names the knowledge base holds.

## The thirty documents

`lot07_helper.py prepare` writes them in a folder `kb-lot07` beside your Documents folder (invented, French; the same as lot 6):

- **Dr Jean Dupont**: `dupont-bilan-2026.txt`, `dupont-lettre-confrere.txt`, `dupont-horaires.txt`. And `rappel-m-dupont.txt`, which only says « M. Dupont ».
- **Mrs Claire Martin**: `martin-controle.txt`, `martin-ordonnance.txt`. **Dr Paul Martin**: `paul-martin.txt`. **Dr Pierre Martin**: `pierre-martin.txt`.
- Twenty-two notes: `note-01.txt` to `note-21.txt` and `note-photocopieuse.txt`.

Questions that name a person who has a file of their own (`Que dit Pierre Martin ?`) are read by the folder's own router as naming that file (defect KBD-18), so none of the steps uses one.

## Preparation

**You start with:** the application closed, your own settings (your Documents folder chosen), the AI machine up.

1. One line at a time:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot07_helper.py backup
python docs\test-reports\knowledge-base-pass-1\human-tests\lot07_helper.py status
python docs\test-reports\knowledge-base-pass-1\human-tests\lot07_helper.py prepare
python docs\test-reports\knowledge-base-pass-1\human-tests\lot07_helper.py clear-log
```

   `backup` copies your index, settings and identifier key to `C:\Users\elise\kb-backup-lot-07`. `prepare` makes `kb-lot07`, writes the thirty documents in it, points the Documents folder at it, **points the Data folder at an empty folder `kb-lot07-data`** (so that the names of your own spreadsheets are not offered by the `@` list), sets the mode to `suggest` and switches the timing log on. Your own documents and spreadsheets are never touched; `restore` puts your folders back.
2. Start the application:

```text
cd apps\desktop
pnpm tauri dev
```

3. On the **Documents** card click « Analyser ». **Expected:** thirty documents analysed, and a line `... personnes et organisations repérées, dont ...` (do not compare the numbers: your index may already know `Jean Dupont` from the mail-merge file). Then run:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py
```

   **Expected:** four people **active** (`Claire Martin`, `Jean Dupont`, `Paul Martin`, `Pierre Martin`) and two organisations only **candidates** (`cabinet des Lilas`, `cabinet du Parc`). Only active names are offered by the `@` list.
4. Open « Documents utilisés » in the Documents card and **tick** « Tous les documents ». **Expected:** « Documents utilisés : tous ».
5. Run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_diagnostics.py`. **Expected:** `Questions recorded: 0`.

**You leave with:** the application running on `kb-lot07`, mode `suggest`, « Tous les documents » ticked, an empty conversation, no question recorded.

## Part A - `suggest` (the default): nothing is narrowed, names are asked about

**You start with:** what Preparation left. Ask the questions in the order given. After each step look **under the answer**.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| A1 | Ask `Que dit le Docteur Jean Dupont du bilan sanguin ?` | An answer as before. **Nothing about a targeted search under it**: in `suggest` the base only measures | A line « Recherche ciblée » |
| A2 | Ask `Que dit le Dr Martin ?` | The question is answered. Under the answer: « Plusieurs personnes correspondent à « Dr Martin » : » and three buttons, in this order: **« Claire Martin (2 fichiers) »**, **« Paul Martin (1 fichier) »**, **« Pierre Martin (1 fichier) »** | No buttons; a name that is not in your thirty documents |
| A3 | Click **« Paul Martin (1 fichier) »** | The answer is replaced. Under it: **« Recherche ciblée à votre demande : 1 document sur 30 — Paul Martin »** with the buttons « Élargir à ma sélection » and « Voir les fichiers ». The three Martin buttons are gone. Click « Voir les fichiers »: the list shows `paul-martin.txt` and the button now reads « Masquer les fichiers ». Open « Sources »: only `paul-martin.txt`. (If the line reads « Recherche élargie à toute votre sélection : peu de résultats dans les documents ciblés. » instead, say so: the closest passage was far from the question.) | The Martin buttons still there; a file other than `paul-martin.txt` in « Sources » |
| A4 | Click **« Élargir à ma sélection »** | The answer is replaced again. The line and every button under it are gone: the search ran on everything you selected, and the question's names were not read this time. « Sources » may list several files | The line still there |
| A5 | Ask `Que dit le Docteur Dupond du bilan sanguin ?` (a misspelt name) | Answered. Under the answer: « Vouliez-vous dire : » and one button **« Jean Dupont (3 fichiers) »** | No button |
| A6 | Click **« Jean Dupont (3 fichiers) »** | The answer is replaced. Under it: **« Recherche ciblée à votre demande : 4 documents sur 30 — Jean Dupont »**. The « Vouliez-vous dire » row is gone. « Voir les fichiers » lists `dupont-bilan-2026.txt`, `dupont-horaires.txt`, `dupont-lettre-confrere.txt` and `rappel-m-dupont.txt` | Another count; the row still there |
| A7 | Run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_diagnostics.py` | The lines of A3 and A6 read `chip_files` with `applied 1`; the first line (A1) reads `applied 0` and `would 1` | A1 with `applied 1` |

**You leave with:** `suggest`, « Tous les documents » ticked, the application running, a conversation holding steps A1 to A6.

## Part B - `auto`: the line, the files, the button that widens

**You start with:** what Part A left.

1. Click « Effacer la conversation » (under the conversation) and confirm with « Effacer la conversation » in the window « Effacer cette conversation ? ». **Expected:** an empty conversation.
2. Click « Réglages » (top right). In the group **Connaissances**, the choice **Ciblage intelligent** reads « Suggestions seulement ». Choose **« Automatique »**, then click « Fermer ». **Resulting state:** mode `auto`, « Tous les documents » still ticked.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| B1 | Ask `Que dit le Docteur Jean Dupont du bilan sanguin ?` | Under the answer: **« Recherche ciblée : 4 documents sur 30 — Jean Dupont »**, with « Élargir à ma sélection » and « Voir les fichiers ». « Voir les fichiers » lists the four files of A6. « Sources » lists only files among those four. (« Recherche élargie à toute votre sélection... » instead: say so) | A file outside the four; no line |
| B2 | Click **« Élargir à ma sélection »** | The answer is replaced; the line is gone. « Sources » is what the whole selection gives | The line still there |
| B3 | Ask `Que contiennent les notes internes ?` (the previous answer is the widened one of B2: it has no names to carry) | **No line** under the answer: the question names nobody | A line |
| B4 | Ask `Que dit Madame Claire Martin du contrôle ?` | **« Recherche ciblée : 2 documents sur 30 — Claire Martin »**; « Voir les fichiers »: `martin-controle.txt` and `martin-ordonnance.txt` | Another count |
| B5 | Ask again `Que dit le Docteur Jean Dupont du bilan sanguin ?`, wait for the answer, then ask `Quand reçoit-il ?` | The second answer carries **« Recherche ciblée : 4 documents sur 30 — Jean Dupont »**: the question names nobody and is short (8 words or fewer), so the names of the previous answer are carried over | No line on the second answer |
| B6 *(a known limit, defect KBD-19; observed on 11 October 2026)* | Right after B5's answer, ask `Que contiennent les notes internes ?` | The question follows an answer that named Jean Dupont. The product already reads any question of 8 words or fewer as the follow-up of the previous one, so the names are carried: the narrowed search finds nothing about notes and widens by itself, so the line reads **« Recherche élargie à toute votre sélection : peu de résultats dans les documents ciblés. »**. **The answer is still about the Dupont documents, not about the notes**, because a short question is also searched together with the previous question: the widen button cannot fix it. Workaround: a longer question, or « Effacer la conversation » | Nothing to file; tell the agent if the answer is about the notes |
| B7 *(a known limit, defect KBD-17, and its remedy)* | Ask `Le Docteur Dupont a-t-il parlé de la photocopieuse ?` | The line reads « Recherche ciblée : 4 documents sur 30 — Jean Dupont ». The answer may say the documents do not mention the photocopier although `note-photocopieuse.txt` does. If it does, click **« Élargir à ma sélection »**. **Expected after the click:** `note-photocopieuse.txt` appears in « Sources ». *Could not be walked: it depends on the embedding model.* If it does not appear, tell the agent | Nothing to file if the first answer was wrong; tell the agent what you see after the click |

**You leave with:** mode `auto`, « Tous les documents » ticked, the application running, a conversation holding B1 to B7.

## Part C - the `@` list, the chips, the name that is outside the selection

**You start with:** what Part B left. The box is empty and no chip is above it.

1. Click « Effacer la conversation » and confirm.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| C1 | In the box type `Que dit @Dup` | Under the box a list with one line: **« Jean Dupont »**, « Personne · 3 fichiers », highlighted. Press **Enter**: the box now reads `Que dit Jean Dupont ` and a **chip « Jean Dupont »** is above the box. Type `du bilan sanguin ?` and press Enter to send. The answer carries **« Recherche ciblée à votre demande : 4 documents sur 30 — Jean Dupont »** and the chip is gone from above the box | The list shows nothing; the question sent has anything added to it |
| C2 | Type `@Mar` | Three lines: « Claire Martin · Personne · 2 fichiers », « Paul Martin · Personne · 1 fichier », « Pierre Martin · Personne · 1 fichier ». Press **↓** twice: the third is highlighted; press **↑**: the second is. Press **Escape**: the list closes and the box still reads `@Mar`. Then select all the text in the box (Ctrl+A) and delete it | The list does not close; a fourth name |
| C3 | Click the **« @ »** button (next to the model choice) | The box reads `@` and the list says « Tapez les premières lettres d'un nom. ». Type `Cl`: one line, « Claire Martin ». **Click it with the mouse**: the box reads `Claire Martin ` and a chip « Claire Martin » appears. Click the **×** of the chip: it goes away. Clear the box (Ctrl+A, Delete) | The chip stays after the ×; the mouse click does nothing |
| C4 | Type `@cab` | « Aucun nom de votre sélection ne commence ainsi. » (`cabinet des Lilas` is only a candidate, never offered). Clear the box | A name offered |
| C5 | Open « Documents utilisés » and **untick** these four files: `dupont-bilan-2026.txt`, `dupont-horaires.txt`, `dupont-lettre-confrere.txt`, `rappel-m-dupont.txt`. The line reads « Documents utilisés : 26 fichiers ». Type `@Dup` in the box | No name in the list. One clearly separate line: **« Jean Dupont figure dans 3 fichiers non sélectionnés »** with the button **« Ajouter à ma sélection »** | Jean Dupont offered as a name of the selection |
| C6 | Click **« Ajouter à ma sélection »** | The three Dupont files are now ticked in the Documents card (`rappel-m-dupont.txt` is not, it does not name him): « Documents utilisés : 29 fichiers ». The box still reads `@Dup` and the list now has the line « Jean Dupont · Personne · 3 fichiers » and no separate line. Press Escape, clear the box, then **tick** `rappel-m-dupont.txt`: « Documents utilisés : tous » | The files are not ticked; the separate line stays |
| C7 | **Untick** « Tous les documents » (« Documents utilisés : aucun »). Hover the « @ » button, then type `@Dup` in the box | The « @ » button is greyed; its hover says « Choisissez d'abord des fichiers dans le panneau de gauche. » (a missing tooltip is not a bug, say so). The box offers **no list at all**. Clear the box, then **tick** « Tous les documents » again | A list with nothing selected |

**You leave with:** mode `auto`, « Tous les documents » ticked (« Documents utilisés : tous »), the application running, a conversation holding C1 only, an empty box, no chip.

## Part D - the settings, the developer panel, the two languages

**You start with:** what Part C left.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| D1 | Click « Réglages ». Look at the group **Connaissances**. Open the choice **Ciblage intelligent** | It reads « Automatique »; the three entries, in order: « Désactivé », « Suggestions seulement », « Automatique ». Close the list without changing it. The box « Masquer les noms dans les suggestions » is ticked | Another order or label |
| D2 | Click **« Avancé »** | Five numbers: « Nombre minimal de fichiers pour cibler » **8**; « Part maximale de fichiers pour qu'un nom cible » **60** %; « Part minimale de fichiers écartés » **30** %; « Nombre maximal de documents ciblés » **15**; « Seuil d'élargissement » **0.65** (or 0,65). Also the boxes « Noter les durées dans un journal » (ticked) and « Afficher le panneau de diagnostics » (not ticked). *(These are the product's defaults; yours differ only if you changed them before.)* | |
| D3 | In « Nombre minimal de fichiers pour cibler » type `99999` and press **Tab**; then type `8` and press Tab. In « Part maximale... » type `250`, Tab; then `60`, Tab. In « Seuil d'élargissement » type `3`, Tab; then `0.65`, Tab | After each first entry the field shows the limit it was pulled back to: **10000**, **100**, **1.5**. After the second entries: 8, 60, 0.65 again | A value outside the limit stays on screen |
| D4 | Tick **« Afficher le panneau de diagnostics »**, click « Fermer » | Under the last answer a closed disclosure **« Diagnostics (développeur) »**. Open it. « Cette réponse »: Chemin « documents », Sélection « 30 documents, 0 classeurs », Mode « automatique », Résultat « appliqué », Stratégie « fichiers des noms choisis », Fichiers ciblés « 4 documents, 0 classeurs », Réduction « 86 % ». « Questions enregistrées »: a row **Questions** with a number N. **No question, no name and no file name anywhere in the panel** | A name or a file name in the panel |
| D5 | Run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_diagnostics.py` | `Questions recorded:` is the **same N** as the panel's « Questions » | Two different numbers |
| D6 | « Réglages », **untick** « Afficher le panneau de diagnostics », « Fermer » | The disclosure under the answer is gone | The panel stays |
| D7 | « Réglages », « Langue », choose the English entry, « Fermer » | The window is in English. The answer of C1 now reads **« Targeted search, as you asked: 4 documents out of 30 — Jean Dupont »** with « Widen to my selection » and « See the files ». Then « Settings », « Language », choose the French entry, close | A French sentence left in the English window, or the reverse |

**You leave with:** `auto`, « Tous les documents » ticked, French, the diagnostics panel off, the five numbers at their defaults, the application running.

## Part E - the old screen, with the knowledge base off

**You start with:** what Part D left.

1. « Réglages », **Ciblage intelligent** : **« Désactivé »**, « Fermer ». **Resulting state:** mode `off`, « Tous les documents » still ticked.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| E1 | Ask `Que dit le Docteur Jean Dupont du bilan sanguin ?` | An ordinary answer with « Sources ». **No line and no button under it** | A line |
| E2 | Ask `Que dit le Dr Martin ?` | An ordinary answer. **No « Plusieurs personnes correspondent » buttons**: with the base off nothing is read | The buttons |
| E3 | Run `python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_diagnostics.py` | `Questions recorded:` is **still N** (the mode `off` writes no row) | N + 2 |
| E4 | Ask `Combien de documents y a-t-il dans le dossier ?` | Answered from the folder's inventory, no AI: **30** | |
| E5 | Ask `Quand le Docteur Dupont reçoit-il ?` | An answer from `dupont-horaires.txt` (lundi et jeudi, de 9 h à 12 h), cited in « Sources » | |
| E6 | « Réglages », **Ciblage intelligent** : « Suggestions seulement », « Fermer » | | |

**You leave with:** mode `suggest`, « Tous les documents » ticked, the application running. **Close the application.**

## Final restore (the only one before Part F)

**You start with:** the application closed.

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot07_helper.py restore
python docs\test-reports\knowledge-base-pass-1\human-tests\lot07_helper.py status
```

**Expected:** « restored index.sqlite3 », « restored settings.json », and `status` showing **your own Documents folder** (not `kb-lot07`) and `writeTimingLog : False`. The `kb-lot07` folder can be deleted. **If you do not run Part F, the test ends here.**

## Part F (optional, last) - two spreadsheets: « Classeur choisi »

**You start with:** the application closed, your own settings and your own index put back by the final restore above (your own Documents folder, your own mode). This part changes your Data folder and your settings; **its own last step undoes exactly those and nothing else**. It does not use the thirty documents: the Documents card stays on your own folder, with nothing ticked (the selection is empty each time the application starts).

1. One line at a time, then start the application:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py backup
python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py prepare-data
cd apps\desktop
pnpm tauri dev
```

2. « Réglages » : **Ciblage intelligent** « Automatique »; « Avancé », « Nombre minimal de fichiers pour cibler » : `2` then Tab; « Fermer ». Data card : « Analyser ». The three files are green. On the Documents card make sure « Documents utilisés : aucun ». In the Data card's list tick **only** `rdv-mars-2026.xlsx` and `factures-fournisseurs-2026.xlsx`.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| F1 | Ask `Combien de lignes pour Dupont J ?` | **No question about which spreadsheet.** `Nombre de lignes : 2.` and, under the answer, **« Classeur choisi : rdv-mars-2026.xlsx — Dupont J y figure »** (the name may read « Jean Dupont » if your index linked the two) with the button « Choisir un autre classeur ». *The figure 2 is the one lot 6 gave. The name's wording could not be walked on your index: judge the file and the button. Because the thirty documents are not your Documents folder here, the base knows the patient only from the spreadsheet, so « Dupont J » is the likely wording.* | The which-spreadsheet question |
| F2 | Click **« Choisir un autre classeur »** | The answer is replaced by « Plusieurs tableurs sont sélectionnés. Lequel voulez-vous ? Nommez-le dans la question : » with the two spreadsheets as buttons, and **no line** | The line still there |

3. **Undo this part, and only this part** (close the application first):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot05_helper.py restore
python docs\test-reports\knowledge-base-pass-1\human-tests\lot07_helper.py status
```

   **Expected:** « restored index.sqlite3 », « restored settings.json », your own Data folder and your own Documents folder, `writeTimingLog : False`.

## Results table

| Step | Expected | Observed | OK? | Screenshot |
| --- | --- | --- | --- | --- |
| P3 | four people active, two organisations candidates | | | |
| A1 | no line in `suggest` | | | |
| A2 | « Plusieurs personnes correspondent à « Dr Martin » » + 3 buttons | | | |
| A3 | « Recherche ciblée à votre demande : 1 document sur 30 — Paul Martin » | | | |
| A4 | line gone after « Élargir à ma sélection » | | | |
| A5 | « Vouliez-vous dire : » + « Jean Dupont (3 fichiers) » | | | |
| A6 | « Recherche ciblée à votre demande : 4 documents sur 30 — Jean Dupont » | | | |
| A7 | `chip_files` `applied 1` on A3 and A6 | | | |
| B1 | « Recherche ciblée : 4 documents sur 30 — Jean Dupont » + files | | | |
| B2 | widen | | | |
| B3 | no line | | | |
| B4 | « 2 documents sur 30 — Claire Martin » | | | |
| B5 | names carried into « Quand reçoit-il ? » | | | |
| B6 | a short question carries the names (KBD-19) | | | |
| B7 | the remedy for KBD-17 | | | |
| C1 | `@Dup`, Enter, chip, sent | | | |
| C2 | `@Mar`, arrows, Escape | | | |
| C3 | « @ » button, mouse, × | | | |
| C4 | `@cab`: nothing | | | |
| C5 | outside line | | | |
| C6 | « Ajouter à ma sélection » | | | |
| C7 | nothing selected: no list | | | |
| D1 | settings group | | | |
| D2 | five numbers | | | |
| D3 | limits | | | |
| D4 | developer panel | | | |
| D5 | same N | | | |
| D6 | panel gone | | | |
| D7 | English and back | | | |
| E1 | off: no line | | | |
| E2 | off: no buttons | | | |
| E3 | N unchanged | | | |
| E4 | 30 | | | |
| E5 | schedule | | | |
| F1 | « Classeur choisi » | | | |
| F2 | « Choisir un autre classeur » | | | |

## Findings

Bugs go to the same lot branch before the merge; ideas go to the report's open questions. The agent says first whether the product or the protocol was wrong.
