# Results of the human test (large test)

Played by the owner on 16 October 2026, **stopped after Part D** (Parts F, E and R not played). Expected values are in `HUMAN-TEST.md`.
The owner kept her notes in a text file, archived unchanged as `HUMAN-TEST-raw-2026-10-16.txt` beside this one (1444 lines; the original is `C:\Users\elise\Desktop\Test\Human-TEST-large-2026-10-16.txt`);
this file is the analysis of it, checked against the code. Nothing was fixed: this is a record.

**Conditions.** Branch `feat/new-readers`, `pnpm tauri dev`. Every answer written by a model says « Généré par ministral-3:3b ». Step 0.6 (which AI machine the « Réglages » point at) was
left empty. Analyse of the documents: **9 min 9 s** (expected 6 to 12). Analyse of the data: **1 min 2 s** (expected under a minute).

**Part R (restore) deliberately not run**, by the owner's choice (16 October 2026): the application keeps pointing at the test folders and the test analysis, to go on testing with them.
The backup of step 0.2 stays in `%USERPROFILE%\acai-backup`; run R1 to R3 when she wants her own state back.

## Summary

| Part | Steps | OK | Not OK | To check | Note |
| --- | --- | --- | --- | --- | --- |
| A (Analyse) | 6 | 6 | 0 | 0 | Figures as expected; remarks on the progress display (H-7, H-8) |
| B (documents) | 23 | 19 | 1 (B17) | 2 (B4, B5) | B21 could not be scored: the script puts it in the wrong place (H-4). B22 and B23 refuse correctly. |
| C (spreadsheets) | 11 | 11 | 0 | 0 | Every figure matches the expected one; refusals correct (C8, C9, C11) |
| D (mixed) | 5 | 3 | 1 (D5, first try) | 1 (D2) | D5 passes once the selection is narrowed to the two files (H-1) |

The documents answers and the deterministic engine for spreadsheets hold. The mixed selection is where the findings are (H-1, H-2, H-3).

## Part A - Analyse

| Step | Seen | OK / not OK | Note |
| --- | --- | --- | --- |
| A1, A2 | Both folders chosen. | OK | |
| A3 | 9 min 9 s. Bar read « Lecture des documents »; « Recherche par sens : x % » never seen. | OK | The owner: too long without a percentage, a time left or the file being read (H-7). She also finds « Recherche par sens » useless and not working. |
| A4 | 1251 fichiers, 1244 analysés, 3 illisibles; 91 lus par reconnaissance de texte; 2 trop peu lisibles pour être indexés; 1 .ppt not read; 396 personnes et organisations (0 déjà connue); 762 identifiants. | OK | No line « n'ont pas pu être analysés ». Matches the runner's own summary (91 OCR, 2 too faint, 3 empty, 1 `.ppt`). |
| A5 | 1 min 2 s. | OK | Not « un instant » as the card says (H-7). |
| A6 | 510 fichiers, 499 analysés, 11 illisibles; 510 tableurs lus; 435 personnes et organisations (263 déjà connues); 846 identifiants; 1154 colonnes reconnues; 5494 liens; **1247 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA**. | OK | The re-reading of the 1247 documents is probably most of the minute. |

## Part B - Documents (all of « Tous les documents », no spreadsheet)

| Step | Question | Seen | OK / not OK | Note |
| --- | --- | --- | --- | --- |
| B1 | D-003 Combien de documents ai-je ? | « 1244 fichiers ont été analysés. » Folder answer, no model. | OK | In the accepted range 1153 to 1247. |
| B2 | D-004 Combien de fichiers y a-t-il dans le dossier ? | « Votre dossier de documents contient 1251 fichiers. » | OK | |
| B3 | D-001 Quel est le dernier courrier reçu ? | 339 readable files named « courrier »; first: `scans/2026-10/2026-10-09_courrier-neurologie-dupont-jean-pierre.png`, 2026-10-09. | OK | |
| B4 | D-009 Quels documents parlent de LAMal ? | « 97 fichiers »; 50 with an extract, 47 without; 7 files without readable text not searched. First: `.../2026-01-07_demande-remboursement-luthi-zoe.pdf`. | To check | Expected 92 and a different first file. The scorer accepts a total from 92 up to 92 plus the scans (it never checks the first file); the difference is probably the scans read by text recognition (three `.jpg` among the first fifty). To confirm with the runner's report (H-6). |
| B5 | D-010 Quels documents mentionnent franchise ? | « 99 fichiers ». First: `.../2026-01-06_decompte-sanitas.pdf`. | To check | Same as B4 (92 expected). |
| B6 | D-021 Quels documents contiennent 101047437907802 ? | « 1 fichier »: `caisses-assurances/france/2026-07-02_courrier-mgen.pdf`. | OK | |
| B7 | D-086 prochain rendez-vous de Jean Dupontet | « 18 août 2026 », from `2026-05-20_courrier-compte-rendu-dupontet-jean.docx`. | OK | The first source listed is `dupont-jean.pdf`, a different person (H-5). |
| B8 | D-090 montant de la facture d'honoraires de Marie-Claire Rappaz | « 60,00 € », `patients/honoraires/2026-02-21_note-honoraires.docx`. | OK | The right file is first. |
| B9 | D-065 À qui appartient le numéro AVS 756.0029.3590.44 ? | « Jérémie Perrenoud », `2026-07-21_courrier-rhumatologie.docx`. | OK | The right file is **last** of 8 sources, after 7 documentation files (H-5). |
| B10 | D-076 total hors taxes de la facture FAC-2026-114 | « Total HT : 727,50 € », `2026-06-05_facture-garage-martin-fac-2026-114.pdf`. | OK | Third of 14 sources. |
| B11 | D-025 bon de commande BC-2026-0400 | « 4 675,50 € », `2026-05-28_transcription-preparation.vtt`. | OK | Sixth of 14 sources. |
| B12 | D-031 bon de commande BC-2026-0401 | « CHF 4'884.50 », `2026-06-17_transcription-organisation.srt`. | OK | Tenth of 14 sources. |
| B13 | D-098 hémoglobine glyquée de Thierry Baumann | « 5,5 % », `2026-09-19_resultats-analyses.rtf`. | OK | |
| B14 | D-104 montant pris en charge pour David Monnerat | « CHF 746,69 », `2026-04-07_decompte-groupe-mutuel-monnerat-david.eml`. | OK | |
| B15 | D-110 jours d'arrêt de Roy Lucas | « 10 jours », `patients/certificats/2026-04-02_arret-travail.odt`. | OK | |
| B16 | D-039 intervenant du module MOD-0105 | « Dr Martin », `2026-06-12_formation-gestion.odp`, pages 1, 2. | OK | |
| B17 | D-044 date de la session du module MOD-0102 | Three dates: « 10 avril 2026 », « 3 avril 2026 », « 7 avril 2026 », taken from three different decks; a fourth (4 mars) set aside as « hors module ». Sources: `2026-01-05_formation-chaine.pptx` pages 1, 2, 3 first. | **Not OK** | The right date is in the answer but with two wrong ones: the code MOD-0102 is issued once in the library (generator `Codes`), so it has one session. The model mixed the decks (H-6). |
| B18 | D-059 Que dit le fichier ...o-brien-sean.txt ? | A summary of the letter; « Prochain rendez-vous : 26 avril 2026 ». | OK | Only that file is cited. |
| B19 | D-049 formation à l'hygiène déclarée auprès de qui | « Ordre des médecins de la Haute-Savoie », `guide-pratique-cabinet.pdf` page 9. | OK | The guide is the **12th** of 14 sources, behind eleven hygiene decks (H-5). |
| B20 | D-129 prochain rendez-vous de Christelle Délèze | « 3 novembre 2026 », `scans/2026-08/2026-08-05_courrier-orl-deleze-christelle.jpg` (text recognition). | OK | |
| B21 | D-091 Et son prochain rendez-vous... | Asked right after B20 (as the script orders it): « 12 octobre 2026 » from a cardiology letter, « 28 octobre 2026 » from a dermatology letter, then « **Pas de courrier trouvé pour Christelle Délèze.** » | Not scored | Script defect: the book asks it after D-090 (Rappaz), the script puts it after B20 (H-4). Asked about Mme Rappaz in full, the answer is right: « 7 octobre 2026 », `2026-07-09_courrier-endocrinologie.pdf` (the owner's own check). The answer given after B20 is still a finding (H-4). |
| B22 | D-143 Zénobie Quarante | « Aucun extrait ne mentionne ... **Zénobie Quarante**. » No date, no amount. | OK | 14 patient letters listed as sources under a refusal (H-5). |
| B23 | D-144 Hyacinthe Vaudroz | « Aucun extrait ne mentionne ... Je n'ai pas trouvé de correspondance liée à cette personne. » | OK | Same remark. |

## Part C - Spreadsheets (all of « Tous les tableurs », no document; every answer « calculée ... sans l'IA »)

| Step | Question | Seen | OK / not OK | Note |
| --- | --- | --- | --- | --- |
| C1 | T-001 colonnes de compta-depenses-2026-01-site-1.xlsx | « Colonnes de la feuille Journal : Date, Libellé, Catégorie, Débit, Crédit. » | OK | |
| C2 | T-074 moyenne de Débit (.ods) | « 983,844615 », 13 lignes. | OK | Six decimals where the file has two (a display remark). |
| C3 | T-075 maximum de Montant TTC | « 1 461,85 », 10 lignes. | OK | |
| C4 | T-035 lignes avec Catégorie Frais généraux | « 17 ». « Compris comme : Catégorie = Frais généraux. » | OK | |
| C5 | T-036 lignes le mardi | « 9 ». « Compris comme : Date un mardi. » | OK | |
| C6 | T-056 Catégorie au Débit total le plus élevé | « Charges », « 11 822,12 », 3 groupes, 40 lignes. | OK | The expected 11822.1 is the same figure rounded. |
| C7 | T-009 somme de Honoraires (.csv) | « 1 088,5 », 29 lignes. | OK | |
| C8 | T-016 somme de Débit (juin, formula) | Refuses: the column holds formulas whose result cannot be checked. Lists the columns, suggests questions. | OK | |
| C9 | T-020 lignes du fichier d'août (two sheets) | Asks which sheet: « Journal, Notes ». | OK | The answer adds « Votre question précédente n'a pas pu être calculée non plus : nommez exactement l'une des colonnes ci-dessus. » : it speaks of C8 and of columns not shown (H-9). |
| C10 | T-025 somme de Débit (février, .csv) | « 27 884,57 », 31 lignes. | OK | |
| C11 | T-028 Quelle est la somme de Montant ? | Asks which workbook; a flat list of **90** `rendez-vous-...` files. | OK | A long list to scroll (H-9). |

## Part D - Mixed (« Tous les documents » and « Tous les tableurs »)

| Step | Question | Seen | OK / not OK | Note |
| --- | --- | --- | --- | --- |
| D1 | M-006 Combien de documents ai-je ? | « 1244 fichiers ont été analysés. » No model. | OK | |
| D2 | M-008 Quels documents parlent de Genève ? | Documents: « 177 fichiers », and « « Genève » est reconnu comme Genève : les fichiers qui lui sont liés sont listés avec ceux qui écrivent l'un de ses noms. » First: `comptabilite/factures/2026-04-14_facture-sig-fac-2026-142.txt`. Tables: « 43 fichiers ». | To check | Tables: 43 as expected. Documents: 161 expected, first file different; the 16 more are probably the scans and the files tied to the place name (H-6). |
| D3 | M-012 prochain rendez-vous de Pierre Thomas | « 24 octobre 2026 », `2026-07-26_courrier-rhumatologie-thomas-pierre.md` first. | OK | |
| D4 | M-013 prochain rendez-vous d'Alain Jaccard | « 10 octobre 2026 », `2026-07-12_courrier-compte-rendu.pdf` first. | OK | |
| D5 | M-001 Rédige un courrier de relance pour chaque facture du fichier ...bureautique-leman-sa.xlsx | **First try, everything ticked:** no plan. Six letters written by the model, then « Factures non trouvées dans les extraits : Aucun document ne mentionne une facture fournisseurs-2026-t1-bureautique-leman-sa.xlsx ». Under it: « Cette question ne portait sur aucune de vos tables. » **Second try, only `modeles/modele-relance-facture.txt` and the one workbook selected:** « J'ai trouvé le modèle ... et les lignes du tableau à y insérer. Vérifiez la correspondance des champs et l'aperçu : rien n'est écrit tant que vous n'avez pas validé. » The card « Courrier à générer » shows the template, « Un courrier par : chaque ligne », the field mapping (Date, Libellé, Montant TTC, Statut) and a preview; « Aucune IA n'a lu vos données ». | First try not OK; second try OK | The sentence was right; the selection was the problem (H-1). In the preview (screenshot): « Nous n'avons pas encore reçu le règlement de la facture du 2026-02-12 (Livraison), d'un montant de 73.83. Son statut est : à payer. » |

## Findings

Numbered for this test; none fixed. The code references are the ones read to answer the owner's questions.

**H-1. The mail merge needs a narrow selection, and the test told her to select everything (important).**
Test script defect: D5 follows D0 (« Tous les documents » and « Tous les tableurs »). The book itself asks M-001 with the template and the one workbook selected (`book_mixed.py`, `fill`).
In the code, `commands::fill_tier` calls `tabular_answer::locate_fill_source`, which keeps a workbook only if it qualifies and then insists that **exactly one** qualifies. With « chaque » in the question
("every row" is enough to qualify), every selected workbook with a single sheet qualifies, so hundreds do and nothing is picked. The workbook **named in the question is not used to choose**.
`fill_tier` then gives up and the question goes on to the mixed tier, where a model writes letters from the invoice documents. The model-written letters carry payment deadlines that nothing in the question or the plan gave
(« avant le 14 juillet 2026 », « 14 juin », « 14 novembre »; to check against the excerpts), contradict themselves (« Aucun document ne mentionne ... ») and raise no warning on a figure. The line « Cette question ne portait sur aucune de vos tables »
is misleading here: the question named a table.
To record for the owner's decision: a workbook named exactly in the question could win over the rest of the selection, as it already does elsewhere (`names_a_workbook`: « a question that names a workbook is never narrowed »).
**Status (24 October 2026): closed by the TABULAR series.** Step 08 (`fix/named-file-wins`): the named workbook wins and is the only one opened (`FillLocation`), and a merge the program cannot prepare is asked about, never written by a model; step 09: with no template that fits, the template is drafted from the column names alone and shown for her to edit. Case H of the series plan. See `docs/DECISIONS.md`, "Settled by the TABULAR series, wave 1", rows W-4 and W-5.

**H-2. In a mixed selection the model never reads a table, even one named exactly (important).**
Her pasted case: the question is only `factures-fournisseurs-2026-t1-bureautique-leman-sa.xlsx`, everything ticked. The answer lists invoices of that supplier, all from `comptabilite/factures/*.docx|pdf` and `comptabilite/devis/*`, and says « Cette question ne portait sur aucune de vos tables ».
This is the designed behaviour (`docs/SELECTION-AND-MEMORY.md`, « Documents and tables together »: « a question with no recognisable data component at all answers from the documents alone »):
- the table engine only acts when it reads an **operation** in the question (sum, count, list of the values of a column, group with the largest total...). A file name alone is none;
- `mixed_answer.rs`: the model receives « Document excerpts » (retrieved from the documents) and « Table results » (one finished figure from the engine), never a cell or a row (`docs/ARCHITECTURE.md`, line 362);
- the search by meaning (retrieval) covers the documents only: workbooks are read by the engine, not cut into passages. So the sources can only be documents.
It is not a filter and not a full context: nothing from the table was ever put in front of the model. The same question with spreadsheets only goes to the table engine instead, which cannot read an operation in it either and would answer with the file's columns and example questions, as in C8 (not tried; read from the code).
Open decision: what a mixed question that names a workbook without an operation should do (say so plainly and offer the engine's examples, rather than answering from the documents about the same supplier).
**Status (24 October 2026): closed by the TABULAR series.** Steps 02 and 03: the card of every named or implied workbook reaches the writer (structure only), so a named workbook with no operation is read and the misleading line is gone; step 03b adds a controlled sample and the engine's profile, step 04 the usage trace (a named file never disappears without a line). Case C of the series plan. See `docs/DECISIONS.md`, rows W-1 to W-3.

**H-3. The routes towards the model after the deterministic engine (answer to « are documents priority? »).**
Order in a mixed selection (`commands::sourced_answer`): 1) « Demander à l'IA » goes straight to the mixed tier; 2) a request to write a letter with one table and a template: a plan (`fill_tier`);
3) a question about the folder itself (« Combien de documents ai-je ? », a list « parlent de X »): the folder engines, no model (`mixed_folder_tier`); 4) a question clearly and only about the data: the table engine alone, no model (`mixed_data_only_tier`);
5) everything else: the mixed tier. There the documents are **always** searched; the table takes part only if an operation is recognised, and then only as one computed value; with no value, nothing of the table reaches the model.
The model is opened up in three places, and always for the words, never for the numbers: the hidden interpreter that proposes a query plan when the engine cannot read the question (the plan is checked and run by the engine); the written answer of the mixed tier
(held to the table's figure and the excerpts afterwards, corrections appended); the letters of the plan, which the program fills with no model. The document share of the mixed turn is capped (`MIXED_DOCUMENT_SHARE_CHARS`, 4000 characters at the default window, scaled with the model's window).
So the documents are the only text source: « priority » is right in that sense; « saturated context » is not the cause.

**H-4. Follow-up question: wrong person, and contradicting itself.**
B21 as scripted follows B20 (Délèze). The model answered with the appointments of two other people, then said no letter was found for Délèze, while the previous answer had just found hers (3 novembre).
Known: `REPORT.md`, section 3, defect 5 (« the follow-up question brings the wrong person's letter 4 times of 6 »). **Script defect to correct:** B21 must come right after B8 (Rappaz), where the book puts it; asked there in full, it is right.

**H-5. Sources do not show which one the answer came from.**
Sources are listed as the passages the model was given, not the one it used: the right file is first in B8, third in B10, last in B9 (after seven documentation files) and twelfth in B19; under the refusals B22 and B23 fourteen unrelated letters are listed.
A reader can take them for evidence. Presentation, not a wrong answer.

**H-6. Counts and first files of the word searches differ from the script; one wrong answer on a deck.**
B4, B5 and D2: the counts are above the script's (97 and 99 against 92; 177 against 161). The scorer accepts the expected total plus the scans read by text recognition and never checks which file comes first; the script's « first of them » is not a requirement.
To confirm with the runner's report. D2 also lists the files tied to the place name (« Genève est reconnu comme Genève »). Suggest dropping « first of them » from the script for lists.
B17: the model attributed three dates to one module code that exists once; known defect 2 of `REPORT.md` (decks) in another form: the passage was found this time, the model mixed four decks of the same family (ministral-3:3b).

**H-7. Analyse display.**
Owner: 9 min 9 s with no percentage, time left or file being read; the data Analyse (1 min 2 s) is not « un instant ». « Recherche par sens : x % » was never seen and the owner wants it removed
(it is shown only when the progress carries a vector percentage: `WorkFolderCard.tsx`, `progressMeaningLabel` and `meaningShare`; why it never appeared is not investigated). The data Analyse also re-reads the 1247 documents for names, which is likely most of its minute.

**H-8. The data card changed between A6 and the screenshot.** A6: 510 fichiers, 499 analysés, 11 illisibles. Screenshot (later): **511** fichiers, 499 analysés, **12** illisibles. One file appeared in `Data` and is unreadable. Not investigated (a temporary file of a spreadsheet program is the first suspect).

**H-9. Small message defects (spreadsheets).** C9 repeats a sentence about the previous question and about columns it does not show; C11 offers 90 workbooks in one flat list; C2 prints six decimals.

**H-10. Script defects to correct in `HUMAN-TEST.md`** (not edited: the owner has not finished the test): B21 order (H-4); D5 must say to narrow the selection to the template and the workbook, by the search of each card, before asking (H-1);
« first of them » on lists (H-6); A3 announces a « Recherche par sens » bar that was not seen (H-7).

## Open discussion (16 October 2026, nothing decided, nothing coded)

The owner's idea: in a mixed selection, when a workbook is **named explicitly** in the question, the deterministic engine should take the table side first, even when the question asks for no calculation;
and routing by the content of the question and the types of the selected files could trigger targeted answers, as the mail merge already does (recognise, locate the sources, plan or preview, confirm).

Options put on the table, from the smallest: (1) a workbook named in the question wins the table side everywhere, mail merge included (H-1); (2) a named workbook with no operation read gets the engine's
orientation card (columns, row count, example questions) instead of an answer from the documents; (3) for a vague request on a named workbook, the program itself shows a preview of the first rows and totals per column
(the model, if used, only writes from finished figures); (4) an explicit routing table, as data, crossing the signals of the sentence (workbook named, document named, writing verb, calculation word, person name) with the selected file types,
checked by the `routes` command on the 300 questions plus new cases.
Safeguards named: a sentence that names a workbook **and** a document stays mixed; an ambiguous name still asks which; « Demander à l'IA » stays the way out.

Questions left open for the owner: (a) may rows of a table go to the model (today no; `docs/ARCHITECTURE.md`, and the model runs on the AI machine, not the workstation) — showing them to her is not the same decision;
(b) orientation card, row preview, or both; (c) order of work; (d) exact name only, or also a partial name (the code already handles a partial one and asks which when several match).

## Not played

Part F (controls, optional), Part E (the runner on a model: E1 to E4) and Part R (restore). The 300-question runner results of `REPORT.md` (248 of 300 with ministral-3:3b) are the only measure of the book so far.
