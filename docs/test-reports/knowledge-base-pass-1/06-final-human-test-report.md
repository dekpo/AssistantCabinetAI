# 06 - The final human test of the knowledge programme: study, record, consequences

13 October 2026 · branch `fix/kb-wrap-up` · protocol `human-tests/lot-11-validation.md` (version 1 played, commit `b442d9d`; version 2 corrected after the run) · raw record `human-tests/_KB-final-test.txt` (2 263 lines, the owner's own paste) · results book `02-results.md` (135 blocks, filled from that record)

Written for the owner, and for the agent or developer who follows her. English; the lines of the interface are quoted in French as printed. Every figure below is read from the owner's record or from the agent's replay (`knowledge_validation_walk`, `print_the_figures_of_the_final_protocol`); nothing is estimated unless the sentence says so.

---

## 1. What was played

The owner played Parts 0, A, B, C, D, S, G and E of the final protocol on her Windows PC, with the AI machine she uses (chat model `ministral-3:3b`, printed under every answer), on the invented sandbox library (30, then 100, then 200 documents plus the extra Swiss files). She pasted every command output, every answer and every card line into one file, answered the deviations in her own words, and, at step E2, studied a defect of the context that is sent to the model, fixed it on a separate branch and merged that branch into `fix/kb-wrap-up` (commit `03fd4ef`, merge `a661455`).

Not played, or not written down: Part Z (optional), Part R (the restore: the log ends at E3), Part M (the Mac mini is not there). Steps 0.6, 0.7, A3, C3-0, C3-z, S7, G0 and E1 have no record; C4, C5, C6 and S-K5 are partial. The list is in section 3.

## 2. Verdict

| Question the test asks | Answer from the record |
| --- | --- |
| Does the « Analyser » button stop asking for files the pass cannot read (KBD-47)? | **Yes.** C3-a: the button reads « Analyser (30) » after the Reset, then plain. C3-b: « Analyser (7) », then plain. S0, with 8 unread files, three tables among the documents and an `.ods` in the data folder: « les deux boutons sont en clair ». |
| Is « Suggestions seulement » really the old search? | **Yes, on the ten sentences.** The six sources under each answer are identical, file for file and in the same order, to those of « Désactivé » (A4 against A5, ten of ten). No line and no change but the « Plusieurs personnes correspondent » row. |
| Does a name that fits several people get asked about, never picked? | **Yes.** `J. Dupont` (Jean Dupont 3 fichiers, Jean-Pierre Dupont 1 fichier) and `Dr Martin` (Pierre Martin 3 fichiers, Paul Martin 3 fichiers: the patients called Martin are out of the choice). Pressing a name re-asks and says « Recherche ciblée à votre demande : 3 documents sur 28 — Jean Dupont ». |
| Does the widen button give the old answer back? | **Yes.** B3: the list of sources after « Élargir à ma sélection » equals the list of A5-3, file for file. |
| Is a file that is not ticked never read? | **Yes**, in both modes; naming it says « ... n'est pas sélectionné pour cette conversation ». |
| Does a question create a name? | **No.** B5: the list is identical before and after four questions about nobody (582 names, same fingerprint). |
| Does the base follow the files (new, renamed, changed, deleted, grown, shrunk)? | **For what was written down, yes** (C1, C2, C3-a/b/c, C5, C6). Not observed by the owner: the equality of the list before and after the three Resets (C3-0 and C3-z were not played) and the names after C4 to C6. The replay proves both. |
| Does the Swiss day work? | **Yes.** The AVS number is found (S1), `Dre` is read and the surname alone does not ask (S3), `HUG` and `CHUV` find their hospital (S4, S6), `Genève` no longer narrows (S5), `D'Amico`, `Lucas Roy` / `Roy Lucas`, `N'Diaye` are found (S8, S9, S10), every amount, date and workbook answer is exactly the replay's (S-T1 to S-T11-3), the file routes and the mixed tier behave (S-F1 to S-M3). |
| Is the application consistent with the replay? | **Figure for figure on everything that does not depend on the text recognition** (the data card, the Swiss tables, the file names, the lines of the Analyse that count files). The counts of names are a few higher on her machine, and two scans are misread (section 5). |
| Are there product defects? | **Three new rows**, none blocking: KBD-49 (false alarm on page numbers), KBD-50 (an observation), KBD-51 (the context sent to the model is capped at six passages; fixed in code, effect not measured). See section 4. |

Tally of the results book: 135 blocks; 107 OK (72 plain, 35 with a stated reason: machine, wording, widened search, known false alarm); 1 not OK (C3, a protocol fault); 22 not reported or not played; 5 titles.

## 3. What the record does not contain

| Step | What is missing | Why it matters | How to close it |
| --- | --- | --- | --- |
| 0.6, 0.7 | The settings check and the three Resets of the opening | Proven indirectly by 0.8 (`entities: 0`) | none |
| A3, G0, E1 | The selection and mode steps | Proven indirectly: every line says « sur 28 » / « sur 179 », and the E2 questions were asked in the right mode | none |
| C3-0, C3-z | The `names` before and after the three Resets | It is the one place where « the base is a function of the files » was to be seen on the owner's machine | section 8, RR-5 to RR-8 |
| C4 | The card, `names`, the window « Entités » (`Quéméner`, `Laurence Fabre`) | The change-of-file lifecycle was seen only through the question (no line, correct) | section 8, RR-1 |
| C5, C6 | The cards and `names` | Idem | the replay covers them; RR is optional |
| S7 | The sentence about the full name of the Genève hospitals | Its twin S4 passed | section 8, RR-3 |
| S-K5 | Whether the `patients-ch.xlsx` line (KBD-48) was absent | Decides whether KBD-48 is a lone-surname effect or an order effect | section 8, RR-2 |
| R1 to R4 | The restore | **Unless `restore` was run after E3, her application still points at the test folders and carries the test index** | section 8, RR-10 |

None of these is a failure of the product. Most are a consequence of how the protocol was written (section 7, "dense steps").

## 4. The product findings

### 4.1 KBD-49 - a correct page citation draws a false alarm (new, deferred)

Seen at D2-11 (« Le chiffre « 66 » ne figure dans aucun des extraits utilisés ni dans votre question »), S4 (« 84 »), G2-11 (« 66 » again), and four at once in E2 for « Comment éliminer les aiguilles usagées ? » (« 32 », « 48 », « 57 », « 69 »). In each case the model cited the page of an excerpt correctly. The code (`commands.rs`, document answer, about line 1910) builds the list of known figures from the excerpts' text, the cited files' paths and the question; the **page of an excerpt is not in it**. The check keeps two-digit numbers, so a page is a candidate figure.

The same record shows that the check **works**: under A4-1 it caught two figures the model invented (« 200.00 » and « 286.63 »). The defect is therefore not the check, but that a false alarm sits beside a real one and teaches her to read neither. The fix is one line (add the page numbers to `known`) with a test, but it is not part of the wrap-up: the owner ended the programme. Register: `defects-register.md`, KBD-49, `deferred`, with the proving test written.

### 4.2 KBD-50 - `62 documents déjà analysés relus` at C6 (new, observation)

The replay says 29 for the growth from 30 to 100; her machine says 62. Everything else on that line is a machine effect (5 scans read against 7, 124 names against 115, 108 known against 87, 77 identifiers against 78), but the count of re-read documents is not obviously one. The same line matches exactly at C3-c (30), S0 (92) and G1 (92). Working hypothesis, not confirmed: wrong names from misread scans become candidates, and a candidate confirmed by a second file makes more earlier documents due for a re-read. No wrong answer followed. Register: KBD-50, `deferred`.

### 4.3 KBD-51 - six passages, whatever the model (found by the owner at E2, fixed in code, not measured)

At E2 the number of sources was far below the number of files that hold the answer (« Quels documents viennent de Genève ? »: 6 cited for 33). The owner read the code and found five constants that fixed what is sent to the model (6 passages and 6 000 characters; 12 000 for a question on every document, about 12 files of 200; 4 000 for the documents' share of a mixed answer; 24 000 for the whole prompt) whatever the model and its window. She changed them to follow the window the server publishes (`fix/kb-context-budget`, `03fd4ef`; `docs/CONTEXT-BUDGET.md`) and set `ministral-3:3b=32768` on the tower after trying 65 536 (HTTP 500 after 17 s) and 49 152 (« extrêmement lent »).

What this means for the final test:

- **Every « Sources » list of Parts A to G was produced with the old cap**: six files at most. The « Missed? » column of the results book is bounded by six; a miss is not necessarily the base's.
- **Nothing of the targeting depends on it**: « Recherche ciblée : N documents sur M » counts the files the base kept, not the passages sent.
- **The improvement is not measured.** She states it herself: replay the 19 sentences of E2 at 32 768 and compare, source by source, with the record (section 8, RR-9).
- `docs/DECISIONS.md` still carried three lines that described the 24 000-character ceiling; they now say they are superseded by `docs/CONTEXT-BUDGET.md`.

### 4.4 KBD-17 - the widened search is frequent (known, measured here)

Counting every question of the record for which the base reduced or tried to reduce the search: A6 (6 attempts: 3 applied, 3 widened), D2-13 (applied), S (9 attempts: 5 applied, 4 widened: S3, S6, S8, S10), G2 (2 attempts: G2-13 applied, G2-15 widened). **Eighteen attempts, ten applied, eight widened.** In six of the eight widened cases (A6-1, A6-2, A6-10, S6, S8, S10) every file the sources could show was there (S6: all five that hold the answer, plus one); in the other two (S3, G2-15) the answer lies in seven files and a list of sources holds six at most (KBD-51), the scan `maillard` being also misread in S3. So widening costs the narrowing, not the answer. The release-gate report agrees: of 38 questions the base would reduce, 27 were reduced for real and 12 widened again (E3). The owner decided on 12 October that « Suggestions seulement » stays the setting, so this is information, not a task.

### 4.5 What is not the product

The model (`ministral-3:3b`) is the source of the wrong content in the record: it invents a franchise (A4-1), mixes patients (A4-2, A4-9), answers « Lundi : 14 h à 18 h » from a doctor's file to a question about the opening hours when no document says (E2, Q003), says « aucune mention explicite » four times for a cold-chain question whose file is among its sources (E2, Q006), and says « Aucun courrier ... ne mentionne explicitement la CSS Assurance » about a letter that writes it (C5). None of these is the knowledge base's doing, and none of them is hidden by it: in every case the sources show what the model had. The one defence the product has against invented figures worked (A4-1).

### 4.6 KBD-47, confirmed

The button is plain, without a number, after every Analyse of the record that had unread files: C3-a, C3-b, S0 (both cards). The vitest case is the proof in code (`analysis.test.ts`, seen failing first); the owner's screens are the proof in the window.

## 5. What her machine changes against the replay

The replay reads every scan perfectly and embeds with a fake model; the owner's machine reads them with the real engine and embeds with the real one. Two consequences are visible.

- **Names**: more names in total, a few of them candidates where the replay has actives (the extra ones are « à confirmer » and narrow nothing). `names` totals: 582 against 577 (A, B5, C0), 632 against 622 (S0), 705 against 693 (G1).
- **Scans**: two documents the replay finds are misread and drop out of a narrowing: `resultats-analyses-maillard-pascal.pdf` (S2: 3 files for 4) and `decompte-sanitas.jpg` (G2-13: 5 for 6).
- **Widening**: more frequent (section 4.4), because the real embedding finds fewer results in a narrow set than the replay's does.

Counts that do **not** move, and that therefore check the product exactly: the whole data card (A2, S0), the Swiss tables (S-T1 to S-T11), the file routes, the workbook list, the number of files and of analysed documents on every card (`30 fichiers, 28 analysés` ... `203 fichiers, 179 analysés`), the unread-format lines, the re-read counts of C3-c, S0 and G1.

## 6. The owner's reference figures (13 October 2026)

Use these, not the replay's, as the expected values of the machine-dependent lines on her PC. (The AI machine, the model and the text recognition are the same as for this run.)

| Step | Line | Replay | Owner |
| --- | --- | --- | --- |
| A1 | names found / already known | 42 / 0 | **47 / 0** (12 s, 3 scans) |
| B5, C0 | `names`: entities; org candidate; person candidate | 577; 5; 1 | **582; 6; 5**, fingerprint `521f5df9eda6faee` |
| C1 | names found / known | 46 / 45 | **50 / 49** |
| C3-a | names found / known | 45 / 24 | **49 / 24** (13 s) |
| C3-b | data card names / known | (not stated) | **210 / 24** |
| C3-c | documents names / known; data names / known | 43 / 0; 210 / 21 | **48 / 0; 210 / 21** |
| C6 | OCR files; names / known; identifiers; re-read | 7; 115 / 87; 78; 29 | **5; 124 / 108; 77; 62** (2 min 2 s, replayed 2 min 5 s) |
| D1 | as C6 | | 2 min 5 s |
| S0 | documents card; data card | exact | **exact** |
| S0-names | entities; org candidate; person active; person candidate | 622; 6; 215; 5 | **632; 7; 214; 15**, fingerprint `f1e362dde4d9d188` |
| S2 | files kept | 4 | **3** (scan misread) |
| G1 | OCR files; names / known; identifiers; re-read | 18; 156 / 128; 139; 92 | **18; 169 / 137; 135; 92** (43 s) |
| G1-names | entities; identifier; org candidate; person active / candidate | 693; 350; 5; 220 / 17 | **705; 348; 8; 222 / 26**, fingerprint `408031198a83050b` |
| G2-13 | files kept | 6 | **5** (scan misread) |

Times to remember: an Analyse of 30 documents with 3 scans takes 12 s; of the 61 documents added by the growth to 100 (including the 115-page guide and 7 scans), about 2 min 5 s, **repeated to the second**; of the 100 added by the growth to 200 (18 scans), 43 s: the guide is the cost. A question takes 20 to 56 s with this model, of which the first word is about 25 s; the base's lookup is 4 to 5 ms in the median (8 to 16 ms worst). The median total in « Automatique » is 1.4 s (4 %) below « Désactivé » at A7 and 5.1 s (14 %) below it at G3: the difference is the model's time on a shorter context, not the base's.

## 7. Faults of the protocol, and what was done about them

The rule: **a deviation she reports is a finding about the protocol first** (`.cursor/rules/kb-programme.mdc`, item 9.6). She reported two explicitly (B1 and C3) and the record shows the others.

| # | Fault | Evidence | Correction (version 2 of the protocol) |
| --- | --- | --- | --- |
| 1 | **C3 carried an action inside a title** (« Run `names` and write the output (before) ») | Her words: « je ne suis pas certain d'avoir compris les consignes en C3 ... Il faut être très clair et précis dans toutes les consignes » | C3 is now declared a title (« do nothing here »); the readings have their own ids, C3-0 and C3-z, each with its command in its own block and the comparison written |
| 2 | **Dense steps**: several actions in one paragraph (C4, C5, C6 each ask for a command, a click, a `names` and a question) | The `names` of C1, C4, C5, C6 and the window of C4 were not written down | Rule 10 added: one action per id, a title says so. The long steps of C4 to C6 are left as written because their replay is proven, but section 8 gives the missing readings as separate ids |
| 3 | **B1 promised a chip** | « en cliquant sur "Jean Dupont" la question reposée est identique à la première et il n'y a aucune pastille visible » | Wrong for this row: it re-asks through `useChat.retarget`; chips belong to the picker above the typing box. B1 now says no chip and quotes the line `Recherche ciblée à votre demande : ...` |
| 4 | Wording of the interface not checked against the screen: « (3 documents) » for « (3 fichiers) »; « 92 ont été analysés » for « 92 fichiers ont été analysés »; « dont 0 déjà connues » for « connue »; a name after « sur 28 » for an identifier | A4-8, A4-9, A6-5, A6-8, A6-9, B1, B2, S-M2, A1, C3-c | Corrected in place |
| 5 | **The widened line was not allowed** where the real embeddings produce it (S6, S8, S10) | S6, S8, S10 | Allowed, with the reason, as in A6 |
| 6 | **Counts of names stated as if machine-independent** | 582 for 577 and the rest of section 5 | Rule 9 added; section 6 holds her reference figures |
| 7 | The known false alarm on a cited page was not announced | D2-11, S4, G2-11, E2 | Rule 8 added (KBD-49) |
| 8 | A scan-dependent line stated as exact (S2: 4 files) | S2, G2-13 | S2 now says 3 or 4 and why |

The version played (1) is `b442d9d`; the corrected one (2) is the file in the working tree. The results book is generated from version 2's step ids, so its « Expected » lines are the corrected ones, and the block's note says when version 1 differed.

### Lessons recorded for every later human test

1. **One action per id.** A paragraph that holds a command, a click and a question is read as a whole and done in part. The owner followed the steps to the letter where they were single; she lost the ones that were embedded.
2. **A title says it is a title** and names the ids to do.
3. **Never state a count that depends on the owner's text recognition or embeddings as exact.** State the rule (equal before and after, one more after, fewer after) and give the replay's figure as a reference only.
4. **Quote the interface from the catalogue, not from memory** (« fichiers », « connue »).
5. **Say what the model may do** (widen, mix patients, cite a page) before it surprises her.
6. **Ask for the evidence the next reader needs**: the full output of `names` and of every card, not a verdict.

## 8. The short ordered re-run (about 25 minutes)

Not a new test: the readings the first run could not give, in the order that costs least. Every command is run from `C:\Users\elise\Documents\CURSOR\AssistantCabinetAI`. Answers go in `02-results.md` under the id in brackets; for ids that do not exist in it yet, add them under the block named.

**You start with:** the application open on `fix/kb-wrap-up` (with the context budget), mode « Suggestions seulement », « Dossier des documents » = `kb-lot11` (203 files, 179 analysed), « Dossier des données » = `kb-lot11-data` (13 files, 12 analysed), « Tous les documents » ticked, nothing in the data card, the conversation cleared. (If the state is different, tell the agent which step to start from.)

**RR-1** (C4, the window) « Réglages » → « Gérer… » → tab « Entités ». Search `Quéméner`; then `Laurence Fabre`. Close the window.
Expected: `Quéméner` is **one person**, tag « À confirmer », in `2026-02-24_courrier-neurologie-fabre-laurence.txt`; `Laurence Fabre` exists, tag « À confirmer », and **no longer in that letter**.

**RR-2** (S-K5, KBD-48) « Gérer… » → tab « Entités » → search `Lucas Roy` → click the row named exactly **Lucas Roy** → read « D'où vient cette entité ». Write every line « Tableur » with its file and column. Close the window.
Expected: lines for `patients-fr.csv` (colonne `nom_patient`) and `patients-france.csv` (colonne `Patient`). Write whether a line for `patients-ch.xlsx` (colonne `Nom`) is there (before the deletion of S-K4 it was).

**RR-3** (S7) « Réglages » → « Ciblage intelligent » → « Automatique ». Clear the conversation. Type `Quels documents concernent les Hôpitaux universitaires de Genève ?`.
Expected: `Recherche ciblée : N documents sur 179 — Hôpitaux universitaires de Genève` (N was 2 at 92 documents, with `courrier-orl-dupont-jean.pdf` and `guide-pratique-cabinet.pdf` among them; it can be larger at 179: write it), or the widened line. Then « Ciblage intelligent » → « Suggestions seulement ».

**RR-4** Read and keep the base **before** the Resets (this is C3-0):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py names
```

Write the whole output, including the fingerprint (on your last run: `entities: 705`, `408031198a83050b`; it can have moved since).

**RR-5** (C3-c) « Réglages » → « Gérer… » → tab « Réinitialiser » → « Tout réinitialiser… » → confirm « Tout réinitialiser ». Close the dialog. Press « Analyser » on « Dossier des documents », then on « Dossier des données ».
Expected: both cards keep `203 fichiers, 179 analysés` / `13 fichiers, 12 analysés`; the documents card says `179 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.` and `N personnes et organisations repérées, dont 0 déjà connue.`; both buttons plain.

**RR-6** (C3-z) Read the base again, same command as RR-4.

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py names
```

Expected: **the same count and the same fingerprint as RR-4**, with one exception worth writing: `Lucas Roy`, restored by hand at S-K4, may now be an automatic name. If the fingerprint differs, paste both outputs.

**RR-7** (KBD-48, second reading) Repeat RR-2.
Expected: write whether the `patients-ch.xlsx` line is back after the rebuild. If it is, the earlier absence was an effect of the restore's order; if not, a lone surname is not tied to a name once several people share it.

**RR-8** (E2 replay, the improvement of KBD-51) In « Suggestions seulement », ask again `Combien de documents ai-je ?` and the sentences Q002 to Q019 of `python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py questions daily` (type them exactly, clear the conversation before each). For each, write the number of lines under « Sources » and, on the questions that ask about every document (`Quels documents viennent de Genève ?`, `Quels documents mentionnent Annecy ?`), the coverage line « N documents sur M ». Compare with the first run (six lines or fewer in every answer of the record).
Expected: more source lines on the questions with many matching files (the first run never showed more than six: checked on the 113 answers of the record that carry a list of sources, out of 122 questions); write the times; no false figure, no file wrongly attributed.

**RR-9** (the restore, R2) Close the application, check the terminal that ran `pnpm tauri dev` has stopped, then:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py restore
```

Expected: `restored index.sqlite3`, `restored settings.json`, `restored knowledge-identifier.key` or `removed knowledge-identifier.key (there was none when the backup was made)`. Start the application: your own folders and your own mode are back.

**You leave with:** the application as it was before Part 0.

## 9. What the knowledge programme can conclude

1. **Nothing in the record contradicts an invariant.** A question changes no name; « Suggestions seulement » is the old search; a name that fits several people is asked about; a file not ticked is never read; the widen button restores the old answer; the Swiss tables are exact.
2. **The cost of `Automatique` is real and small to measure**: the base is 4 to 5 ms; the benefit is a shorter context (14 % faster at G3); the price is a widened search in 44 % of the narrowing attempts of the record, and a narrowing that depends on the text recognition of the scans. The owner's decision of 12 October (the setting stays « Suggestions seulement ») is consistent with this.
3. **The weak links are not the base's**: the model's wording (section 4.5), the six-passage cap (KBD-51), the false alarm on pages (KBD-49).
4. **The 98 % gate is abandoned**, so its figures (raw 48.2 %, adjusted 100.0 %) are indicators: the adjusted figure says that every file the base kept names an entity of the question; the raw figure is low because the answer's six « Sources » include filler files by design (`03-release-gate.md`).

## 10. Files, registers and git

Changed by this study: `02-results.md` (regenerated and filled), `human-tests/lot-11-validation.md` (version 2), `defects-register.md` (KBD-49, KBD-50, KBD-51), `docs/DECISIONS.md` (three superseded lines), `docs/SESSION-KB-STATUS.md` (local), this report; added: `human-tests/_KB-final-test.txt` (the raw record, invented data only).

Git (the owner's), after reading this report:

```text
git status
git add docs/test-reports/knowledge-base-pass-1/06-final-human-test-report.md docs/test-reports/knowledge-base-pass-1/02-results.md docs/test-reports/knowledge-base-pass-1/defects-register.md docs/test-reports/knowledge-base-pass-1/human-tests/lot-11-validation.md docs/test-reports/knowledge-base-pass-1/human-tests/_KB-final-test.txt docs/DECISIONS.md
git commit -m "docs: record and analyse the owner's final human test, correct the protocol"
git push
```
