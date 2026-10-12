# Human test - fix 4, batch 3 (table, router and check defects)

Written for the owner, who plays it, and for the agent who reads the results. Instructions in English; every sentence you type and every label you read is French, quoted exactly.

**Branch to be on:** `fix/kb-retrieval-rules`. **Build:** from `apps\desktop`, `pnpm tauri dev`. **Services:** the AI machine your « Réglages » point at (needed only by step D3). **Duration:** about 20 minutes.

**What it shows.** Four things you can see: a doctor written with his title in a table (`Dr Jean Dupont`) is counted like `Jean Dupont`; a month or a year asked of a date column the file does not settle is refused with the column named, not answered `0`; an appointments export with a title line above its header is read as a table; with documents and workbooks ticked together, a question about the documents is no longer blocked by « Plusieurs tableurs sont sélectionnés ». Plus one file route: `mars/neurologie.txt` names the file under `2026\mars`. **Not shown:** the false warning on a figure split by a line break (F-22) depends on the model copying that figure, so it is covered by tests only.

**Rules of the script.** Type each sentence exactly as written. Before every question press « Effacer la conversation » and confirm. A part says what you start with and what you leave with; there is **no undo** in the middle, one backup at the start and **one restore at the very end** (Part Z). Every command is complete and is run from the repository root `C:\Users\elise\Documents\CURSOR\AssistantCabinetAI`. The window was **not seen by the agent**: every answer below was produced by the same code on the same invented files (`print_the_answers_of_the_human_test`), the lines on screen are what you check. The setting stays « Suggestions seulement ».

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

0.3 Add this test's invented files (three workbooks, two documents):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\fix04_batch3_helper.py files
```

You read `wrote rdv-dates-ambigues.csv`, `wrote rdv-export-titre.csv`, `wrote rdv-medecins.csv`, `wrote 2026\janvier\neurologie.txt`, `wrote 2026\mars\neurologie.txt`.

**You leave with:** the application closed; « Dossier des documents » at `kb-lot11` (30 invented documents plus the two `neurologie.txt`, not analysed); « Dossier des données » at `kb-lot11-data` (11 files, not analysed); the mode « Suggestions seulement ».

---

## Part A - Analyse (about 5 min)

**You start with:** what Part 0 leaves. Start the application: from `apps\desktop`, `pnpm tauri dev`.

A1 On « Dossier des documents » press « Analyser ». Wait for the end. Note the number of documents analysed that the card shows: call it **N** (the same code reads 27 in a copy without text recognition; yours may differ by the scans).

A2 On « Dossier des données » press « Analyser ». You read `10 analysés` or close (`rendez-vous-2026.ods` is « pas un tableur (CSV ou XLSX) »).

**You leave with:** both folders analysed, nothing ticked yet in either card, mode « Suggestions seulement ».

---

## Part B - The tables (about 7 min)

**You start with:** what Part A leaves. Selection: on « Dossier des documents » tick **nothing** (« Documents utilisés : aucun »); on « Dossier des données » tick « Tous les tableurs » (« Données utilisées : toutes »). Each question names its workbook, so no choice of workbook is asked.

B1 Type: `Combien de lignes dans rdv-export-titre.csv ?`

- **Now:** `La feuille rdv-export-titre contient 10 lignes.` The file has a title line (`Export du 12/03/2026`) above its header.
- **Before the batch:** the file was read as one column, a table you could not question (register KBD-14).

B2 Type: `Combien de lignes pour Dr Jean Dupont dans rdv-medecins.csv ?` then, after clearing, `Combien de lignes pour Jean Dupont dans rdv-medecins.csv ?`

- **Now:** both read `Nombre de lignes : 3.` and `Compris comme : medecin = Dr Jean Dupont.`
- **Before the batch:** the first one read 0 (the word `Dr` was a filter no cell equals).

B3 Type: `Quelle est la somme de montant pour Dr Jean Dupont dans rdv-medecins.csv ?`

- **Now:** a sum of **150** with `Compris comme : medecin = Dr Jean Dupont.` **Before:** « Il n'y a aucune donnée à calculer ».

B4 Type: `Combien de lignes pour Dr Paul Martin dans rdv-medecins.csv ?` then, after clearing, `Combien de lignes pour Martin dans rdv-medecins.csv ?`

- **Now:** the first reads 2 (`medecin = Dr Paul Martin`); the second reads **3** and `Compris comme :` starts with `medecin parmi` (the two doctors called Martin).

B5 Type: `Combien de lignes pour Alice Archer en mars 2026 dans rdv-medecins.csv ?`

- **Now:** 2, with the months and the year in `Compris comme` (this file has dates above the 12th, so its date column is settled: unchanged by the batch).

B6 Type: `Combien de lignes pour Alice Archer dans rdv-dates-ambigues.csv ?`

- **Now:** `Nombre de lignes : 2.` (no date in the question).

B7 Type: `Combien de lignes pour Alice Archer en mars 2026 dans rdv-dates-ambigues.csv ?` then, after clearing, `Combien de lignes en 2026 dans rdv-dates-ambigues.csv ?` and `Combien de lignes pour Alice Archer en mars dans rdv-dates-ambigues.csv ?`

- **Now (the three):** no count. The message reads `Ce filtre porte sur un jour, un mois, une année ou une période, mais cette colonne de rdv-dates-ambigues.csv n'est pas une colonne de dates reconnue.`, then `Colonne concernée : date.`, then `Ses colonnes sont : patient, date, medecin.` Every day of that file is 12 or below, so the file does not say which number is the day.
- **Before the batch:** `0` for the first two, and `2` for the third with the month silently dropped.

**You leave with:** « Données utilisées : toutes », « Documents utilisés : aucun », conversation cleared after B7.

---

## Part C - A folder and a file name (about 3 min)

**You start with:** what Part B leaves. Selection: on « Dossier des documents » tick « Tous les documents » (« Documents utilisés : tous »); on « Dossier des données » untick everything (« Données utilisées : aucune »). The AI machine must be reachable for the answers; what you check is which file is used.

C1 Type: `Que dit mars/neurologie.txt ?`

- **Now:** the answer is built from one file; under « Sources » only `2026/mars/neurologie.txt` (the text speaks of March). **Before the batch:** « Aucun fichier nommé mars/neurologie.txt ne se trouve dans votre dossier de documents. »

C2 Type: `Que dit neurologie.txt ?`

- **Now (unchanged):** « Plusieurs fichiers s'appellent neurologie.txt. Lequel voulez-vous ? » with the two files `2026/janvier/neurologie.txt` and `2026/mars/neurologie.txt`.

C3 Type: `Que dit avril/neurologie.txt ?`

- **Now:** « Aucun fichier nommé avril/neurologie.txt ne se trouve dans votre dossier de documents. » (no `avril` folder: nothing is guessed).

**You leave with:** « Documents utilisés : tous », « Données utilisées : aucune », conversation cleared.

---

## Part D - Documents and tables together (about 5 min)

**You start with:** what Part C leaves. Selection: keep « Tous les documents » ticked and now also tick « Tous les tableurs » on « Dossier des données » (« Données utilisées : toutes »): documents and tables are both used.

D1 Type: `Combien de lignes ?`

- **Now (unchanged):** « Plusieurs tableurs sont sélectionnés. Lequel voulez-vous ? Nommez-le dans la question : » followed by the ten usable workbooks. A question about a table still asks.

D2 Type: `Combien de documents ai-je ?`

- **Now:** a folder answer of the kind « **N** ont été analysés. » with the number **N** of step A1, and no choice of workbook. **Before the batch:** a row count of a workbook.

D3 Type: `Que disent mes documents à propos de Jean Dupont ?`

- **Now:** no « Plusieurs tableurs sont sélectionnés » at all: the question is answered from the documents (an answer and « Sources » listing documents). The wording depends on the model; judge only that the choice of a workbook is **not** shown. **Before the batch:** this exact sentence was answered « Quel classeur ? ».

**You leave with:** both folders fully ticked, conversation cleared.

---

## Part Z - Restore (2 min)

**You start with:** anything. Close the application.

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
| A2 | `10 analysés` or close | | |
| B1 | `La feuille rdv-export-titre contient 10 lignes.` | | |
| B2 | 3 and `medecin = Dr Jean Dupont` for both sentences | | |
| B3 | sum 150 | | |
| B4 | 2 for `Dr Paul Martin`; 3 and `medecin parmi` for `Martin` | | |
| B5 | 2 | | |
| B6 | 2 | | |
| B7 | the message « n'est pas une colonne de dates reconnue », `Colonne concernée : date.`, three times | | |
| C1 | only `2026/mars/neurologie.txt` in « Sources » | | |
| C2 | the choice between the two `neurologie.txt` | | |
| C3 | « Aucun fichier nommé avril/neurologie.txt ... » | | |
| D1 | « Plusieurs tableurs sont sélectionnés. Lequel voulez-vous ? » | | |
| D2 | « N ont été analysés. », N of A1 | | |
| D3 | no choice of workbook; documents in « Sources » | | |
| Z1 | `restored settings.json` | | |

## Findings

A step that does not match is first a finding about this protocol (say which step), then about the product. Write what you saw; nothing is asked in the chat.
