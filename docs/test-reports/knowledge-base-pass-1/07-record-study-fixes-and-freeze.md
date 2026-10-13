# 07 - The owner's record studied answer by answer: verdict, fixes proposed, and how to freeze the state

13 October 2026 · branch `fix/kb-wrap-up` at `a661455` (the fix of `fix/kb-context-budget` included) · it completes `06-final-human-test-report.md`, it does not replace it · no line of code was changed to write it.

Written for the owner, and for the agent or developer who follows her. English; the lines of the interface are quoted in French as printed.

**Evidence.** The owner's record `human-tests/_KB-final-test.txt` (2 264 lines, read in full); the protocol `human-tests/lot-11-validation.md` (version 2, read in full); the ground truth of the invented library, `fixtures/knowledge-sandbox/questions.json` (205 questions, with the files that hold each answer); the application's own timing log `retrieval-timings.jsonl` (134 rows, numbers and codes only); a **read-only copy** of her live index (`index.sqlite3`) for one offline check (section 5); and the code at the lines quoted. A "measured" below is read from one of these; an "inferred" says so.

---

## 1. Status: the test is not finished, and nothing here concludes it

| Part | State | Note |
| --- | --- | --- |
| 0, A, B, C, D, S, G | played | the gaps are listed in `06` section 3 (C3-0, C3-z, C4 to C6 readings, S7, S-K5) |
| E2 | **stopped at 19 distinct sentences** (22 asks, three are repeats of one sentence in the three modes) out of the 183 of `questions daily` | asked in « Suggestions seulement » only, so the base narrowed nothing: E2 as played measures the **old search**, not the knowledge base |
| E2 replay at 32 768 tokens | **not done** | only synthetic requests were sent to the tower after the change (log rows 126 to 134, section 6) |
| E3 | read once, at 106 recorded questions | |
| Z, R, M | not played | **R must not be played now** (section 9) |

What remains of the list: 164 sentences of `daily` (Q020 onwards), plus the lists `ab`, `safety`, `follow-up`. The 205 sentences she generated are the base for everything that follows; nothing in this report asks her to restore, rebuild or re-analyse anything.

## 2. Verdict, part by part

Legend: ✓ as the protocol expects (or a stated machine effect) · ⚠ works but with a finding · ✗ wrong.

### Part A - the ten sentences in three modes (30 answers)

The sources under A4 (« Suggestions seulement ») equal those under A5 (« Désactivé ») **ten of ten, file for file and in the same order** ✓: the mode really leaves the old search alone. The A7 report agrees: 10 questions per mode, medians of total time 34.98 s (`off`), 34.97 s (`suggest`), 33.53 s (`auto`).

| # | Sentence | A4 suggest | A5 off | A6 auto (line) | Finding |
| --- | --- | --- | --- | --- | --- |
| 1 | `Résume le dossier de Sophie Rochat.` | ⚠ right file; **franchise « CHF 200.00 » and net « 286.63 » invented**, both caught by the figure check | ✓ « CHF 2 000 », 486,63 | ✓ widened line | **same six passages, one answer wrong, one right**: the model's sampling (P-4) |
| 2 | `Que dit le dossier de Jean Dupontet ?` | ⚠ lab result ✓ `6,3 %`, plus Jean Dupont's letter | ⚠ plus the CPAM refund of the other Dupont | ⚠ widened; **writes another person's insurance number** `1 58 03 74 443 192 09 « (lié au dossier de Jean Dupont) »` | five filler files in « Sources » feed the confusion (P-3) |
| 3 | `Quels courriers viennent du Dr Marc Bersier ?` | ✓ the scan | ✓ | ✓ `Recherche ciblée : 1 document sur 28 — Marc Bersier` | the 26 mai fact is not in the answer: the sentence asks for the letters, not the date |
| 4 | `Quels courriers viennent de la CSS Assurance ?` | ✓ and flags Helsana « (non de la CSS) » | ✓ | ✓ `1 document sur 28 — CSS Assurance` | |
| 5 | `Montre la facture FAC-2026-014` | ✓ both, `352,73` and `93,12 €` | ✓ both cited, no amount | ⚠ `2 documents sur 28`, amounts given are the HT ones (`326.30`, `77,60 €`) | right documents, partial figures |
| 6 | `Quels documents parlent de la LAMal ?` | ✓ | ✓ | ✓ no line (a subject) | |
| 7 | `Que dit le dossier de Madame Zénobie Quarante ?` | ✓ refuses | ✓ | ✓ no line; one garbled sentence (« d'exactement de référence ») | the refusal holds in the three modes |
| 8 | `Que dit J. Dupont ?` | ✓ row « Plusieurs personnes correspondent à « J. Dupont » : » (3 fichiers / 1 fichier) | ✓ no row | ✓ row, no line | |
| 9 | `Qu'a dit le Dr Martin ?` | ✓ row (Pierre 3, Paul 3) | ✓ no row | ✓ row, no line | the answer gives three **patients'** letters as what « Dr Martin » said: model |
| 10 | `Que dit le courrier de Véronique Monnier ?` | ✓ `26 mai 2026`, AVS | ✓ | ⚠ widened (the scan's name is misread); the model writes « Adressé à la CPAM » for a Swiss AVS letter | machine + model |

A6 tally: 3 narrowed (3, 4, 5), 3 narrowed then widened (1, 2, 10), 4 not narrowed by design (6 to 9). Protocol-conform in every case.

### Part B - safety

| Step | Result |
| --- | --- |
| B1 | ✓ row, then `Recherche ciblée à votre demande : 3 documents sur 28 — Jean Dupont`. Her remark (no chip) is right **for version 1**, which promised one; version 2 says no chip (`06` section 7, fault 3) |
| B2 | ✓ |
| B3 | ✓ the widened list equals A5-3 file for file. The first answer garbles a file name (`...monnier-veronique-veronique.pdf`): the model, not the source |
| B4 | ✓ file not cited in either mode; the named-file sentence gives the exact « n'est pas sélectionné pour cette conversation » |
| B5 | ✓ 582 names, same fingerprint `521f5df9eda6faee` before and after four questions about nobody; the window finds none of the three names |
| B6 | ✓ KBD-19 confirmed on the real application: after `Que dit le courrier de Sophie Rochat ?` the question `Que contiennent les notes internes ?` is narrowed to her and then widens; the answer lists **one** note, asked alone it lists **two** (stock and photocopier). The button is absent, probably because the search had already widened by itself (inferred: the step asks for a press that cannot happen) |

### Parts C and D - a library that lives, then 100 documents

C1 ✓ (`1 document sur 29 — Sidonie Quenneville`) · C2 ✓ (rename logged, `2 documents sur 30`) · C3-a/b/c ✓ (« Analyser (30) » then plain; « Analyser (7) » then plain; the data card figures identical to the replay) · C4 ✓ by the question (no line: a candidate never narrows) · C5 ✓ (no document names Sophie Rochat any more, the deleted file is not cited; the CSS answer is honest: no letter *comes from* the CSS, the one that names it is a patient's) · C6 ⚠ KBD-50 (62 re-read against 29) · D1 ✓ (2 min 5 s, the same to the second as C6) · D2 ✓ (Sandrine Colas page 66, Nadia Bensaïd, `3 documents sur 90 — Fatima Benali`, the three expected files).
Not recorded: C3-0 and C3-z (the equality of the list before and after the three Resets), and the cards and `names` of C4 to C6.

### Part S - Switzerland, tables, file names

| Block | Result |
| --- | --- |
| Cards after the extra files | ✓ exact: `103 fichiers, 92 analysés`, `13 fichiers, 12 analysés`, `25 ... dont 22 déjà connues`, `8 colonnes ... dans 5 tableurs`, `19 liens`, both buttons plain |
| S1 to S10 | ✓ AVS found (S1); `Dre` read, no row (S3); HUG and CHUV found (S4, S6); `Genève` no longer narrows (S5); `Lucas Roy` / `Roy Lucas` two sets (S9); D'Amico, N'Diaye found (S8, S10). Machine effects: S2 3 files for 4 (a scan misread), S6/S8/S10 widened. Model: S5 is faithful to the fixture (the letters really print « Hôpital fribourgeois, Route de Lausanne 36, 1201 Genève »: the fixture pairs hospitals and addresses at random, checked in the index); S3 lists three patients' letters, then adds generic sentences (« toujours décrit comme rassurant ») |
| S-T1 to S-T11-3 | ✓ **fifteen of fifteen exact**, no model: sums, maximum, counts, the filter `medecin = Dr Jean Dupont`, `medecin parmi Dr Paul Martin, Dr Pierre Martin`, the unsettled-dates refusal with the exact message. S-T11-2 and -3 add « Votre question précédente n'a pas pu être calculée non plus » because the conversation was not cleared between them |
| S-F1 to S-F3 | ✓ three of three, texts exact |
| S-M1 to S-M3 | ✓ twelve workbooks listed; « 92 fichiers ont été analysés » ; no « Quel classeur ? » |
| S-K1 to S-K5 | reported « tout est conforme » without the lines; KBD-48 not written down |

**The deterministic engine (tables, file names, folder questions) is the strongest result of the programme: every answer it gave in the record is right.** The weak answers all come from the generative path.

### Part G - 200 documents

Cards ✓ (`203 fichiers, 179 analysés`, `18` scans, the 21 unread files by format, exact). Names a few higher (machine, `06` section 6). G2-11/12 ✓ both modes; G2-13: `5 documents sur 179` (the Sanitas scan is lost, as `06` says); G2-15 widened; G2-16 and G2-6 ✓ no line. G3 report: 78 questions, 33 would be reduced, 27 reduced for real, **12 widened again**, median reduction 96 %, release gate raw 45.7 % (63 of 138), adjusted 100.0 %.

## 3. What the knowledge base proved and what it did not

**Proved on her machine**
- It never makes a name up (B5), never reads a file that is not ticked (B4), never picks between two people (A4-8/9, B1/B2), and `Désactivé` / `Suggestions seulement` are the old search byte for byte.
- When it narrows to a named person, the files searched shrink by 89 to 98 % (median 96 %) and the text sent drops from 3 000 to 5 500 characters to 250 to 2 800 (log rows 23, 24, 34, 47, 48, 51, 58, 65, 66), and the sources become the right ones (A6-3, A6-4, A6-5, C1, C2, D2-13, S1, S4, S9). The median lookup is 4 to 5 ms.
- Every kept file names an entity of the question (adjusted gate 100 %).

**Not proved**
- That `Automatique` is **faster**: the model's first word comes after 16 to 25 s whatever the prompt (section 6); narrowing saves about 5 s, 14 % at G3. The benefit of the base is in precision and in what the model is not shown, not in time.
- That the base **finds every file of a set**: it was not asked, because E2 ran in « Suggestions seulement » and the question shapes where it matters (sets, ranks) are the ones the answer path cannot serve at all (next section).
- That `Automatique` should be the default: the owner decided on 12 October that it stays « Suggestions seulement »; this record agrees with that decision (E3: 27 questions really narrowed, 12 of them widened again, 44 %).

## 4. E2, question by question

`rel` = files that hold the answer according to the ground truth; `cited` = distinct files under « Sources »; `hit` = cited files among `rel`; `named` = files of `rel` that the model's text mentions. Computed from the record by a throw-away script, not versioned.

| # | Sentence | rel | cited | hit | named | Judgement |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `Combien de documents ai-je ?` | - | folder tier | - | - | ✓ « 179 fichiers ont été analysés », no model |
| 2 | `Quel est le dernier courrier reçu ?` | - | 6 | - | - | ✗ answers `2026-09-13_courrier-rhumatologie.pdf`; the library holds `2026-09-26_courrier-endocrinologie.pdf` and `2026-10-01_courrier-dermatologie.pdf` later: **a rank over the inventory sent to the model** (KBD-54) |
| 3 | `Quels sont les horaires d'ouverture du cabinet ?` | 0 | 6 | - | - | ✗ answers « Lundi : 14 h à 18 h » from a doctor's file; no document carries opening hours, the answer must be a refusal |
| 4 | `Que dit la procédure sur le lavage des mains ?` | 3 | 6 | 1 | 1 | ✓ content right, one of three files |
| 5 | `Comment éliminer les aiguilles usagées ?` | 2 | 3 | 2 | 2 | ✓ (four false alarms on pages: KBD-49) |
| 6-9 | `Qu'est-ce qui est prévu pour la chaîne du froid des vaccins ?` (suggest, auto, off, auto) | 1 | 3 | 1 | 0-1 | ⚠ the file is among the sources; the model says « aucune mention explicite » **four times out of four**, in the three modes: a stable false negative of the model (the manual only says deviations are reported to Nadia Bensaïd) |
| 10 | `Que dit le contrat de la photocopieuse ?` | 2 | 6 | 2 | 0 | ✓ answer right from the two notes |
| 11 | `Quels résultats parlent de Staphylococcus aureus ?` | 1 | 6 | 1 | 1 | ✓ |
| 12 | `Y a-t-il un compte rendu qui cite Klebsiella pneumoniae ?` | 1 | 6 | 1 | 1 | ✓ |
| 13 | `Quels documents parlent de la LAMal ?` | **13** | 6 | 6 | **3** | ⚠ set |
| 14 | `Quels courriers évoquent la franchise annuelle ?` | **13** | 6 | 6 | 6 | ⚠ set |
| 15 | `Quels documents viennent de Genève ?` | **33** | 6 | 6 | 5 | ⚠ set |
| 16 | `Quels courriers sont liés à Lausanne ?` | **20** | 6 | 6 | 4 | ⚠ set |
| 17 | `Y a-t-il des documents qui parlent de Nyon ?` | **8** | 6 | 6 | 4 | ⚠ set |
| 18 | `Quels documents mentionnent Annecy ?` | **24** | 6 | 6 | 4 | ⚠ set |
| 19 | `Que dit le courrier de Marie Dupont ?` | 2 | 6 | 2 | 2 | ✓ |
| 20 | `Que dit le courrier de Jean Dupontet ?` | 3 | 6 | 3 | 1 | ⚠ all three retrieved, the answer keeps the MGEN letter and drops the lab result |
| 21 | `Y a-t-il des résultats d'analyses pour Jean-Pierre Dupont ?` | 3 | 6 | 2 | 0 | ✓ « Non » is right: the three files are letters and a prescription |
| 22 | `Que dit le courrier de Marie Martin ?` | 6 | 6 | 5 | 1 | ✓ the singular « le courrier » is answered with one letter |

**The six sentences whose answer is a set (13 to 18).** 111 files hold the answer. The application retrieved 36, **all 36 correct** (precision 100 %, recall 32 %); the model named 26 (23 %). Reading: the search is good; the answer path loses the rest in two places, the cap of six passages and the model's habit of listing fewer than it was given (3 of 6 for the LAMal). This confirms her hypothesis for this family of sentences and shows it is only half of the problem.

## 5. The loss has three layers, and raising the window treats one

1. **The cap** (`retrieval.rs:278-291`: the loop stops at `max_chunks`, 6 at the baseline). Raising the window raises it (30 at 32 768 tokens).
2. **The model's listing**: given six files it names three to five (rows 13 to 18 above). More passages make this worse, not better, for a 3B model.
3. **What « Sources » shows**: every retrieved passage's file (`commands.rs:1884-1887`, `:1921`), relevant or not. At E3, of the 168 files cited under the 38 questions the base would reduce, 87 named no entity of the question (« 87 cited files that mention no name of the question »). These are the six passages the search always returns, not what the model used, and they are what lets the model mix two people (A6-2).

**An offline check of what a plain word search would have returned** (SQLite FTS5 on a copy of her index, the table `chunks_fts` the product already maintains; no model, no embedding):

| Sentence | Files holding the answer | Found by the word | False files | Missed |
| --- | --- | --- | --- | --- |
| `LAMal` | 13 | 11 | 0 | 2 (see below) |
| `franchise annuelle` | 13 | 11 | 0 | 2 |
| `Genève` | 33 | **33** | 0 | 0 |
| `Lausanne` | 20 | 19 | 0 | 1 |
| `Nyon` | 8 | **8** | 0 | 0 |
| `Annecy` | 24 | 23 | 0 | 1 |
| **Total** | **111** | **105** | **0** | **6** |

The six misses are two files only: `2026-03-20_decompte-css-rochat-sophie.pdf`, which the lifecycle step C5 **deleted** (so it is correctly absent), and `2026-03-04_decompte-suva.pdf`, a scan whose text recognition lost the word. In other words **a word search returns 105 of the 109 files that still exist and read, with no wrong file**, where the application retrieved 36. The index already holds the answer to every « Quels documents ... X ? » sentence; the product just never asks it as a set.

## 6. Time: what the tower does, measured

From `retrieval-timings.jsonl` (`estimatedInputChars` counts the excerpts only):

| Situation | Passages | Characters | First word | Whole answer |
| --- | --- | --- | --- | --- |
| a named file or a narrowing to one or two files (rows 47, 48, 84) | 1 to 2 | 121 to 323 | **16 to 18 s** | 19 to 22 s |
| narrowed to one person | 1 | 578 to 669 | 19 to 20 s | 23 to 24 s |
| an ordinary question (the E2 rows) | 6 | 2 000 to 5 800 | 20 to 29 s | 24 to 49 s |
| after the context-budget change, 32 768 window | **30** | 20 624 | **89 to 94 s** | 105 to 112 s |
| | 29 | 28 998 | 82 to 83 s | 92 to 97 s |
| | 33 | 32 847 | 91 s | 95 to 96 s |
| | 37 | 36 720 | 112 s | 117 s |
| | 62 | 46 847 | failed after 21 s (the HTTP 500 she noted at 65 536) | |

Three readings (measured, then inferred):
- **A fixed floor of 16 to 18 s** exists before any text is read (measured). A 3B model answering 120 characters in 18 s is not what a GPU does; the cause is not in the log (inferred: the model is partly or wholly on the processor, or is reloaded, or the gateway adds a long prompt). It is a **property of the tower, not of the design**, and it decides how every figure in this study must be read.
- Beyond the floor, the prompt costs about **90 tokens per second** (65 s for the 5 700 more tokens between 3 500 and 20 600 characters, at 3 characters per token). Six passages add about 8 s to the floor; thirty add about 70 s.
- **Her own observation is confirmed**: 49 152 and 65 536 are unusable on this machine, and 32 768 multiplies the time of a question by three or four before any gain in quality has been measured. Rows 129 and 131 to 134 also show that 29 to 37 passages came from **four or five distinct files** (`cited` in the log is the number of distinct files): raising the cap adds pages of the same long file rather than more files (P-5). The questions behind these rows are not in the record; the guide of 115 pages is the likely source (inferred).

## 7. My opinion on 200 and 1 000 files

1. **The size of the library is not the constraint; the number of passages the model reads at once is.** Indexing is paid once, by hand, in an Analyse (12 s for 30 documents; 2 min 5 s for the 70 added at D1, which include the 115-page guide; 43 s for the 100 added at G1); the search over the index takes milliseconds (2 to 97 ms for 968 passages in the log; 16 ms at 878 files in the bench of `04`); the base's lookup is 4 to 5 ms. Whatever happens in minutes happens in the model.
2. **200 files is workable today** for every question that is about a person, an organisation, a number or a named file, because the base narrows to 1 to 7 files and the model reads 1 to 6 passages. It is **not** workable for questions whose answer is a set (sections 4 and 5) as long as the model is asked to produce the set.
3. **1 000 files is out of reach of any model on any machine in one request** (about 700 000 characters, 230 000 tokens for one passage per file). On this tower a batch of 30 files costs about 100 s, so a pass over 1 000 files is about 34 batches, about an hour, plus a merge. That is acceptable only as a pass **started by hand, with the interface showing it runs** (`AGENTS.md`), never as an answer to a question. Raising the window does not change this; `CONTEXT-BUDGET.md` says it too.
4. **The right lever is to make the model read less and the engine do more**: the sets, the ranks and the counts from the index (P-1, P-2), the narrowing for people and organisations (already built), and the model only for summarising what the engine hands it. With that split, 200 and 1 000 files behave alike for every question shape except « summarise everything », because the cost no longer grows with the library.
5. **The Mac mini M5 Pro 64 GB** will lift the floor and the 90 tokens per second, and will allow a larger model that reads better; I cannot say by how much (not measured). It does not remove points 3 and 4. The tool to measure it on arrival already exists: `apps/server/scripts/measure_context.py`. Do not choose the window or the model from this study's figures; they describe the tower.
6. **A recommendation on the tower today**, her call: go back to `ministral-3:3b=12288` (10 passages, about 10 000 characters) or try 16 384, not 32 768, until P-1 and P-3 are in. One setting, no code (`CONTEXT-BUDGET.md`, « Putting it in service » and « To go back »). Before anything else, three read-only checks on the tower, which would tell whether the floor is the hardware: `ollama ps` (the column PROCESSOR: « 100% GPU » or a split), `nvidia-smi` if the card is NVIDIA, and `curl http://192.168.1.83:8080/health` (windows and `max_context_chars`).

## 8. Fixes proposed (nothing coded)

Ordered by value for effort. "Owner" = a decision of hers is needed first. Sizes are inferred, not measured.

### P-1 A set question is answered by the index, not by the model (KBD-53) - owner

- **Shape**: « Quels documents / courriers / résultats / factures (parlent de | mentionnent | évoquent | viennent de | concernent) X ? », « Y a-t-il des documents qui parlent de X ? », « Combien de documents mentionnent X ? ».
- **Behaviour**: the answer is the list of files that contain the term (or the files the base ties to the entity, when X resolves to one), with the count, one short excerpt per file, and no model call and no embedding call. Zero seconds, and the same for 20 files or 2 000.
- **Where**:
  - `src/folder_questions.rs`: `route` (line 243) and `QuestionRoute` (enum, same file) get one arm, `Enumeration { terms }`, decided before `GlobalRetrieval` (line 314). `Operations` (line 80) gets a list for the new intent. Note the guard in the header of that file (lines 13 to 19: « no word the pack does not know may be a word the documents contain »): it was written to keep content questions away from the inventory, and this change reverses it for one shape, so it needs the owner's decision, recorded in `docs/DECISIONS.md`.
  - `resources/work-folder-questions/fr-FR.json` and `en-US.json`: the vocabulary, as data (the language guard forbids French in Rust).
  - `src/retrieval.rs`: one function that returns the distinct files and a first passage per file from `IndexStore::search_lexical_terms_in` (`index_store.rs:818`) with a limit far above 6; the table `chunks_fts` already exists (`index_store.rs:149`).
  - `src/knowledge/targeting.rs`: when X is an entity (HUG, CHUV, `Dre Vuagniaux`), use the base's files; the cap `knowledgeMaxCandidates` (15) must not truncate a **listing** (it is a narrowing safeguard).
  - `src/commands.rs` near line 1686: an arm beside `QuestionRoute::Deterministic`; the answer is a new variant of `FolderAnswer` (machine code plus data).
  - React: `apps/desktop/src/lib/folderAnswer.ts` (the catalogue that writes the sentence in the user's language) and its test `folderAnswer.test.ts`.
- **Dependencies**: none new. SQLite FTS5 and the router are in place.
- **Risks**: synonyms are not covered (« assurance-maladie » for « LAMal »); a very frequent word returns hundreds of files (show the first 50 with the total and a « Voir tout »); accents (the `Genève` test above matched 33 of 33, so the tokenizer folds them; check `Hôpital` / `hopital`).
- **Tests**: the six sentences of section 5 against the index of the 200-document library (expected 105 of 111 with the C5 deletion); a sentence that must **not** be taken for a set (« Que dit le courrier de Marie Dupont ? »); `Q102` (`Dr Hélène Marchetti`, 107 files).
- **Size**: medium. This single change would answer the whole family of E2 sentences 13 to 18 (the offline check of section 5 gives 105 files of 111, none wrong) and every other « Quels documents ... X ? » of the 164 sentences still to play.

### P-2 A rank over the inventory is answered by the inventory (KBD-54)

- « Quel est le dernier courrier reçu ? », « le plus récent », « le premier ». The folder tier already counts and lists from `WorkFolderInventory`; the date is the `YYYY-MM-DD_` prefix the clean-name rule produces, else `FileRecord.modified_at` (`file_record.rs:194`).
- **Where**: `folder_questions.rs` (`Operations`, line 80; `folder_answer`, line 451), the two packs `*.json`, `commands.rs` (the `Deterministic` arm), `folderAnswer.ts`.
- **Risk**: « courrier » is a kind, not a folder; filter by the subject words of the pack and by path (`courriers-entrants/`), and say what was ranked. Size: small.

### P-3 « Sources » says what mattered, not everything that was retrieved (KBD-52)

- Add one field to `Evidence` (`retrieval.rs:63-72`), whether the passage's file names an entity of the question or holds a question term, filled where the base already knows it (`targeting.rs`, `distinct_paths`, line 719); send it with the event (`commands.rs:1884-1887`); show the matching files first and fold the others under « autres extraits consultés (n) » in `apps/desktop/src/components/MessageList.tsx` (lines 302-306, `groupSources`).
- **Why not filter after the answer**: the 3B model garbles file names (« ...monnier-veronique-veronique.pdf », « administratif/planning/2026-01-08_courrier-endocrinologie... »), so matching its text to paths is unreliable.
- **Dependencies**: the TypeScript type in `lib/ipc.ts`, the catalogue. The diagnostics keep reading the full evidence. Size: small to medium.

### P-4 Ask the model for a steady answer (KBD-57)

- The desktop never sends a temperature (`gateway.rs:202-210` builds the payload without one); the server accepts it already (`schemas.py:32`, `chat.py:151`, `ollama.py:62`). A4-1 and A5-1 give a wrong and a right answer on the **same six passages**. For a question answered from excerpts, send `temperature` 0 (or 0.1).
- **Effects**: reproducible tests, fewer invented figures; a risk of drier wording. One line, one test, no new setting. Size: tiny.

### P-5 Do not let one long file fill the cap (KBD-56)

- In the cap loop (`retrieval.rs:278-291`) take at most N passages per file (2 or 3) before filling with the next file, **only when** the cap is above the baseline. Rows 129 to 134: 29 to 37 passages from 4 to 5 files. Depends on P-1 being decided: with P-1 in, the cap matters far less. Size: small. Test: the guide of 115 pages against a question that names no file.

### P-6 A widened search should add, not replace (KBD-17) - owner

- Today the widened search discards the narrowed one (`targeting.rs:598-602`: `run(None)` alone). At E3, 12 of the 27 real narrowings widened, and in A6-2 the widened answer is the one that mixes two people. Option: keep the narrowed passages first and fill the remaining slots from the full selection. The decision of 12 October (« Suggestions seulement » stays) makes this information, not urgency. Tests: the calibration script of lot 6 and the cases A6-1, A6-2, A6-10, S6, S8, S10. Size: small.

### P-7 The false alarm on a page number (KBD-49, already proposed in `06`)

- `commands.rs:1914-1917`: add each excerpt's `page_number` to `known`. Four false alarms in one answer at E2 (« 32 », « 48 », « 57 », « 69 »). Tiny.

### P-8 Measure before choosing a window (no code)

- On the tower, the three read-only checks of section 7, point 6. Then, with the application on `fix/kb-wrap-up`, replay E2 at 12 288 and 16 384 (the existing log gives the times). Nothing else changes.

### P-9 The batch pass, later (not now)

- « Résume chaque document » over hundreds of files, in batches of about 30 with a merge, **started by hand in an Analyse** with the interface showing it runs (`CONTEXT-BUDGET.md`, « What this does not solve »; `AGENTS.md`). Worth designing once the Mac mini is measured.

**Suggested order**: P-4, P-7 (an hour together), then P-2, then P-1 and P-3 (the two that change what she sees), then P-5 and P-6 if the replay still shows the need. P-8 can start now.

## 9. Freezing the state

**Two things to freeze, both outside git: the application's data and the test folders.** The code is frozen by git (section 10).

What lives where (read from her disk):
- `C:\Users\elise\AppData\Local\com.assistantcabinetai.desktop\`: `index.sqlite3` (7.9 MB; holds the documents' passages, the knowledge base **and** the 106+ rows of `kb_diagnostics`), `knowledge-identifier.key` (without it the hashed identifiers of the base cannot be matched again), `tabular.sqlite3`, `workbooks.sqlite3`, `retrieval-timings.jsonl` (the 134 rows of section 6), `analysis-timings.jsonl`, `renamed-files.jsonl`, `generated-files.jsonl`.
- `C:\Users\elise\AppData\Roaming\com.assistantcabinetai.desktop\settings.json` (the tower URL, `ministral-3:3b`, `kb-lot11`, `kb-lot11-data`, `knowledgeMode: suggest`, both health packs).
- `C:\Users\elise\AssistantCabinetAI\kb-lot11` (2.8 MB, 203 files), `kb-lot11-data` (13 files), and the two helper state files `kb-lot11.lot11-state.json`, `kb-lot11.undo.json`.

**Never run any of these commands from now on** (each destroys what she wants to keep):
- `lot11_helper.py restore`: puts her **pre-test** index and settings back over the current ones.
- `lot11_helper.py backup`: **overwrites** `C:\Users\elise\kb-backup-lot-11`, her only copy of the pre-test state.
- `lot11_helper.py clear-log`: deletes `retrieval-timings.jsonl` and empties `kb_diagnostics`.
- `lot11_helper.py prepare ...` and `tier ...` and `lifecycle ...`: rewrite or resize the test folders. Part R of the protocol is `restore`.
- « Réglages » → « Gérer… » → « Tout réinitialiser », and the two « Reset » buttons of the cards.
Allowed and harmless: `status`, `names`, `report`, `questions ...`, `mode ...`.

**The freeze, with the application closed** (the index is a SQLite file; copy it while the window is shut). Run in PowerShell, one block:

```powershell
$stamp = "C:\Users\elise\kb-freeze-2026-10-13"
New-Item -ItemType Directory -Force "$stamp\appdata-local","$stamp\appdata-roaming","$stamp\test-folders" | Out-Null
Copy-Item "C:\Users\elise\AppData\Local\com.assistantcabinetai.desktop\*" "$stamp\appdata-local" -Recurse -Force -Exclude "EBWebView"
Copy-Item "C:\Users\elise\AppData\Roaming\com.assistantcabinetai.desktop\settings.json" "$stamp\appdata-roaming" -Force
Copy-Item "C:\Users\elise\AssistantCabinetAI\kb-lot11" "$stamp\test-folders" -Recurse -Force
Copy-Item "C:\Users\elise\AssistantCabinetAI\kb-lot11-data" "$stamp\test-folders" -Recurse -Force
Copy-Item "C:\Users\elise\AssistantCabinetAI\kb-lot11.lot11-state.json","C:\Users\elise\AssistantCabinetAI\kb-lot11.undo.json" "$stamp\test-folders" -Force
Get-ChildItem $stamp -Recurse | Measure-Object -Property Length -Sum
```

Check the copy holds `index.sqlite3`, `knowledge-identifier.key` and `retrieval-timings.jsonl` under `appdata-local`. **To come back to this state later**: close the application, copy those files back over the two AppData folders and the two test folders over `C:\Users\elise\AssistantCabinetAI\`. Do the same exchange to run the clean `main` build: the two builds share one identifier, so they share one `AppData` folder; **never open the `main` build on the knowledge base's index without swapping the files first** (the knowledge build adds tables and an index to `index.sqlite3`).

## 10. Git: freeze the code on `kb/integration`, keep `main` as it is

*The final, corrected command set (tags instead of branches, the agent settings file left out, the local branches cleaned) is in section 13. Where the two differ, section 13 wins.*

**Facts read from the repository (13 October 2026)**
- `origin/kb/integration` = `c85fae6`, which is an **ancestor** of `fix/kb-wrap-up` (`a661455`, three commits ahead, none behind): the update is a fast-forward, no merge commit.
- `main` and `origin/main` = `f27505f`, **59 commits behind** `fix/kb-wrap-up`, and no pull request from `kb/integration` to `main` is open (`gh pr list`: all 17 pull requests of the programme are `MERGED`, into `kb/integration`).
- Every intermediate branch (18 on the remote) is **merged into `kb/integration`** (`git branch --merged kb/integration`) except `fix/kb-context-budget`, which becomes merged by the fast-forward (`03fd4ef` is inside `a661455`). All 17 `kb-after-*` tags are on the remote.
- Uncommitted: `docs/DECISIONS.md`, `02-results.md`, `defects-register.md`, `lot-11-validation.md` (modified); `06-final-human-test-report.md`, `07-...md` (this file), `_KB-final-test.txt` (new); and `.claude/settings.json` (modified by the owner to give the agent read access; **do not commit it**).

**My position on deleting the intermediate branches from the remote: I agree, with three conditions.**
1. **Order**: push `kb/integration` first and delete only after; `fix/kb-context-budget` is the one branch whose commit is not yet on `kb/integration` before the push.
2. **Nothing is lost by the deletion**: the commits stay reachable from `kb/integration` and from the tags `kb-after-*`; the 17 pull requests stay on GitHub with their diffs and discussions after the branch is deleted (GitHub keeps `refs/pull/N/head`).
3. **The branch names will then exist on one disk only** (hers). The commits do not; the names do. A `git bundle` of everything is the cheap insurance, made before the deletion.
I would **not** delete `main`, `kb/integration`, nor the tags. A suggestion, not an objection: to keep `main` intact only by discipline is enough (no `gh pr create --base main` from `kb/integration`, no `git merge kb/integration` while on `main`); if she wants a guarantee, GitHub can require a pull request on `main` (Settings, Branches), which stops an accidental push to it.

**The commands, in this order** (PowerShell, from `C:\Users\elise\Documents\CURSOR\AssistantCabinetAI`, on `fix/kb-wrap-up`):

```powershell
git status
git add docs/DECISIONS.md docs/test-reports/knowledge-base-pass-1/02-results.md docs/test-reports/knowledge-base-pass-1/defects-register.md docs/test-reports/knowledge-base-pass-1/human-tests/lot-11-validation.md docs/test-reports/knowledge-base-pass-1/human-tests/_KB-final-test.txt docs/test-reports/knowledge-base-pass-1/06-final-human-test-report.md docs/test-reports/knowledge-base-pass-1/07-record-study-fixes-and-freeze.md
git commit -m "docs: record the final knowledge base human test, study it and freeze the state"
git bundle create C:\Users\elise\kb-freeze-2026-10-13\repository-all-branches.bundle --all
git switch kb/integration
git merge --ff-only fix/kb-wrap-up
git tag -a kb-frozen-2026-10-13 -m "knowledge base frozen after the final human test, 13 October 2026"
git push origin kb/integration
git push origin kb-frozen-2026-10-13
git ls-remote origin refs/heads/kb/integration
git rev-parse kb/integration
```

The last two lines must print the same hash. Only then delete the intermediate branches **on the remote** (local branches stay):

```powershell
git push origin --delete docs/kb-lot-10-withdrawn docs/kb-lot-3-answers feat/kb-document-ingestion feat/kb-management-dialog feat/kb-names-calibration feat/kb-ports-and-packs feat/kb-shared-vocabulary feat/kb-table-semantics feat/kb-targeted-scope feat/kb-targeting-ui feat/kb-validation fix/ci-pnpm-and-ruff fix/gateway-thinking-and-keep-alive fix/kb-context-budget fix/kb-names-and-organisations fix/kb-retrieval-rules fix/kb-surname-initial fix/kb-swiss-first
git fetch --prune
git branch -vv
git branch -r
```

Expected at the end: `git branch -r` lists `origin/HEAD`, `origin/kb/integration`, `origin/main` only. `fix/kb-wrap-up` was never pushed and stays local. `main` is untouched: `git rev-parse main origin/main` still prints `f27505f...` twice.

**To work on `main` and on the knowledge base side by side** without checking out back and forth, one extra folder, a worktree (it needs its own `pnpm install` and its own Rust build):

```powershell
git worktree add C:\Users\elise\Documents\CURSOR\AssistantCabinetAI-main main
git switch -c feat/kb-next kb/integration
```

The first creates a second working folder on `main`; the second starts the next knowledge-base work from the frozen branch. In the `main` folder, never work on `main` itself: `git switch -c feat/<topic> main` first. Remember the shared `AppData` of section 9.

## 11. What to run, and when

1. **Now**: the freeze of section 9, then the git block of section 10. Nothing in the application changes.
2. **Before any code**: the three read-only checks on the tower (section 7, point 6). They decide whether the floor is the machine.
3. **When P-4 and P-7 are done**: replay Q001 to Q019 of `questions daily` in « Suggestions seulement », at 12 288 and then 16 384, and compare with the table of section 4 (cited, hit, named, times).
4. **When P-1 and P-3 are done**: Q020 onwards, then the lists `ab`, `safety`, `follow-up`, with the three modes.
5. **When the Mac mini is there**: `measure_context.py`, Part M of the protocol, and a fresh study of the window and the model; this report's figures are the tower's.

## 12. Files touched by this study

Added: this file. Changed: `defects-register.md` (KBD-52 to KBD-57 added after KBD-51), `.env.example` (a comment, no value changed), `docs/CONTEXT-BUDGET.md` (two sections added). Not changed: any source file, any setting, any fixture, any document of the application's data.


## 13. Decisions and answers after the owner read this report (13 October 2026, evening)

**What the 36-against-105 finding means.** The index is not badly built: the 105 files were found in the application's **own** index (`chunks_fts`), which the application's own Analyse produced. What is missing is a way to ask it for *every* file that holds a word; the only road the question path has takes the six best passages, which is right for « what does this letter say » and wrong for « which documents mention Genève ». Verdict: **no rewrite of the indexing**. The extraction, the text recognition, the tables, the entity base, the deterministic tiers and their tests all work and are what the good results of the record come from. What is built next is a **second, separate road** (P-1: « list the files », and P-2: « rank the files ») with its own module and its own tests, in front of the existing one, with the 205 questions as the exam. Two honest limits: the word search is exact (no synonyms: that is what the meaning search is kept for), and the entity base itself loses recall as the library grows (`04-final-report.md` section 7.2: 85.9 % of the relevant files kept at 200 documents, 44.2 % at 1 000, almost all of the loss « capped » by `knowledgeMaxCandidates` = 15): that cap must not apply to a listing.

**The aim, as agreed.** The engine finds, counts, ranks and sources; the model only writes, from a few precise, sourced passages. A better search does not need a larger window; it needs fewer passages: a set question reads none, a person question reads one to six (2 000 to 4 000 characters), which the 8 192 and 12 288 windows already hold.

**Recorded decisions**

1. The tower goes back to `ministral-3:3b=12288` (16 384 at most). Written in `.env.example` and in `docs/CONTEXT-BUDGET.md` (« Measured on the tower »). The `.env` of the tower is the owner's and was not touched.
2. The fixes P-1 to P-9 stay proposals. Nothing is coded. Before code, a short design note for the second road is written first (the project's rule).
3. `fix/kb-context-budget` (`03fd4ef`) is **not** the last branch: `fix/kb-wrap-up` holds two more commits (`b442d9d`, the wrap-up with the Analyse-button fix KBD-47, and the merge). The last branch of the knowledge base is `fix/kb-wrap-up`; it is published under the name `kb/integration` (moved forward by fast-forward, no new commit). The context-budget change is kept reachable by the tag `kb-context-budget-change` and documented for reuse without the base in `docs/CONTEXT-BUDGET.md` (a dry run on `main` stops on three files only).
4. `main` has no knowledge-base code (0 files named `knowledge`, 0 occurrences of `kb_`, `knowledge_base` or `knowledgeMode` under `apps/`; last commit `f27505f`, pull request 19). New work starts on a **new branch cut from `main`**, never on `main` itself.
5. The steps are kept by **tags**, not by branches: `kb-baseline`, `kb-after-lot-00` to `kb-after-lot-11`, `kb-after-fix-03`, `kb-after-fix-04-batch-1` to `-3`, plus `kb-frozen-2026-10-13` and `kb-context-budget-change`. To go back to a step: `git switch -c retry/<topic> <tag>`. The indexing steps are `kb-after-lot-01` (store), `kb-after-lot-03` (extractors and packs), `kb-after-lot-04` (documents), `kb-after-lot-05` (tables); the search steps are `kb-after-lot-06` (targeted scope) and `kb-after-lot-07` (interface). A `git bundle` is an offline copy of the whole history, branches and tags, in one file.
6. `.claude/settings.json` leaves the tracking of the knowledge-base branch; the personal copy lives in `.claude/settings.local.json`. `main` still tracks it; switching to `main` puts main's copy back in the working folder and switching away removes it, so the personal rules go in the local file, which git never touches.
7. The generators and the answers are already in git: `scripts/gen-knowledge-sandbox.py` and `scripts/knowledge_sandbox/` (`docs.py` the documents, `tables.py` the workbooks, `questions.py` the 205 questions), `fixtures/knowledge-sandbox/` (214 files: 200 documents, the workbooks, `questions.json`), and `human-tests/lot11_helper.py`. Each question of `questions.json` carries `relevant_documents` and `relevant_readable`, the exact files that hold the answer: that is the ground truth the application is to be compared with. Only the 1 000-document library (`data/knowledge-sandbox-1000`) is outside git, by design; it is rebuilt by the same script.
8. What to freeze outside git: the two folders, `settings.json`, and the 134-line timing log with the question table of the index. The rest of the index is rebuilt by pressing « Analyser » (about 3 minutes for 200 documents), except the names the owner confirmed, deleted or restored by hand and the record of the test itself.

**The final command set**

```powershell
cd C:\Users\elise\Documents\CURSOR\AssistantCabinetAI
git status
Copy-Item .claude\settings.json .claude\settings.local.json
git add .env.example docs/CONTEXT-BUDGET.md docs/DECISIONS.md docs/test-reports/knowledge-base-pass-1/02-results.md docs/test-reports/knowledge-base-pass-1/defects-register.md docs/test-reports/knowledge-base-pass-1/human-tests/lot-11-validation.md docs/test-reports/knowledge-base-pass-1/human-tests/_KB-final-test.txt docs/test-reports/knowledge-base-pass-1/06-final-human-test-report.md docs/test-reports/knowledge-base-pass-1/07-record-study-fixes-and-freeze.md
git commit -m "docs: record the final knowledge base test, the tower measures and the context budget reuse"
git rm --cached .claude/settings.json
Add-Content .gitignore "`n.claude/settings.json`n.claude/settings.local.json"
git add .gitignore
git commit -m "chore: stop tracking the local agent permission file"
git tag -a kb-context-budget-change 03fd4ef -m "context budget change, reusable without the knowledge base"
git bundle create C:\Users\elise\kb-freeze-2026-10-13\repository-all-branches.bundle --all
git switch kb/integration
git merge --ff-only fix/kb-wrap-up
git tag -a kb-frozen-2026-10-13 -m "knowledge base frozen after the final human test, 13 October 2026"
git push origin kb/integration kb-frozen-2026-10-13 kb-context-budget-change
git ls-remote origin refs/heads/kb/integration
git rev-parse kb/integration
```

The last two lines must print the same hash. Then, on the remote and then locally:

```powershell
git push origin --delete docs/kb-lot-10-withdrawn docs/kb-lot-3-answers feat/kb-document-ingestion feat/kb-management-dialog feat/kb-names-calibration feat/kb-ports-and-packs feat/kb-shared-vocabulary feat/kb-table-semantics feat/kb-targeted-scope feat/kb-targeting-ui feat/kb-validation fix/ci-pnpm-and-ruff fix/gateway-thinking-and-keep-alive fix/kb-context-budget fix/kb-names-and-organisations fix/kb-retrieval-rules fix/kb-surname-initial fix/kb-swiss-first
git fetch --prune
git branch -d docs/kb-lot-10-withdrawn docs/kb-lot-3-answers feat/kb-document-ingestion feat/kb-management-dialog feat/kb-names-calibration feat/kb-ports-and-packs feat/kb-shared-vocabulary feat/kb-table-semantics feat/kb-targeted-scope feat/kb-targeting-ui feat/kb-validation fix/ci-pnpm-and-ruff fix/gateway-thinking-and-keep-alive fix/kb-context-budget fix/kb-names-and-organisations fix/kb-retrieval-rules fix/kb-surname-initial fix/kb-swiss-first fix/kb-wrap-up
git branch -a
git tag -l
```

Expected at the end: local branches `kb/integration` and `main`; remote branches `origin/kb/integration` and `origin/main`; 19 tags. `git branch -d` refuses a branch that is not merged, which is the safety: if it refuses one, stop and say which.
