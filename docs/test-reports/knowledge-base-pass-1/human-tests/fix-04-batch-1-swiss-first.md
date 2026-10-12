# Human test - fix 4, batch 1 (Switzerland first)

Written for the owner, who plays it, and for the agent who reads the results. Instructions in English; every sentence you type and every label you read is French, quoted exactly.

**Branch to be on:** `fix/kb-swiss-first`. **Build:** from `apps\desktop`, `pnpm tauri dev`. **Services:** the AI machine your « Réglages » point at (the tower `http://192.168.1.83:8080`, or the Docker stack of this PC); say which one in the results table. **Duration:** about 25 minutes, of which about 10 are the Analyse of 100 documents.

**What it shows.** Seven things that were wrong for a Swiss practice and are fixed: a number of the AVS is looked up in the documents instead of being « a file that does not exist »; a doctor written `Dre` is one person with her own name; a commune written in a table does not narrow the documents; an acronym (`HUG`) is searched, not listed as a file; the Analyse says which files it did not read; `CHF 1'450.00` is added up by the table engine; a workbook typed by name (`factures-fournisseurs-2026.xlsx`) is counted whole.

**Rules of the script.** Type each sentence exactly as written. Before every question, press « Effacer la conversation » and confirm (a short question is searched together with the previous one: product rule, KBD-19). A step says what you start with and what you leave with; there is **no undo** in the middle, one backup at the start and **one restore at the very end** (Part Z). Every command is written in full and is run from the repository root `C:\Users\elise\Documents\CURSOR\AssistantCabinetAI`. The window cannot be walked by the agent: every figure comes from the same code run on the same files (said in each step), the lines on screen are **not seen by the agent**.

---

## Part 0 - Preparation (5 min)

**You start with:** the application **closed**; your settings and index as you left them; a Documents folder already chosen in the application (the helper builds the test folders beside it).

0.1 Back up your index, your settings and the key file (the only backup of this test):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py backup
```

You read `saved   settings.json`, and for `index.sqlite3` and `knowledge-identifier.key` either `saved` or `absent (nothing to save)`, then `Backup in C:\Users\elise\kb-backup-lot-11`.

0.2 Write the invented library of 100 documents and its eight workbooks beside your Documents folder, and point the application at them:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py prepare 100
```

You read `100 documents: 89 the application reads, ...` (the others are in formats the product does not read) and `wrote 8 invented workbooks in ...\kb-lot11-data`.

0.3 Add the two amount tables (the same ten invented patients, written the Swiss way and the French way):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py swiss-file
```

You read `wrote patients-suisse.csv and patients-france.csv in ...\kb-lot11-data`.

0.4 Switch the targeting to « Automatique »:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py mode auto
```

You read `knowledgeMode is now "auto"`.

**You leave with:** the application closed; « Dossier des documents » pointing at `kb-lot11` (100 invented documents, not analysed), « Dossier des données » at `kb-lot11-data` (10 invented tables, not analysed); the knowledge mode « Automatique »; your own files untouched.

---

## Part A - Analyse (about 12 min)

**You start with:** what Part 0 leaves. Start the application: from `apps\desktop`, `pnpm tauri dev`.

A1 Open « Réglages », group « Connaissances »: « Ciblage intelligent » reads « Automatique ». Go back to the conversation.

A2 On « Dossier des documents », press « Analyser ». Wait for the end (about 10 minutes; the bar says « Analyse en cours »). Open the detail under the card. Among its lines you read **this one, exactly**:

> `8 fichiers n'ont pas été lus : leur format n'est pas pris en charge (2 .odt, 1 .eml, 1 .odp, 1 .ppt, 1 .pptx, 1 .rtf, 1 .vtt).`

Why: the product's own Analyse summary on these 100 documents (`print_the_unread_formats_of_the_sandbox_tiers`). The helper said « 11 it does not read »: three of those are tables (`.csv`, `.xlsx`) that belong to the other card, so they are not counted here. You also read `89 analysés` or close (a scan that OCR cannot read is counted apart). **If the line is missing or the numbers differ, it is a bug.**

A3 On « Dossier des données », press « Analyser ». You read `9 analysés` of the 10 files (`rendez-vous-2026.ods` is « pas un tableur (CSV ou XLSX) »; the card does not read that format).

A4 Selection: on « Dossier des documents » tick « Tous les documents » (the summary reads « Documents utilisés : tous »). On « Dossier des données » tick **nothing** (« Données utilisées : aucune »).

**You leave with:** both folders analysed, « Tous les documents » ticked, nothing ticked in the data card, mode « Automatique ».

---

## Part B - The documents (about 8 min)

**You start with:** what Part A leaves. Before **every** question: press « Effacer la conversation » and confirm.

B1 Type: `À qui appartient le numéro AVS 756.3052.5778.43 ?`

- **Before the fix** the answer was « Aucun fichier nommé 756.3052.5778.43 ne se trouve dans votre dossier de documents. », with no AI.
- **Now:** a normal answer written by the AI; the line « Réponse établie depuis votre dossier de documents, sans l'IA. » is **not** there; under the answer, a line `Recherche ciblée : 1 document sur 89`; « Sources » lists `2026-03-26_decompte-helsana-martin-marie.pdf` (the file that carries that number). What the AI writes in the sentence depends on the model; judge the sources.
- Source of the expectation: the real stack at 100 documents (`real-tier-100.md`, Q138): 1 candidate, the Helsana statement cited.

B2 Type: `Quels courriers a écrit la Dre Anne-Sophie Vuagniaux ?`

- **Now:** a line `Recherche ciblée : 3 documents sur 89 — Anne-Sophie Vuagniaux` (the name is **without** `Dre`). Press « Voir les fichiers »: three files, among them `2026-06-25_courrier-orl.docx` and `2026-10-01_courrier-dermatologie.pdf`. Write the three names in the results table.
- **Before the fix** the name in that line would have been `Dre Anne-Sophie Vuagniaux`, and `Anne-Sophie Vuagniaux` was found nowhere.

B3 Type: `Que dit la Dre Vuagniaux ?`

- **Now:** there is **no** row « Plusieurs personnes correspondent à « Dre Vuagniaux » : » (a patient, Gilles Vuagniaux, shares the surname; the title tells them apart). The line is either the same `Recherche ciblée : 3 documents sur 89 — Anne-Sophie Vuagniaux`, **or**, as the real run at 100 documents did for this very sentence, « Recherche élargie à toute votre sélection : peu de résultats dans les documents ciblés. » (the narrowed search found too little and the whole selection was searched: the known limit KBD-17, not part of this batch). If you see the widened line, note it; it is not a failure of this step.
- Source: the real stack, Q092 and Q093 (`real-tier-100.md`): the same 3 candidates; Q092 widened, Q093 did not.

B4 Type: `Quels documents concernent HUG ?`

- **Before the fix** the product answered with the list of **every file of the folder** (« Réponse établie depuis votre dossier de documents, sans l'IA. »).
- **Now:** a normal answer with « Sources » (the sentence itself depends on the model); **no** list of the whole folder; no « Recherche ciblée » line (the base does not know `HUG` as an organisation yet: next batch).

B5 Type: `Quels documents viennent de Genève ?`

- **Now:** **no** « Recherche ciblée » line at all (`Genève` is a commune of the tables: it no longer narrows the documents). Up to six sources are cited.
- **Before the fix** the search was narrowed to the files that contain the word, and the cap of 15 cut the list.

**You leave with:** the conversation cleared after B5 (press « Effacer la conversation »), same selection and mode as at the start of the part.

---

## Part C - The tables (about 5 min)

**You start with:** what Part B leaves. Prepare the selection: on « Dossier des documents » **untick** « Tous les documents » (the summary reads « Documents utilisés : aucun »); on « Dossier des données » tick « Tous les tableurs » (« Données utilisées : toutes »). Before every question press « Effacer la conversation » and confirm. The answers are computed by the table engine, without the AI: here, unlike in Part B, a label « sans l'IA » is the right one. The numbers may show a thin space: `8 941,54` is the same number.

C1 Type: `Quelle est la somme des montants dans patients-suisse.csv ?` You read **`Somme de Montant : 8 941,54 CHF.`** (then `Calculé sur 10 lignes.`). Before the fix the engine gave up and the question went to the AI.

C2 Type: `Quel est le montant maximum dans patients-suisse.csv ?` You read **`Plus grande valeur de Montant : 2 310,25 CHF.`** Before the fix it answered « which patient has the largest total » because the word `patients` of the file name was read as the column `Patient`.

C3 Type: `Quelle est la somme des montants dans patients-france.csv ?` You read **`Somme de Montant : 8 941,54 €.`** (the French file, unchanged: same total, other unit).

C4 Type: `Combien de lignes dans factures-fournisseurs-2026.xlsx ?` You read **`La feuille Factures contient 39 lignes.`** and **no** line « Compris comme : ... ». Before the fix the year of the file name was a filter and the answer was 0 lines with « Compris comme : N° facture = 2026 ».

**You leave with:** « Documents utilisés : aucun », « Données utilisées : toutes », the conversation cleared.

*Why the agent trusts C1 to C4:* the same engine, on the same two files and the same workbook, printed these exact answers (`print_the_data_answers_of_the_human_test`); the sentences above are the catalogue's, with the thin spaces of the French number format.

---

## Part Z - Restore (2 min)

**You start with:** anything. Close the application.

Z1 Put your index and settings back:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py restore
```

You read `restored settings.json`, `restored index.sqlite3` (and a note that the test folders `kb-lot11` and `kb-lot11-data` are still on disk: delete them when you wish, they hold only invented files).

**You leave with:** the application closed, your own settings and index back, as before Part 0.

---

## Results table

| Step | Expected | Observed | OK? |
| --- | --- | --- | --- |
| A1 | « Ciblage intelligent » reads « Automatique » | | |
| A2 | the line `8 fichiers n'ont pas été lus ... (2 .odt, 1 .eml, 1 .odp, 1 .ppt, 1 .pptx, 1 .rtf, 1 .vtt).` | | |
| A3 | `9 analysés` of 10 | | |
| B1 | no « Aucun fichier nommé »; `Recherche ciblée : 1 document sur 89`; source `...decompte-helsana-martin-marie.pdf` | | |
| B2 | `Recherche ciblée : 3 documents sur 89 — Anne-Sophie Vuagniaux`; the three file names: | | |
| B3 | no « Plusieurs personnes correspondent »; the same line as B2 or « Recherche élargie ... » | | |
| B4 | an answer with sources, not the list of the folder, no targeting line | | |
| B5 | no « Recherche ciblée » line | | |
| C1 | `Somme de Montant : 8 941,54 CHF.` | | |
| C2 | `Plus grande valeur de Montant : 2 310,25 CHF.` | | |
| C3 | `Somme de Montant : 8 941,54 €.` | | |
| C4 | `La feuille Factures contient 39 lignes.`, no « Compris comme » | | |
| Z1 | `restored settings.json`, `restored index.sqlite3` | | |

## Findings

A step that does not match is first a finding about this protocol (say which step), then about the product. Write what you saw; nothing is asked in the chat.
