# Human test - the file finder (search field, set sentences, rank sentence)

Written for the owner, who plays it, and for the agent who reads the results. Instructions in English; every sentence you type and every label you read is French, quoted exactly.

**Branch to be on:** `feat/file-finder`. **Build:** from `apps\desktop`, `pnpm tauri dev`. **Services:** none needed except for step E3 (the AI machine). **Duration:** about 25 minutes.

**What it shows.** The search field in the two folder cards: typing a word selects the analysed files that hold it (the only ones the chat then uses), finds a number written with or without separators, says which files it found but cannot select, and keeps your selection when you clear it. The chat sentence « Quels documents parlent de X ? » answered from the index with no AI call, limited to the selected files. The sentence « Quel est le dernier courrier reçu ? » answered with its ranking. The message that tells you to press « Analyser » when the index was written before the file profile existed.

**Rules of the script.** Type each sentence exactly as written. Before every chat question press « Effacer la conversation » and confirm. A part says what you start with and what you leave with; there is **no undo** in the middle, one backup at the start and **one restore at the very end** (Part Z). Every command is complete and is run from the repository root `C:\Users\elise\Documents\CURSOR\AssistantCabinetAI`. The window was **not seen by the agent**: every answer below was produced by the same code on the same invented files (`print_the_answers_of_the_human_test` in `tests/file_finder_walk.rs`, run on temporary copies of the 30 documents and 8 workbooks that `prepare 30` writes); the lines on screen are what you check. The setting stays « Suggestions seulement ».

---

## Part 0 - Preparation (3 min)

**You start with:** the application **closed**; your settings and index as you left them; a Documents folder already chosen in the application.

0.1 Back up your index, your settings and the key file (the only backup of this test):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py backup
```

You read `saved   settings.json`, then `saved` or `absent (nothing to save)` for the index and the key, then `Backup in C:\Users\elise\kb-backup-lot-11`.

0.2 Write the invented library of 30 documents and its 8 workbooks beside your Documents folder and point the application at them:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py prepare 30
```

You read `wrote 30 invented documents in ...`, `wrote 8 invented workbooks in ...`, and `"knowledgeMode" is "suggest"`.

**You leave with:** the application closed; « Dossier des documents » at `kb-lot11` (30 invented documents, not analysed); « Dossier des données » at `kb-lot11-data` (8 workbooks, not analysed); the mode « Suggestions seulement ».

---

## Part A - The old index, then Analyse (about 5 min)

**You start with:** what Part 0 leaves. Start the application: from `apps\desktop`, `pnpm tauri dev`.

A1 Read the two cards before pressing anything. If your index was last analysed **before** this branch (before the file profile existed), you read under the counts of « Dossier des documents » the line **`L'index doit être refait : appuyez sur Analyser.`**, and the same line on « Dossier des données » if you ever analysed data. If you read it on neither card, your index already holds file profiles: that is correct too, go on. Nothing is analysed by itself in either case: no spinner, no progress bar.

A2 On « Dossier des documents » press « Analyser ». Wait for the end. You read **30 fichiers** and **25 analysés** (the five files in formats the product does not read yet, among them `2026-09-09_courrier-orl.odt`, stay unread).

A3 On « Dossier des données » press « Analyser ». You read **8 fichiers**. (The count of analysed workbooks is not checked here: it only has to be the number the card already showed you for these eight files in earlier tests.)

A4 The line `L'index doit être refait : appuyez sur Analyser.` is gone from both cards.

**You leave with:** both folders analysed, nothing ticked (« Documents utilisés : aucun », « Données utilisées : aucune »), mode « Suggestions seulement ». Open the list on « Dossier des documents » (click « Documents utilisés : aucun »): above it, a field with the grey text « Rechercher dans les documents ».

---

## Part B - The search field of the Documents card (about 8 min)

**You start with:** what Part A leaves, the list open. Each search is typed in the field after clearing it by hand (select the text, delete it, then type); wait one second after typing.

B1 Type `lamal`.

- **Now:** under the field `Sélection par la recherche « lamal » : 2 fichiers.`; the list shows only `caisses-assurances/suisse/2026-03-20_decompte-css-rochat-sophie.pdf` and `caisses-assurances/suisse/2026-03-26_decompte-helsana-martin-marie.pdf`, both ticked; the line above the list reads « Documents utilisés : 2 fichiers »; two buttons « Tous les fichiers » and « Réinitialiser ».

B2 Replace the text by `Genève` (with the accent), then, as a second search, by `geneve` (without).

- **Now (both):** `Sélection par la recherche « ... » : 2 fichiers.` with `courriers-entrants/2026-02/2026-02-12_courrier-orl-dupont-jean.pdf` and `laboratoire/2026-07/2026-07-18_resultats-analyses-favre-laurent.pdf`. The previous selection is replaced, not added to.

B3 Replace the text by `orl`.

- **Now:** `Sélection par la recherche « orl » : 4 fichiers. Trouvés mais non sélectionnables : 1 fichier non analysé.` The list shows five files: four ticked (`2026-01-31_courrier-orl-cuendet-mathieu.pdf`, `2026-02-12_courrier-orl-dupont-jean.pdf`, `2026-06-25_courrier-orl.docx`, `2026-08-17_courrier-orl-roy-lucas.md`) and `courriers-entrants/2026-09/2026-09-09_courrier-orl.odt`, a format the product does not read, with a box you cannot tick.

B4 Replace the text by `7563052577843` (an AVS number without its dots).

- **Now:** `Sélection par la recherche « 7563052577843 » : 1 fichier.` and `caisses-assurances/suisse/2026-03-26_decompte-helsana-martin-marie.pdf`. That file writes the number `756.3052.5778.43`.

B5 Replace the text by `zzzz`.

- **Now:** `Aucun fichier trouvé pour « zzzz ». Seuls les mots eux-mêmes sont cherchés.`; the list is empty; the line above reads « Documents utilisés : aucun » with its warning triangle.

B6 Replace the text by `annecy`, then press « Tous les fichiers ».

- **Now:** `Sélection par la recherche « annecy » : 8 fichiers.`; after the button the list shows the 30 files (the button now reads « Résultats seulement »), only the 8 files of `annecy` still ticked, and the line above still reads « Documents utilisés : 8 fichiers ». Press « Résultats seulement »: the 8 files again.

B7 Select the text in the field and delete it (the field is empty).

- **Now:** the field is empty, the two buttons and the line under the field are gone, the list shows the 30 files, and **the 8 files stay ticked** (« Documents utilisés : 8 fichiers »).

B8 Type `lamal` again, then press « Réinitialiser ».

- **Now:** the field is empty, the list shows the 30 files, and the 2 `lamal` files are the ones ticked (« Documents utilisés : 2 fichiers »): « Réinitialiser » ends the filter and changes nothing in the selection.

**You leave with:** « Documents utilisés : 2 fichiers » (the two `decompte` files), the field empty, the list open.

---

## Part C - The chat sentences, with no AI call (about 6 min)

**You start with:** what Part B leaves. The answers below need no AI machine: you may even leave it stopped. The answer comes at once, with no « Recherche » wait.

C1 With the two `decompte` files ticked, type: `Quels documents parlent de la LAMal ?`

- **Now:** `Recherche du mot « LAMal » : 2 fichiers dans les documents sélectionnés.` then the line `Seul le mot lui-même est cherché : un synonyme ou une autre forme du mot ne serait pas trouvé.` then the two files, each with `(page 1)` and a line starting `Extrait : tient lieu de décision au sens de la LAMal.` There is no « Sources » block and no generated sentence.

C2 Press « Effacer la conversation » and confirm. Tick « Tous les documents » (« Documents utilisés : tous »). Type: `Quels documents parlent de Genève ?`

- **Now:** `Recherche du mot « Genève » : 2 fichiers dans les documents sélectionnés.` and the two files of B2; the first extract reads `février 2026 Dr Jean Dupont Hôpitaux universitaires de Genève Route de Lausanne 36, 1201 Genève ...`. Last line: `Sans texte lisible, donc non cherchés : 5 fichiers.` (the five files nothing could be read from, among the 30 you now use).

C3 Clear the conversation. In the field type `geneve` (2 files selected), then type: `Quels documents parlent de la LAMal ?`

- **Now:** `Recherche du mot « LAMal » : 0 fichier dans les documents sélectionnés.` The two `LAMal` files exist but are outside your selection: nothing outside it is searched.

C4 Clear the conversation. Tick « Tous les documents ». Type: `Quels documents parlent de la franchise ?`

- **Now:** 2 fichiers, the two `decompte` files, with extracts `...après déduction de la franchise annuelle de CHF 2000.00 ...` (CSS) and `... de CHF 1000.00 ...` (Helsana), and the last line `Sans texte lisible, donc non cherchés : 5 fichiers.`

C5 Clear the conversation. Type: `Quel est le dernier courrier reçu ?`

- **Now:** `Parmi 13 fichiers lisibles dont le nom contient « courrier », le plus récent est le premier de cette liste :` then five lines, each `- <path> — <date> (date au début du nom du fichier)`: `courriers-entrants/pharmacies/2026-08-26_courrier-pharmacie.pdf — 2026-08-26`, `courriers-entrants/2026-08/2026-08-17_courrier-orl-roy-lucas.md — 2026-08-17`, `caisses-assurances/france/2026-08-11_courrier-cpam74-dupont-jean.pdf — 2026-08-11`, `courriers-entrants/2026-07/2026-07-25_courrier-cardiologie-girard-frederic.pdf — 2026-07-25`, `courriers-entrants/2026-07/2026-07-20_courrier-dermatologie.docx — 2026-07-20`. No sentence about mixed dates (every date comes from a name).

C6 Clear the conversation. Untick « Tous les documents » (« Documents utilisés : aucun »). Type: `Quels documents parlent de la LAMal ?`

- **Now:** `Aucun document sélectionné pour cette conversation.` (nothing is searched when nothing is selected).

**You leave with:** « Documents utilisés : aucun », the conversation cleared.

---

## Part D - The search field of the Data card (about 3 min)

**You start with:** what Part C leaves. Open the list on « Dossier des données » (click « Données utilisées : aucune »): above it a field « Rechercher dans les tableurs ».

D1 Type `dupont`.

- **Now:** `Sélection par la recherche « dupont » : 2 fichiers.` with `patients-fr.csv` and `planning-garde-2026.xlsx` ticked; « Données utilisées : 2 fichiers ».

D2 Replace the text by `montant`.

- **Now:** 4 fichiers: `contrats-2026.xls`, `contrats-2026.xlsx`, `factures-fournisseurs-2026.xlsx`, `rendez-vous-2026.xlsx` (the word is a column header).

D3 Delete the text; press « Tous les tableurs » to untick everything.

- **Now:** « Données utilisées : aucune ». The Documents card was not touched by any of D1 to D3.

**You leave with:** « Documents utilisés : aucun », « Données utilisées : aucune », conversation cleared.

---

## Part E - A sentence that keeps its road (about 2 min)

**You start with:** what Part D leaves.

E1 Search `lamal` in the Documents card (2 files selected). Type: `Combien de documents ?`

- **Now (unchanged by this branch):** a folder answer `2 fichiers ont été analysés.` (the two selected files) with no extract.

E2 Type: `Quels documents de 2026 parlent de la LAMal ?` (a filter word between « documents » and « parlent »).

- **Now:** this one is **not** a word search: no line « Recherche du mot ». It goes the ordinary way (the AI machine answers with « Sources »). Skip E2 if the AI machine is stopped; the point is only that the answer does not start with « Recherche du mot ».

E3 (needs the AI machine) Type: `Que dit le courrier de Marie Dupont ?`

- **Now:** an ordinary sourced answer; judge only that it is not a list of files.

**You leave with:** anything. Close the application.

---

## Part Z - Restore (2 min)

**You start with:** the application closed.

Z1 Put your index and settings back:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py restore
```

You read `restored settings.json` (and `restored index.sqlite3` if there was one), and a note that `kb-lot11` and `kb-lot11-data` stay on disk with invented files only.

**You leave with:** the application closed, your own settings and index back, as before Part 0.

---

## Results table

| Step | Expected | Observed | OK? |
| --- | --- | --- | --- |
| A1 | the line « L'index doit être refait » on a pre-profile index, nothing running by itself | | |
| A2 / A3 | 30 fichiers, 25 analysés / 8 fichiers | | |
| A4 | the line is gone from both cards | | |
| B1 | « lamal » : 2 fichiers (the two `decompte`) | | |
| B2 | « Genève » and « geneve » : the same 2 fichiers, selection replaced | | |
| B3 | « orl » : 4 selected, « 1 fichier non analysé » (the `.odt`, no box) | | |
| B4 | the number without dots finds the Helsana file | | |
| B5 | « Aucun fichier trouvé pour « zzzz » ... », « Documents utilisés : aucun » | | |
| B6 | « Tous les fichiers » shows 30, 8 stay ticked | | |
| B7 | clearing the field keeps the 8 ticked | | |
| B8 | « Réinitialiser » keeps the selection (2 ticked) | | |
| C1 | « Recherche du mot « LAMal » : 2 fichiers » with extracts, no « Sources » | | |
| C2 | Genève : 2 fichiers | | |
| C3 | LAMal outside the selection : 0 fichier | | |
| C4 | franchise : 2 fichiers, CHF 2000.00 and CHF 1000.00 | | |
| C5 | the ranking of 13, first `2026-08-26_courrier-pharmacie.pdf` | | |
| C6 | « Aucun document sélectionné pour cette conversation. » | | |
| D1 / D2 / D3 | dupont : 2 ; montant : 4 ; « aucune » | | |
| E1 / E2 / E3 | folder answer ; no « Recherche du mot » ; ordinary answer | | |
| Z1 | `restored settings.json` | | |

## Findings

A step that does not match is first a finding about this protocol (say which step), then about the product. Write what you saw; nothing is asked in the chat.
