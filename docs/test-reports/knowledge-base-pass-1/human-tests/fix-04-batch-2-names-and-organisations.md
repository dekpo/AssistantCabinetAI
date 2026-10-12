# Human test - fix 4, batch 2 (names and organisations)

Written for the owner, who plays it, and for the agent who reads the results. Instructions in English; every sentence you type and every label you read is French, quoted exactly.

**Branch to be on:** `fix/kb-names-and-organisations`. **Build:** from `apps\desktop`, `pnpm tauri dev`. **Services:** the AI machine your « Réglages » point at (the tower `http://192.168.1.83:8080`, or the Docker stack of this PC); say which one in the results table. **Duration:** about 25 minutes, of which about 10 are the Analyse of 100 documents.

**What it shows.** Four things the extractor now reads: a hospital or an insurer that carries no marker word (`CHUV`, `HUG`) is an organisation, and the acronym and the full name are one entity; a person alone on a line in the address block of a letter is found; a surname with an apostrophe (`D'Amico`) is one person, and a question for `Giuseppe D'Amico` can no longer land on another `D'Amico`; two people whose given name and surname are each other's reverse (`Lucas Roy`, `Roy Lucas`) stay two people. Plus one repair: a name you delete and restore gets its table back at the next Analyse of the Data folder.

**Rules of the script.** Type each sentence exactly as written. Before every question, press « Effacer la conversation » and confirm. A step says what you start with and what you leave with; there is **no undo** in the middle, one backup at the start and **one restore at the very end** (Part Z). Every command is written in full and is run from the repository root `C:\Users\elise\Documents\CURSOR\AssistantCabinetAI`. The window itself was **not seen by the agent**: every figure comes from the same code run on the same files (said in each step); the lines on screen are what you check. The commands of Part 0 and Part Z are the ones of the previous batch's test, which the agent walked on copies; nothing in them changed.

---

## Part 0 - Preparation (5 min)

**You start with:** the application **closed**; your settings and index as you left them; a Documents folder already chosen in the application.

0.1 Back up your index, your settings and the key file (the only backup of this test):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py backup
```

You read `saved   settings.json`, then `saved` or `absent (nothing to save)` for `index.sqlite3` and `knowledge-identifier.key`, then `Backup in C:\Users\elise\kb-backup-lot-11`.

0.2 Write the invented library of 100 documents and its workbooks beside your Documents folder, and point the application at them:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py prepare 100
```

You read `100 documents: 89 the application reads, ...` and `wrote 8 invented workbooks in ...\kb-lot11-data`.

0.3 Switch the targeting to « Automatique »:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py mode auto
```

You read `knowledgeMode is now "auto"`.

**You leave with:** the application closed; « Dossier des documents » pointing at `kb-lot11` (100 invented documents, not analysed), « Dossier des données » at `kb-lot11-data` (8 invented tables, not analysed); the knowledge mode « Automatique »; your own files untouched.

---

## Part A - Analyse (about 12 min)

**You start with:** what Part 0 leaves. Start the application: from `apps\desktop`, `pnpm tauri dev`.

A1 Open « Réglages », group « Connaissances »: « Ciblage intelligent » reads « Automatique ». Go back to the conversation.

A2 On « Dossier des documents », press « Analyser ». Wait for the end (about 10 minutes: one 116-page guide takes most of it). You read `89 analysés` or close (a scanned page that OCR cannot read is counted apart).

A3 On « Dossier des données », press « Analyser ». You read `7 analysés` of the 8 files or close (`rendez-vous-2026.ods` is « pas un tableur (CSV ou XLSX) »).

A4 Selection: on « Dossier des documents » tick « Tous les documents » (« Documents utilisés : tous »). On « Dossier des données » tick **nothing** (« Données utilisées : aucune »).

**You leave with:** both folders analysed, « Tous les documents » ticked, nothing ticked in the data card, mode « Automatique ».

---

## Part B - The documents (about 8 min)

**You start with:** what Part A leaves. Before **every** question: press « Effacer la conversation » and confirm. The text of the AI's answer depends on the model: judge the line under it and the sources. For each question, press « Voir les fichiers » under the line to read the files. Source of every expectation below: the same code on the same 100 documents (`print_the_targeting_lines_of_the_human_test`).

B1 Type: `Quels courriers viennent du CHUV ?`

- **Now:** the line `Recherche ciblée : 5 documents sur 89 — Centre hospitalier universitaire vaudois`. The five files: `2026-03-09_courrier-orl-petrovic-elena.docx`, `2026-03-12_courrier-cardiologie.docx`, `2026-06-25_courrier-orl.docx`, `2026-08-01_courrier-rhumatologie.pdf`, `2026-10-01_courrier-dermatologie.pdf`. The name is the full one although you typed the acronym.
- **Before the batch:** no « Recherche ciblée » line at all (the base did not know `CHUV`).

B2 Type: `Quels documents concernent HUG ?`

- **Now:** `Recherche ciblée : 2 documents sur 89 — Hôpitaux universitaires de Genève`; files `2026-02-12_courrier-orl-dupont-jean.pdf` and `guide-pratique-cabinet.pdf`.

B3 Type: `Quels documents concernent les Hôpitaux universitaires de Genève ?`

- **Now:** the same line and the same two files as B2 (the acronym and the full name are one entity).

B4 Type: `Résume le dossier de Giuseppe D'Amico.`

- **Now:** `Recherche ciblée : 1 document sur 89 — Giuseppe D'Amico`; the file `2026-03-26_courrier-cpam74.pdf`. He is written alone on a line, above his street, in that letter.
- **Before the batch:** the question was narrowed on another person, `Hélène D'Amico` (a patient of a table), through the word `Amico`, and the line named her.

B5 Type: `Quels documents parlent de Lucas Roy ?` then, after clearing, `Quels documents parlent de Roy Lucas ?`

- **Now:** the first line reads `Recherche ciblée : 2 documents sur 89 — Lucas Roy` with `2026-08-17_courrier-orl-roy-lucas.md` and `2026-10-01_courrier-dermatologie.pdf`; the second reads `Recherche ciblée : 2 documents sur 89 — Roy Lucas` with `2026-04-20_courrier-rhumatologie-lucas-roy.pdf` and `2026-10-01_courrier-dermatologie.pdf`. Two different people, two different sets (the dermatology letter names both).
- **Before the batch:** one entity `Lucas Roy` held both, and the two questions searched the same files.

**You leave with:** the conversation cleared after B5, same selection and mode as at the start of the part.

---

## Part C - The knowledge window (about 6 min)

**You start with:** what Part B leaves. Open « Réglages », group « Connaissances », press « Gérer… ». The window « Connaissances » opens on the tab « Entités ».

C1 In « Rechercher un nom » type `chuv`; in « Type » choose « Organisation ». One row: **Centre hospitalier universitaire vaudois**, tag **Confirmée**, « 5 fichiers ». Click it: under « Noms reconnus » you read `CHUV` and the full name; under « D'où vient cette entité » the lines read « Repéré par : organisation connue du vocabulaire. Confiance : 0,80. ».

C2 Clear the search, keep « Type » on « Organisation », and look through the list: you find `Hôpitaux universitaires de Genève`, `Hospices Civils de Lyon`, `Réseau hospitalier neuchâtelois`, `CPAM du Rhône`, `Dupont Immobilier SA`, `Bureautique Léman SA`, `Papeterie du Lac SARL`. You do **not** find a name that starts with `Entre`, nor `Fils SARL`. Set « Type » to « Personne » and search `hôtel`: « Aucune entité ne correspond. »

C3 Tab « À vérifier », section « Correspondances possibles »: a pair `Roy Lucas` / `Lucas Roy` with the reason « prénom et nom dans l'ordre inverse », among a number of other pairs of the same reason (a person written surname first in plain case in one table and given name first in another: the base does not guess, it asks). Do nothing with them.

C4 Back to « Entités », « Type » on « Personne », search `Lucas Roy`: two rows, `Lucas Roy` and `Roy Lucas` (the second knows the first's name as its reversed order). Click the row named exactly **Lucas Roy**, then « Supprimer », confirm « Supprimer ». The note « Cette entité a été supprimée : elle n'apparaît plus et l'analyse ne la recrée pas. » and the button « Restaurer ». The strip says that **89 documents seront relus à la prochaine analyse**. Click « Restaurer ».

C5 Close the window. On « Dossier des données » press « Analyser » (nothing changed in the files). Reopen « Gérer… », search `Lucas Roy`, click the row named exactly **Lucas Roy**: « D'où vient cette entité » lists, besides the documents, a line « Tableur » for `patients-fr.csv`, colonne « nom_patient ». **Before the batch** that line was missing until the file changed.

**You leave with:** « Lucas Roy » restored (the tag « Saisie à la main » is normal: a restored name is yours from then on), the window closed, the conversation cleared.

*Why the agent trusts C1 to C5:* the same code, on the same 100 documents and the workbooks, printed the organisations, their short forms, the methods, the pairs and the restore walk (`print_the_names_of_the_human_test`, `print_the_restore_walk_of_the_human_test`: before 1 table mention, after the deletion 0, after the restore 0, after the Data Analyse 1).

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
| A2 | `89 analysés` or close | | |
| A3 | `7 analysés` or close | | |
| B1 | `Recherche ciblée : 5 documents sur 89 — Centre hospitalier universitaire vaudois`; the five files | | |
| B2 | `2 documents sur 89 — Hôpitaux universitaires de Genève`; `...orl-dupont-jean.pdf`, `guide-pratique-cabinet.pdf` | | |
| B3 | same line and files as B2 | | |
| B4 | `1 document sur 89 — Giuseppe D'Amico`; `2026-03-26_courrier-cpam74.pdf` | | |
| B5 | `2 documents sur 89 — Lucas Roy` / `2 documents sur 89 — Roy Lucas`, two different sets | | |
| C1 | one row, « Confirmée », « 5 fichiers », `CHUV` among the names, « organisation connue du vocabulaire » | | |
| C2 | the organisations listed; no `Entre...`, no `Fils SARL`; no person `hôtel` | | |
| C3 | the pair `Roy Lucas` / `Lucas Roy`, reason « prénom et nom dans l'ordre inverse » | | |
| C4 | « Cette entité a été supprimée... »; « 89 documents seront relus à la prochaine analyse » | | |
| C5 | a line « Tableur » for `patients-fr.csv` after the Data Analyse | | |
| Z1 | `restored settings.json`, `restored index.sqlite3` | | |

## Findings

A step that does not match is first a finding about this protocol (say which step), then about the product. Write what you saw; nothing is asked in the chat.
