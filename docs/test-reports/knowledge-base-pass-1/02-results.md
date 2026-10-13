# 02 - The results book of the final human test (lot 11, readjusted)

**State: filled on 13 October 2026** from the owner's own run, whose raw record is kept beside the protocol as `human-tests/_KB-final-test.txt` (the source of every « Seen » line below; nothing in this file was invented). Written for the owner and for the developer or agent who reads her results. English; the questions and the lines of the interface are quoted in French as they were typed and printed.

**How it was filled.** The owner played `human-tests/lot-11-validation.md` as it stood in commit `b442d9d` (version 1) and pasted her screens and outputs into one text file. The agent read it line by line and filled one block per step id. « Expected » is the protocol's text (version 2, corrected after the run, see `06-final-human-test-report.md` section 7); where version 1 said something else, the block's note says so. **Result** values: `OK`; `OK (...)` with the reason of the difference; `not OK (protocol)` when the step could not be done as written; `not reported` / `not played` when the log holds nothing for the step; `n/a (title)`.

**Reading key.** *machine* = the real text recognition and the real AI machine of the owner against the agent's replay (perfect recognition, fake embedding): counts of names run a few higher, widened searches are more frequent. *Widened* = the line « Recherche élargie à toute votre sélection : peu de résultats dans les documents ciblés. », a correct reading by design (defect KBD-17, known).

Machine and run: date 13 October 2026 · workstation: the owner's Windows 11 PC · AI machine: the tower (the log does not write its address; the E2 analysis names it) · chat model `ministral-3:3b` (printed under every answer) · embedding model: not written in the log · build: `fix/kb-wrap-up` (the log does not give the commit; the merge of the context budget, `a661455`, came after step E2) · library: the invented sandbox of `fixtures/knowledge-sandbox` (30, 100, 200 documents).

How to read a question block: **Right?** the answer says what the library says (Y / N / partly), **Sources?** the files cited are the ones that hold the answer, **Missed?** a file that holds the answer was not used, **Added?** a file that does not hold the answer was used (the « Sources » list always shows six files: the filler ones are the display of the search), and the time under the answer.


## Part 0 - Preparation

### 0.1

**Expected:** `Application     : closed` and `Core library    : present`.

- Seen: Application closed; knowledgeMode suggest; Core library present; Documents folder `C:\Users\elise\AssistantCabinetAI\kb-lot10`; Data folder `...\kb-lot11-data`; backup folder already existed.
- Result: OK
- Note: The test folders are made beside the previous Documents folder, outside `Documents\CURSOR`.
- Register row: 

### 0.2

**Expected:** `saved   index.sqlite3` (or `absent  index.sqlite3 (nothing to save)`), `saved   settings.json`, `saved   knowledge-identifier.key` (or `absent  knowledge-identifier.key (nothing to save)`), then `Backup in C:\Users\elise\kb-backup-lot-11`.

- Seen: `saved index.sqlite3`, `saved settings.json`, `saved knowledge-identifier.key`, `Backup in C:\Users\elise\kb-backup-lot-11`.
- Result: OK
- Note: A backup folder from the earlier batches existed and was overwritten by this one.
- Register row: 

### 0.3

**Expected:** `wrote 30 invented documents in ...\kb-lot11`, `30 documents: 28 the application reads, 2 it does not read (formats: odt, vtt)`, `scans (OCR): 3; PDF pages: 16`, `wrote 8 invented workbooks in ...\kb-lot11-data (formats: csv, ods, xls, xlsx)`, your two folders, then `"knowledgeMode" is "suggest", "writeTimingLog" is true and both health modules (France and Switzerland) are on.`

- Seen: 30 documents: 28 read, 2 not read (odt, vtt); scans 3; PDF pages 16; 8 workbooks (csv, ods, xls, xlsx); folders `C:\Users\elise\AssistantCabinetAI\kb-lot11` and `...-data`; knowledgeMode suggest, timing log on, both health modules.
- Result: OK
- Note: 
- Register row: 

### 0.4

**Expected:** `deleted retrieval-timings.jsonl` or `no retrieval-timings.jsonl to delete`, then `emptied kb_diagnostics (N row(s))` or `kb_diagnostics does not exist yet`.

- Seen: `deleted retrieval-timings.jsonl`, `emptied kb_diagnostics (5 row(s))`.
- Result: OK
- Note: 
- Register row: 

### 0.5

**Expected:** at the top, the AI status reads « IA Disponible » (if it reads « IA Indisponible », start the AI machine and press « Vérifier l'IA »).

- Seen: The window opened (`cargo run` finished the dev build in 19.54 s). The AI status line was not written down.
- Result: OK
- Note: « IA Disponible » not reported; every answer afterwards was generated, so the AI machine answered.
- Register row: 

### 0.6

**Expected:** group « Connaissances »: « Ciblage intelligent » = « Suggestions seulement »; « Module santé »: both « Santé : France » and « Santé : Suisse » ticked; group « Avancé »: « Noter les durées dans un journal » ticked.

- Seen: Not reported.
- Result: not reported
- Note: From the answers: the chat model is `ministral-3:3b` (« Généré par ministral-3:3b »); the address of the AI machine and the embedding model are not in the log (the E2 analysis names the tower and its published window, 12 288 tokens).
- Register row: 

### 0.7

**Expected:** after (c) the message « La base de connaissances a été réinitialisée. »; the cards read `30 fichiers, 0 analysés, 0 illisibles` (documents) and `8 fichiers, 0 analysés, 1 illisible` (data).

- Seen: Not reported as such.
- Result: not reported
- Note: Proven by 0.8: the base was empty (`entities: 0`, fingerprint `e3b0c44298fc1c14`). The cards' lines after the Resets were not written.
- Register row: 

### 0.8

**Expected:** `entities: 0   fingerprint of the list: e3b0c44298fc1c14` and `names of the test questions found in the base: none`.

- Seen: `entities: 0   fingerprint of the list: e3b0c44298fc1c14`; `names of the test questions found in the base: none`.
- Result: OK
- Note: 
- Register row: 


## Part A - 30 documents

### A1

**Expected:** the line under the title reads `30 fichiers, 28 analysés, 0 illisibles`; under the card: `3 fichiers lus par reconnaissance de texte.`, `2 fichiers n'ont pas été lus : leur format n'est pas pris en charge (1 .odt, 1 .vtt).`, `42 personnes et organisations repérées, dont 0 déjà connue.`, `30 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).` **The button « Analyser » is plain, without a number** (before the Analyse-button fix it read « Analyser (2) » for ever, because of the `.odt` and the `.vtt`: if you see a number, it is a bug, defect KBD-47).

- Seen: 12 s. `3 fichiers lus par reconnaissance de texte.` `2 fichiers n'ont pas été lus : leur format n'est pas pris en charge (1 .odt, 1 .vtt).` `47 personnes et organisations repérées, dont 0 déjà connue.` `30 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).` The status line and the button were not written down at this step.
- Result: OK (figure differs because of the machine: real text recognition)
- Note: 47 against the replay's 42: the real text recognition reads some scans into wrong names (all « à confirmer »). `0 déjà connue` is the singular the interface uses for 0 and 1 (the protocol said « connues »). The button state was written later: plain after C3-a, S0 (both cards): the KBD-47 fix holds.
- Register row: KBD-47 (fixed)

### A2

**Expected:** `8 fichiers, 7 analysés, 1 illisible` (`rendez-vous-2026.ods` is the one that cannot be read: « pas un tableur (CSV ou XLSX) »); under the card: `7 tableurs lus sur cet ordinateur, sans l'IA : les tableurs s'analysent en un instant.`, `210 personnes et organisations repérées, dont 21 déjà connues.`, `278 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).`, `22 colonnes de tableur reconnues (personnes, organisations, identifiants...) dans 7 tableurs lus.`, `102 liens lus entre deux colonnes d'un tableur.`, `28 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.` The button « Analyser » of this card is plain, without a number.

- Seen: 1 s. `7 tableurs lus sur cet ordinateur, sans l'IA : les tableurs s'analysent en un instant.` `210 personnes et organisations repérées, dont 21 déjà connues.` `278 identifiants repérés (...)` `22 colonnes de tableur reconnues (...) dans 7 tableurs lus.` `102 liens lus entre deux colonnes d'un tableur.` `28 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.`
- Result: OK
- Note: Every figure equals the replay's, the status line and the button were not written down.
- Register row: 

### A3

**Expected:** the summaries read « Documents utilisés : tous » and « Données utilisées : aucune » (with documents and several workbooks ticked together, a question about a table asks « Plusieurs tableurs sont sélectionnés », which Part S shows on purpose).

- Seen: Not reported as a step.
- Result: not reported
- Note: Done in effect: every « Recherche ciblée » line of A6 says « sur 28 », the 28 analysed documents.
- Register row: 

### A4

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: Title of the ten sentences in « Suggestions seulement ».
- Result: n/a (title)
- Note: 
- Register row: 

### A4-1

**Expected:** right; source `...decompte-css-rochat-sophie.pdf`; the refund `CHF 486.63`; no targeting line.

- Seen: Refund `CHF 486.63` (and the AVS number); the product printed two warnings: « 200.00 » and « 286.63 » in no excerpt. Sources (6): courrier-patient.txt, courrier-patient-meunier-veronique.jpg (OCR), courrier-orl.docx, courrier-cpam74-dupont-jean.pdf, decompte-css-rochat-sophie.pdf, resultats-analyses-favre-laurent.pdf. No targeting line.
- Right? / Sources? / Missed? / Added?: Y (the model also invented a franchise and a net figure; the check caught both) / Y / N / Y (5 filler files)
- Time under the answer: 35 s
- Result: OK
- Note: The number check works on real inventions.
- Register row: 

### A4-2

**Expected:** right; source `...2026-06-20_resultats-analyses.pdf`; `6,3 %`; no line.

- Seen: `Hémoglobine glyquée : 6,3 %` found in 2026-06-20_resultats-analyses.pdf; the answer also quotes the extract of another patient (Jean Dupont, bisoprolol, 13 mai 2026). Sources (6) include resultats-analyses.pdf. No line.
- Right? / Sources? / Missed? / Added?: partly (the model mixed in Jean Dupont) / Y / N / Y (5 filler)
- Time under the answer: 41 s
- Result: OK
- Note: 
- Register row: 

### A4-3

**Expected:** right; source `...courrier-cardiologie-monnier-veronique.pdf`; `26 mai 2026`; no line.

- Seen: Lists `2026-02-25_courrier-cardiologie-monnier-veronique.pdf`. Sources (6): the scan first (« lu par reconnaissance de texte »), then 5 other files. No line.
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y (5 filler)
- Time under the answer: 31 s
- Result: OK
- Note: The fact `26 mai 2026` is not in the answer: the question asks for the list of courriers.
- Register row: 

### A4-4

**Expected:** right; source `...decompte-css-rochat-sophie.pdf`; `CHF 486.63`; no line.

- Seen: Lists decompte-css-rochat-sophie.pdf and decompte-helsana-martin-marie.pdf « (non de la CSS) ». Sources (6) include the CSS decompte. No line.
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y (5 filler)
- Time under the answer: 31 s
- Result: OK
- Note: 
- Register row: 

### A4-5

**Expected:** both invoices cited (`...facture-medsupply-fac-2026-014.pdf`, `...facture-papeterie-lac-fac-2026-014.pdf`); `CHF 352.73` and `93,12 €`; no line.

- Seen: Both invoices: `CHF 352,73` (MedSupply) and `93,12 €` (Papeterie du Lac, typed « FAC-2026-14 » by the model). Sources: both invoices + 4 others. No line.
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y (4 filler)
- Time under the answer: 44 s
- Result: OK
- Note: 
- Register row: 

### A4-6

**Expected:** the Swiss insurers' letters cited (`...decompte-helsana-martin-marie.pdf`, `...decompte-css-rochat-sophie.pdf`); no line.

- Seen: decompte-helsana-martin-marie.pdf and decompte-css-rochat-sophie.pdf listed and cited, 4 other files in the sources. No line.
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y (4 filler)
- Time under the answer: 31 s
- Result: OK
- Note: 
- Register row: 

### A4-7

**Expected:** the answer says that the documents do not say; no line.

- Seen: « Je n'ai pas trouvé d'exemples ou de documents mentionnant Madame Zénobie Quarante dans les extraits fournis. » Sources: 6 files with no link to her. No line.
- Right? / Sources? / Missed? / Added?: Y (says the documents do not say) / n/a / n/a / Y (6 filler)
- Time under the answer: 29 s
- Result: OK
- Note: The six sources under a « not found » answer are the display of the search, not claims.
- Register row: 

### A4-8

**Expected:** the row « Plusieurs personnes correspondent à « J. Dupont » : » offering `Jean Dupont (3 fichiers)` and `Jean-Pierre Dupont (1 fichier)`; no line.

- Seen: Answer about Jean Dupont (bisoprolol, 13 mai 2026). Row: « Plusieurs personnes correspondent à « J. Dupont » : Jean Dupont (3 fichiers) Jean-Pierre Dupont (1 fichier) ». No targeting line.
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y (filler)
- Time under the answer: 35 s
- Result: OK (the protocol's wording was wrong; corrected in version 2)
- Note: The row says « fichiers », not « documents » (protocol wording, corrected).
- Register row: 

### A4-9

**Expected:** the row « Plusieurs personnes correspondent à « Dr Martin » : » offering `Pierre Martin (3 fichiers)` and `Paul Martin (3 fichiers)`; no line.

- Seen: Row: « Plusieurs personnes correspondent à « Dr Martin » : Pierre Martin (3 fichiers) Paul Martin (3 fichiers) ». No targeting line. The model's answer lists Marie Martin, Lucas Roy, Jean-Pierre Dupont (patients, not doctors).
- Right? / Sources? / Missed? / Added?: partly (model) / Y / N / Y (filler)
- Time under the answer: 35 s
- Result: OK
- Note: The row is the product's; the answer text is the model's.
- Register row: 

### A4-10

**Expected:** right; source `...courrier-cardiologie-monnier-veronique.pdf`; `26 mai 2026`; no line.

- Seen: `Objet : Consultation pour un bilan endocrinologique`, `N° AVS : 756.9204 3092 25`, `Prochaine consultation : 26 mai 2026`; the scan is cited « (lu par reconnaissance de texte) ». No line.
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y (5 filler)
- Time under the answer: 35 s
- Result: OK
- Note: 
- Register row: 

### A5

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: Title of the ten sentences in « Désactivé ».
- Result: n/a (title)
- Note: 
- Register row: 

### A5-1

**Expected:** the same answer and sources as A4-1; no line.

- Seen: Answer built on the CSS decompte (`CHF 486,63`; the franchise figure differs from A4-1's: « CHF 2 000 » against « 200.00 », the document was not opened); no number warning this time; the six sources are the same, in the same order, as A4-1. No line, no row.
- Right? / Sources? / Missed? / Added?: partly (model) / Y / N / Y (5 filler)
- Time under the answer: 38 s
- Result: OK
- Note: The six sources are exactly those of A4: « Suggestions seulement » narrows nothing, as designed.
- Register row: 

### A5-2

**Expected:** the same as A4-2; no line.

- Seen: `6,3 %`, `103 µmol/L`...; the extract of Jean Dupont and of the CPAM letter are quoted too; the six sources are the same as A4-2.
- Right? / Sources? / Missed? / Added?: partly (model) / Y / N / Y
- Time under the answer: 41 s
- Result: OK
- Note: The six sources are exactly those of A4: « Suggestions seulement » narrows nothing, as designed.
- Register row: 

### A5-3

**Expected:** the same as A4-3; no line. **Write the list of « Sources »: Part B compares with it.**

- Seen: Lists `2026-02-25_courrier-cardiologie-monnier-veronique.pdf`. **Sources (6), written for B3:** the Monnier scan (OCR), courrier-neurologie-fabre-laurence.txt, courrier-orl-roy-lucas.md, courrier-pharmacie.pdf (2026-08-26), courrier-dermatologie.docx (2026-07-20), note-interne-stock.md. Identical to A4-3.
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y (5 filler)
- Time under the answer: 29 s
- Result: OK
- Note: 
- Register row: 

### A5-4

**Expected:** the same as A4-4; no line.

- Seen: Same answer and same six sources as A4-4.
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y
- Time under the answer: 31 s
- Result: OK
- Note: The six sources are exactly those of A4: « Suggestions seulement » narrows nothing, as designed.
- Register row: 

### A5-5

**Expected:** the same as A4-5; no line.

- Seen: Both invoices; same six sources as A4-5.
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y
- Time under the answer: 36 s
- Result: OK
- Note: 
- Register row: 

### A5-6

**Expected:** the same as A4-6; no line.

- Seen: Both Swiss decompte files; same six sources as A4-6.
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y
- Time under the answer: 31 s
- Result: OK
- Note: 
- Register row: 

### A5-7

**Expected:** the documents do not say; no line.

- Seen: « Je n'ai pas trouvé d'exemple de document lié à Madame Zénobie Quarante dans les extraits fournis. »
- Right? / Sources? / Missed? / Added?: Y / n/a / n/a / Y
- Time under the answer: 28 s
- Result: OK
- Note: 
- Register row: 

### A5-8

**Expected:** an answer drawn from the documents; **no** row « Plusieurs personnes... » (the base is off).

- Seen: Answer about Jean Dupont (bisoprolol, 13 mai 2026); **no** row « Plusieurs personnes... », no line.
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y
- Time under the answer: 46 s
- Result: OK
- Note: 
- Register row: 

### A5-9

**Expected:** an answer drawn from the documents; no row.

- Seen: Answer lists Marie Martin, Lucas Roy, Jean-Pierre Dupont; **no** row, no line.
- Right? / Sources? / Missed? / Added?: partly (model) / Y / N / Y
- Time under the answer: 42 s
- Result: OK
- Note: 
- Register row: 

### A5-10

**Expected:** the same as A4-10; no line.

- Seen: `26 mai 2026`, `N° AVS : 756.9204 3092 25`; the scan is cited; no line.
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y
- Time under the answer: 34 s
- Result: OK
- Note: 
- Register row: 

### A6

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: Title of the ten sentences in « Automatique ».
- Result: n/a (title)
- Note: 
- Register row: 

### A6-1

**Expected:** `Recherche ciblée : 1 document sur 28 — Sophie Rochat` (or the widened line).

- Seen: « Recherche élargie à toute votre sélection : peu de résultats dans les documents ciblés. » Sources: the same six as A5-1.
- Right? / Sources? / Missed? / Added?: Y (as A5-1) / Y / N / Y
- Time under the answer: 37 s
- Result: OK (widened search: a correct reading, defect KBD-17 known)
- Note: One document kept, below the widening score: the base widened by design.
- Register row: 

### A6-2

**Expected:** `Recherche ciblée : 1 document sur 28 — Jean Dupontet` (or the widened line).

- Seen: « Recherche élargie à toute votre sélection : ... » Answer: 6,3 %, bisoprolol, and a social-security number of another Dupont; sources as A5-2.
- Right? / Sources? / Missed? / Added?: partly (model) / Y / N / Y
- Time under the answer: 38 s
- Result: OK (widened search: a correct reading, defect KBD-17 known)
- Note: As A6-1.
- Register row: 

### A6-3

**Expected:** `Recherche ciblée : 1 document sur 28 — Marc Bersier`.

- Seen: `Recherche ciblée : 1 document sur 28 — Marc Bersier`. Source: the Monnier scan only.
- Right? / Sources? / Missed? / Added?: Y / Y / N / N
- Time under the answer: 25 s
- Result: OK
- Note: 
- Register row: 

### A6-4

**Expected:** `Recherche ciblée : 1 document sur 28 — CSS Assurance`.

- Seen: `Recherche ciblée : 1 document sur 28 — CSS Assurance`. Source: decompte-css-rochat-sophie.pdf only.
- Right? / Sources? / Missed? / Added?: Y / Y / N / N
- Time under the answer: 24 s
- Result: OK
- Note: 
- Register row: 

### A6-5

**Expected:** `Recherche ciblée : 2 documents sur 28` (a number is not shown as a name: nothing after « 28 »).

- Seen: `Recherche ciblée : 2 documents sur 28` (nothing after « 28 »). Sources: the two invoices only. The answer lists products and « Montant HT » of each.
- Right? / Sources? / Missed? / Added?: Y / Y / N / N
- Time under the answer: 37 s
- Result: OK (the protocol's wording was wrong; corrected in version 2)
- Note: A number is not shown as a name; corrected in version 2.
- Register row: 

### A6-6

**Expected:** **no line** (`LAMal` is a subject, not a name: nothing is narrowed).

- Seen: No targeting line. Sources: the six of A5-6.
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y (4 filler)
- Time under the answer: 29 s
- Result: OK
- Note: 
- Register row: 

### A6-7

**Expected:** **no line**; the documents do not say.

- Seen: No line. « Je n'ai pas trouvé d'exactement de référence ou de document nommé Madame Zénobie Quarante dans les extraits fournis. »
- Right? / Sources? / Missed? / Added?: Y / n/a / n/a / Y
- Time under the answer: 27 s
- Result: OK
- Note: 
- Register row: 

### A6-8

**Expected:** **no line**; the row « Plusieurs personnes correspondent à « J. Dupont » : » with `Jean Dupont (3 fichiers)` and `Jean-Pierre Dupont (1 fichier)`.

- Seen: No targeting line; row « Plusieurs personnes correspondent à « J. Dupont » : Jean Dupont (3 fichiers) Jean-Pierre Dupont (1 fichier) ».
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y
- Time under the answer: 34 s
- Result: OK (the protocol's wording was wrong; corrected in version 2)
- Note: « fichiers » in the row.
- Register row: 

### A6-9

**Expected:** **no line**; the row « Plusieurs personnes correspondent à « Dr Martin » : » with `Pierre Martin (3 fichiers)` and `Paul Martin (3 fichiers)`.

- Seen: No targeting line; row « ... « Dr Martin » : Pierre Martin (3 fichiers) Paul Martin (3 fichiers) ».
- Right? / Sources? / Missed? / Added?: partly (model) / Y / N / Y
- Time under the answer: 45 s
- Result: OK (the protocol's wording was wrong; corrected in version 2)
- Note: « fichiers » in the row.
- Register row: 

### A6-10

**Expected:** `Recherche ciblée : 1 document sur 28 — Véronique Monnier` (or the widened line). This letter is a **scan**: on the real text recognition its name is read right in some runs and wrong in others; if there is no line at all, the scan was misread, which is a result to write down.

- Seen: « Recherche élargie à toute votre sélection : ... ». `26 mai 2026` and the AVS number found; the scan is cited.
- Right? / Sources? / Missed? / Added?: Y / Y / N / Y
- Time under the answer: 33 s
- Result: OK (widened search: a correct reading, defect KBD-17 known)
- Note: The scan's name was read well enough to find it, then the base widened.
- Register row: 

### A7

**Expected:** `Questions recorded            : 20` (the ten of « Suggestions seulement » and the ten of « Automatique »; « Désactivé » writes nothing in this table) and a block « By mode » with 10 questions for each of the three modes (`off`, `suggest`, `auto`): the knowledge lookup in milliseconds, the search, the first word and the whole answer. **Write the median** `total` **of the three modes in the results book**: it is the answer to « is `Automatique` faster », and it is the model's time, not the base's.

- Seen: Questions recorded 20; with a name the base knows 12; the base would reduce 12 (60.0 %); reduced for real 6; widened again 3; median reduction 96 %. Release gate raw 24.1 % (14 of 58), adjusted 100.0 %. Why not reduced: ambiguous 4, no_entity 4. By mode, median / worst total: off 34 976 / 46 242 ms, suggest 34 966 / 43 560, auto 33 527 / 44 544; search 2 / 3 / 2 ms; knowledge lookup 0 / 5 / 4 ms; first word ~25 s in each.
- Result: OK
- Note: `Automatique` is 1.4 s (4 %) faster than « Désactivé » at 28 documents, which is noise next to a first word at ~25 s: the time is the model's. The base costs 4 to 5 ms.
- Register row: 


## Part B - safety

### B1

**Expected:** the row « Plusieurs personnes correspondent à « J. Dupont » : » offers `Jean Dupont (3 fichiers)` and `Jean-Pierre Dupont (1 fichier)`; **no** « Recherche ciblée » line before you press (nothing was narrowed). After the press, the same question is asked again with the same words (there is **no chip**: the chips belong to the picker above the typing box, not to this row) and **now** the line `Recherche ciblée à votre demande : 3 documents sur 28 — Jean Dupont` appears (checked on the owner's run of 13 October 2026).

- Seen: Row « Plusieurs personnes correspondent à « J. Dupont » : Jean Dupont (3 fichiers) Jean-Pierre Dupont (1 fichier) », no line. After pressing « Jean Dupont »: the question is asked again, identical; line `Recherche ciblée à votre demande : 3 documents sur 28 — Jean Dupont`; the owner saw **no chip**.
- Result: OK (the protocol was wrong about the chip)
- Note: The owner reported the deviation correctly. The row's button re-asks the question through `useChat.retarget` and says the choice to the knowledge base; chips belong to the picker above the typing box (`Composer`, `EntityChips`). Product right, protocol wrong; corrected in version 2.
- Register row: 

### B2

**Expected:** the row offers `Pierre Martin (3 fichiers)` and `Paul Martin (3 fichiers)` (the title « Dr » keeps the patients called Martin out of the choice); no line.

- Seen: Row « ... « Dr Martin » : Pierre Martin (3 fichiers) Paul Martin (3 fichiers) »; no line.
- Result: OK (the protocol's wording was wrong; corrected in version 2)
- Note: Two doctors, no patient named Martin in the choice.
- Register row: 

### B3

**Expected:** the line reads `Recherche ciblée : 1 document sur 28 — Marc Bersier` with the buttons « Voir les fichiers » and « Élargir à ma sélection »; after the press the question is answered on all the documents and **the list is the one you wrote for A5-3** (sentence 3 in « Désactivé »), file for file. If it is not, it is a bug.

- Seen: `Recherche ciblée : 1 document sur 28 — Marc Bersier`, buttons « Élargir à ma sélection » and « Voir les fichiers ». After the press: the same question answered on all documents; **sources identical to A5-3**, file for file, in the same order.
- Result: OK
- Note: 
- Register row: 

### B4

**Expected:** the summary reads « Documents utilisés : 27 fichiers »; **that file is not in « Sources »**, in « Automatique » (no targeting line: no ticked document names her) as in « Désactivé »; the named-file question is answered « 2026-03-20_decompte-css-rochat-sophie.pdf se trouve dans votre dossier de documents mais n'est pas sélectionné pour cette conversation. Cochez-le dans la liste des documents pour l'utiliser. »

- Seen: File unticked: « Résume le dossier de Sophie Rochat. » answered « Aucune information trouvée. » in « Automatique » and in « Désactivé »; the unticked file is in no source list. `Que dit 2026-03-20_decompte-css-rochat-sophie.pdf ?` → « ... se trouve dans votre dossier de documents mais n'est pas sélectionné pour cette conversation. Cochez-le dans la liste des documents pour l'utiliser. » « Réponse établie depuis votre dossier de documents, sans l'IA. »
- Result: OK
- Note: The summary « Documents utilisés : 27 fichiers » and the re-tick of « Tous les documents » were not written down.
- Register row: 

### B5

**Expected:** both outputs are identical (`identifier active 295`, `location active 51`, `organization active 29`, `organization candidate 5`, `person active 196`, `person candidate 1`, `entities: 577`, the same fingerprint) and the last line says `names of the test questions found in the base: none`; the window finds nothing for the three names.

- Seen: `names` before and after the four questions: identifier 295, location 51, organization active 29 / candidate 6, person active 196 / candidate 5, **entities: 582**, fingerprint `521f5df9eda6faee` both times; last line « none ». The window found nothing for Zénobie, Vaudroz, Blondeau.
- Result: OK (figure differs because of the machine: real text recognition)
- Note: 582 against the replay's 577: five more candidates from the real text recognition. The invariant (identical before and after, no name created by a question) holds.
- Register row: 

### B6

**Expected:** for `Et son numéro AVS ?` the line names Sophie Rochat (the follow-up is meant to); for `Que contiennent les notes internes ?` the line **still names Sophie Rochat** (the previous person narrowed it) and the search then widens by itself (« Recherche élargie à toute votre sélection : ... »); the answer may describe her letter instead of the notes; « Élargir à ma sélection » does not repair it; asked alone, the answer is about the internal notes. Write what you saw.

- Seen: « Que dit le courrier de Sophie Rochat ? » → « Recherche élargie à toute votre sélection... ». « Et son numéro AVS ? » → widened, answers 756.9617.2765.34. « Que contiennent les notes internes ? » → widened, answer about note-interne-stock.md; **no « Élargir à ma sélection » button**. Alone after « Effacer la conversation »: answer lists note-interne-stock.md and note-interne-photocopieuse.txt.
- Result: OK
- Note: Consistent with the described known defect KBD-19 (a short question is read as a follow-up); the button is absent because the search had already widened.
- Register row: KBD-19


## Part C - a library that lives

### C0

**Expected:** `identifier active 295`, `location active 51`, `organization active 29`, `organization candidate 5`, `person active 196`, `person candidate 1`, then `entities: 577` (write the fingerprint: it is compared at C3).

- Seen: `names` run at the start of Part C: the same output as B5 (entities 582, fingerprint 521f5df9eda6faee).
- Result: OK (figure differs because of the machine: real text recognition)
- Note: Same machine effect as B5.
- Register row: 

### C1

**Expected:** the card reads `31 fichiers, 29 analysés, 0 illisibles` and the lines `2 fichiers n'ont pas été lus : leur format n'est pas pris en charge (1 .odt, 1 .vtt).`, `46 personnes et organisations repérées, dont 45 déjà connues.`, `30 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).`, `28 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.`; `names`: **one more person** (`person active 197`, `entities: 578`); the line `Recherche ciblée : 1 document sur 29 — Sidonie Quenneville`, and the answer is about the new letter.

- Seen: `wrote courriers-entrants\courrier-quenneville-nouveau.txt`. Card: `31 fichiers, 29 analysés , 0 illisible .` `2 fichiers n'ont pas été lus ... (1 .odt, 1 .vtt).` `50 personnes et organisations repérées, dont 49 déjà connues.` `30 identifiants repérés ...` `28 documents déjà analysés relus ...`. Question: `Recherche ciblée : 1 document sur 29 — Sidonie Quenneville`; the answer is about the new letter.
- Result: OK (figure differs because of the machine: real text recognition)
- Note: 50 / 49 against the replay's 46 / 45 (wrong names from the scans). `names` after C1 was not written down.
- Register row: 

### C2

**Expected:** the card reads `32 fichiers, 30 analysés, 0 illisibles`; `1 fichier renommé pour retirer les espaces et les accents.` and `Courrier Quenneville été 2026.txt → Courrier-Quenneville-ete-2026.txt`; in the folder the file has the clean name; the line reads `Recherche ciblée : 2 documents sur 30 — Sidonie Quenneville` (two files now).

- Seen: `wrote Courrier Quenneville été 2026.txt`. Card: `32 fichiers, 30 analysés , 0 illisible .` `1 fichier renommé pour retirer les espaces et les accents.` `Courrier Quenneville été 2026.txt → Courrier-Quenneville-ete-2026.txt` `2 fichiers n'ont pas été lus ...` `1 personne ou organisation repérée, dont 1 déjà connue.` Question: `Recherche ciblée : 2 documents sur 30 — Sidonie Quenneville`; sources: Courrier-Quenneville-ete-2026.txt and courrier-quenneville-nouveau.txt.
- Result: OK
- Note: The « Voir » check of the clean name was not written; the sources show it.
- Register row: 

### C3

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: The owner wrote: « Attention je ne suis pas certain d'avoir compris les consignes en C3 ... Il faut être très clair et précis dans toutes les consignes des tests. » She read C3 as an introduction and did C3-a, C3-b, C3-c only.
- Result: not OK (protocol)
- Note: The step carried an action (`names` before) buried in a title sentence. Version 2 makes C3 a declared title and gives the two readings their own ids, C3-0 and C3-z.
- Register row: 

### C3-0

**Expected:** `person active` one more than at C0 (the new person of C1), `entities` one more than at C0 (578 on the agent's replay), the other lines as at C0; write the fingerprint.

- Seen: Not played (the `names` before the Resets).
- Result: not played (protocol fault)
- Note: Last `names` available: before C1 (entities 582). To be run now: see section 8 of the report.
- Register row: 

### C3-a

**Expected:** the card reads `32 fichiers, 0 analysés, 0 illisibles` and the button reads « Analyser (30) » (lit; the `.odt` and the `.vtt` are not counted). Press « Analyser ». Expected after: `32 fichiers, 30 analysés, 0 illisibles`, `3 fichiers lus par reconnaissance de texte.`, `2 fichiers n'ont pas été lus ...`, `45 personnes et organisations repérées, dont 24 déjà connues.`, `30 identifiants repérés ...`; the button plain again.

- Seen: Button « Analyzer (30) » after the Reset, `32 fichiers, 0 analysé , 0 illisible .`. After « Analyser » (about 13 s): `32 fichiers, 30 analysés , 0 illisible .` `3 fichiers lus par reconnaissance de texte.` `2 fichiers n'ont pas été lus ...` `49 personnes et organisations repérées, dont 24 déjà connues.` `30 identifiants repérés ...`; the button plain again.
- Result: OK (figure differs because of the machine: real text recognition)
- Note: 49 against 45; 24 known equals the replay's. KBD-47: lit with 30 before, plain after.
- Register row: KBD-47 (fixed)

### C3-b

**Expected:** `8 fichiers, 0 analysés, 1 illisible` and the button « Analyser (7) ». Press « Analyser ». Expected after: `8 fichiers, 7 analysés, 1 illisible`; the button plain.

- Seen: Button « Analyzer (7) » after the Reset, `8 fichiers, 0 analysé , 1 illisible .`. After « Analyser » (1 s): `8 fichiers, 7 analysés , 1 illisible .` `7 tableurs lus ...` `210 personnes et organisations repérées, dont 24 déjà connues.` `278 identifiants ...` `22 colonnes ... dans 7 tableurs lus.` `102 liens ...` `30 documents déjà analysés relus ...`; the button plain again.
- Result: OK
- Note: `dont 24 déjà connues` (21 at A2): probably the documents re-analysed in C3-a taught 3 more names to the data pass (not confirmed); the protocol had not stated the data card's names line at this step.
- Register row: 

### C3-c

**Expected:** both cards keep `32 fichiers, 30 analysés, 0 illisibles` and `8 fichiers, 7 analysés, 1 illisible`; documents card lines: `2 fichiers n'ont pas été lus ...`, `43 personnes et organisations repérées, dont 0 déjà connue.`, `30 identifiants repérés ...`, `30 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.`; data card lines: `7 tableurs lus sur cet ordinateur, sans l'IA : ...`, `210 personnes et organisations repérées, dont 21 déjà connues.`, `278 identifiants repérés ...`, `22 colonnes de tableur reconnues (...) dans 7 tableurs lus.`, `102 liens lus entre deux colonnes d'un tableur.`, `30 documents déjà analysés relus ...`.

- Seen: After « Tout réinitialiser »: documents `32 fichiers, 30 analysés , 0 illisible .` `2 fichiers n'ont pas été lus ...` `48 personnes et organisations repérées, dont 0 déjà connue.` `30 identifiants ...` `30 documents déjà analysés relus ...` (1 s). Data: `8 fichiers, 7 analysés , 1 illisible .` `7 tableurs lus ...` `210 ... dont 21 déjà connues.` `278 identifiants ...` `22 colonnes ...` `102 liens ...` `30 documents déjà analysés relus ...` (1 s).
- Result: OK (figure differs because of the machine: real text recognition)
- Note: 48 against 43 on the documents card; every other figure equals the replay's. `names` after was not run (it belongs to C3-z).
- Register row: 

### C3-z

**Expected:** **the same count and the same fingerprint as C3-0**: the base is a function of the files, nothing was typed by hand, so nothing is lost. If the fingerprint differs, it is a bug: write both outputs.

- Seen: Not played (the `names` after the Resets).
- Result: not played (protocol fault)
- Note: The check « the base rebuilt from the same files holds the same list » was therefore not observed by the owner. The replay proves it (list identical, 578 names); her machine needs the short re-run of section 8.
- Register row: 

### C4

**Expected:** the card reads `32 fichiers, 30 analysés, 0 illisibles`, `4 personnes et organisations repérées, dont 3 déjà connues.`, `2 identifiants repérés ...`; `names`: `person active 196`, `person candidate 3`, `entities: 579`; the window: `Quéméner` is **one person**, tagged « À confirmer », found in that file only; `Laurence Fabre` is still there (the workbooks name her, tagged « À confirmer ») but **no longer in that letter**; the question gives **no « Recherche ciblée » line** (a name met once, with no title, is only a candidate, and a candidate never narrows a search).

- Seen: `changed courriers-entrants/2026-02/2026-02-24_courrier-neurologie-fabre-laurence.txt: Laurence Fabre -> Laurence Quéméner`. `Que dit Laurence Quéméner ?` → no « Recherche ciblée » line; the answer is drawn from the fabre letter. The card, `names` and the window « Entités » were not written down.
- Result: OK (partial: only the question was reported)
- Note: No line is the expected reading (a name met once with no title is a candidate and a candidate never narrows).
- Register row: 

### C5

**Expected:** the card reads `31 fichiers, 29 analysés, 0 illisibles` and `1 document retiré du dossier, oublié de l'analyse.`; `names`: `person active 195`, `person candidate 4`, `entities: 579`; for Sophie Rochat **no « Recherche ciblée » line** (no document names her any more) and the answer does not cite the deleted file; for the CSS question the line reads `Recherche ciblée : 1 document sur 29 — CSS Assurance` and the answer is the new letter of Sidonie Quenneville (`courrier-quenneville-nouveau.txt`), which writes « la CSS Assurance ».

- Seen: `deleted caisses-assurances/suisse/2026-03-20_decompte-css-rochat-sophie.pdf`. `Résume le dossier de Sophie Rochat.` → « Aucune information trouvée. », no line, the deleted file is not cited. `Quels courriers viennent de la CSS Assurance ?` → `Recherche ciblée : 1 document sur 29 — CSS Assurance`, source courrier-quenneville-nouveau.txt. The next card (C6, before its Analyse) still reads `1 document retiré du dossier, oublié de l'analyse.`
- Result: OK (partial: card and `names` not written)
- Note: The model answered « Aucun courrier... ne mentionne explicitement la CSS Assurance » although the letter says « la CSS Assurance »: the model's reading, not the base's.
- Register row: 

### C6

**Expected after the growth:** **only the new files are read**, the card reads `101 fichiers, 90 analysés, 0 illisibles`, `7 fichiers lus par reconnaissance de texte.`, `8 fichiers n'ont pas été lus : leur format n'est pas pris en charge (2 .odt, 1 .eml, 1 .odp, 1 .ppt, 1 .pptx, 1 .rtf, 1 .vtt).`, `115 personnes et organisations repérées, dont 87 déjà connues.`, `78 identifiants repérés ...`, `29 documents déjà analysés relus ...`, the button plain (before the Analyse-button fix it read « Analyser (11) »: the three tables and the eight unread files); it takes much less than analysing 100 from scratch; `names`: `identifier active 307`, `location active 51`, `organization active 38`, `organization candidate 6`, `person active 211`, `person candidate 6`, `entities: 619`. Expected after the shrink: the card reads `31 fichiers, 29 analysés, 0 illisibles` and `61 documents retirés du dossier, oubliés de l'analyse.`; `names`: **the same counts as before the growth** (`identifier active 295`, `location active 51`, `organization active 29`, `organization candidate 5`, `person active 195`, `person candidate 4`, `entities: 579`).

- Seen: Growth: `added 70 documents...`; Analyse 2 min 2 s (replayed: 2 min 5 s). Card: `101 fichiers, 90 analysés , 0 illisible .` `5 fichiers lus par reconnaissance de texte.` `8 fichiers n'ont pas été lus ... (2 .odt, 1 .eml, 1 .odp, 1 .ppt, 1 .pptx, 1 .rtf, 1 .vtt).` `124 personnes et organisations repérées, dont 108 déjà connues.` `77 identifiants repérés ...` `62 documents déjà analysés relus ...`. Shrink: `31 fichiers, 29 analysés , 0 illisible .` `61 documents retirés du dossier, oubliés de l'analyse.`
- Result: OK (figures differ; one unexplained)
- Note: Replay: 7 OCR, 115 / 87, 78 identifiers, 29 re-read. The 62 re-read is the one figure with no simple machine explanation: KBD-50, an observation. The button state and `names` after growth and shrink were not written.
- Register row: KBD-50


## Part D - 100 documents

### D1

**Expected:** the helper says `added 70 documents: the library goes from 30 to 100`; the card, once finished, reads `101 fichiers, 90 analysés, 0 illisibles` and the same lines as after the growth of C6 (`7 fichiers lus par reconnaissance de texte.`, the 8 unread files, `115 personnes et organisations repérées, dont 87 déjà connues.`, `78 identifiants repérés ...`, `29 documents déjà analysés relus ...`); `status` reads `Documents folder: ...kb-lot11 <- the test folder` and `holds 101 files (library tier 100)`; the button is plain.

- Seen: Analyse 2 min 5 s; « mêmes lignes qu'après la croissance de C6 ». `status` printed `Application : RUNNING`, `holds : 101 files (library tier 100)`, `knowledgeMode : auto`, `Documents folder: ...\kb-lot11  <- the test folder`.
- Result: OK (figure differs because of the machine: real text recognition)
- Note: Same remarks as C6. The `status` output matches the protocol.
- Register row: 

### D2

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: Title of the three questions.
- Result: n/a (title)
- Note: 
- Register row: 

### D2-11

**Expected:** in both modes the answer is Sandrine Colas and cites `documentation\guide-pratique-cabinet.pdf` at `page 66`; **no line** in « Automatique » (a question that names nobody).

- Seen: « Désactivé »: `Sandrine Colas`, `documentation/guide-pratique-cabinet.pdf (page 66)`, 28 s; the product added « Le chiffre « 66 » ne figure dans aucun des extraits utilisés... ». « Automatique »: same answer, same warning, no line, 28 s. Sources: guide pages 55, 66, 110; manuel-qualite.docx; courrier-pharmacie.pdf.
- Time under the answer: 28 s / 28 s
- Result: OK (known false alarm)
- Note: The warning is the false alarm on a cited page number: KBD-49.
- Register row: KBD-49

### D2-12

**Expected:** Nadia Bensaïd, from `documentation\manuel-qualite.docx` (a Word file has no pages: the source reads `page 1`); no line.

- Seen: `Les écarts de la chaîne du froid sont signalés à Nadia Bensaïd.` from `documentation/manuel-qualite.docx (page 1)`; « Désactivé » 56 s, « Automatique » 28 s; no line.
- Time under the answer: 56 s / 28 s
- Result: OK
- Note: 
- Register row: 

### D2-13

**Expected:** in « Automatique » `Recherche ciblée : 3 documents sur 90 — Fatima Benali` (or the widened line), the three files `laboratoire\2026-01\2026-01-29_resultats-analyses-benali-fatima.pdf`, `courriers-entrants\2026-08\2026-08-01_courrier-rhumatologie.pdf`, `caisses-assurances\suisse\2026-08-17_decompte-groupe-mutuel.pdf`; HbA1c `7,0 %`, next visit `30 octobre 2026`, `CHF 151.05`.

- Seen: « Désactivé »: HbA1c 7,0 %, next visit 30 octobre 2026, CHF 151,05; sources include the three files. « Automatique »: same facts; `Recherche ciblée : 3 documents sur 90 — Fatima Benali`; sources: courrier-rhumatologie.pdf, resultats-analyses-benali-fatima.pdf, decompte-groupe-mutuel.pdf.
- Time under the answer: 54 s / 44 s
- Result: OK
- Note: « sur 90 »: the 90 analysed documents.
- Register row: 


## Part S - Switzerland, the tables and the file names

### S0

**Expected:** the documents card reads `103 fichiers, 92 analysés, 0 illisibles` and, as its only line, `8 fichiers n'ont pas été lus : leur format n'est pas pris en charge (2 .odt, 1 .eml, 1 .odp, 1 .ppt, 1 .pptx, 1 .rtf, 1 .vtt).`; the data card reads `13 fichiers, 12 analysés, 1 illisible` and `12 tableurs lus sur cet ordinateur, sans l'IA : ...`, `25 personnes et organisations repérées, dont 22 déjà connues.`, `8 colonnes de tableur reconnues (personnes, organisations, identifiants...) dans 5 tableurs lus.`, `19 liens lus entre deux colonnes d'un tableur.`, `92 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.`; **both buttons are plain** (the 8 unread files, the three tables among the documents and the `.ods` are not asked of the pass).

- Seen: `extra-files` printed the seven `wrote` lines. Documents card: `103 fichiers, 92 analysés , 0 illisible .` and `8 fichiers n'ont pas été lus : leur format n'est pas pris en charge (2 .odt, 1 .eml, 1 .odp, 1 .ppt, 1 .pptx, 1 .rtf, 1 .vtt).` Data card: `13 fichiers, 12 analysés , 1 illisible .` `12 tableurs lus ...` `25 personnes et organisations repérées, dont 22 déjà connues.` `8 colonnes de tableur reconnues (...) dans 5 tableurs lus.` `19 liens ...` `92 documents déjà analysés relus ...`. « les deux boutons sont en clair ».
- Result: OK
- Note: Every figure equals the replay's. KBD-47 fix confirmed on a library with 8 unread files, 3 tables among the documents and an `.ods` in the data folder.
- Register row: KBD-47 (fixed)

### S0-names

**Expected:** `identifier active 307`, `location active 51`, `organization active 38`, `organization candidate 6`, `person active 215`, `person candidate 5`, `entities: 622`.

- Seen: identifier 307, location 51, organization active 38 / candidate 7, person active 214 / candidate 15, **entities: 632**, fingerprint f1e362dde4d9d188; last line `['Sidonie Quenneville']`.
- Result: OK (figure differs because of the machine: real text recognition)
- Note: Replay: 622 (organization candidate 6, person active 215, candidate 5). Extra candidates from the real text recognition. The last line lists the person added at C1, which is why it is not « none » here (it was at B5).
- Register row: 

### S1

**Expected:** a normal answer written by the AI (**not** « Aucun fichier nommé 756.3052.5778.43 ne se trouve dans votre dossier de documents. », and the line « Réponse établie depuis votre dossier de documents, sans l'IA. » is not there); the line `Recherche ciblée : 1 document sur 92` (the end of the line, after the dash, is the number or its label: write it); « Sources » lists `2026-03-26_decompte-helsana-martin-marie.pdf`.

- Seen: `Recherche ciblée : 1 document sur 92`; answer: « Le numéro AVS 756.3052.5778.43 appartient à Marie Martin », source decompte-helsana-martin-marie.pdf.
- Time under the answer: 24 s
- Result: OK
- Note: Not « Aucun fichier nommé ... ». The Swiss identifier is found.
- Register row: 

### S2

**Expected:** `Recherche ciblée : 4 documents sur 92 — Anne-Sophie Vuagniaux` (the name **without** `Dre`); « Voir les fichiers »: `2026-01-08_courrier-endocrinologie-girardin-thierry.pdf`, `2026-06-25_courrier-orl.docx`, `2026-10-01_courrier-dermatologie.pdf`, `2026-10-04_resultats-analyses-maillard-pascal.pdf`. Write the names. The last file is a scan: if its text is misread by the recognition on your machine the line says 3 documents (the owner's run of 13 October: 3, without `resultats-analyses-maillard-pascal.pdf`).

- Seen: `Recherche ciblée : 3 documents sur 92 — Anne-Sophie Vuagniaux`; files courrier-orl.docx, courrier-dermatologie.pdf, courrier-endocrinologie-girardin-thierry.pdf.
- Time under the answer: 31 s
- Result: OK (figure differs because of the machine: real text recognition)
- Note: The replay keeps 4: the fourth, `resultats-analyses-maillard-pascal.pdf`, is a scan whose text the real recognition misreads. The title « Dre » is read (no `Dre` in the name).
- Register row: 

### S3

**Expected:** **no** row « Plusieurs personnes correspondent à « Dre Vuagniaux » : » (a patient, Gilles Vuagniaux, shares the surname; the title tells them apart); the line `Recherche ciblée : 4 documents sur 92 — Anne-Sophie Vuagniaux`, or « Recherche élargie à toute votre sélection : peu de résultats dans les documents ciblés. » (the narrowed search found too little by meaning; note it, it is not a failure of this step).

- Seen: No « Plusieurs personnes correspondent à « Dre Vuagniaux » » row. « Recherche élargie à toute votre sélection : peu de résultats dans les documents ciblés. » The answer drifts to Girardin and Roy patients.
- Time under the answer: 50 s
- Result: OK (widened search: a correct reading, defect KBD-17 known)
- Note: The first sub-condition of the batch 1 fix (the title tells the surname's owners apart) holds; the widening is KBD-17.
- Register row: 

### S4

**Expected:** `Recherche ciblée : 2 documents sur 92 — Hôpitaux universitaires de Genève` (the acronym is found); files `2026-02-12_courrier-orl-dupont-jean.pdf` and `guide-pratique-cabinet.pdf`; **not** the list of the whole folder.

- Seen: `Recherche ciblée : 2 documents sur 92 — Hôpitaux universitaires de Genève`; sources: guide-pratique-cabinet.pdf (pages 17, 23, 26, 27, 84) and courrier-orl-dupont-jean.pdf. The model says that the guide was read by « HUG ». A false alarm « Le chiffre « 84 » ... » appeared.
- Time under the answer: 31 s
- Result: OK
- Note: Not the list of the whole folder. The « 84 » is KBD-49.
- Register row: KBD-49

### S5

**Expected:** **no** « Recherche ciblée » line (`Genève` is a commune written in tables: it no longer narrows the documents); up to six sources.

- Seen: No « Recherche ciblée » line. Sources (6): courrier-neurologie.docx, courrier-rhumatologie-zufferey-gael.pdf, courrier-orl-dupont-jean.pdf, courrier-orl-demirci-ibrahim.docx, courrier-dermatologie.docx, resultats-analyses-maillard-pascal.pdf (OCR).
- Time under the answer: 47 s
- Result: OK
- Note: `Genève` no longer narrows the documents.
- Register row: 

### S6

**Expected:** `Recherche ciblée : 5 documents sur 92 — Centre hospitalier universitaire vaudois` (the full name although you typed the acronym; or the widened line « Recherche élargie à toute votre sélection : ... », see the rule of A6: it happened on the owner's run); files `2026-03-09_courrier-orl-petrovic-elena.docx`, `2026-03-12_courrier-cardiologie.docx`, `2026-06-25_courrier-orl.docx`, `2026-08-01_courrier-rhumatologie.pdf`, `2026-10-01_courrier-dermatologie.pdf`.

- Seen: « Recherche élargie à toute votre sélection : peu de résultats dans les documents ciblés. » Sources (6): courrier-cardiologie.docx (03-12), courrier-orl.docx, courrier-dermatologie.pdf, courrier-rhumatologie.pdf, courrier-orl-petrovic-elena.docx, courrier-cpam69-de-la-fontaine-matthieu.pdf.
- Time under the answer: 36 s
- Result: OK (widened search: a correct reading, defect KBD-17 known)
- Note: All five files that hold the answer are among the six sources, plus one. The protocol of the first version did not allow the widened line here; version 2 does.
- Register row: 

### S7

**Expected:** the same line and the same two files as S4 (the acronym and the full name are one entity).

- Seen: Not reported (the sentence is absent from the log).
- Result: not reported
- Note: Skipped between S6 and S8.
- Register row: 

### S8

**Expected:** `Recherche ciblée : 1 document sur 92 — Giuseppe D'Amico`; the file `2026-03-26_courrier-cpam74.pdf` (he is written alone on a line, above his street, in that letter). It must not name another D'Amico. The widened line is also a correct reading (it happened on the owner's run).

- Seen: « Recherche élargie à toute votre sélection : ... ». Answer drawn from courrier-cpam74.pdf (« Giuseppe D'Amico », 37,22 €); that file is among the sources.
- Time under the answer: 32 s
- Result: OK (widened search: a correct reading, defect KBD-17 known)
- Note: 
- Register row: 

### S9-1

**Expected:** `Recherche ciblée : 2 documents sur 92 — Lucas Roy`; files `2026-08-17_courrier-orl-roy-lucas.md` and `2026-10-01_courrier-dermatologie.pdf`.

- Seen: `Recherche ciblée : 2 documents sur 92 — Lucas Roy`; sources courrier-orl-roy-lucas.md and courrier-dermatologie.pdf.
- Time under the answer: 28 s
- Result: OK
- Note: 
- Register row: 

### S9-2

**Expected:** `Recherche ciblée : 2 documents sur 92 — Roy Lucas`; files `2026-04-20_courrier-rhumatologie-lucas-roy.pdf` and `2026-10-01_courrier-dermatologie.pdf`: two different people, two different sets (the dermatology letter names both).

- Seen: `Recherche ciblée : 2 documents sur 92 — Roy Lucas`; sources courrier-rhumatologie-lucas-roy.pdf and courrier-dermatologie.pdf.
- Time under the answer: 27 s
- Result: OK
- Note: Two people, two sets, the dermatology letter in both.
- Register row: 

### S10

**Expected:** `Recherche ciblée : 1 document sur 92 — Aïcha N'Diaye`; the file `2026-03-23_resultats-analyses-n-diaye-aicha.pdf` (or the widened line, as on the owner's run; this file is a scan).

- Seen: « Recherche élargie à toute votre sélection : ... ». Answer: 23 mars 2026, Hémoglobine glyquée 5,8 %...; laboratoire/...resultats-analyses-n-diaye-aicha.pdf (OCR) among the sources.
- Time under the answer: 33 s
- Result: OK (widened search: a correct reading, defect KBD-17 known)
- Note: 
- Register row: 

### S-T1

**Expected:** `Somme de Montant : 8 941,54 CHF.` then `Calculé sur 10 lignes.`

- Seen: `Somme de Montant : 8 941,54 CHF.` `Calculé sur 10 lignes.` « Réponse calculée depuis votre dossier des données, sans l'IA. »
- Result: OK
- Note: Equals the replay's answer.
- Register row: 

### S-T2

**Expected:** `Plus grande valeur de Montant : 2 310,25 CHF.`

- Seen: `Plus grande valeur de Montant : 2 310,25 CHF.` `Calculé sur 10 lignes.`
- Result: OK
- Note: Equals the replay's answer.
- Register row: 

### S-T3

**Expected:** `Somme de Montant : 8 941,54 €.`

- Seen: `Somme de Montant : 8 941,54 €.` `Calculé sur 10 lignes.`
- Result: OK
- Note: Equals the replay's answer.
- Register row: 

### S-T4

**Expected:** `La feuille Factures contient 39 lignes.` and **no** line « Compris comme : ... ».

- Seen: `La feuille Factures contient 39 lignes.` no « Compris comme ».
- Result: OK
- Note: Equals the replay's answer.
- Register row: 

### S-T5

**Expected:** `La feuille rdv-export-titre contient 10 lignes.` (the file has a title line above its header).

- Seen: `La feuille rdv-export-titre contient 10 lignes.`
- Result: OK
- Note: Equals the replay's answer.
- Register row: 

### S-T6-1

**Expected:** `Nombre de lignes : 3.` and `Compris comme : medecin = Dr Jean Dupont.`

- Seen: `Nombre de lignes : 3.` `Compris comme : medecin = Dr Jean Dupont.`
- Result: OK
- Note: Equals the replay's answer.
- Register row: 

### S-T6-2

**Expected:** the same as S-T6-1.

- Seen: `Nombre de lignes : 3.` `Compris comme : medecin = Dr Jean Dupont.`
- Result: OK
- Note: Equals the replay's answer.
- Register row: 

### S-T7

**Expected:** a sum of 150 with `Compris comme : medecin = Dr Jean Dupont.`

- Seen: `Somme de montant : 150.` `Calculé sur 3 lignes.` `Compris comme : medecin = Dr Jean Dupont.`
- Result: OK
- Note: Equals the replay's answer.
- Register row: 

### S-T8-1

**Expected:** 2, `Compris comme : medecin = Dr Paul Martin.`

- Seen: `Nombre de lignes : 2.` `Compris comme : medecin = Dr Paul Martin.`
- Result: OK
- Note: Equals the replay's answer.
- Register row: 

### S-T8-2

**Expected:** **3** and `Compris comme :` starting with `medecin parmi` (the two doctors called Martin).

- Seen: `Nombre de lignes : 3.` `Compris comme : medecin parmi Dr Paul Martin, Dr Pierre Martin.`
- Result: OK
- Note: Equals the replay's answer.
- Register row: 

### S-T9

**Expected:** 2, with the month and the year in `Compris comme` (this file has dates above the 12th, so its date column is settled).

- Seen: `Nombre de lignes : 2.` `Compris comme : patient = Alice Archer et date en mars et date en 2026.`
- Result: OK
- Note: Equals the replay's answer.
- Register row: 

### S-T10

**Expected:** `Nombre de lignes : 2.`

- Seen: `Nombre de lignes : 2.` `Compris comme : patient = Alice Archer.`
- Result: OK
- Note: Equals the replay's answer.
- Register row: 

### S-T11-1

**Expected:** no count; the message `Ce filtre porte sur un jour, un mois, une année ou une période, mais cette colonne de rdv-dates-ambigues.csv n'est pas une colonne de dates reconnue.`, then `Colonne concernée : date.`, then `Ses colonnes sont : patient, date, medecin.` (every day of that file is 12 or below, so the file does not say which number is the day).

- Seen: `Ce filtre porte sur un jour, un mois, une année ou une période, mais cette colonne de rdv-dates-ambigues.csv n'est pas une colonne de dates reconnue.` `Colonne concernée : date.` `Ses colonnes sont : patient, date, medecin.` `Essayez de demander une somme, un minimum, ...`
- Result: OK
- Note: As expected.
- Register row: 

### S-T11-2

**Expected:** the same message as S-T11-1.

- Seen: `Ce filtre porte sur un jour, un mois, une année ou une période, mais cette colonne de rdv-dates-ambigues.csv n'est pas une colonne de dates reconnue.` `Colonne concernée : date.` `Ses colonnes sont : patient, date, medecin.` `Essayez de demander une somme, un minimum, ...` + `Votre question précédente n'a pas pu être calculée non plus : nommez exactement l'une des colonnes ci-dessus.`
- Result: OK (one more line than written)
- Note: The extra line is the product's own follow-up note: the previous question of the conversation had also failed (the conversation was not cleared between the three).
- Register row: 

### S-T11-3

**Expected:** the same message as S-T11-1.

- Seen: `Ce filtre porte sur un jour, un mois, une année ou une période, mais cette colonne de rdv-dates-ambigues.csv n'est pas une colonne de dates reconnue.` `Colonne concernée : date.` `Ses colonnes sont : patient, date, medecin.` `Essayez de demander une somme, un minimum, ...` + the same follow-up line.
- Result: OK (one more line than written)
- Note: As S-T11-2.
- Register row: 

### S-F1

**Expected:** the answer is built from one file; under « Sources » only `2026/mars/neurologie.txt` (the text speaks of March).

- Seen: Answer built on `2026/mars/neurologie.txt (page 1)`; the only source is `2026/mars/neurologie.txt`.
- Time under the answer: 22 s
- Result: OK
- Note: 
- Register row: 

### S-F2

**Expected:** « Plusieurs fichiers s'appellent neurologie.txt. Lequel voulez-vous ? » with the two files `2026/janvier/neurologie.txt` and `2026/mars/neurologie.txt`.

- Seen: « Plusieurs fichiers s'appellent neurologie.txt. Lequel voulez-vous ? » with `2026/janvier/neurologie.txt` and `2026/mars/neurologie.txt`, « Cliquez sur un fichier pour poser la question à son sujet. »
- Result: OK
- Note: 
- Register row: 

### S-F3

**Expected:** « Aucun fichier nommé avril/neurologie.txt ne se trouve dans votre dossier de documents. » (no `avril` folder: nothing is guessed).

- Seen: « Aucun fichier nommé avril/neurologie.txt ne se trouve dans votre dossier de documents. » « Réponse établie depuis votre dossier de documents, sans l'IA. »
- Result: OK
- Note: 
- Register row: 

### S-M1

**Expected:** « Plusieurs tableurs sont sélectionnés. Lequel voulez-vous ? Nommez-le dans la question : » followed by **twelve** workbooks: `contrats-2026.xls`, `contrats-2026.xlsx`, `factures-fournisseurs-2026.xlsx`, `patients-ch.xlsx`, `patients-fr.csv`, `patients-france.csv`, `patients-suisse.csv`, `planning-garde-2026.xlsx`, `rdv-dates-ambigues.csv`, `rdv-export-titre.csv`, `rdv-medecins.csv`, `rendez-vous-2026.xlsx`.

- Seen: « Plusieurs tableurs sont sélectionnés. Lequel voulez-vous ? Nommez-le dans la question : » then twelve workbooks: contrats-2026.xls, contrats-2026.xlsx, factures-fournisseurs-2026.xlsx, patients-ch.xlsx, patients-fr.csv, patients-france.csv, patients-suisse.csv, planning-garde-2026.xlsx, rdv-dates-ambigues.csv, rdv-export-titre.csv, rdv-medecins.csv, rendez-vous-2026.xlsx.
- Result: OK
- Note: Exactly the twelve of the replay.
- Register row: 

### S-M2

**Expected:** a folder answer « 92 fichiers ont été analysés. » and no choice of workbook (before the fix: a row count of a workbook).

- Seen: « 92 fichiers ont été analysés. » « Réponse établie depuis votre dossier de documents, sans l'IA. » No choice of workbook.
- Result: OK (the protocol's wording was wrong; corrected in version 2)
- Note: The sentence says « 92 fichiers » (the protocol said « 92 ont été analysés »).
- Register row: 

### S-M3

**Expected:** no « Plusieurs tableurs sont sélectionnés » at all: the question is answered from the documents (an answer and « Sources » listing documents). The wording depends on the model; judge only that the choice of a workbook is **not** shown (before the fix: « Quel classeur ? »).

- Seen: Answer from the documents (Jean Dupont, Jean-Pierre Dupont); « Cette question ne portait sur aucune de vos tables. » « Recherche élargie à toute votre sélection... » No « Plusieurs tableurs ». Sources: four documents.
- Time under the answer: 38 s
- Result: OK
- Note: 
- Register row: 

### S-K1

**Expected:** one row, **Centre hospitalier universitaire vaudois**, tag **Confirmée**, « 5 fichiers »; under « Noms reconnus » `CHUV` and the full name; under « D'où vient cette entité » « Repéré par : organisation connue du vocabulaire. Confiance : 0,80. »

- Seen: The owner wrote for S-K1 to S-K5: « tout est conforme ».
- Result: OK
- Note: No detail was written; the deviation KBD-48 (the `patients-ch.xlsx` line not coming back) was not described either way.
- Register row: 

### S-K2

**Expected:** you find `Hôpitaux universitaires de Genève`, `Hospices Civils de Lyon`, `Réseau hospitalier neuchâtelois`, `CPAM du Rhône`, `Dupont Immobilier SA`, `Bureautique Léman SA`, `Papeterie du Lac SARL`; you do **not** find a name that starts with `Entre`, nor `Fils SARL` alone; for `hôtel` among the persons: « Aucune entité ne correspond. »

- Seen: The owner wrote for S-K1 to S-K5: « tout est conforme ».
- Result: OK
- Note: No detail was written; the deviation KBD-48 (the `patients-ch.xlsx` line not coming back) was not described either way.
- Register row: 

### S-K3

**Expected:** a pair `Roy Lucas` / `Lucas Roy` with the reason « prénom et nom dans l'ordre inverse », among a number of other pairs of the same reason (a person written surname first in one table and given name first in another: the base does not guess, it asks).

- Seen: The owner wrote for S-K1 to S-K5: « tout est conforme ».
- Result: OK
- Note: No detail was written; the deviation KBD-48 (the `patients-ch.xlsx` line not coming back) was not described either way.
- Register row: 

### S-K4

**Expected:** two rows, `Lucas Roy` and `Roy Lucas` (the second knows the first's name as its reversed order); after the deletion the note « Cette entité a été supprimée : elle n'apparaît plus et l'analyse ne la recrée pas. » and the button « Restaurer »; the strip says that **92 documents seront relus à la prochaine analyse**. Click « Restaurer ».

- Seen: The owner wrote for S-K1 to S-K5: « tout est conforme ».
- Result: OK
- Note: No detail was written; the deviation KBD-48 (the `patients-ch.xlsx` line not coming back) was not described either way.
- Register row: 

### S-K5

**Expected:** « D'où vient cette entité » lists, besides the documents, a line « Tableur » for `patients-fr.csv` (colonne « nom_patient ») and one for `patients-france.csv` (colonne « Patient »). **Known, defect KBD-48, deferred:** before the deletion the name also had a line for `patients-ch.xlsx` (colonne « Nom »); it does not come back. Write it if you see it. The tag « Saisie à la main » is normal: a restored name is yours from then on. Close the window.

- Seen: The owner wrote for S-K1 to S-K5: « tout est conforme ».
- Result: OK
- Note: « tout est conforme » — no detail. Whether the `patients-ch.xlsx` line (KBD-48) was absent was not written: to be asked.
- Register row: 


## Part G - 200 documents

### G0

**Expected:** the summaries read « Documents utilisés : tous » and « Données utilisées : aucune ».

- Seen: Not reported as a step.
- Result: not reported
- Note: Done in effect: every line of G2 says « sur 179 » and the questions did not hit the mixed tier.
- Register row: 

### G1

**Expected:** `203 fichiers, 179 analysés, 0 illisibles`; `18 fichiers lus par reconnaissance de texte.`; `21 fichiers n'ont pas été lus : leur format n'est pas pris en charge (7 .odt, 5 .rtf, 4 .eml, 2 .vtt, 1 .odp, 1 .ppt, 1 .pptx).`; `156 personnes et organisations repérées, dont 128 déjà connues.`; `139 identifiants repérés ...`; `92 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.` (the flag of S-K4 and S-K5); the button plain (before the Analyse-button fix it read « Analyser (24) »).

- Seen: Analyse 43 s. `203 fichiers, 179 analysés , 0 illisible .` `18 fichiers lus par reconnaissance de texte.` `21 fichiers n'ont pas été lus ... (7 .odt, 5 .rtf, 4 .eml, 2 .vtt, 1 .odp, 1 .ppt, 1 .pptx).` `169 personnes et organisations repérées, dont 137 déjà connues.` `135 identifiants repérés ...` `92 documents déjà analysés relus ...`
- Result: OK (figure differs because of the machine: real text recognition)
- Note: Replay: 156 / 128 and 139 identifiers. The 92 re-read equals the replay's (the flag of S-K4 and S-K5). The button was not written down.
- Register row: 

### G1-names

**Expected:** `identifier active 350`, `location active 51`, `organization active 50`, `organization candidate 5`, `person active 220`, `person candidate 17`, `entities: 693`.

- Seen: identifier 348, location 51, organization active 50 / candidate 8, person active 222 / candidate 26, **entities: 705**, fingerprint 408031198a83050b.
- Result: OK (figure differs because of the machine: real text recognition)
- Note: Replay: 693 (identifier 350, organization candidate 5, person active 220, candidate 17).
- Register row: 

### G2

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: Title of the six questions.
- Result: n/a (title)
- Note: 
- Register row: 

### G2-11

**Expected:** Sandrine Colas, `page 66` of the guide, in both modes; no line.

- Seen: Both modes: `Sandrine Colas`, page 66; the false alarm on « 66 » in both; no line. 26 s / 25 s.
- Time under the answer: 26 s / 25 s
- Result: OK (known false alarm)
- Note: 
- Register row: KBD-49

### G2-12

**Expected:** Nadia Bensaïd in both modes; no line.

- Seen: `Nadia Bensaïd`, manuel-qualite.docx in both; no line. 29 s / 27 s.
- Time under the answer: 29 s / 27 s
- Result: OK
- Note: 
- Register row: 

### G2-13

**Expected:** in « Automatique » `Recherche ciblée : 6 documents sur 179 — Fatima Benali` (or the widened line); the six files include `2026-01-29_resultats-analyses-benali-fatima.pdf`, `2026-08-01_courrier-rhumatologie.pdf`, `2026-08-17_decompte-groupe-mutuel.pdf`.

- Seen: « Désactivé »: a summary that mixes the doctor and the Sanitas decompte. « Automatique »: `Recherche ciblée : 5 documents sur 179 — Fatima Benali`; sources: note-honoraires (OCR), courrier-rhumatologie.pdf, courrier-pharmacie.docx, resultats-analyses-benali-fatima.pdf, decompte-groupe-mutuel.pdf.
- Time under the answer: 44 s / 37 s
- Result: OK (figure differs because of the machine: real text recognition)
- Note: The replay keeps 6; the sixth (`decompte-sanitas.jpg`) is a scan the real recognition misreads. The scan note-honoraires (31 mai) is found.
- Register row: 

### G2-15

**Expected:** in « Automatique » `Recherche ciblée : 7 documents sur 179 — Anne-Sophie Vuagniaux` (7 kept, the 7 that hold the answer: the title « Dre » is read; before the batch 1 fix 2 were kept for 7) or the widened line.

- Seen: « Désactivé »: four courriers + two planning files. « Automatique »: « Recherche élargie à toute votre sélection : ... ».
- Time under the answer: 39 s / 39 s
- Result: OK (widened search: a correct reading, defect KBD-17 known)
- Note: The replay with a perfect recognition keeps 7; the real run widened. The `Dre` is read (batch 1 fix); KBD-17 is the cost.
- Register row: 

### G2-16

**Expected:** **no line**.

- Seen: No « Recherche ciblée » line in either mode; five Genève documents listed.
- Time under the answer: 49 s / 37 s
- Result: OK
- Note: 
- Register row: 

### G2-6

**Expected:** **no line** (a subject, not a name).

- Seen: No line in either mode; three Swiss decompte files listed.
- Time under the answer: 35 s / 34 s
- Result: OK
- Note: 
- Register row: 

### G3

**Expected:** for each mode a `median total` and a `median search`; **the gain of** `Automatique` **over « Désactivé » is the difference of the medians**; the base's own time is `knowledge ... / ...`. Write the lines.

- Seen: 78 questions recorded; with a name the base knows 34; the base would reduce 33 (42.3 %); reduced for real 27; widened again 12; median reduction 96 %. Release gate raw 45.7 % (63 of 138), adjusted 100.0 %. Why not reduced: ambiguous 6, named_file 17, no_discriminating_entity 1, no_entity 20, user_widened 1. By mode, median / worst total: off 35 606 / 56 275 ms (20 questions), suggest 34 966 / 43 560 (10), auto 30 461 / 49 721 (52); search 6 / 3 / 4 ms; knowledge lookup 0 / 5 / 5 ms.
- Result: OK
- Note: `Automatique` is 5.1 s (14 %) faster than « Désactivé » (median total), almost all the model's time; the gate figures are explained in `03-release-gate.md`.
- Register row: 


## Part E - the figures of the release gate

### E1

**Expected:** the group « Connaissances » reads « Ciblage intelligent » = « Suggestions seulement ».

- Seen: Not reported as a step; E2 is headed « Questions posées en mode Suggestion seulement ».
- Result: not reported
- Note: The mode was « Suggestions seulement » for the twenty-one questions, then changed for one comparison (see E2).
- Register row: 

### E2

**Expected:** the list begins `List 'daily' for a library of 200 documents: type the sentences exactly as written.`

- Seen: Nineteen sentences of the `daily` list were typed (Q001 to Q019), in « Suggestions seulement »; Q006 (cold chain of the vaccines) was asked three more times: « Automatique », « Désactivé », « Automatique » again. Observation: the number of sources cited was far below the number of files expected (6 for 33 on « Genève »). Cause found in the code by the owner (five constants); fix `fix/kb-context-budget` (03fd4ef), merged as a661455; the improvement is **not measured yet**.
- Result: OK (more than the step asked: a defect found and fixed)
- Note: Also seen: the false alarm on four page numbers under Q005 (KBD-49); Q003 (`horaires d'ouverture`, 0 files hold the answer): the model answered « Paul Martin : Lundi : 14 h à 18 h » from a doctor's file instead of saying the documents do not say; Q006 (1 file holds the answer, manuel-qualite.docx, cited): the model said « aucune mention explicite » in all four asks; Q018 (3 files hold the answer): « Non, aucun extrait ne mentionne de résultats d'analyses pour Jean-Pierre Dupont ». These are the model's readings of six excerpts (KBD-51), not the base's. Nineteen of the 25 sentences the step asked for were typed; the record reached 106 questions with the earlier ones.
- Register row: KBD-51

### E3

**Expected:** `Questions recorded` above 100.

- Seen: 106 questions recorded; with a name the base knows 39; the base would reduce 38 (35.8 %); reduced for real 27; widened again 12; median reduction 96 %. Release gate raw 48.2 % (81 of 168), adjusted 100.0 % (lost: 87 cited files that mention no name of the question, 0 left out by the cap, 0 files that name an entity the base did not use). Why not reduced: ambiguous 6, named_file 17, no_discriminating_entity 1, no_entity 43, user_widened 1. By mode (median / worst total): off 22 questions 35 606 / 242 091 ms, suggest 36 questions 34 966 / 117 842, auto 54 questions 30 461 / 49 721.
- Result: OK
- Note: More than 100 questions: the step's condition is met. The worst times (242 s, 118 s) are probably questions asked while the tower loaded a model or was being tested with larger windows (49 152 and 65 536 tokens, per the owner's analysis); not confirmed.
- Register row: 


## Part Z - optional: 500 and 1 000 documents

### Z1

**Expected:** `running: ...gen-knowledge-sandbox.py --documents 1000 ...` then a block that ends with `"documents": 1000,`, `"readable": 878`, `"unreadable": 122` (or `already there: ...`).

- Seen: Not played (optional).
- Result: not played
- Note: Part Z is optional and overnight.
- Register row: 

### Z2

**Expected:** the answers of G2 still hold; write the times.

- Seen: Not played (optional).
- Result: not played
- Note: Part Z is optional and overnight.
- Register row: 

### Z3

**Expected:** a line « N fichiers n'ont pas été lus : leur format n'est pas pris en charge (...) » listing the formats.

- Seen: Not played (optional).
- Result: not played
- Note: Part Z is optional and overnight.
- Register row: 

### Z4

**Expected:** the page is cited (« page N ») and the answer is right; a Word file has **no page** (the application keeps it as one page; known).

- Seen: Not played (optional).
- Result: not played
- Note: Part Z is optional and overnight.
- Register row: 


## Part R - the one restore

### R1

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: Not reported.
- Result: not reported
- Note: The log ends at E3. **Until `restore` is run, the application still points at the test folders and carries the test index.**
- Register row: 

### R2

**Expected:** `restored index.sqlite3`, `restored settings.json`, then `restored knowledge-identifier.key` (or `removed knowledge-identifier.key (there was none when the backup was made)`), then the note that `kb-lot11` and `kb-lot11-data` are still on disk.

- Seen: Not reported.
- Result: not reported
- Note: The log ends at E3. **Until `restore` is run, the application still points at the test folders and carries the test index.**
- Register row: 

### R3

**Expected:** the commands and clicks of this step, as written in the protocol; no figure to compare.

- Seen: Not reported.
- Result: not reported
- Note: The log ends at E3. **Until `restore` is run, the application still points at the test folders and carries the test index.**
- Register row: 

### R4

**Expected:** your own folders and your own mode are back, as before Part 0.

- Seen: Not reported.
- Result: not reported
- Note: The log ends at E3. **Until `restore` is run, the application still points at the test folders and carries the test index.**
- Register row: 


## Part M - on the Mac mini (skippable)

### M1

**Expected:** `test result: ok.` and `0 failed`.

- Seen: Not played.
- Result: not played
- Note: Part M waits for the Mac mini.
- Register row: 

### M2

**Expected:** `Settings folder : /Users/<you>/Library/Application Support/com.assistantcabinetai.desktop`, `Index folder    : /Users/<you>/Library/Application Support/com.assistantcabinetai.desktop`, `Application     : closed`, `Core library    : present`.

- Seen: Not played.
- Result: not played
- Note: Part M waits for the Mac mini.
- Register row: 

### M3

**Expected:** the same lines as steps 0.2, 0.3 and 0.4, with `/` in the paths.

- Seen: Not played.
- Result: not played
- Note: Part M waits for the Mac mini.
- Register row: 

### M4

**Expected:** the same figures as A1 and A2 (`30 fichiers, 28 analysés, 0 illisibles`, `8 fichiers, 7 analysés, 1 illisible`); the text recognition is the Mac's own engine, so `3 fichiers lus par reconnaissance de texte.` may differ by one: write it. The two buttons are plain.

- Seen: Not played.
- Result: not played
- Note: Part M waits for the Mac mini.
- Register row: 

### M5

**Expected:** the lines of A6-1 and A6-3 (`Recherche ciblée : 1 document sur 28 — Sophie Rochat`, `... — Marc Bersier`).

- Seen: Not played.
- Result: not played
- Note: Part M waits for the Mac mini.
- Register row: 

### M6

**Expected:** `entities: 577` for `names` (the figure of C0), then `restored index.sqlite3` and `restored settings.json`.

- Seen: Not played.
- Result: not played
- Note: Part M waits for the Mac mini.
- Register row: 


## Summary by part

| Part | Steps | OK | Not OK | Not reported / not played | Titles | Findings (register rows) |
| --- | --- | --- | --- | --- | --- | --- |
| Part 0 - Preparation | 8 | 6 | 0 | 2 | 0 | none |
| Part A - 30 documents | 37 | 33 | 0 | 1 | 3 | KBD-47 confirmed fixed; KBD-49 seen later |
| Part B - safety | 6 | 6 | 0 | 0 | 0 | KBD-19 as described; B1 protocol |
| Part C - a library that lives | 12 | 9 | 1 | 2 | 0 | C3 protocol; KBD-50; C3-0 and C3-z never played |
| Part D - 100 documents | 5 | 4 | 0 | 0 | 1 | KBD-49 |
| Part S - Switzerland, the tables and the file names | 39 | 38 | 0 | 1 | 0 | KBD-49; KBD-17 (known); S7 not reported |
| Part G - 200 documents | 11 | 9 | 0 | 1 | 1 | KBD-49; KBD-17 (known) |
| Part E - the figures of the release gate | 3 | 2 | 0 | 1 | 0 | KBD-51 found and fixed in code; KBD-49 |
| Part Z - optional: 500 and 1 000 documents | 4 | 0 | 0 | 4 | 0 | optional |
| Part R - the one restore | 4 | 0 | 0 | 4 | 0 | not reported: run `restore` |
| Part M - on the Mac mini (skippable) | 6 | 0 | 0 | 6 | 0 | waits for the Mac mini |
| **Total** | 135 | 107 | 1 | 22 | 5 | |

## Findings

| # | What happened | Step | Product or protocol? | Register row |
| --- | --- | --- | --- | --- |
| 1 | The row's name button re-asks the question; no chip appears | B1 | protocol (product right) | none; protocol corrected |
| 2 | C3 read as an introduction: the `names` before and after the Resets were not run | C3, C3-0, C3-z | protocol | none; protocol corrected; short re-run in the report, section 8 |
| 3 | The ambiguity row says « fichiers », an identifier is not shown as a name after « sur 28 », « 92 fichiers ont été analysés », « 0 déjà connue » (singular) | A4-8, A4-9, A6-5, A6-8, A6-9, B1, B2, S-M2, A1, C3-c | protocol | none; corrected |
| 4 | A correct answer that cites a page draws « Le chiffre « 66 » ne figure dans aucun des extraits... » | D2-11, S4, G2-11, E2 | product | KBD-49 (new, deferred) |
| 5 | The real text recognition adds wrong names: every count of names is a few higher than the replay's (582 against 577, 632 against 622, 705 against 693) and two scans are misread (S2 3 files for 4, G2-13 5 for 6) | A1, B5, C1, C3-a, C3-c, C6, S0-names, S2, G1, G1-names, G2-13 | environment | none (candidates, by design) |
| 6 | `62 documents déjà analysés relus` at C6 where the replay says 29 | C6 | product? (observation) | KBD-50 (new, deferred) |
| 7 | The widened line is frequent on the owner's embeddings: A6-1, A6-2, A6-10, S3, S6, S8, S10, G2-15, B6 | see steps | product, known | KBD-17 (unchanged) |
| 8 | Sources cited far below the files that hold the answer (6 for 33); cause found in the code and fixed on `fix/kb-context-budget`; effect not measured | E2 | product | KBD-51 (new, fixed in code) |
| 9 | Steps with no record: 0.6, 0.7, A3, C3-0, C3-z, S7, G0, E1, Z, R, M; partial: C4, C5, C6 (cards and `names` not written), S-K5 (the KBD-48 line) | see steps | protocol (steps too dense) / owner's choice | none |
| 10 | Part R not reported: the application may still point at the test folders and carry the test index | R2 | to do | none: run `restore` |

A deviation is a finding about the **protocol** first (`.cursor/rules/kb-programme.mdc`, item 9.6): the agent says which of the two was wrong, fixes the protocol and gives the short re-run. That analysis is in `06-final-human-test-report.md`.
