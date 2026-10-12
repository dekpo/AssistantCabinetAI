# Human test - lot 11, Prove it (validation of the knowledge base)

Written for the owner, who plays it, and for the agent who reads the results. Instructions in English; every sentence you type and every
label you read is French, quoted exactly.

**Branch to be on:** `feat/kb-validation`. **Build:** from `apps\desktop`, `pnpm tauri dev`. **Services:** the AI machine your
« Réglages » point at (the Docker stack of this PC, `docker compose up -d`, or the tower at `http://192.168.1.83:8080`); write which one
in `02-results.md`. **Duration:** Part 0 15 min · A 45 · B 25 · C 40 · D 1 to 2 h (mostly waiting for an Analyse) · E spread over your
days · Z optional, overnight · R 5 min. You may stop between two parts: each part says the state it leaves, and nothing in the middle
needs undoing.

**What it is for.** Nothing in this application changed in lot 11. You are checking, with a library whose answers are known, whether the
intelligent targeting (« Ciblage intelligent ») can be trusted enough to become `Automatique` by default. You compare the three modes on the
**same** sentences: « Désactivé » (the search as it has always been), « Suggestions seulement » (the default: it never narrows the search by itself)
and « Automatique » (it searches only the files that talk about the names of the question). For each answer you note: **is it right**, **are the
right files cited**, **did it miss a file that holds the answer**, **did it add a file that does not**.

**Rules of the script** (broken, any of them makes a result worthless):

1. **Type each sentence exactly as written**, including the accents and the final point or question mark.
2. **Before every question, press « Effacer la conversation » and confirm.** A question of eight words or fewer is searched together with the
   previous one (the product's own rule): without this, the three modes would not be asked the same thing.
3. **The mode is changed in « Réglages » → group « Connaissances » → « Ciblage intelligent »**; it applies to the next question, no restart.
   The names are read **at the Analyse**, so the Analyse is done once, in « Suggestions seulement », before the comparison.
4. **A step says what you must start with and what you leave with.** If your screen does not match the start, stop and tell the agent which
   step; the protocol is wrong before the product is.
5. What the agent did **not** observe is said in the step (« not seen by the agent »): the application window cannot be walked by the agent;
   every figure below comes from the same code run on the same files.

The commands are run from the repository root (`C:\Users\elise\Documents\CURSOR\AssistantCabinetAI`); `H` below is shorthand for
nothing: **every command is written in full.**

---

## Part 0 - Preparation (15 min)

**You start with:** the application **closed**; your settings and index as you left them.

0.1 Look at what the script sees:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py status
```

You read: `Application     : closed`, `Core library    : present`.

0.2 Back up your index, your settings and the key file (the **only** backup of the test; the restore is the last step of the test):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py backup
```

You read `saved   settings.json` and, for each of `index.sqlite3` and `knowledge-identifier.key`, `saved` or `absent (nothing to save)`, then
`Backup in C:\Users\elise\kb-backup-lot-11`.

0.3 Write the large library of 1 000 documents (once, about ten seconds; it is used from part D, and kept out of git):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py generate
```

You read `running: ...gen-knowledge-sandbox.py --documents 1000 ...` then a block that ends with `"documents": 1000,`, `"readable": 878`,
`"unreadable": 122`.

0.4 Make the test folders beside your current Documents folder, with the first 30 documents of the library and its workbooks, and point the
application at them:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py prepare 30
```

You read: `wrote 30 invented documents in ...\kb-lot11`, `30 documents: 28 the application reads, 2 it does not read (formats: odt, vtt)`,
`scans (OCR): 3; PDF pages: 16`, `wrote 8 invented workbooks in ...\kb-lot11-data (formats: csv, ods, xls, xlsx)`, then your two folders, then
`"knowledgeMode" is "suggest", "writeTimingLog" is true and both health modules (France and Switzerland) are on.`

0.5 Empty the record of questions (numbers and codes only; no knowledge is touched):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py clear-log
```

You read `deleted retrieval-timings.jsonl` or `no retrieval-timings.jsonl to delete`, then `emptied kb_diagnostics (N row(s))` or
`kb_diagnostics does not exist yet`.

0.6 Start the application: in a terminal, `cd apps\desktop` then `pnpm tauri dev`. Wait for the window. At the top, the AI status reads
« IA Disponible » (if it reads « IA Indisponible », start the AI machine and press « Vérifier l'IA »).

0.7 Open « Réglages ». Check, and write in `02-results.md` the first one: « Adresse de l'IA » (the machine); group « Connaissances »:
« Ciblage intelligent » = « Suggestions seulement »; « Module santé »: both « Santé : France » and « Santé : Suisse » ticked; group « Avancé »:
« Noter les durées dans un journal » ticked. Close with « Fermer ».

0.8 **Start from an empty base.** (a) On the card « Dossier des documents », press « Reset », and confirm « Effacer l'analyse ? ». (b) On the card
« Dossier des données », press « Reset », and confirm « Effacer l'analyse des données ? ». (c) « Réglages » → « Gérer… » (group
« Connaissances ») → tab « Réinitialiser » → « Tout réinitialiser… » → confirm « Tout réinitialiser » → you read « La base de connaissances a été
réinitialisée. » Close the dialog.

0.9 Check it from the outside (read only; the application may stay open):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py names
```

You read `entities: 0   fingerprint of the list: e3b0c44298fc1c14` and `names of the test questions found in the base: none`.

**You leave with:** the application open; « Dossier des documents » = `kb-lot11` (30 files, **not analysed**); « Dossier des données » =
`kb-lot11-data` (8 files, **not analysed**); mode « Suggestions seulement »; both health modules ticked; the timing log on; the index and the
knowledge base empty; nothing ticked.

---

## Part A - 30 documents, the library that started the whole programme (45 min)

**You start with:** what Part 0 leaves.

A1. On « Dossier des documents », press « Analyser ». Write the **time it takes** (from the click to the end of the progress bar). You read on the card
(the exact figures come from the same code on the same files; the application window was **not seen by the agent**):

- `28 analysés` (30 files in the folder; the two the application does not read are an `.odt` and a `.vtt`, listed with the state that the card gives
  them: write what it says for them),
- the scanned documents read by text recognition: `3 fichiers lus par reconnaissance de texte`,
- the lines of the knowledge base: `40 personnes et organisations repérées, dont 0 déjà connues.` and `30 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).`

A2. On « Dossier des données », press « Analyser ». You read `7 analysés` of the 8 files (`rendez-vous-2026.ods` is « pas un tableur (CSV ou XLSX) »;
`contrats-2026.xls` is read: the engine reads that format although the card does not advertise it). The knowledge lines of that card: `194 personnes et organisations repérées, dont 17 déjà connues.`, `278 identifiants repérés ...`, `21 colonnes de tableur reconnues (...) dans 7 tableurs lus.`, `102 liens lus entre deux colonnes d'un tableur.` and `28 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.`

A3. Selection: on « Dossier des documents », tick « Tous les documents » (the summary reads « Documents utilisés : tous »). On « Dossier des données »
tick **nothing** (« Données utilisées : aucune »): with documents and several workbooks ticked, a plain question about the documents would be answered
« Quel classeur ? » (defect KBD-21, known).

A4. **The ten sentences, in « Suggestions seulement »** (the mode you are in). For each: press « Effacer la conversation » and confirm, type the sentence,
wait for the end, write the time under the answer (« Généré par ... en ... »), open « Sources », and fill the row in `02-results.md`. In this mode
**nothing visible changes** (that is the design) except the row « Plusieurs ... correspondent à ... » for questions 8 and 9.

| # | Sentence | The files that hold the answer | A fact to find in the answer |
| --- | --- | --- | --- |
| 1 | `Résume le dossier de Sophie Rochat.` | `2026-03-20_decompte-css-rochat-sophie.pdf` | a refund of `CHF 486.63` |
| 2 | `Que dit le dossier de Jean Dupontet ?` | `2026-06-20_resultats-analyses.pdf` (and two more) | hémoglobine glyquée `6,3 %` |
| 3 | `Quels courriers viennent du Dr Marc Bersier ?` | `2026-02-25_courrier-cardiologie-monnier-veronique.pdf` (a scan) | prochain rendez-vous `26 mai 2026` |
| 4 | `Quels courriers viennent de la CSS Assurance ?` | `2026-03-20_decompte-css-rochat-sophie.pdf` | `CHF 486.63` |
| 5 | `Montre la facture FAC-2026-014` | two invoices carry this number: `2026-04-14_facture-medsupply-fac-2026-014.pdf` and `2026-06-17_facture-papeterie-lac-fac-2026-014.pdf` | both, `CHF 352.73` and `93,12 €` |
| 6 | `Quels documents parlent de la LAMal ?` | the Swiss insurers' letters (`decompte-helsana-martin-marie.pdf`, `decompte-css-rochat-sophie.pdf`...) | |
| 7 | `Que dit le dossier de Madame Zénobie Quarante ?` | **none: nobody of that name is in the library** | the answer must say that the documents do not say |
| 8 | `Que dit J. Dupont ?` | **ambiguous on purpose**: Jean Dupont and Jean-Pierre Dupont are both `J. Dupont` | a row « Plusieurs personnes correspondent à « J. Dupont » : » |
| 9 | `Qu'a dit le Dr Martin ?` | **ambiguous on purpose**: three Martin are named in the library (Pierre, Marie, Paul) | a row « Plusieurs personnes correspondent à « Dr Martin » : » |
| 10 | `Que dit le courrier de Véronique Monnier ?` | `2026-02-25_courrier-cardiologie-monnier-veronique.pdf` (a scanned PDF read by text recognition) | `26 mai 2026` |

A5. Open « Réglages » → « Ciblage intelligent » → **« Désactivé »** (no restart). Repeat the ten sentences, same rule (clear the conversation first),
same recording. Nothing visible changes in this mode either; it is the search as it was before the knowledge base.

A6. « Réglages » → « Ciblage intelligent » → **« Automatique »**. Repeat the ten sentences. Now the application shows a line under the answer when it narrowed
the search. **What each sentence must show** (computed on the same files; « sur 28 » is the 28 documents the application analysed):

| # | Under the answer, in « Automatique » |
| --- | --- |
| 1 | `Recherche ciblée : 1 document sur 28 — Sophie Rochat`, **or**, if the search by meaning finds too little in that one file, `Recherche élargie à toute votre sélection : peu de résultats dans les documents ciblés.` (on the real embeddings this sentence widened in the agent's run; both are correct, and the answer then equals « Désactivé ») |
| 2 | `Recherche ciblée : 7 documents sur 28 — Dupontet Jean` (the base also keeps the files that write a form of `Dupont`: 7 files for 3 that hold the answer, on purpose) |
| 3 | `Recherche ciblée : 1 document sur 28 — Marc Bersier` |
| 4 | `Recherche ciblée : 1 document sur 28 — CSS` |
| 5 | `Recherche ciblée : 2 documents sur 28 — FAC-2026-014` |
| 6 | **no line**: `LAMal` is a subject, not a name (nothing is narrowed) |
| 7 | **no line**: nobody of that name is in the library; the answer must say that the documents do not say |
| 8 | **no line**; the row « Plusieurs personnes correspondent à « J. Dupont » : » (see B1) |
| 9 | **no line**; the row « Plusieurs personnes correspondent à « Dr Martin » : » (see B2) |
| 10 | `Recherche ciblée : 1 document sur 28 — Véronique Monnier`, **or** `Recherche élargie à toute votre sélection : ...` (this letter is a **scan**: on the real text recognition its name is read right in some runs and wrong in others; if there is no line at all, the scan was misread, which is a result to write down) |

If a line reads « Recherche élargie à toute votre sélection : peu de résultats dans les documents ciblés. », the application searched the narrow set, found too
little by meaning and repeated the search on everything: the answer is then **the one « Désactivé » gives**, by design. A line that **names a person who is not
in the question**, or an answer that **cites a file outside the 28**, is a bug.

A7. Read what the application recorded (numbers and codes only; the application may stay open):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py report
```

You read, for the 30 questions of A4 to A6: `Questions recorded            : 20` (the ten of « Suggestions seulement » and the ten of « Automatique »; « Désactivé » writes nothing in
this table) and a block « By mode » with 10 questions for each of the three modes: the knowledge lookup in milliseconds (`knowledge ... / ...`), the search
(`search ...`), the first word and the whole answer (`first word ...`, `total ...`). **Write the median `total` of the three modes in `02-results.md`**: it
is the answer to « is `Automatique` faster », and it is the model's time, not the base's.

**You leave with:** « Dossier des documents » analysed (28), « Dossier des données » analysed, « Tous les documents » ticked, nothing ticked in the data card, mode
**« Automatique »**, the conversation empty or not (it does not matter), 30 answers recorded.

---

## Part B - safety (25 min)

**You start with:** what Part A leaves (mode « Automatique », all documents ticked, no data ticked).

B1. **A name that fits several people is asked about, never picked.** Clear the conversation. Type `Que dit J. Dupont ?`. Under the answer, the row
« Plusieurs personnes correspondent à « J. Dupont » : » offers `Jean Dupont (3 documents)` and `Jean-Pierre Dupont (1 document)`. **No « Recherche ciblée » line** (nothing was narrowed). Press one of the names: the question
is asked again with that person as a chip (the chip reads the name; « Retirer ... » removes it) and **now** the line appears.

B2. Clear the conversation. Type `Qu'a dit le Dr Martin ?`. The row offers `Pierre Martin (3 documents)`, `Marie Martin (2 documents)` and `Paul Martin (3 documents)`. Nothing narrowed.

B3. **The widen button gives the old answer back.** Clear the conversation. Type `Quels courriers viennent du Dr Marc Bersier ?`. The line reads `Recherche ciblée : 1 document sur 28 — Marc Bersier`
with the buttons « Voir les fichiers » and « Élargir à ma sélection ». Open « Sources » and write the list. Press « Élargir à ma sélection »: the same
question is answered on all the documents. Open « Sources »: **the list must be the one you wrote for sentence 3 in « Désactivé »** (Part A5), file for
file. If it is not, it is a bug.

B4. **A file you did not tick is never read.** Clear the conversation. On « Dossier des documents », open the list and **untick
`2026-03-20_decompte-css-rochat-sophie.pdf`** (the summary reads « Documents utilisés : 27 fichiers »). Type `Résume le dossier de Sophie Rochat.`. Open « Sources »:
**that file must not be in the list**, in « Automatique » as in « Désactivé » (try both). Then type `Que dit 2026-03-20_decompte-css-rochat-sophie.pdf ?`:
the answer is « 2026-03-20_decompte-css-rochat-sophie.pdf se trouve dans votre dossier de documents mais n'est pas sélectionné pour cette conversation. Cochez-le
dans la liste des documents pour l'utiliser. » Tick « Tous les documents » again.

B5. **A question creates no name.** Close nothing. Read the state:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py names
```

Write the whole output (the count and the fingerprint). Then, in « Automatique », clearing the conversation each time, ask these four, one after the other:
`Que dit le dossier de Madame Zénobie Quarante ?`, `Quels documents concernent Hyacinthe Vaudroz ?`, `Que dit le Dr Ernest Blondeau ?`,
`Que dit Léopold Tschumi ?`. Read the state again with the same command: **the count and the fingerprint must be the same**, and the last line must still
say `names of the test questions found in the base: none`. Then « Réglages » → « Gérer… » → tab « Entités »: search for `Zénobie`, `Vaudroz`, `Blondeau`: **nothing**.
Close the dialog.

B6. **A short question that names nobody is read as the follow-up of the previous answer** (known defect KBD-19; you are checking that it is exactly what is
described, and that the button does not repair it). Clear the conversation, type `Que dit le courrier de Sophie Rochat ?`, wait. Without clearing, type
`Et son numéro AVS ?`: the line names Sophie Rochat (the follow-up is meant to). Without clearing, type `Que contiennent les notes internes ?`: **expected, and
known**: the line still names Sophie Rochat for this new question (the previous person narrowed it), and the search then widens by itself
(« Recherche élargie à toute votre sélection : ... »); the answer may describe her letter instead of the notes. Press « Élargir à ma sélection »: it does not
repair it (the question is still searched together with the previous one). Press « Effacer la conversation », type `Que contiennent les notes internes ?` alone:
the answer is about the internal notes. Write what you saw.

**You leave with:** mode « Automatique », « Tous les documents » ticked, nothing in the data card, the conversation cleared, the base unchanged by the questions.

---

## Part C - a library that lives (40 min)

**You start with:** what Part B leaves: the 30-document library, analysed (28 documents); mode « Automatique »; « Tous les documents » ticked; nothing in the data card.

Every step here changes a file in `kb-lot11` (or resets), presses « Analyser » on « Dossier des documents », and checks three things: what the card says, what the base
holds (the command `names`, which may run with the application open), and one question. **No step is undone**; the next one starts from the state the previous one
leaves, which is written. Every number below comes from running these very steps on a copy of the library with the same code (the window itself was **not seen by the agent**).

C0. Read the base and write the output: `python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py names`. Expected: `identifier active 295`, `location active 51`,
`organization active 25`, `organization candidate 4`, `person active 181`, `person candidate 2`, then `entities: 558` (write the fingerprint: nothing is compared with it yet).

C1. **A new file, about somebody the library has never seen.**

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py lifecycle add
```

You read `wrote courriers-entrants\courrier-quenneville-nouveau.txt (a person the library has never seen: Sidonie Quenneville)`. In the application press « Analyser » on
« Dossier des documents ». Expected on the card: `1 analysé` and the lines `41 personnes et organisations repérées, dont 40 déjà connues.`, `30 identifiants repérés ...`,
`28 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.` Run `names`: **one more person** (`person active 182`, `entities: 559`). Clear the conversation, ask
`Que dit Madame Sidonie Quenneville ?`: the line reads `Recherche ciblée : 1 document sur 29 — Sidonie Quenneville`, and the answer is about the new letter.

C2. **A name with spaces and an accent: the Analyse renames the file first** (the file-name exception of 25 September 2026: name only, nothing overwritten, logged).

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py lifecycle add-spaced
```

You read `wrote Courrier Quenneville été 2026.txt (spaces and an accent in the name: the next Analyse renames it)`. Press « Analyser ». Expected on the card:
« 1 fichier renommé pour retirer les espaces et les accents » and the line « Courrier Quenneville été 2026.txt → Courrier-Quenneville-ete-2026.txt ». In the folder (« Voir ») the file
has the clean name and the first letter is untouched. Ask `Que dit Madame Sidonie Quenneville ?` again: the line reads `Recherche ciblée : 2 documents sur 30 — Sidonie Quenneville` (two files now).

C3. **The three Resets, then the base is rebuilt** (placed here, before any file is changed or deleted, so that the rebuilt list can be compared with the old one name for name).
Run `names` and write the output (**before**: `entities: 559`). (a) Card « Dossier des documents » → « Reset » → confirm; press « Analyser »: `30 analysés`. (b) Card « Dossier des données »
→ « Reset » → confirm; « Analyser »: `7 analysés`. (c) « Réglages » → « Gérer… » → tab « Réinitialiser » → « Tout réinitialiser… » → confirm « Tout réinitialiser »; then « Analyser » on both cards
(`30 analysés`, `7 analysés`). Run `names` (**after**): **the same count and the same fingerprint as before** (`entities: 559`): the base is a function of the files, nothing was typed by hand, so nothing is lost.

C4. **A changed file: the old associations go, the new ones come.**

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py lifecycle modify
```

You read `changed courriers-entrants/2026-02/2026-02-24_courrier-neurologie-fabre-laurence.txt: Laurence Fabre -> Laurence Quéméner`. Press « Analyser »: `1 analysé`, `4 personnes et organisations repérées, dont 3 déjà connues.`,
`2 identifiants repérés ...`. Run `names`: `person candidate 3` (**one more**, `entities: 560`). Then « Réglages » → « Gérer… » → tab « Entités »: search `Quéméner`: **one person**, tagged « À confirmer », found
in that file; search `Laurence Fabre`: the person is still there (the workbooks name her) but **no longer in that letter**. Close the dialog. Ask `Que dit Laurence Quéméner ?`: **no « Recherche ciblée » line** (a name met once,
with no title, is only a candidate, and a candidate never narrows a search).

C5. **A deleted file: its associations go, the others stay.**

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py lifecycle delete
```

You read `deleted caisses-assurances/suisse/2026-03-20_decompte-css-rochat-sophie.pdf (a letter about Sophie Rochat)`. Press « Analyser »: expected on the card
« 1 document retiré du dossier, oublié de l'analyse ». `names` does not change (`entities: 560`: the workbooks still name her). Ask `Résume le dossier de Sophie Rochat.`: **no « Recherche ciblée » line** (no document names her any more)
and the answer does not cite the deleted file. Ask `Quels courriers viennent de la CSS Assurance ?`: the line reads `Recherche ciblée : 1 document sur 29 — CSS` and the answer is the new letter of Sidonie Quenneville, which writes
« la CSS Assurance » (the deleted letter was the other one).

C6. **The library grows, then shrinks, as a real folder does.**

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py tier 100
```

You read `added 70 documents: the library goes from 30 to 100` and `100 documents: 89 the application reads, 11 it does not read (formats: csv, eml, odp, odt, ppt, pptx, rtf, vtt, xlsx)`
(the `.csv` and the two `.xlsx` found among the documents are tables, read by the other card). Press « Analyser »: **only the new files are read** (`61 analysés`; the 29 already analysed are unchanged;
`7 fichiers lus par reconnaissance de texte`) and it takes much less than analysing 100 from scratch: write the time. Run `names`: more people and organisations (`entities: 594`). Then:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py tier 30
```

You read `removed 70 documents: the library goes from 100 to 30`. Press « Analyser »: the card says « 61 documents retirés du dossier, oubliés de l'analyse ». Run `names`: **fewer than at 100**
(`entities: 560`, the same total as before the growth). The list is not exactly the one before the growth: two organisation candidates are now active (`organization active 27`, `organization candidate 2`) and one person
candidate too (`person active 183`, `person candidate 2`): a name that two files agreed on was promoted to « active » and stays active when one of them is removed (defect KBD-12, known and deferred).

**You leave with:** the 30-document folder with a new letter, a renamed letter, a changed letter and one letter fewer (29 documents analysed); the base rebuilt from it; mode « Automatique »; « Tous les documents » ticked.

---

## Part D - 100 and 200 documents (1 to 2 hours, mostly waiting)

**You start with:** what Part C leaves.

D1. Grow the library to 100 and read what the Analyse costs:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py tier 100
```

Press « Analyser » on « Dossier des documents ». The folder holds the **guide of 115 pages** and the **manual** among others: this is the slow Analyse. Write the time, and the card's lines (`61 analysés` and `7 fichiers lus par reconnaissance de texte`; the knowledge lines are whatever the card prints, write them). While it runs, you can read the state of the test folders:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py status
```

You read `Documents folder: ...kb-lot11 <- the test folder` and `holds ... files (library tier 100)`.

D2. **Questions at 100 documents**, « Désactivé » then « Automatique » (the mode change is in « Réglages »; clear the conversation before each; note the time under the answer and, in « Automatique », the line):

| # | Sentence | Files that hold the answer | Fact | « Automatique » must show |
| --- | --- | --- | --- | --- |
| 11 | `Qui renouvelle le code de l'armoire à pharmacie ?` | `documentation\guide-pratique-cabinet.pdf` (page 66 of 115) | Sandrine Colas | **no line** (a question that names nobody; the answer must come from the long guide and cite `page 66`) |
| 12 | `À qui signaler un écart de la chaîne du froid ?` | `documentation\manuel-qualite.docx` | Nadia Bensaïd | **no line** (a Word file has no pages: the source reads `page 1`) |
| 13 | `Résume le dossier de Fatima Benali.` | `laboratoire\2026-01\2026-01-29_resultats-analyses-benali-fatima.pdf`, `courriers-entrants\2026-08\2026-08-01_courrier-rhumatologie.pdf`, `caisses-assurances\suisse\2026-08-17_decompte-groupe-mutuel.pdf` | HbA1c `7,0 %`, next visit `30 octobre 2026`, `CHF 151.05` | `Recherche ciblée : 3 documents sur 90 — Fatima Benali` |
| 14 | `Quels courriers viennent du CHUV ?` | the letters of the CHUV | | **no line** (known: the base does not find this hospital, defect KBD-27; no line is the correct reading of what the base knows) |
| 15 | `Quels documents concernent la Dre Vuagniaux ?` | 7 letters and results written by Dr Anne-Sophie Vuagniaux (a Swiss doctor) | | `Recherche ciblée : 4 documents sur 90 — Dre Anne-Sophie Vuagniaux`: **4 files kept for 7 that hold the answer** (known: the title « Dre » is not read as a title, defect KBD-26; this is the measured loss the gate is about) |
| 16 | `Quels documents viennent de Genève ?` | many (the word is in about 30 documents) | | **no line** with the data card unticked (known, defect KBD-29: when workbooks are ticked together with the documents, a commune written in a table steers the documents of a topic question and the cap of 15 cuts the list) |

D3. Grow to 200 and repeat:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py tier 200
```

Press « Analyser » (only the new files are read; write the time). (`87 analysés`, `18 fichiers lus par reconnaissance de texte`.) Ask sentences 11, 12, 13 and 16 again in « Désactivé » and « Automatique »; sentence 13 now reads `Recherche ciblée : 6 documents sur 177 — Fatima Benali` and sentence 15 `Recherche ciblée : 2 documents sur 177 — Dre Vuagniaux` (**2 kept for 7**: the loss grows with the library). Run:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py report
```

Write, for each mode, `median total` and `median search`: **the gain of `Automatique` over « Désactivé » is the difference of the medians**; the base's own time is `knowledge ... / ...`.

**You leave with:** the 200-document library, analysed (`177 analysés`), mode « Automatique », « Tous les documents » ticked, about 40 questions recorded in total.

---

## Part E - the figures of the release gate, on your use (spread over days; at least 100 questions)

**You start with:** what Part D leaves.

E1. « Réglages » → « Ciblage intelligent » → **« Suggestions seulement »** (the default: the base measures what it would have done, and narrows nothing).

E2. Ask the sentences that the script prints (they have known answers; `questions daily` lists the ones of a library of 200 documents):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py questions daily
```

Type 25 of them, clearing the conversation between them. Then **use the application for your own work in this mode** (your own questions on these documents, or on your own real folder once
the legal conditions of `AGENTS.md` allow it) until the record holds more than 100 questions.

E3. Read the figures:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py report
```

Write the whole output in `02-results.md`. The lines that decide: `Release gate, raw` and `Release gate, adjusted`, the number of questions the base would reduce, the median reduction, and `Why a question was not reduced`.
`03-release-gate.md` says how they are read and why the raw figure is low by construction.

**You leave with:** « Suggestions seulement » as the mode, more than 100 questions recorded.

---

## Part Z - optional: 500 and 1 000 documents, and your own books (overnight)

**You start with:** what Part E leaves. This part comes last because it is long and changes a lot; **its undo is Part R**, which restores your index and your settings (the test folders are left on disk).

Z1. `python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py tier 500`, then « Analyser » (hours on a CPU-only machine: the embedding is nine tenths of it; write the time). Ask sentences 11, 13 and 16 in
« Désactivé » and « Automatique », and the sentence `Quels documents parlent de la LAMal ?`; note the time under each.

Z2. `python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py tier 1000`, « Analyser », the same questions. At this size the 122 files the application does not read are about an eighth of the folder (878 are read): write what the card says about them.

Z3. Copy into `kb-lot11` (a new sub-folder `livres`) any large PDF or DOCX of your own that is not a patient file (a medical or legal handbook, a manual of a hundred pages or more), press « Analyser », write the time, and ask
three questions whose answer you know is on a page you can name. Note whether the page is cited (« page N ») and whether the answer is right: a Word file has **no page** (the application keeps it as one page; known).

---

## Part R - the one restore (5 min)

**You start with:** the application open, in any state.

R1. Close the application (close the window; check the terminal that ran `pnpm tauri dev` has stopped).

R2. Put back your index, your settings and the key file:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py restore
```

You read `restored settings.json`, then `restored index.sqlite3` and `restored knowledge-identifier.key` (or `removed knowledge-identifier.key (there was none when the backup was made)`).

R3. (Optional) delete the test folders, which hold only invented files: `kb-lot11`, `kb-lot11-data`, and `data\knowledge-sandbox-1000` in the repository.

R4. Start the application: your own folders and your own mode are back.

**You leave with:** the application as it was before Part 0.

---

## Results and findings

Fill `docs/test-reports/knowledge-base-pass-1/02-results.md`. A deviation is a finding about **this protocol first**: tell the agent which step, what you saw, what the step said; the agent fixes the protocol and gives the short ordered re-run.
Anything the product did that is wrong goes into `defects-register.md` as a new row, with the step.
