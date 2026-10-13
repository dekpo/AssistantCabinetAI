# Human test - the final test of the knowledge programme (lot 11, readjusted)

Written for the owner, who plays it once, and for the agent who reads her results book. Instructions in English; every sentence you type and every label you read is French, quoted exactly.

**Branch to be on:** `fix/kb-wrap-up` (it holds the fixes of batches 1, 2 and 3 and the Analyse-button fix). **Build:** from `apps\desktop`, `pnpm tauri dev`. **Services:** the AI machine your « Réglages » point at (the tower `http://192.168.1.83:8080`, or the Docker stack of this PC, `docker compose up -d`); write which one in `02-results.md`, step 0.6. **Duration:** Part 0 15 min · A 45 · B 25 · C 45 · D 1 to 2 h (mostly waiting for an Analyse) · S 50 · G 30 · E spread over your days · Z optional, overnight · R 5 · M later, on the Mac mini. You may stop between two parts: each part says the state it leaves, and nothing in the middle needs undoing.

**Your answers go in one file:** `docs/test-reports/knowledge-base-pass-1/02-results.md`**.** It has one block per step id (`0.1`, `A4-3`, `C5`...), the expected value already written; you fill « Seen », OK / not OK and a note. Nothing is asked of you in the chat.

**What it is for.** To see, once, on one library whose answers are known, everything the knowledge programme built and the three fix batches repaired: the intelligent targeting (« Ciblage intelligent ») in its three modes on the same sentences (Part A), its safety (B), a library that lives (C), the library at 100 documents (D), **Switzerland, the tables and the file names** (S), 200 documents (G), your own figures (E). It also checks that the « Analyser » button stops asking for files the pass cannot read (step A1 and every Analyse after it).

**Rules of the script** (broken, any of them makes a result worthless):

1. **Type each sentence exactly as written**, including the accents and the final point or question mark.
2. **Before every question, press « Effacer la conversation » and confirm.** A question of eight words or fewer is searched together with the previous one (the product's own rule, known defect KBD-19): without this, the modes would not be asked the same thing.
3. **The mode is changed in « Réglages » → group « Connaissances » → « Ciblage intelligent »**; it applies to the next question, no restart. The names are read **at the Analyse**, so each Analyse is done in the mode the step says.
4. **A part says what you start with and what you leave with.** If your screen does not match the start, stop and tell the agent which step: the protocol is wrong before the product is.
5. **What the agent did not observe is said in the step.** The application window cannot be walked by the agent: every figure below was produced by the same code run on the same invented files, in the same order as this script (the folders were also built by the real helper commands on temporary copies and compared with the replay's: the same 216 files, the same names and sizes, but for two letters that the helper writes with Windows line breaks). The lines on screen, the buttons and the colours are what you check. The text recognition of the scans uses the real engine on your machine, so a figure that depends on a scan can differ by one: write what you see.
6. **« Analyser » is « inactive » when it has the plain style and no number** (« Analyser », not « Analyser (3) »): the button always works, it is only lit and numbered while a pass would still change something.
7. Every command is run from the repository root `C:\Users\elise\Documents\CURSOR\AssistantCabinetAI` and is written in full.

---



## Part 0 - Preparation (15 min)

**You start with:** the application **closed**; your settings and index as you left them; a Documents folder already chosen in the application (the helper builds the test folders beside it).

**0.1** Look at what the script sees:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py status
```

Expected: `Application     : closed` and `Core library    : present`.

**0.2** Back up your index, your settings and the key file (the **only** backup of the test; the restore is Part R):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py backup
```

Expected: `saved   index.sqlite3` (or `absent  index.sqlite3 (nothing to save)`), `saved   settings.json`, `saved   knowledge-identifier.key` (or `absent  knowledge-identifier.key (nothing to save)`), then `Backup in C:\Users\elise\kb-backup-lot-11`.

**0.3** Make the test folders beside your current Documents folder, with the first 30 documents of the invented library and its 8 workbooks, and point the application at them:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py prepare 30
```

Expected: `wrote 30 invented documents in ...\kb-lot11`, `30 documents: 28 the application reads, 2 it does not read (formats: odt, vtt)`, `scans (OCR): 3; PDF pages: 16`, `wrote 8 invented workbooks in ...\kb-lot11-data (formats: csv, ods, xls, xlsx)`, your two folders, then `"knowledgeMode" is "suggest", "writeTimingLog" is true and both health modules (France and Switzerland) are on.`

**0.4** Empty the record of questions (numbers and codes only; no knowledge is touched):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py clear-log
```

Expected: `deleted retrieval-timings.jsonl` or `no retrieval-timings.jsonl to delete`, then `emptied kb_diagnostics (N row(s))` or `kb_diagnostics does not exist yet`.

**0.5** Start the application: in a terminal, `cd apps\desktop` then `pnpm tauri dev`. Wait for the window.
Expected: at the top, the AI status reads « IA Disponible » (if it reads « IA Indisponible », start the AI machine and press « Vérifier l'IA »).

**0.6** Open « Réglages ». Write « Adresse de l'IA » (the machine) in the results book. Check the rest and close with « Fermer ».
Expected: group « Connaissances »: « Ciblage intelligent » = « Suggestions seulement »; « Module santé »: both « Santé : France » and « Santé : Suisse » ticked; group « Avancé »: « Noter les durées dans un journal » ticked.

**0.7** **Start from an empty base.** (a) On the card « Dossier des documents », press « Reset » and confirm « Effacer l'analyse ? ». (b) On the card « Dossier des données », press « Reset » and confirm « Effacer l'analyse des données ? ». (c) « Réglages » → « Gérer… » (group « Connaissances ») → tab « Réinitialiser » → « Tout réinitialiser… » → confirm « Tout réinitialiser ». Close the dialog.
Expected: after (c) the message « La base de connaissances a été réinitialisée. »; the cards read `30 fichiers, 0 analysés, 0 illisibles` (documents) and `8 fichiers, 0 analysés, 1 illisible` (data).

**0.8** Check it from the outside (read only; the application may stay open):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py names
```

Expected: `entities: 0   fingerprint of the list: e3b0c44298fc1c14` and `names of the test questions found in the base: none`.

**You leave with:** the application open; « Dossier des documents » = `kb-lot11` (30 files, **not analysed**); « Dossier des données » = `kb-lot11-data` (8 files, **not analysed**); mode « Suggestions seulement »; both health modules ticked; the timing log on; the index and the knowledge base empty; nothing ticked.

---



## Part A - 30 documents, the library that started the whole programme (45 min)

**You start with:** what Part 0 leaves.

**A1** On « Dossier des documents », press « Analyser ». Write the **time it takes** (from the click to the end of the progress bar).
Expected: the line under the title reads `30 fichiers, 28 analysés, 0 illisibles`; under the card: `3 fichiers lus par reconnaissance de texte.`, `2 fichiers n'ont pas été lus : leur format n'est pas pris en charge (1 .odt, 1 .vtt).`, `42 personnes et organisations repérées, dont 0 déjà connues.`, `30 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).` **The button « Analyser » is plain, without a number** (before the Analyse-button fix it read « Analyser (2) » for ever, because of the `.odt` and the `.vtt`: if you see a number, it is a bug, defect KBD-47).

**A2** On « Dossier des données », press « Analyser ».
Expected: `8 fichiers, 7 analysés, 1 illisible` (`rendez-vous-2026.ods` is the one that cannot be read: « pas un tableur (CSV ou XLSX) »); under the card: `7 tableurs lus sur cet ordinateur, sans l'IA : les tableurs s'analysent en un instant.`, `210 personnes et organisations repérées, dont 21 déjà connues.`, `278 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).`, `22 colonnes de tableur reconnues (personnes, organisations, identifiants...) dans 7 tableurs lus.`, `102 liens lus entre deux colonnes d'un tableur.`, `28 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.` The button « Analyser » of this card is plain, without a number.

**A3** Selection: on « Dossier des documents », tick « Tous les documents ». On « Dossier des données » tick **nothing**.
Expected: the summaries read « Documents utilisés : tous » and « Données utilisées : aucune » (with documents and several workbooks ticked together, a question about a table asks « Plusieurs tableurs sont sélectionnés », which Part S shows on purpose).

**The ten sentences.** Each is asked three times: in « Suggestions seulement » (A4, the mode you are in), in « Désactivé » (A5) and in « Automatique » (A6). For each answer note: **is it right**, **are the right files cited** (open « Sources »), **did it miss a file that holds the answer**, **did it add one that does not**, and the time under the answer (« Généré par ... en ... »). The table gives the files of the 30 that hold the answer.


| #   | Sentence                                          | The file(s) that hold the answer                                                                                                        | A fact to find                                                |
| --- | ------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| 1   | `Résume le dossier de Sophie Rochat.`             | `2026-03-20_decompte-css-rochat-sophie.pdf`                                                                                             | a refund of `CHF 486.63`                                      |
| 2   | `Que dit le dossier de Jean Dupontet ?`           | `2026-06-20_resultats-analyses.pdf`                                                                                                     | hémoglobine glyquée `6,3 %`                                   |
| 3   | `Quels courriers viennent du Dr Marc Bersier ?`   | `2026-02-25_courrier-cardiologie-monnier-veronique.pdf` (a scan)                                                                        | prochain rendez-vous `26 mai 2026`                            |
| 4   | `Quels courriers viennent de la CSS Assurance ?`  | `2026-03-20_decompte-css-rochat-sophie.pdf`                                                                                             | `CHF 486.63`                                                  |
| 5   | `Montre la facture FAC-2026-014`                  | two invoices carry this number: `2026-04-14_facture-medsupply-fac-2026-014.pdf` and `2026-06-17_facture-papeterie-lac-fac-2026-014.pdf` | both, `CHF 352.73` and `93,12 €`                              |
| 6   | `Quels documents parlent de la LAMal ?`           | `2026-03-26_decompte-helsana-martin-marie.pdf` and `2026-03-20_decompte-css-rochat-sophie.pdf`                                          |                                                               |
| 7   | `Que dit le dossier de Madame Zénobie Quarante ?` | **none: nobody of that name is in the library**                                                                                         | the answer must say that the documents do not say             |
| 8   | `Que dit J. Dupont ?`                             | **ambiguous on purpose**: Jean Dupont and Jean-Pierre Dupont are both `J. Dupont`                                                       | a row « Plusieurs personnes correspondent à « J. Dupont » : » |
| 9   | `Qu'a dit le Dr Martin ?`                         | **ambiguous on purpose**: two doctors Martin (Pierre, Paul)                                                                             | a row « Plusieurs personnes correspondent à « Dr Martin » : » |
| 10  | `Que dit le courrier de Véronique Monnier ?`      | `2026-02-25_courrier-cardiologie-monnier-veronique.pdf` (a scanned PDF read by text recognition)                                        | `26 mai 2026`                                                 |


**A4** In « Suggestions seulement », ask the ten sentences, one by one, clearing the conversation before each. In this mode **nothing visible changes** (that is the design: the base measures what it would do and narrows nothing) except the row « Plusieurs personnes correspondent à ... » for sentences 8 and 9. The ids below are the ten sentences:

**A4-1** `Résume le dossier de Sophie Rochat.` Expected: right; source `...decompte-css-rochat-sophie.pdf`; the refund `CHF 486.63`; no targeting line.

**A4-2** `Que dit le dossier de Jean Dupontet ?` Expected: right; source `...2026-06-20_resultats-analyses.pdf`; `6,3 %`; no line.

**A4-3** `Quels courriers viennent du Dr Marc Bersier ?` Expected: right; source `...courrier-cardiologie-monnier-veronique.pdf`; `26 mai 2026`; no line.

**A4-4** `Quels courriers viennent de la CSS Assurance ?` Expected: right; source `...decompte-css-rochat-sophie.pdf`; `CHF 486.63`; no line.

**A4-5** `Montre la facture FAC-2026-014` Expected: both invoices cited (`...facture-medsupply-fac-2026-014.pdf`, `...facture-papeterie-lac-fac-2026-014.pdf`); `CHF 352.73` and `93,12 €`; no line.

**A4-6** `Quels documents parlent de la LAMal ?` Expected: the Swiss insurers' letters cited (`...decompte-helsana-martin-marie.pdf`, `...decompte-css-rochat-sophie.pdf`); no line.

**A4-7** `Que dit le dossier de Madame Zénobie Quarante ?` Expected: the answer says that the documents do not say; no line.

**A4-8** `Que dit J. Dupont ?` Expected: the row « Plusieurs personnes correspondent à « J. Dupont » : » offering `Jean Dupont (3 documents)` and `Jean-Pierre Dupont (1 document)`; no line.

**A4-9** `Qu'a dit le Dr Martin ?` Expected: the row « Plusieurs personnes correspondent à « Dr Martin » : » offering `Pierre Martin (3 documents)` and `Paul Martin (3 documents)`; no line.

**A4-10** `Que dit le courrier de Véronique Monnier ?` Expected: right; source `...courrier-cardiologie-monnier-veronique.pdf`; `26 mai 2026`; no line.

**A5** « Réglages » → « Ciblage intelligent » → **« Désactivé »** (no restart). Ask the ten sentences again, same rule. This is the search as it was before the knowledge base: no line and no row at all.

**A5-1** `Résume le dossier de Sophie Rochat.` Expected: the same answer and sources as A4-1; no line.

**A5-2** `Que dit le dossier de Jean Dupontet ?` Expected: the same as A4-2; no line.

**A5-3** `Quels courriers viennent du Dr Marc Bersier ?` Expected: the same as A4-3; no line. **Write the list of « Sources »: Part B compares with it.**

**A5-4** `Quels courriers viennent de la CSS Assurance ?` Expected: the same as A4-4; no line.

**A5-5** `Montre la facture FAC-2026-014` Expected: the same as A4-5; no line.

**A5-6** `Quels documents parlent de la LAMal ?` Expected: the same as A4-6; no line.

**A5-7** `Que dit le dossier de Madame Zénobie Quarante ?` Expected: the documents do not say; no line.

**A5-8** `Que dit J. Dupont ?` Expected: an answer drawn from the documents; **no** row « Plusieurs personnes... » (the base is off).

**A5-9** `Qu'a dit le Dr Martin ?` Expected: an answer drawn from the documents; no row.

**A5-10** `Que dit le courrier de Véronique Monnier ?` Expected: the same as A4-10; no line.

**A6** « Réglages » → « Ciblage intelligent » → **« Automatique »**. Ask the ten sentences again, same rule. Now the application shows a line under the answer when it narrowed the search (« sur 28 » is the 28 documents the application analysed). If a line reads « Recherche élargie à toute votre sélection : peu de résultats dans les documents ciblés. », the application searched the narrow set, found too little by meaning and repeated the search on everything: the answer is then the one « Désactivé » gives, by design (it depends on the embeddings of your AI machine; both readings are correct). A line that **names a person who is not in the question**, or an answer that **cites a file outside the 28**, is a bug.

**A6-1** `Résume le dossier de Sophie Rochat.` Expected: `Recherche ciblée : 1 document sur 28 — Sophie Rochat` (or the widened line).

**A6-2** `Que dit le dossier de Jean Dupontet ?` Expected: `Recherche ciblée : 1 document sur 28 — Jean Dupontet` (or the widened line).

**A6-3** `Quels courriers viennent du Dr Marc Bersier ?` Expected: `Recherche ciblée : 1 document sur 28 — Marc Bersier`.

**A6-4** `Quels courriers viennent de la CSS Assurance ?` Expected: `Recherche ciblée : 1 document sur 28 — CSS Assurance`.

**A6-5** `Montre la facture FAC-2026-014` Expected: `Recherche ciblée : 2 documents sur 28 — FAC-2026-014`.

**A6-6** `Quels documents parlent de la LAMal ?` Expected: **no line** (`LAMal` is a subject, not a name: nothing is narrowed).

**A6-7** `Que dit le dossier de Madame Zénobie Quarante ?` Expected: **no line**; the documents do not say.

**A6-8** `Que dit J. Dupont ?` Expected: **no line**; the row « Plusieurs personnes correspondent à « J. Dupont » : » with `Jean Dupont (3 documents)` and `Jean-Pierre Dupont (1 document)`.

**A6-9** `Qu'a dit le Dr Martin ?` Expected: **no line**; the row « Plusieurs personnes correspondent à « Dr Martin » : » with `Pierre Martin (3 documents)` and `Paul Martin (3 documents)`.

**A6-10** `Que dit le courrier de Véronique Monnier ?` Expected: `Recherche ciblée : 1 document sur 28 — Véronique Monnier` (or the widened line). This letter is a **scan**: on the real text recognition its name is read right in some runs and wrong in others; if there is no line at all, the scan was misread, which is a result to write down.

**A7** Read what the application recorded (numbers and codes only; the application may stay open):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py report
```

Expected: `Questions recorded            : 20` (the ten of « Suggestions seulement » and the ten of « Automatique »; « Désactivé » writes nothing in this table) and a block « By mode » with 10 questions for each of the three modes (`off`, `suggest`, `auto`): the knowledge lookup in milliseconds, the search, the first word and the whole answer. **Write the median** `total` **of the three modes in the results book**: it is the answer to « is `Automatique` faster », and it is the model's time, not the base's.

**You leave with:** « Dossier des documents » analysed (28 of 30), « Dossier des données » analysed (7 of 8), « Tous les documents » ticked, nothing ticked in the data card, mode **« Automatique »**, 30 answers recorded.

---



## Part B - safety (25 min)

**You start with:** what Part A leaves (mode « Automatique », all documents ticked, no data ticked).

**B1** **A name that fits several people is asked about, never picked.** Clear the conversation. Type `Que dit J. Dupont ?`. Under the answer, press the name `Jean Dupont`.
Expected: the row « Plusieurs personnes correspondent à « J. Dupont » : » offers `Jean Dupont (3 documents)` and `Jean-Pierre Dupont (1 document)`; **no** « Recherche ciblée » line before you press (nothing was narrowed). After the press, the question is asked again with that person as a chip (the chip reads the name; « Retirer ... » removes it) and **now** a « Recherche ciblée » line appears.

**B2** Clear the conversation. Type `Qu'a dit le Dr Martin ?`.
Expected: the row offers `Pierre Martin (3 documents)` and `Paul Martin (3 documents)` (the title « Dr » keeps the patients called Martin out of the choice); no line.

**B3** **The widen button gives the old answer back.** Clear the conversation. Type `Quels courriers viennent du Dr Marc Bersier ?`. Open « Sources » and write the list. Press « Élargir à ma sélection ». Open « Sources » again.
Expected: the line reads `Recherche ciblée : 1 document sur 28 — Marc Bersier` with the buttons « Voir les fichiers » and « Élargir à ma sélection »; after the press the question is answered on all the documents and **the list is the one you wrote for A5-3** (sentence 3 in « Désactivé »), file for file. If it is not, it is a bug.

**B4** **A file you did not tick is never read.** Clear the conversation. On « Dossier des documents », open the list and **untick** `2026-03-20_decompte-css-rochat-sophie.pdf`. Type `Résume le dossier de Sophie Rochat.` and open « Sources ». Repeat in « Désactivé » (« Réglages », then back to « Automatique »). Then type `Que dit 2026-03-20_decompte-css-rochat-sophie.pdf ?`. At the end, tick « Tous les documents » again.
Expected: the summary reads « Documents utilisés : 27 fichiers »; **that file is not in « Sources »**, in « Automatique » (no targeting line: no ticked document names her) as in « Désactivé »; the named-file question is answered « 2026-03-20_decompte-css-rochat-sophie.pdf se trouve dans votre dossier de documents mais n'est pas sélectionné pour cette conversation. Cochez-le dans la liste des documents pour l'utiliser. »

**B5** **A question creates no name.** Make sure the mode is « Automatique ». Read the state and write the whole output:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py names
```

Then ask these four, clearing the conversation each time: `Que dit le dossier de Madame Zénobie Quarante ?`, `Quels documents concernent Hyacinthe Vaudroz ?`, `Que dit le Dr Ernest Blondeau ?`, `Que dit Léopold Tschumi ?`. Read the state again with the same command. Then « Réglages » → « Gérer… » → tab « Entités »: search `Zénobie`, `Vaudroz`, `Blondeau`. Close the dialog.
Expected: both outputs are identical (`identifier active 295`, `location active 51`, `organization active 29`, `organization candidate 5`, `person active 196`, `person candidate 1`, `entities: 577`, the same fingerprint) and the last line says `names of the test questions found in the base: none`; the window finds nothing for the three names.

**B6** **A short question that names nobody is read as the follow-up of the previous answer** (known defect KBD-19; you are checking that it is exactly what is described, and that the button does not repair it; the agent could not walk this step, it depends on the conversation). Clear the conversation, type `Que dit le courrier de Sophie Rochat ?`, wait. Without clearing, type `Et son numéro AVS ?`. Without clearing, type `Que contiennent les notes internes ?`. Press « Élargir à ma sélection ». Then press « Effacer la conversation » and type `Que contiennent les notes internes ?` alone.
Expected: for `Et son numéro AVS ?` the line names Sophie Rochat (the follow-up is meant to); for `Que contiennent les notes internes ?` the line **still names Sophie Rochat** (the previous person narrowed it) and the search then widens by itself (« Recherche élargie à toute votre sélection : ... »); the answer may describe her letter instead of the notes; « Élargir à ma sélection » does not repair it; asked alone, the answer is about the internal notes. Write what you saw.

**You leave with:** mode « Automatique », « Tous les documents » ticked, nothing in the data card, the conversation cleared, the base unchanged by the questions.

---



## Part C - a library that lives (45 min)

**You start with:** what Part B leaves: the 30-document library, analysed (28 documents, 30 files); mode « Automatique »; « Tous les documents » ticked; nothing in the data card.

Every step here changes a file in `kb-lot11` (or resets), presses « Analyser » where it says, and checks what the card says, what the base holds (the command `names`, which may run with the application open) and one question. **No step is undone**; the next one starts from the state the previous one leaves, which is written. Every number comes from running these very steps, in this order, on a copy of the library with the same code.

**C0** Read the base and write the output: `python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py names`
Expected: `identifier active 295`, `location active 51`, `organization active 29`, `organization candidate 5`, `person active 196`, `person candidate 1`, then `entities: 577` (write the fingerprint: it is compared at C3).

**C1** **A new file, about somebody the library has never seen.**

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py lifecycle add
```

You read `wrote courriers-entrants\courrier-quenneville-nouveau.txt (a person the library has never seen: Sidonie Quenneville)`. Press « Analyser » on « Dossier des documents ». Run `names`. Clear the conversation, ask `Que dit Madame Sidonie Quenneville ?`.
Expected: the card reads `31 fichiers, 29 analysés, 0 illisibles` and the lines `2 fichiers n'ont pas été lus : leur format n'est pas pris en charge (1 .odt, 1 .vtt).`, `46 personnes et organisations repérées, dont 45 déjà connues.`, `30 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).`, `28 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.`; `names`: **one more person** (`person active 197`, `entities: 578`); the line `Recherche ciblée : 1 document sur 29 — Sidonie Quenneville`, and the answer is about the new letter.

**C2** **A name with spaces and an accent: the Analyse renames the file first** (the file-name exception of 25 September 2026: name only, nothing overwritten, logged).

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py lifecycle add-spaced
```

You read `wrote Courrier Quenneville été 2026.txt (spaces and an accent in the name: the next Analyse renames it)`. Press « Analyser ». Open « Voir » (the folder). Ask `Que dit Madame Sidonie Quenneville ?` again.
Expected: the card reads `32 fichiers, 30 analysés, 0 illisibles`; `1 fichier renommé pour retirer les espaces et les accents.` and `Courrier Quenneville été 2026.txt → Courrier-Quenneville-ete-2026.txt`; in the folder the file has the clean name; the line reads `Recherche ciblée : 2 documents sur 30 — Sidonie Quenneville` (two files now).

**C3** **The three Resets, then the base is rebuilt** (placed here, before any file is changed or deleted, so the rebuilt list can be compared with the old one name for name). Run `names` and write the output (**before**).
Expected before: `person active 197`, `person candidate 1`, `entities: 578` (the other lines as at C0).

**C3-a** Card « Dossier des documents » → « Reset » → confirm « Effacer l'analyse ? ».
Expected: the card reads `32 fichiers, 0 analysés, 0 illisibles` and the button reads « Analyser (30) » (lit; the `.odt` and the `.vtt` are not counted). Press « Analyser ». Expected after: `32 fichiers, 30 analysés, 0 illisibles`, `3 fichiers lus par reconnaissance de texte.`, `2 fichiers n'ont pas été lus ...`, `45 personnes et organisations repérées, dont 24 déjà connues.`, `30 identifiants repérés ...`; the button plain again.

**C3-b** Card « Dossier des données » → « Reset » → confirm « Effacer l'analyse des données ? ».
Expected: `8 fichiers, 0 analysés, 1 illisible` and the button « Analyser (7) ». Press « Analyser ». Expected after: `8 fichiers, 7 analysés, 1 illisible`; the button plain.

**C3-c** « Réglages » → « Gérer… » → tab « Réinitialiser » → « Tout réinitialiser… » → confirm « Tout réinitialiser ». Close the dialog. Press « Analyser » on « Dossier des documents », then on « Dossier des données ». Run `names` (**after**).
Expected: both cards keep `32 fichiers, 30 analysés, 0 illisibles` and `8 fichiers, 7 analysés, 1 illisible`; documents card lines: `2 fichiers n'ont pas été lus ...`, `43 personnes et organisations repérées, dont 0 déjà connues.`, `30 identifiants repérés ...`, `30 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.`; data card lines: `7 tableurs lus sur cet ordinateur, sans l'IA : ...`, `210 personnes et organisations repérées, dont 21 déjà connues.`, `278 identifiants repérés ...`, `22 colonnes de tableur reconnues (...) dans 7 tableurs lus.`, `102 liens lus entre deux colonnes d'un tableur.`, `30 documents déjà analysés relus ...`. `names` **after**: the same count and the same fingerprint as before (`entities: 578`): the base is a function of the files, nothing was typed by hand, so nothing is lost.

**C4** **A changed file: the old associations go, the new ones come.**

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py lifecycle modify
```

You read `changed courriers-entrants/2026-02/2026-02-24_courrier-neurologie-fabre-laurence.txt: Laurence Fabre -> Laurence Quéméner`. Press « Analyser ». Run `names`. Then « Réglages » → « Gérer… » → tab « Entités »: search `Quéméner`, then `Laurence Fabre`. Close the dialog. Ask `Que dit Laurence Quéméner ?`.
Expected: the card reads `32 fichiers, 30 analysés, 0 illisibles`, `4 personnes et organisations repérées, dont 3 déjà connues.`, `2 identifiants repérés ...`; `names`: `person active 196`, `person candidate 3`, `entities: 579`; the window: `Quéméner` is **one person**, tagged « À confirmer », found in that file only; `Laurence Fabre` is still there (the workbooks name her, tagged « À confirmer ») but **no longer in that letter**; the question gives **no « Recherche ciblée » line** (a name met once, with no title, is only a candidate, and a candidate never narrows a search).

**C5** **A deleted file: its associations go, the others stay.**

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py lifecycle delete
```

You read `deleted caisses-assurances/suisse/2026-03-20_decompte-css-rochat-sophie.pdf (a letter about Sophie Rochat)`. Press « Analyser ». Run `names`. Ask `Résume le dossier de Sophie Rochat.`, then `Quels courriers viennent de la CSS Assurance ?`.
Expected: the card reads `31 fichiers, 29 analysés, 0 illisibles` and `1 document retiré du dossier, oublié de l'analyse.`; `names`: `person active 195`, `person candidate 4`, `entities: 579`; for Sophie Rochat **no « Recherche ciblée » line** (no document names her any more) and the answer does not cite the deleted file; for the CSS question the line reads `Recherche ciblée : 1 document sur 29 — CSS Assurance` and the answer is the new letter of Sidonie Quenneville (`courrier-quenneville-nouveau.txt`), which writes « la CSS Assurance ».

**C6** **The library grows, then shrinks, as a real folder does.**

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py tier 100
```

You read `added 70 documents: the library goes from 30 to 100` and `100 documents: 89 the application reads, 11 it does not read (formats: csv, eml, odp, odt, ppt, pptx, rtf, vtt, xlsx)` (the `.csv` and the two `.xlsx` found among the documents are tables, read by the other card). Press « Analyser »: write the time. Run `names`. Then:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py tier 30
```

You read `removed 70 documents: the library goes from 100 to 30`. Press « Analyser ». Run `names`.
Expected after the growth: **only the new files are read**, the card reads `101 fichiers, 90 analysés, 0 illisibles`, `7 fichiers lus par reconnaissance de texte.`, `8 fichiers n'ont pas été lus : leur format n'est pas pris en charge (2 .odt, 1 .eml, 1 .odp, 1 .ppt, 1 .pptx, 1 .rtf, 1 .vtt).`, `115 personnes et organisations repérées, dont 87 déjà connues.`, `78 identifiants repérés ...`, `29 documents déjà analysés relus ...`, the button plain (before the Analyse-button fix it read « Analyser (11) »: the three tables and the eight unread files); it takes much less than analysing 100 from scratch; `names`: `identifier active 307`, `location active 51`, `organization active 38`, `organization candidate 6`, `person active 211`, `person candidate 6`, `entities: 619`. Expected after the shrink: the card reads `31 fichiers, 29 analysés, 0 illisibles` and `61 documents retirés du dossier, oubliés de l'analyse.`; `names`: **the same counts as before the growth** (`identifier active 295`, `location active 51`, `organization active 29`, `organization candidate 5`, `person active 195`, `person candidate 4`, `entities: 579`).

**You leave with:** the 30-document folder with a new letter, a renamed letter, a changed letter and one letter fewer (31 files, 29 documents analysed); the base rebuilt from it; mode « Automatique »; « Tous les documents » ticked; nothing in the data card.

---



## Part D - 100 documents (1 hour, mostly waiting)

**You start with:** what Part C leaves.

**D1** Grow the library to 100 again and read what the Analyse costs:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py tier 100
```

Press « Analyser » on « Dossier des documents ». The folder holds the **guide of 115 pages** and the **manual** among others: this is the slow Analyse. Write the time. While it runs you can read the state of the test folders:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py status
```

Expected: the helper says `added 70 documents: the library goes from 30 to 100`; the card, once finished, reads `101 fichiers, 90 analysés, 0 illisibles` and the same lines as after the growth of C6 (`7 fichiers lus par reconnaissance de texte.`, the 8 unread files, `115 personnes et organisations repérées, dont 87 déjà connues.`, `78 identifiants repérés ...`, `29 documents déjà analysés relus ...`); `status` reads `Documents folder: ...kb-lot11 <- the test folder` and `holds 101 files (library tier 100)`; the button is plain.

**D2** Three questions at 100 documents, in « Désactivé » first and then in « Automatique » (change the mode in « Réglages »; clear the conversation before each; note the time under the answer and, in « Automatique », the line). The Swiss and hospital questions are in Part S.

**D2-11** `Qui renouvelle le code de l'armoire à pharmacie ?` Expected: in both modes the answer is Sandrine Colas and cites `documentation\guide-pratique-cabinet.pdf` at `page 66`; **no line** in « Automatique » (a question that names nobody).

**D2-12** `À qui signaler un écart de la chaîne du froid ?` Expected: Nadia Bensaïd, from `documentation\manuel-qualite.docx` (a Word file has no pages: the source reads `page 1`); no line.

**D2-13** `Résume le dossier de Fatima Benali.` Expected: in « Automatique » `Recherche ciblée : 3 documents sur 90 — Fatima Benali` (or the widened line), the three files `laboratoire\2026-01\2026-01-29_resultats-analyses-benali-fatima.pdf`, `courriers-entrants\2026-08\2026-08-01_courrier-rhumatologie.pdf`, `caisses-assurances\suisse\2026-08-17_decompte-groupe-mutuel.pdf`; HbA1c `7,0 %`, next visit `30 octobre 2026`, `CHF 151.05`.

**You leave with:** the 100-document library (101 files) analysed (90 documents), « Tous les documents » ticked, nothing in the data card, mode **« Automatique »**.

---



## Part S - Switzerland, the tables and the file names (50 min)

**You start with:** what Part D leaves (mode « Automatique », « Tous les documents » ticked, nothing in the data card).

**S0** Add the invented files of the Swiss day: two amount tables (the same ten patients written the Swiss way, `CHF 1'450.00`, and the French way, `1 450,00 €`), three small workbooks and two documents named `neurologie.txt` (one in `2026\janvier`, one in `2026\mars`):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py extra-files
```

You read `wrote patients-france.csv in ...\kb-lot11-data`, `wrote patients-suisse.csv`, `wrote rdv-dates-ambigues.csv`, `wrote rdv-export-titre.csv`, `wrote rdv-medecins.csv`, `wrote 2026/janvier/neurologie.txt in ...\kb-lot11`, `wrote 2026/mars/neurologie.txt`. Press « Analyser » on « Dossier des documents », then on « Dossier des données ».
Expected: the documents card reads `103 fichiers, 92 analysés, 0 illisibles` and, as its only line, `8 fichiers n'ont pas été lus : leur format n'est pas pris en charge (2 .odt, 1 .eml, 1 .odp, 1 .ppt, 1 .pptx, 1 .rtf, 1 .vtt).`; the data card reads `13 fichiers, 12 analysés, 1 illisible` and `12 tableurs lus sur cet ordinateur, sans l'IA : ...`, `25 personnes et organisations repérées, dont 22 déjà connues.`, `8 colonnes de tableur reconnues (personnes, organisations, identifiants...) dans 5 tableurs lus.`, `19 liens lus entre deux colonnes d'un tableur.`, `92 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.`; **both buttons are plain** (the 8 unread files, the three tables among the documents and the `.ods` are not asked of the pass).

**S0-names** Run `python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py names`.
Expected: `identifier active 307`, `location active 51`, `organization active 38`, `organization candidate 6`, `person active 215`, `person candidate 5`, `entities: 622`.

**Documents.** Selection: « Tous les documents » ticked; nothing in the data card; mode « Automatique ». Clear the conversation before every question. The text of the AI's answer depends on the model: judge the line under it and « Sources ». « Voir les fichiers » under the line lists the files.

**S1** `À qui appartient le numéro AVS 756.3052.5778.43 ?` Expected: a normal answer written by the AI (**not** « Aucun fichier nommé 756.3052.5778.43 ne se trouve dans votre dossier de documents. », and the line « Réponse établie depuis votre dossier de documents, sans l'IA. » is not there); the line `Recherche ciblée : 1 document sur 92` (the end of the line, after the dash, is the number or its label: write it); « Sources » lists `2026-03-26_decompte-helsana-martin-marie.pdf`.

**S2** `Quels courriers a écrit la Dre Anne-Sophie Vuagniaux ?` Expected: `Recherche ciblée : 4 documents sur 92 — Anne-Sophie Vuagniaux` (the name **without** `Dre`); « Voir les fichiers »: `2026-01-08_courrier-endocrinologie-girardin-thierry.pdf`, `2026-06-25_courrier-orl.docx`, `2026-10-01_courrier-dermatologie.pdf`, `2026-10-04_resultats-analyses-maillard-pascal.pdf`. Write the four names.

**S3** `Que dit la Dre Vuagniaux ?` Expected: **no** row « Plusieurs personnes correspondent à « Dre Vuagniaux » : » (a patient, Gilles Vuagniaux, shares the surname; the title tells them apart); the line `Recherche ciblée : 4 documents sur 92 — Anne-Sophie Vuagniaux`, or « Recherche élargie à toute votre sélection : peu de résultats dans les documents ciblés. » (the narrowed search found too little by meaning; note it, it is not a failure of this step).

**S4** `Quels documents concernent HUG ?` Expected: `Recherche ciblée : 2 documents sur 92 — Hôpitaux universitaires de Genève` (the acronym is found); files `2026-02-12_courrier-orl-dupont-jean.pdf` and `guide-pratique-cabinet.pdf`; **not** the list of the whole folder.

**S5** `Quels documents viennent de Genève ?` Expected: **no** « Recherche ciblée » line (`Genève` is a commune written in tables: it no longer narrows the documents); up to six sources.

**S6** `Quels courriers viennent du CHUV ?` Expected: `Recherche ciblée : 5 documents sur 92 — Centre hospitalier universitaire vaudois` (the full name although you typed the acronym); files `2026-03-09_courrier-orl-petrovic-elena.docx`, `2026-03-12_courrier-cardiologie.docx`, `2026-06-25_courrier-orl.docx`, `2026-08-01_courrier-rhumatologie.pdf`, `2026-10-01_courrier-dermatologie.pdf`.

**S7** `Quels documents concernent les Hôpitaux universitaires de Genève ?` Expected: the same line and the same two files as S4 (the acronym and the full name are one entity).

**S8** `Résume le dossier de Giuseppe D'Amico.` Expected: `Recherche ciblée : 1 document sur 92 — Giuseppe D'Amico`; the file `2026-03-26_courrier-cpam74.pdf` (he is written alone on a line, above his street, in that letter). It must not name another D'Amico.

**S9-1** `Quels documents parlent de Lucas Roy ?` Expected: `Recherche ciblée : 2 documents sur 92 — Lucas Roy`; files `2026-08-17_courrier-orl-roy-lucas.md` and `2026-10-01_courrier-dermatologie.pdf`.

**S9-2** `Quels documents parlent de Roy Lucas ?` Expected: `Recherche ciblée : 2 documents sur 92 — Roy Lucas`; files `2026-04-20_courrier-rhumatologie-lucas-roy.pdf` and `2026-10-01_courrier-dermatologie.pdf`: two different people, two different sets (the dermatology letter names both).

**S10** `Que dit le courrier de Aïcha N'Diaye ?` Expected: `Recherche ciblée : 1 document sur 92 — Aïcha N'Diaye`; the file `2026-03-23_resultats-analyses-n-diaye-aicha.pdf`.

**Tables.** Selection: on « Dossier des documents » **untick** « Tous les documents » (« Documents utilisés : aucun »); on « Dossier des données » tick « Tous les tableurs » (« Données utilisées : toutes »). Each question names its workbook. The answers are computed by the table engine, without the AI: here a label « sans l'IA » is the right one. The numbers may show a thin space.

**S-T1** `Quelle est la somme des montants dans patients-suisse.csv ?` Expected: `Somme de Montant : 8 941,54 CHF.` then `Calculé sur 10 lignes.`

**S-T2** `Quel est le montant maximum dans patients-suisse.csv ?` Expected: `Plus grande valeur de Montant : 2 310,25 CHF.`

**S-T3** `Quelle est la somme des montants dans patients-france.csv ?` Expected: `Somme de Montant : 8 941,54 €.`

**S-T4** `Combien de lignes dans factures-fournisseurs-2026.xlsx ?` Expected: `La feuille Factures contient 39 lignes.` and **no** line « Compris comme : ... ».

**S-T5** `Combien de lignes dans rdv-export-titre.csv ?` Expected: `La feuille rdv-export-titre contient 10 lignes.` (the file has a title line above its header).

**S-T6-1** `Combien de lignes pour Dr Jean Dupont dans rdv-medecins.csv ?` Expected: `Nombre de lignes : 3.` and `Compris comme : medecin = Dr Jean Dupont.`

**S-T6-2** `Combien de lignes pour Jean Dupont dans rdv-medecins.csv ?` Expected: the same as S-T6-1.

**S-T7** `Quelle est la somme de montant pour Dr Jean Dupont dans rdv-medecins.csv ?` Expected: a sum of 150 with `Compris comme : medecin = Dr Jean Dupont.`

**S-T8-1** `Combien de lignes pour Dr Paul Martin dans rdv-medecins.csv ?` Expected: 2, `Compris comme : medecin = Dr Paul Martin.`

**S-T8-2** `Combien de lignes pour Martin dans rdv-medecins.csv ?` Expected: **3** and `Compris comme :` starting with `medecin parmi` (the two doctors called Martin).

**S-T9** `Combien de lignes pour Alice Archer en mars 2026 dans rdv-medecins.csv ?` Expected: 2, with the month and the year in `Compris comme` (this file has dates above the 12th, so its date column is settled).

**S-T10** `Combien de lignes pour Alice Archer dans rdv-dates-ambigues.csv ?` Expected: `Nombre de lignes : 2.`

**S-T11-1** `Combien de lignes pour Alice Archer en mars 2026 dans rdv-dates-ambigues.csv ?` Expected: no count; the message `Ce filtre porte sur un jour, un mois, une année ou une période, mais cette colonne de rdv-dates-ambigues.csv n'est pas une colonne de dates reconnue.`, then `Colonne concernée : date.`, then `Ses colonnes sont : patient, date, medecin.` (every day of that file is 12 or below, so the file does not say which number is the day).

**S-T11-2** `Combien de lignes en 2026 dans rdv-dates-ambigues.csv ?` Expected: the same message as S-T11-1.

**S-T11-3** `Combien de lignes pour Alice Archer en mars dans rdv-dates-ambigues.csv ?` Expected: the same message as S-T11-1.

**File names.** Selection: tick « Tous les documents » again, and on « Dossier des données » untick « Tous les tableurs » (« Données utilisées : aucune »). The AI machine must be reachable for the answers; what you check is which file is used.

**S-F1** `Que dit mars/neurologie.txt ?` Expected: the answer is built from one file; under « Sources » only `2026/mars/neurologie.txt` (the text speaks of March).

**S-F2** `Que dit neurologie.txt ?` Expected: « Plusieurs fichiers s'appellent neurologie.txt. Lequel voulez-vous ? » with the two files `2026/janvier/neurologie.txt` and `2026/mars/neurologie.txt`.

**S-F3** `Que dit avril/neurologie.txt ?` Expected: « Aucun fichier nommé avril/neurologie.txt ne se trouve dans votre dossier de documents. » (no `avril` folder: nothing is guessed).

**Documents and tables together.** Selection: keep « Tous les documents » ticked and also tick « Tous les tableurs » (« Données utilisées : toutes »).

**S-M1** `Combien de lignes ?` Expected: « Plusieurs tableurs sont sélectionnés. Lequel voulez-vous ? Nommez-le dans la question : » followed by **twelve** workbooks: `contrats-2026.xls`, `contrats-2026.xlsx`, `factures-fournisseurs-2026.xlsx`, `patients-ch.xlsx`, `patients-fr.csv`, `patients-france.csv`, `patients-suisse.csv`, `planning-garde-2026.xlsx`, `rdv-dates-ambigues.csv`, `rdv-export-titre.csv`, `rdv-medecins.csv`, `rendez-vous-2026.xlsx`.

**S-M2** `Combien de documents ai-je ?` Expected: a folder answer « 92 ont été analysés. » and no choice of workbook (before the fix: a row count of a workbook).

**S-M3** `Que disent mes documents à propos de Jean Dupont ?` Expected: no « Plusieurs tableurs sont sélectionnés » at all: the question is answered from the documents (an answer and « Sources » listing documents). The wording depends on the model; judge only that the choice of a workbook is **not** shown (before the fix: « Quel classeur ? »).

**The knowledge window.** Any selection. « Réglages », group « Connaissances », « Gérer… »: the window « Connaissances » opens on the tab « Entités ».

**S-K1** In « Rechercher un nom » type `chuv`; in « Type » choose « Organisation ». Click the row.
Expected: one row, **Centre hospitalier universitaire vaudois**, tag **Confirmée**, « 5 fichiers »; under « Noms reconnus » `CHUV` and the full name; under « D'où vient cette entité » « Repéré par : organisation connue du vocabulaire. Confiance : 0,80. »

**S-K2** Clear the search, keep « Type » on « Organisation », and look through the list. Then set « Type » to « Personne » and search `hôtel`.
Expected: you find `Hôpitaux universitaires de Genève`, `Hospices Civils de Lyon`, `Réseau hospitalier neuchâtelois`, `CPAM du Rhône`, `Dupont Immobilier SA`, `Bureautique Léman SA`, `Papeterie du Lac SARL`; you do **not** find a name that starts with `Entre`, nor `Fils SARL` alone; for `hôtel` among the persons: « Aucune entité ne correspond. »

**S-K3** Tab « À vérifier », section « Correspondances possibles ». Do nothing with them.
Expected: a pair `Roy Lucas` / `Lucas Roy` with the reason « prénom et nom dans l'ordre inverse », among a number of other pairs of the same reason (a person written surname first in one table and given name first in another: the base does not guess, it asks).

**S-K4** Back to « Entités », « Type » on « Personne », search `Lucas Roy`. Click the row named exactly **Lucas Roy**, then « Supprimer », confirm « Supprimer ».
Expected: two rows, `Lucas Roy` and `Roy Lucas` (the second knows the first's name as its reversed order); after the deletion the note « Cette entité a été supprimée : elle n'apparaît plus et l'analyse ne la recrée pas. » and the button « Restaurer »; the strip says that **92 documents seront relus à la prochaine analyse**. Click « Restaurer ».

**S-K5** Close the window. On « Dossier des données » press « Analyser » (nothing changed in the files). Reopen « Gérer… », search `Lucas Roy`, click the row named exactly **Lucas Roy**.
Expected: « D'où vient cette entité » lists, besides the documents, a line « Tableur » for `patients-fr.csv` (colonne « nom_patient ») and one for `patients-france.csv` (colonne « Patient »). **Known, defect KBD-48, deferred:** before the deletion the name also had a line for `patients-ch.xlsx` (colonne « Nom »); it does not come back. Write it if you see it. The tag « Saisie à la main » is normal: a restored name is yours from then on. Close the window.

**You leave with:** « Tous les documents » and « Tous les tableurs » both ticked, the conversation cleared, mode « Automatique », « Lucas Roy » restored, the 100-document library plus the extra files analysed (103 files, 92 documents; 13 data files, 12 analysed); 92 documents flagged to be re-read at the next Analyse of the documents.

---



## Part G - 200 documents (30 min, mostly waiting)

**You start with:** what Part S leaves.

**G0** Selection: leave « Tous les documents » ticked; on « Dossier des données » untick everything (« Données utilisées : aucune »).
Expected: the summaries read « Documents utilisés : tous » and « Données utilisées : aucune ».

**G1** Grow to 200:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py tier 200
```

You read `added 100 documents: the library goes from 100 to 200` and `200 documents: 176 the application reads, 24 it does not read (formats: csv, eml, odp, odt, ppt, pptx, rtf, vtt, xlsx)`. Press « Analyser » on « Dossier des documents » (only the new files are read; write the time).
Expected: `203 fichiers, 179 analysés, 0 illisibles`; `18 fichiers lus par reconnaissance de texte.`; `21 fichiers n'ont pas été lus : leur format n'est pas pris en charge (7 .odt, 5 .rtf, 4 .eml, 2 .vtt, 1 .odp, 1 .ppt, 1 .pptx).`; `156 personnes et organisations repérées, dont 128 déjà connues.`; `139 identifiants repérés ...`; `92 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.` (the flag of S-K4 and S-K5); the button plain (before the Analyse-button fix it read « Analyser (24) »).

**G1-names** Run `python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py names`.
Expected: `identifier active 350`, `location active 51`, `organization active 50`, `organization candidate 5`, `person active 220`, `person candidate 17`, `entities: 693`.

**G2** Ask these in « Désactivé » and then in « Automatique » (clear the conversation before each; write the time and, in « Automatique », the line). « sur 179 » is the number of documents analysed.

**G2-11** `Qui renouvelle le code de l'armoire à pharmacie ?` Expected: Sandrine Colas, `page 66` of the guide, in both modes; no line.

**G2-12** `À qui signaler un écart de la chaîne du froid ?` Expected: Nadia Bensaïd in both modes; no line.

**G2-13** `Résume le dossier de Fatima Benali.` Expected: in « Automatique » `Recherche ciblée : 6 documents sur 179 — Fatima Benali` (or the widened line); the six files include `2026-01-29_resultats-analyses-benali-fatima.pdf`, `2026-08-01_courrier-rhumatologie.pdf`, `2026-08-17_decompte-groupe-mutuel.pdf`.

**G2-15** `Quels documents concernent la Dre Vuagniaux ?` Expected: in « Automatique » `Recherche ciblée : 7 documents sur 179 — Anne-Sophie Vuagniaux` (7 kept, the 7 that hold the answer: the title « Dre » is read; before the batch 1 fix 2 were kept for 7) or the widened line.

**G2-16** `Quels documents viennent de Genève ?` Expected: **no line**.

**G2-6** `Quels documents parlent de la LAMal ?` Expected: **no line** (a subject, not a name).

**G3** Read the figures:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py report
```

Expected: for each mode a `median total` and a `median search`; **the gain of** `Automatique` **over « Désactivé » is the difference of the medians**; the base's own time is `knowledge ... / ...`. Write the lines.

**You leave with:** the 200-document library analysed (203 files, 179 documents), mode « Automatique », « Tous les documents » ticked, nothing in the data card.

---



## Part E - the figures of the release gate, on your use (spread over days; at least 100 questions)

**You start with:** what Part G leaves.

**E1** « Réglages » → « Ciblage intelligent » → **« Suggestions seulement »** (the default: the base measures what it would have done, and narrows nothing).
Expected: the group « Connaissances » reads « Ciblage intelligent » = « Suggestions seulement ».

**E2** Ask the sentences that the script prints (they have known answers):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py questions daily
```

Type 25 of them, clearing the conversation between them. Then **use the application for your own work in this mode** (your own questions on these documents, or on your own real folder once the legal conditions of `AGENTS.md` allow it) until the record holds more than 100 questions.
Expected: the list begins `List 'daily' for a library of 200 documents: type the sentences exactly as written.`

**E3** Read the figures:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py report
```

Write the whole output in the results book. The lines that decide: `Release gate, raw` and `Release gate, adjusted`, the number of questions the base would reduce, the median reduction, and `Why a question was not reduced`. `03-release-gate.md` says how they are read and why the raw figure is low by construction.
Expected: `Questions recorded` above 100.

**You leave with:** « Suggestions seulement » as the mode, more than 100 questions recorded.

---



## Part Z - optional: 500 and 1 000 documents, and your own books (overnight)

**You start with:** what Part E leaves. This part comes last because it is long and changes a lot; **its undo is Part R**, which restores your index and your settings (the test folders are left on disk).

**Z1** Write the large library of 1 000 documents (once, about ten seconds; it is kept out of git):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py generate
```

Expected: `running: ...gen-knowledge-sandbox.py --documents 1000 ...` then a block that ends with `"documents": 1000,`, `"readable": 878`, `"unreadable": 122` (or `already there: ...`).

**Z2** `python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py tier 500`, then « Analyser » (hours on a CPU-only machine: the embedding is nine tenths of it; write the time). Set the mode to « Désactivé », then « Automatique », and ask `Qui renouvelle le code de l'armoire à pharmacie ?`, `Résume le dossier de Fatima Benali.`, `Quels documents viennent de Genève ?` and `Quels documents parlent de la LAMal ?` in each; note the time under each.
Expected: the answers of G2 still hold; write the times.

**Z3** `python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py tier 1000`, « Analyser », the same questions. At this size the 122 files the application does not read are about an eighth of the folder (878 are read): write what the card says about them.
Expected: a line « N fichiers n'ont pas été lus : leur format n'est pas pris en charge (...) » listing the formats.

**Z4** Copy into `kb-lot11` (a new sub-folder `livres`) any large PDF or DOCX of your own that is not a patient file (a medical or legal handbook, a manual of a hundred pages or more), press « Analyser », write the time, and ask three questions whose answer you know is on a page you can name.
Expected: the page is cited (« page N ») and the answer is right; a Word file has **no page** (the application keeps it as one page; known).

**You leave with:** the large libraries analysed in the application (the restore of Part R puts your own index back).

---



## Part R - the one restore (5 min)

**You start with:** the application open, in any state.

**R1** Close the application (close the window; check that the terminal that ran `pnpm tauri dev` has stopped).

**R2** Put back your index, your settings and the key file:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py restore
```

Expected: `restored index.sqlite3`, `restored settings.json`, then `restored knowledge-identifier.key` (or `removed knowledge-identifier.key (there was none when the backup was made)`), then the note that `kb-lot11` and `kb-lot11-data` are still on disk.

**R3** (Optional) delete the test folders, which hold only invented files: `kb-lot11` and `kb-lot11-data` beside your Documents folder, and `data\knowledge-sandbox-1000` in the repository.

**R4** Start the application (`cd apps\desktop`, `pnpm tauri dev`).
Expected: your own folders and your own mode are back, as before Part 0.

**You leave with:** the application as it was before Part 0.

---



## Part M - on the Mac mini (skippable now; play it when the Mac mini is there)

Nothing in Parts 0 to R depends on this part. **Not walked by the agent: the Mac mini was not available on 13 October 2026**, so every macOS spelling below is written from the code (`os.name`, `Library/Application Support/<identifier>`) and not from a run; a step that does not match is a finding about this part first.

**You start with:** the repository at the same commit on the Mac, `python3`, `pnpm` and the Rust toolchain installed, `pnpm install` done in `apps/desktop`, the application closed, and the AI machine reachable.

**M1** The first automated test run on the Mac (the one exception to « no suites in a human test »: nothing of this product has ever run on macOS, and the CI runs the same command on macOS):

```text
cd apps/desktop/src-tauri
cargo test --lib
cd ../../..
```

Expected: `test result: ok.` and `0 failed`.

**M2** Look at what the helper sees (run from the repository root):

```text
python3 docs/test-reports/knowledge-base-pass-1/human-tests/lot11_helper.py status
```

Expected: `Settings folder : /Users/<you>/Library/Application Support/com.assistantcabinetai.desktop`, `Index folder    : /Users/<you>/Library/Application Support/com.assistantcabinetai.desktop`, `Application     : closed`, `Core library    : present`.

**M3** Back up, prepare 30 documents, empty the record of questions:

```text
python3 docs/test-reports/knowledge-base-pass-1/human-tests/lot11_helper.py backup
python3 docs/test-reports/knowledge-base-pass-1/human-tests/lot11_helper.py prepare 30
python3 docs/test-reports/knowledge-base-pass-1/human-tests/lot11_helper.py clear-log
```

Expected: the same lines as steps 0.2, 0.3 and 0.4, with `/` in the paths.

**M4** Start the application (`cd apps/desktop`, `pnpm tauri dev`), press « Analyser » on « Dossier des documents » and then on « Dossier des données », and read the cards.
Expected: the same figures as A1 and A2 (`30 fichiers, 28 analysés, 0 illisibles`, `8 fichiers, 7 analysés, 1 illisible`); the text recognition is the Mac's own engine, so `3 fichiers lus par reconnaissance de texte.` may differ by one: write it. The two buttons are plain.

**M5** Select « Tous les documents », set « Ciblage intelligent » to « Automatique », ask `Résume le dossier de Sophie Rochat.` and `Quels courriers viennent du Dr Marc Bersier ?`.
Expected: the lines of A6-1 and A6-3 (`Recherche ciblée : 1 document sur 28 — Sophie Rochat`, `... — Marc Bersier`).

**M6** Read the base from the outside, then put everything back (close the application first):

```text
python3 docs/test-reports/knowledge-base-pass-1/human-tests/lot11_helper.py names
python3 docs/test-reports/knowledge-base-pass-1/human-tests/lot11_helper.py restore
```

Expected: `entities: 577` for `names` (the figure of C0), then `restored index.sqlite3` and `restored settings.json`.

**You leave with:** the Mac as it was before M1, apart from the test folders `kb-lot11` and `kb-lot11-data`.

---



## Results and findings

Fill `docs/test-reports/knowledge-base-pass-1/02-results.md`. A deviation is a finding about **this protocol first**: tell the agent which step, what you saw, what the step said; the agent fixes the protocol and gives the short ordered re-run. Anything the product did that is wrong goes into `defects-register.md` as a new row, with the step. This test ends the knowledge programme: nothing is planned after it but the fixes that your findings call for.