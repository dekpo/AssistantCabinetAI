# 02 - The results book of the final human test (lot 11, readjusted)

Filled in by the owner while she plays `human-tests/lot-11-validation.md`; nothing below is a result until a « Seen » line is filled. Written for the owner and for the developer or agent who reads her results. English; the questions are quoted as typed.

**The agent's own results are not in this file.** Everything the agent could measure by itself (entity quality, recall, leaks, time, the real stack, the text recognition, the formats, the walk of this protocol on copies) is in `measurements/` and summed up in `04-final-report.md`; the verdict is `03-release-gate.md`. This file holds only what the owner sees in her window and on her library. **Every « Expected » below was produced by the same code on the same invented files, in the same order as the protocol; the file is generated from the protocol's own text, so the two cannot disagree.**

Machine and run: date ____ · workstation ____ · AI machine and how connected ____ · chat model ____ · embedding model ____ · build (branch, commit) ____ · services (tower or Docker) ____

How to fill a block: **Seen** what the screen or the command printed (a number, a line, a file name); **OK / not OK**; for a question, **Right?** the answer says what the library says (Y / N / partly), **Sources?** the files cited are the ones that hold the answer (Y / N), **Missed?** a file that holds the answer was not used (Y / N, which), **Added?** a file that does not hold the answer was used (Y / N, which), and the time under the answer (« Généré par ... en ... »). **Register row**: the `KBD-nn` row of `defects-register.md` that a finding belongs to, or `new`.


## Part 0 - Preparation

### 0.1

**Expected:** `Application     : closed` and `Core library    : present`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### 0.2

**Expected:** `saved   index.sqlite3` (or `absent  index.sqlite3 (nothing to save)`), `saved   settings.json`, `saved   knowledge-identifier.key` (or `absent  knowledge-identifier.key (nothing to save)`), then `Backup in C:\Users\elise\kb-backup-lot-11`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### 0.3

**Expected:** `wrote 30 invented documents in ...\kb-lot11`, `30 documents: 28 the application reads, 2 it does not read (formats: odt, vtt)`, `scans (OCR): 3; PDF pages: 16`, `wrote 8 invented workbooks in ...\kb-lot11-data (formats: csv, ods, xls, xlsx)`, your two folders, then `"knowledgeMode" is "suggest", "writeTimingLog" is true and both health modules (France and Switzerland) are on.`

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### 0.4

**Expected:** `deleted retrieval-timings.jsonl` or `no retrieval-timings.jsonl to delete`, then `emptied kb_diagnostics (N row(s))` or `kb_diagnostics does not exist yet`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### 0.5

**Expected:** at the top, the AI status reads « IA Disponible » (if it reads « IA Indisponible », start the AI machine and press « Vérifier l'IA »).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### 0.6

**Expected:** group « Connaissances »: « Ciblage intelligent » = « Suggestions seulement »; « Module santé »: both « Santé : France » and « Santé : Suisse » ticked; group « Avancé »: « Noter les durées dans un journal » ticked.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### 0.7

**Expected:** after (c) the message « La base de connaissances a été réinitialisée. »; the cards read `30 fichiers, 0 analysés, 0 illisibles` (documents) and `8 fichiers, 0 analysés, 1 illisible` (data).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### 0.8

**Expected:** `entities: 0   fingerprint of the list: e3b0c44298fc1c14` and `names of the test questions found in the base: none`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 


## Part A - 30 documents

### A1

**Expected:** the line under the title reads `30 fichiers, 28 analysés, 0 illisibles`; under the card: `3 fichiers lus par reconnaissance de texte.`, `2 fichiers n'ont pas été lus : leur format n'est pas pris en charge (1 .odt, 1 .vtt).`, `42 personnes et organisations repérées, dont 0 déjà connues.`, `30 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).` **The button « Analyser » is plain, without a number** (before the Analyse-button fix it read « Analyser (2) » for ever, because of the `.odt` and the `.vtt`: if you see a number, it is a bug, defect KBD-47).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A2

**Expected:** `8 fichiers, 7 analysés, 1 illisible` (`rendez-vous-2026.ods` is the one that cannot be read: « pas un tableur (CSV ou XLSX) »); under the card: `7 tableurs lus sur cet ordinateur, sans l'IA : les tableurs s'analysent en un instant.`, `210 personnes et organisations repérées, dont 21 déjà connues.`, `278 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).`, `22 colonnes de tableur reconnues (personnes, organisations, identifiants...) dans 7 tableurs lus.`, `102 liens lus entre deux colonnes d'un tableur.`, `28 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.` The button « Analyser » of this card is plain, without a number.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A3

**Expected:** the summaries read « Documents utilisés : tous » and « Données utilisées : aucune » (with documents and several workbooks ticked together, a question about a table asks « Plusieurs tableurs sont sélectionnés », which Part S shows on purpose).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A4

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A4-1

**Expected:** right; source `...decompte-css-rochat-sophie.pdf`; the refund `CHF 486.63`; no targeting line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A4-2

**Expected:** right; source `...2026-06-20_resultats-analyses.pdf`; `6,3 %`; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A4-3

**Expected:** right; source `...courrier-cardiologie-monnier-veronique.pdf`; `26 mai 2026`; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A4-4

**Expected:** right; source `...decompte-css-rochat-sophie.pdf`; `CHF 486.63`; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A4-5

**Expected:** both invoices cited (`...facture-medsupply-fac-2026-014.pdf`, `...facture-papeterie-lac-fac-2026-014.pdf`); `CHF 352.73` and `93,12 €`; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A4-6

**Expected:** the Swiss insurers' letters cited (`...decompte-helsana-martin-marie.pdf`, `...decompte-css-rochat-sophie.pdf`); no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A4-7

**Expected:** the answer says that the documents do not say; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A4-8

**Expected:** the row « Plusieurs personnes correspondent à « J. Dupont » : » offering `Jean Dupont (3 documents)` and `Jean-Pierre Dupont (1 document)`; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A4-9

**Expected:** the row « Plusieurs personnes correspondent à « Dr Martin » : » offering `Pierre Martin (3 documents)` and `Paul Martin (3 documents)`; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A4-10

**Expected:** right; source `...courrier-cardiologie-monnier-veronique.pdf`; `26 mai 2026`; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A5

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A5-1

**Expected:** the same answer and sources as A4-1; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A5-2

**Expected:** the same as A4-2; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A5-3

**Expected:** the same as A4-3; no line. **Write the list of « Sources »: Part B compares with it.**

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A5-4

**Expected:** the same as A4-4; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A5-5

**Expected:** the same as A4-5; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A5-6

**Expected:** the same as A4-6; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A5-7

**Expected:** the documents do not say; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A5-8

**Expected:** an answer drawn from the documents; **no** row « Plusieurs personnes... » (the base is off).

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A5-9

**Expected:** an answer drawn from the documents; no row.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A5-10

**Expected:** the same as A4-10; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A6

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A6-1

**Expected:** `Recherche ciblée : 1 document sur 28 — Sophie Rochat` (or the widened line).

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A6-2

**Expected:** `Recherche ciblée : 1 document sur 28 — Jean Dupontet` (or the widened line).

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A6-3

**Expected:** `Recherche ciblée : 1 document sur 28 — Marc Bersier`.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A6-4

**Expected:** `Recherche ciblée : 1 document sur 28 — CSS Assurance`.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A6-5

**Expected:** `Recherche ciblée : 2 documents sur 28 — FAC-2026-014`.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A6-6

**Expected:** **no line** (`LAMal` is a subject, not a name: nothing is narrowed).

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A6-7

**Expected:** **no line**; the documents do not say.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A6-8

**Expected:** **no line**; the row « Plusieurs personnes correspondent à « J. Dupont » : » with `Jean Dupont (3 documents)` and `Jean-Pierre Dupont (1 document)`.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A6-9

**Expected:** **no line**; the row « Plusieurs personnes correspondent à « Dr Martin » : » with `Pierre Martin (3 documents)` and `Paul Martin (3 documents)`.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A6-10

**Expected:** `Recherche ciblée : 1 document sur 28 — Véronique Monnier` (or the widened line). This letter is a **scan**: on the real text recognition its name is read right in some runs and wrong in others; if there is no line at all, the scan was misread, which is a result to write down.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### A7

**Expected:** `Questions recorded            : 20` (the ten of « Suggestions seulement » and the ten of « Automatique »; « Désactivé » writes nothing in this table) and a block « By mode » with 10 questions for each of the three modes (`off`, `suggest`, `auto`): the knowledge lookup in milliseconds, the search, the first word and the whole answer. **Write the median `total` of the three modes in the results book**: it is the answer to « is `Automatique` faster », and it is the model's time, not the base's.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 


## Part B - safety

### B1

**Expected:** the row « Plusieurs personnes correspondent à « J. Dupont » : » offers `Jean Dupont (3 documents)` and `Jean-Pierre Dupont (1 document)`; **no** « Recherche ciblée » line before you press (nothing was narrowed). After the press, the question is asked again with that person as a chip (the chip reads the name; « Retirer ... » removes it) and **now** a « Recherche ciblée » line appears.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### B2

**Expected:** the row offers `Pierre Martin (3 documents)` and `Paul Martin (3 documents)` (the title « Dr » keeps the patients called Martin out of the choice); no line.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### B3

**Expected:** the line reads `Recherche ciblée : 1 document sur 28 — Marc Bersier` with the buttons « Voir les fichiers » and « Élargir à ma sélection »; after the press the question is answered on all the documents and **the list is the one you wrote for A5-3** (sentence 3 in « Désactivé »), file for file. If it is not, it is a bug.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### B4

**Expected:** the summary reads « Documents utilisés : 27 fichiers »; **that file is not in « Sources »**, in « Automatique » (no targeting line: no ticked document names her) as in « Désactivé »; the named-file question is answered « 2026-03-20_decompte-css-rochat-sophie.pdf se trouve dans votre dossier de documents mais n'est pas sélectionné pour cette conversation. Cochez-le dans la liste des documents pour l'utiliser. »

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### B5

**Expected:** both outputs are identical (`identifier active 295`, `location active 51`, `organization active 29`, `organization candidate 5`, `person active 196`, `person candidate 1`, `entities: 577`, the same fingerprint) and the last line says `names of the test questions found in the base: none`; the window finds nothing for the three names.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### B6

**Expected:** for `Et son numéro AVS ?` the line names Sophie Rochat (the follow-up is meant to); for `Que contiennent les notes internes ?` the line **still names Sophie Rochat** (the previous person narrowed it) and the search then widens by itself (« Recherche élargie à toute votre sélection : ... »); the answer may describe her letter instead of the notes; « Élargir à ma sélection » does not repair it; asked alone, the answer is about the internal notes. Write what you saw.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 


## Part C - a library that lives

### C0

**Expected:** `identifier active 295`, `location active 51`, `organization active 29`, `organization candidate 5`, `person active 196`, `person candidate 1`, then `entities: 577` (write the fingerprint: it is compared at C3).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### C1

**Expected:** the card reads `31 fichiers, 29 analysés, 0 illisibles` and the lines `2 fichiers n'ont pas été lus : leur format n'est pas pris en charge (1 .odt, 1 .vtt).`, `46 personnes et organisations repérées, dont 45 déjà connues.`, `30 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).`, `28 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.`; `names`: **one more person** (`person active 197`, `entities: 578`); the line `Recherche ciblée : 1 document sur 29 — Sidonie Quenneville`, and the answer is about the new letter.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### C2

**Expected:** the card reads `32 fichiers, 30 analysés, 0 illisibles`; `1 fichier renommé pour retirer les espaces et les accents.` and `Courrier Quenneville été 2026.txt → Courrier-Quenneville-ete-2026.txt`; in the folder the file has the clean name; the line reads `Recherche ciblée : 2 documents sur 30 — Sidonie Quenneville` (two files now).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### C3

**Expected before:** `person active 197`, `person candidate 1`, `entities: 578` (the other lines as at C0).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### C3-a

**Expected:** the card reads `32 fichiers, 0 analysés, 0 illisibles` and the button reads « Analyser (30) » (lit; the `.odt` and the `.vtt` are not counted). Press « Analyser ». Expected after: `32 fichiers, 30 analysés, 0 illisibles`, `3 fichiers lus par reconnaissance de texte.`, `2 fichiers n'ont pas été lus ...`, `45 personnes et organisations repérées, dont 24 déjà connues.`, `30 identifiants repérés ...`; the button plain again.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### C3-b

**Expected:** `8 fichiers, 0 analysés, 1 illisible` and the button « Analyser (7) ». Press « Analyser ». Expected after: `8 fichiers, 7 analysés, 1 illisible`; the button plain.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### C3-c

**Expected:** both cards keep `32 fichiers, 30 analysés, 0 illisibles` and `8 fichiers, 7 analysés, 1 illisible`; documents card lines: `2 fichiers n'ont pas été lus ...`, `43 personnes et organisations repérées, dont 0 déjà connues.`, `30 identifiants repérés ...`, `30 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.`; data card lines: `7 tableurs lus sur cet ordinateur, sans l'IA : ...`, `210 personnes et organisations repérées, dont 21 déjà connues.`, `278 identifiants repérés ...`, `22 colonnes de tableur reconnues (...) dans 7 tableurs lus.`, `102 liens lus entre deux colonnes d'un tableur.`, `30 documents déjà analysés relus ...`. `names` **after**: the same count and the same fingerprint as before (`entities: 578`): the base is a function of the files, nothing was typed by hand, so nothing is lost.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### C4

**Expected:** the card reads `32 fichiers, 30 analysés, 0 illisibles`, `4 personnes et organisations repérées, dont 3 déjà connues.`, `2 identifiants repérés ...`; `names`: `person active 196`, `person candidate 3`, `entities: 579`; the window: `Quéméner` is **one person**, tagged « À confirmer », found in that file only; `Laurence Fabre` is still there (the workbooks name her, tagged « À confirmer ») but **no longer in that letter**; the question gives **no « Recherche ciblée » line** (a name met once, with no title, is only a candidate, and a candidate never narrows a search).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### C5

**Expected:** the card reads `31 fichiers, 29 analysés, 0 illisibles` and `1 document retiré du dossier, oublié de l'analyse.`; `names`: `person active 195`, `person candidate 4`, `entities: 579`; for Sophie Rochat **no « Recherche ciblée » line** (no document names her any more) and the answer does not cite the deleted file; for the CSS question the line reads `Recherche ciblée : 1 document sur 29 — CSS Assurance` and the answer is the new letter of Sidonie Quenneville (`courrier-quenneville-nouveau.txt`), which writes « la CSS Assurance ».

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### C6

**Expected after the growth:** **only the new files are read**, the card reads `101 fichiers, 90 analysés, 0 illisibles`, `7 fichiers lus par reconnaissance de texte.`, `8 fichiers n'ont pas été lus : leur format n'est pas pris en charge (2 .odt, 1 .eml, 1 .odp, 1 .ppt, 1 .pptx, 1 .rtf, 1 .vtt).`, `115 personnes et organisations repérées, dont 87 déjà connues.`, `78 identifiants repérés ...`, `29 documents déjà analysés relus ...`, the button plain (before the Analyse-button fix it read « Analyser (11) »: the three tables and the eight unread files); it takes much less than analysing 100 from scratch; `names`: `identifier active 307`, `location active 51`, `organization active 38`, `organization candidate 6`, `person active 211`, `person candidate 6`, `entities: 619`. Expected after the shrink: the card reads `31 fichiers, 29 analysés, 0 illisibles` and `61 documents retirés du dossier, oubliés de l'analyse.`; `names`: **the same counts as before the growth** (`identifier active 295`, `location active 51`, `organization active 29`, `organization candidate 5`, `person active 195`, `person candidate 4`, `entities: 579`).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 


## Part D - 100 documents

### D1

**Expected:** the helper says `added 70 documents: the library goes from 30 to 100`; the card, once finished, reads `101 fichiers, 90 analysés, 0 illisibles` and the same lines as after the growth of C6 (`7 fichiers lus par reconnaissance de texte.`, the 8 unread files, `115 personnes et organisations repérées, dont 87 déjà connues.`, `78 identifiants repérés ...`, `29 documents déjà analysés relus ...`); `status` reads `Documents folder: ...kb-lot11 <- the test folder` and `holds 101 files (library tier 100)`; the button is plain.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### D2

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### D2-11

**Expected:** in both modes the answer is Sandrine Colas and cites `documentation\guide-pratique-cabinet.pdf` at `page 66`; **no line** in « Automatique » (a question that names nobody).

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### D2-12

**Expected:** Nadia Bensaïd, from `documentation\manuel-qualite.docx` (a Word file has no pages: the source reads `page 1`); no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### D2-13

**Expected:** in « Automatique » `Recherche ciblée : 3 documents sur 90 — Fatima Benali` (or the widened line), the three files `laboratoire\2026-01\2026-01-29_resultats-analyses-benali-fatima.pdf`, `courriers-entrants\2026-08\2026-08-01_courrier-rhumatologie.pdf`, `caisses-assurances\suisse\2026-08-17_decompte-groupe-mutuel.pdf`; HbA1c `7,0 %`, next visit `30 octobre 2026`, `CHF 151.05`.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 


## Part S - Switzerland, the tables and the file names

### S0

**Expected:** the documents card reads `103 fichiers, 92 analysés, 0 illisibles` and, as its only line, `8 fichiers n'ont pas été lus : leur format n'est pas pris en charge (2 .odt, 1 .eml, 1 .odp, 1 .ppt, 1 .pptx, 1 .rtf, 1 .vtt).`; the data card reads `13 fichiers, 12 analysés, 1 illisible` and `12 tableurs lus sur cet ordinateur, sans l'IA : ...`, `25 personnes et organisations repérées, dont 22 déjà connues.`, `8 colonnes de tableur reconnues (personnes, organisations, identifiants...) dans 5 tableurs lus.`, `19 liens lus entre deux colonnes d'un tableur.`, `92 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.`; **both buttons are plain** (the 8 unread files, the three tables among the documents and the `.ods` are not asked of the pass).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S0-names

**Expected:** `identifier active 307`, `location active 51`, `organization active 38`, `organization candidate 6`, `person active 215`, `person candidate 5`, `entities: 622`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S1

**Expected:** a normal answer written by the AI (**not** « Aucun fichier nommé 756.3052.5778.43 ne se trouve dans votre dossier de documents. », and the line « Réponse établie depuis votre dossier de documents, sans l'IA. » is not there); the line `Recherche ciblée : 1 document sur 92` (the end of the line, after the dash, is the number or its label: write it); « Sources » lists `2026-03-26_decompte-helsana-martin-marie.pdf`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S2

**Expected:** `Recherche ciblée : 4 documents sur 92 — Anne-Sophie Vuagniaux` (the name **without** `Dre`); « Voir les fichiers »: `2026-01-08_courrier-endocrinologie-girardin-thierry.pdf`, `2026-06-25_courrier-orl.docx`, `2026-10-01_courrier-dermatologie.pdf`, `2026-10-04_resultats-analyses-maillard-pascal.pdf`. Write the four names.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S3

**Expected:** **no** row « Plusieurs personnes correspondent à « Dre Vuagniaux » : » (a patient, Gilles Vuagniaux, shares the surname; the title tells them apart); the line `Recherche ciblée : 4 documents sur 92 — Anne-Sophie Vuagniaux`, or « Recherche élargie à toute votre sélection : peu de résultats dans les documents ciblés. » (the narrowed search found too little by meaning; note it, it is not a failure of this step).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S4

**Expected:** `Recherche ciblée : 2 documents sur 92 — Hôpitaux universitaires de Genève` (the acronym is found); files `2026-02-12_courrier-orl-dupont-jean.pdf` and `guide-pratique-cabinet.pdf`; **not** the list of the whole folder.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S5

**Expected:** **no** « Recherche ciblée » line (`Genève` is a commune written in tables: it no longer narrows the documents); up to six sources.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S6

**Expected:** `Recherche ciblée : 5 documents sur 92 — Centre hospitalier universitaire vaudois` (the full name although you typed the acronym); files `2026-03-09_courrier-orl-petrovic-elena.docx`, `2026-03-12_courrier-cardiologie.docx`, `2026-06-25_courrier-orl.docx`, `2026-08-01_courrier-rhumatologie.pdf`, `2026-10-01_courrier-dermatologie.pdf`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S7

**Expected:** the same line and the same two files as S4 (the acronym and the full name are one entity).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S8

**Expected:** `Recherche ciblée : 1 document sur 92 — Giuseppe D'Amico`; the file `2026-03-26_courrier-cpam74.pdf` (he is written alone on a line, above his street, in that letter). It must not name another D'Amico.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S9-1

**Expected:** `Recherche ciblée : 2 documents sur 92 — Lucas Roy`; files `2026-08-17_courrier-orl-roy-lucas.md` and `2026-10-01_courrier-dermatologie.pdf`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S9-2

**Expected:** `Recherche ciblée : 2 documents sur 92 — Roy Lucas`; files `2026-04-20_courrier-rhumatologie-lucas-roy.pdf` and `2026-10-01_courrier-dermatologie.pdf`: two different people, two different sets (the dermatology letter names both).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S10

**Expected:** `Recherche ciblée : 1 document sur 92 — Aïcha N'Diaye`; the file `2026-03-23_resultats-analyses-n-diaye-aicha.pdf`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T1

**Expected:** `Somme de Montant : 8 941,54 CHF.` then `Calculé sur 10 lignes.`

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T2

**Expected:** `Plus grande valeur de Montant : 2 310,25 CHF.`

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T3

**Expected:** `Somme de Montant : 8 941,54 €.`

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T4

**Expected:** `La feuille Factures contient 39 lignes.` and **no** line « Compris comme : ... ».

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T5

**Expected:** `La feuille rdv-export-titre contient 10 lignes.` (the file has a title line above its header).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T6-1

**Expected:** `Nombre de lignes : 3.` and `Compris comme : medecin = Dr Jean Dupont.`

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T6-2

**Expected:** the same as S-T6-1.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T7

**Expected:** a sum of 150 with `Compris comme : medecin = Dr Jean Dupont.`

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T8-1

**Expected:** 2, `Compris comme : medecin = Dr Paul Martin.`

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T8-2

**Expected:** **3** and `Compris comme :` starting with `medecin parmi` (the two doctors called Martin).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T9

**Expected:** 2, with the month and the year in `Compris comme` (this file has dates above the 12th, so its date column is settled).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T10

**Expected:** `Nombre de lignes : 2.`

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T11-1

**Expected:** no count; the message `Ce filtre porte sur un jour, un mois, une année ou une période, mais cette colonne de rdv-dates-ambigues.csv n'est pas une colonne de dates reconnue.`, then `Colonne concernée : date.`, then `Ses colonnes sont : patient, date, medecin.` (every day of that file is 12 or below, so the file does not say which number is the day).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T11-2

**Expected:** the same message as S-T11-1.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-T11-3

**Expected:** the same message as S-T11-1.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-F1

**Expected:** the answer is built from one file; under « Sources » only `2026/mars/neurologie.txt` (the text speaks of March).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-F2

**Expected:** « Plusieurs fichiers s'appellent neurologie.txt. Lequel voulez-vous ? » with the two files `2026/janvier/neurologie.txt` and `2026/mars/neurologie.txt`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-F3

**Expected:** « Aucun fichier nommé avril/neurologie.txt ne se trouve dans votre dossier de documents. » (no `avril` folder: nothing is guessed).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-M1

**Expected:** « Plusieurs tableurs sont sélectionnés. Lequel voulez-vous ? Nommez-le dans la question : » followed by **twelve** workbooks: `contrats-2026.xls`, `contrats-2026.xlsx`, `factures-fournisseurs-2026.xlsx`, `patients-ch.xlsx`, `patients-fr.csv`, `patients-france.csv`, `patients-suisse.csv`, `planning-garde-2026.xlsx`, `rdv-dates-ambigues.csv`, `rdv-export-titre.csv`, `rdv-medecins.csv`, `rendez-vous-2026.xlsx`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-M2

**Expected:** a folder answer « 92 ont été analysés. » and no choice of workbook (before the fix: a row count of a workbook).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-M3

**Expected:** no « Plusieurs tableurs sont sélectionnés » at all: the question is answered from the documents (an answer and « Sources » listing documents). The wording depends on the model; judge only that the choice of a workbook is **not** shown (before the fix: « Quel classeur ? »).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-K1

**Expected:** one row, **Centre hospitalier universitaire vaudois**, tag **Confirmée**, « 5 fichiers »; under « Noms reconnus » `CHUV` and the full name; under « D'où vient cette entité » « Repéré par : organisation connue du vocabulaire. Confiance : 0,80. »

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-K2

**Expected:** you find `Hôpitaux universitaires de Genève`, `Hospices Civils de Lyon`, `Réseau hospitalier neuchâtelois`, `CPAM du Rhône`, `Dupont Immobilier SA`, `Bureautique Léman SA`, `Papeterie du Lac SARL`; you do **not** find a name that starts with `Entre`, nor `Fils SARL` alone; for `hôtel` among the persons: « Aucune entité ne correspond. »

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-K3

**Expected:** a pair `Roy Lucas` / `Lucas Roy` with the reason « prénom et nom dans l'ordre inverse », among a number of other pairs of the same reason (a person written surname first in one table and given name first in another: the base does not guess, it asks).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-K4

**Expected:** two rows, `Lucas Roy` and `Roy Lucas` (the second knows the first's name as its reversed order); after the deletion the note « Cette entité a été supprimée : elle n'apparaît plus et l'analyse ne la recrée pas. » and the button « Restaurer »; the strip says that **92 documents seront relus à la prochaine analyse**. Click « Restaurer ».

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### S-K5

**Expected:** « D'où vient cette entité » lists, besides the documents, a line « Tableur » for `patients-fr.csv` (colonne « nom_patient ») and one for `patients-france.csv` (colonne « Patient »). **Known, defect KBD-48, deferred:** before the deletion the name also had a line for `patients-ch.xlsx` (colonne « Nom »); it does not come back. Write it if you see it. The tag « Saisie à la main » is normal: a restored name is yours from then on. Close the window.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 


## Part G - 200 documents

### G0

**Expected:** the summaries read « Documents utilisés : tous » and « Données utilisées : aucune ».

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### G1

**Expected:** `203 fichiers, 179 analysés, 0 illisibles`; `18 fichiers lus par reconnaissance de texte.`; `21 fichiers n'ont pas été lus : leur format n'est pas pris en charge (7 .odt, 5 .rtf, 4 .eml, 2 .vtt, 1 .odp, 1 .ppt, 1 .pptx).`; `156 personnes et organisations repérées, dont 128 déjà connues.`; `139 identifiants repérés ...`; `92 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.` (the flag of S-K4 and S-K5); the button plain (before the Analyse-button fix it read « Analyser (24) »).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### G1-names

**Expected:** `identifier active 350`, `location active 51`, `organization active 50`, `organization candidate 5`, `person active 220`, `person candidate 17`, `entities: 693`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### G2

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### G2-11

**Expected:** Sandrine Colas, `page 66` of the guide, in both modes; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### G2-12

**Expected:** Nadia Bensaïd in both modes; no line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### G2-13

**Expected:** in « Automatique » `Recherche ciblée : 6 documents sur 179 — Fatima Benali` (or the widened line); the six files include `2026-01-29_resultats-analyses-benali-fatima.pdf`, `2026-08-01_courrier-rhumatologie.pdf`, `2026-08-17_decompte-groupe-mutuel.pdf`.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### G2-15

**Expected:** in « Automatique » `Recherche ciblée : 7 documents sur 179 — Anne-Sophie Vuagniaux` (7 kept, the 7 that hold the answer: the title « Dre » is read; before the batch 1 fix 2 were kept for 7) or the widened line.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### G2-16

**Expected:** **no line**.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### G2-6

**Expected:** **no line** (a subject, not a name).

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### G3

**Expected:** for each mode a `median total` and a `median search`; **the gain of `Automatique` over « Désactivé » is the difference of the medians**; the base's own time is `knowledge ... / ...`. Write the lines.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 


## Part E - the figures of the release gate

### E1

**Expected:** the group « Connaissances » reads « Ciblage intelligent » = « Suggestions seulement ».

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### E2

**Expected:** the list begins `List 'daily' for a library of 200 documents: type the sentences exactly as written.`

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### E3

**Expected:** `Questions recorded` above 100.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 


## Part Z - optional: 500 and 1 000 documents

### Z1

**Expected:** `running: ...gen-knowledge-sandbox.py --documents 1000 ...` then a block that ends with `"documents": 1000,`, `"readable": 878`, `"unreadable": 122` (or `already there: ...`).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### Z2

**Expected:** the answers of G2 still hold; write the times.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### Z3

**Expected:** a line « N fichiers n'ont pas été lus : leur format n'est pas pris en charge (...) » listing the formats.

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### Z4

**Expected:** the page is cited (« page N ») and the answer is right; a Word file has **no page** (the application keeps it as one page; known).

- Seen: 
- Right? / Sources? / Missed? / Added?: 
- Time under the answer: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 


## Part R - the one restore

### R1

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### R2

**Expected:** `restored index.sqlite3`, `restored settings.json`, then `restored knowledge-identifier.key` (or `removed knowledge-identifier.key (there was none when the backup was made)`), then the note that `kb-lot11` and `kb-lot11-data` are still on disk.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### R3

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### R4

**Expected:** your own folders and your own mode are back, as before Part 0.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 


## Part M - on the Mac mini (skippable)

### M1

**Expected:** `test result: ok.` and `0 failed`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### M2

**Expected:** `Settings folder : /Users/<you>/Library/Application Support/com.assistantcabinetai.desktop`, `Index folder    : /Users/<you>/Library/Application Support/com.assistantcabinetai.desktop`, `Application     : closed`, `Core library    : present`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### M3

**Expected:** the same lines as steps 0.2, 0.3 and 0.4, with `/` in the paths.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### M4

**Expected:** the same figures as A1 and A2 (`30 fichiers, 28 analysés, 0 illisibles`, `8 fichiers, 7 analysés, 1 illisible`); the text recognition is the Mac's own engine, so `3 fichiers lus par reconnaissance de texte.` may differ by one: write it. The two buttons are plain.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### M5

**Expected:** the lines of A6-1 and A6-3 (`Recherche ciblée : 1 document sur 28 — Sophie Rochat`, `... — Marc Bersier`).

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 

### M6

**Expected:** `entities: 577` for `names` (the figure of C0), then `restored index.sqlite3` and `restored settings.json`.

- Seen: 
- Result: OK / not OK / skipped
- Note: 
- Register row: 


## Summary by part

| Part | Steps | OK | Not OK | Skipped | Findings (register rows) |
| --- | --- | --- | --- | --- | --- |
| Part 0 - Preparation | 8 | | | | |
| Part A - 30 documents | 37 | | | | |
| Part B - safety | 6 | | | | |
| Part C - a library that lives | 10 | | | | |
| Part D - 100 documents | 5 | | | | |
| Part S - Switzerland, the tables and the file names | 39 | | | | |
| Part G - 200 documents | 11 | | | | |
| Part E - the figures of the release gate | 3 | | | | |
| Part Z - optional: 500 and 1 000 documents | 4 | | | | |
| Part R - the one restore | 4 | | | | |
| Part M - on the Mac mini (skippable) | 6 | | | | |
| **Total** | 133 | | | | |

## Findings

| # | What happened | Step | Product or protocol? | Register row |
| --- | --- | --- | --- | --- |
| | | | | |

A deviation is a finding about the **protocol** first (`.cursor/rules/kb-programme.mdc`, item 9.6): say which of the two was wrong, and the agent fixes the protocol and gives the short re-run.
