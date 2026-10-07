# 01 - Protocol: environment, fixtures, question catalogue

## 1. Environment of HAP-1


| Item                      | Value                                                                                                                                                                          |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Date and time             | 3 October 2026, 14:58 to 19:05 (workstation clock, visible in the screenshots)                                                                                                 |
| Client                    | Tauri 2 desktop app run with `pnpm tauri dev` (Windows 11), interface locale `fr-FR`                                                                                           |
| Gateway and models        | Docker Compose stack (`server` + `ollama` + `open-webui`), gateway on `http://127.0.0.1:8080`, CPU inference                                                                   |
| Chat models compared      | `gemma2:2b` and `ministral-3:3b` (the model selector at the bottom right of the chat)                                                                                          |
| Answer idle timeout       | Default 300 s, raised to 600 s in Settings from Q8-c onward                                                                                                                    |
| Documents folder          | `C:\Users\elise\AssistantCabinetAI\Docs\Test` (6 files, all analysed, 0 unreadable)                                                                                            |
| Data folder               | `C:\Users\elise\AssistantCabinetAI\Data\Test` (2 workbooks, both analysed, 0 unreadable)                                                                                       |
| Copies in this repository | `fixtures/documents/` and `fixtures/data/` (byte-identical copies)                                                                                                             |
| Conversation memory       | The in-app conversation is sent back to the model with each question (`docs/SELECTION-AND-MEMORY.md`). The conversation was cleared between sections, and in Q8-f specifically |
| Selection                 | Changed per question: the "Documents utilisés" and "Données utilisées" disclosures in the sidebar. Each screenshot shows the selection in force                                |


All fixture content is fictional (`document fictif, usage de test uniquement`). No real patient file was
used (`AGENTS.md`, rule 2 and 6).

## 2. Fixtures and ground truth

Full text and the hand computation are in [fixtures/README.md](fixtures/README.md). Summary:

**Documents** (`fixtures/documents/`)


| File                                     | Content that matters for the questions                                                                                                                                                                                                        |
| ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bail-cabinet-2024.pdf`                  | Professional lease. Duration: nine years, 1 April 2024 to 31 March 2033. Annual rent 18 400 euros                                                                                                                                             |
| `convention-remplacement-dr-martin.docx` | Dr Antoine Martin replaces Dr Camille Exemple from Monday 6 July 2026 to Friday 31 July 2026. Retrocession of 80 % of the fees collected; the remaining 20 % goes to the titular doctor. Signed 2 June 2026. **Says nothing about the lease** |
| `courrier-cpam-radiation.pdf`            | The health fund informs that M. Hugo Exemple was struck off the general scheme from 1 February 2026 (new employer). Paper forms after that date go to his new fund                                                                            |
| `devis-imprimante-medsupply.pdf`         | Quote DV-2026-0117 of 8 January 2026. One lump sum: **1 200,00 euros HT** for printer + 4 cartridges + 10 reams + consumables. **No separate printer price.** Complementary orders are invoiced separately                                    |
| `2026/janvier/neurologie.pdf`            | Letter about M. Hugo Exemple, consultation of 14 January 2026                                                                                                                                                                                 |
| `2026/mars/neurologie.pdf`               | Letter about Mme Alice Exemple, consultation of 9 March 2026. Same file name as the previous one on purpose                                                                                                                                   |


**Workbooks** (`fixtures/data/`)


| File                              | Sheet      | Columns                                 | Rows | Verified aggregates                                                                                                                                                |
| --------------------------------- | ---------- | --------------------------------------- | ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `factures-fournisseurs-2026.xlsx` | `Factures` | `date`, `fournisseur`, `montant`        | 8    | Sum 2 215; max 620; MedSupply 1 450 (3 rows); Fournitures Dupont 550; Papeterie Lefevre 215                                                                        |
| `rdv-mars-2026.xlsx`              | `RDV`      | `date`, `salle`, `patient`, `duree_min` | 10   | Mean duree_min 24.5; Mondays 3 (2, 9, 16 March); appointments 9-15 March 4; rows per room 5/3/2; total minutes per room: Salle 1 = 100, Salle 2 = 95, Salle 3 = 50 |


Neither workbook has a formula, a second sheet or a column named `cumul`.

## 3. Rules for running a pass

1. Start from a clean state: same two folders, both analysed (green dots), settings as above.
2. One section of the catalogue at a time; use "Effacer la conversation" between sections so that
  memory from an earlier section cannot influence the next (this mattered in Q8, see BUG-09).
3. Choose the selection stated in the question (it is part of the test). When a question asks for a
  single workbook, tick only that workbook, otherwise the product (correctly) asks which one.
4. Run each model-dependent question with both models and record the model name and the duration shown
  under the answer ("Généré par ... en ...").
5. Take a screenshot of the answer *and* of the sidebar selection; do not scroll the page by hand
  before the first screenshot (the scroll behaviour is itself under test, BUG-08).
6. Verify every number against the ground truth above. "The application answered" is not "the answer is
  right".
7. When something hangs, do not kill the app before copying the terminal output of `pnpm tauri dev`.



## 4. Question catalogue (28 questions) and expected behaviour

The catalogue was proposed in the chat session of 2 October 2026 ("what can this app do") and grounded in
the phrasings verified by `apps/desktop/src-tauri/tests/tabular_reference_cases.json` and
`tests/work_folder_inventory.rs`. The column "Expected" is the ground truth for these fixtures, not the
wording of the product.

### I - Documents folder, questions about the folder (deterministic, zero gateway call)


| Q   | Selection     | Question (as typed)                         | Expected                                                                     |
| --- | ------------- | ------------------------------------------- | ---------------------------------------------------------------------------- |
| Q1  | all documents | `Combien de fichiers au total ?`            | 6 files, from the filesystem, "without the AI"                               |
| Q2  | all documents | `Combien de fichiers PDF ?`                 | 5                                                                            |
| Q3  | all documents | `Peux-tu me donner la liste des fichiers ?` | The 6 file names with their analysis state                                   |
| Q4  | all documents | `Que dit neurologie.pdf ?`                  | The product asks which of the two `neurologie.pdf` files; it never picks one |




### II - Documents folder, content questions (retrieval + model, cites the page)


| Q   | Selection                                  | Question                                                         | Expected                                                                                                                                                     |
| --- | ------------------------------------------ | ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Q5  | all documents                              | `Que dit le courrier de la CPAM concernant la radiation ?`       | Hugo Exemple struck off the general scheme from 1 Feb 2026 (new employer); later paper forms go to the new fund. Cites `courrier-cpam-radiation.pdf`, page 1 |
| Q6  | all documents                              | `Résume la convention de remplacement signée avec le Dr Martin.` | Replacement 6-31 July 2026, 80 % retrocession, 20 % to the titular, signed 2 June 2026. **No lease information**                                             |
| Q7  | all documents                              | `Quelle est la durée du bail du cabinet d'après le contrat ?`    | Nine years, 1 April 2024 to 31 March 2033                                                                                                                    |
| Q8  | all documents, then the quote **unticked** | `Quel est le prix de l'imprimante dans le devis MedSupply ?<`    | With the quote: total 1 200,00 EUR HT, printer price alone not stated. Without the quote: "not found in the selected documents". Never 1 000                 |


Q8 was run in six configurations (a to f), see [02-results.md](02-results.md).

### III - Data folder, deterministic aggregates (zero gateway call)


| Q   | Selection                          | Question                                                        | Expected                                                           |
| --- | ---------------------------------- | --------------------------------------------------------------- | ------------------------------------------------------------------ |
| Q9  | both workbooks, then invoices only | `Quelle est la somme des montant ?`                             | Both ticked: asks which workbook. Invoices only: 2 215 over 8 rows |
| Q10 | invoices                           | `Quel est le maximum de montant ?`                              | 620                                                                |
| Q11 | invoices                           | `Quel fournisseur a le plus de montant ?`                       | MedSupply, 1 450 (a total per supplier, not the largest line)      |
| Q12 | invoices                           | `Quel est le total de montant par fournisseur ?`                | MedSupply 1 450, Fournitures Dupont 550, Papeterie Lefevre 215     |
| Q13 | appointments                       | `Combien de rendez-vous le lundi ?`                             | 3                                                                  |
| Q14 | appointments                       | `Combien de rendez-vous entre le 09/03/2026 et le 15/03/2026 ?` | **4** (9, 10, 11, 12 March)                                        |
| Q15 | appointments                       | `Quelle est la moyenne de duree_min ?`                          | 24,5 over 10 rows                                                  |
| Q16 | appointments                       | `Quelle salle a le moins de duree_min ?`                        | **Salle 3, total 50**                                              |
| Q17 | appointments                       | `Combien de rendez-vous par salle ?`                            | Salle 1: 5, Salle 2: 3, Salle 3: 2                                 |
| Q18 | invoices                           | `Combien de factures pour le fournisseur MedSupply ?`           | 3                                                                  |




### IV - Data folder, the model interprets and the engine computes (session 14)


| Q   | Selection | Question                                                                                  | Expected                                                                                                                                                                                                                                  |
| --- | --------- | ----------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Q19 | invoices  | `Est-ce qu'on a dépensé plus de cinq mille euros avec un seul fournisseur ce trimestre ?` | "No": the largest supplier total is 1 450 and the whole table is 2 215. The question needs a per-group threshold, which the engine's vocabulary does not have (BUG-14), so an honest "cannot answer" is acceptable, a wrong number is not |




### V - Data folder, honest refusal


| Q   | Selection | Question                                      | Expected for these fixtures                                                                                                                                                                                 |
| --- | --------- | --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Q20 | invoices  | `Quelle est la somme de cumul dans Calculs ?` | Sheet `Calculs` and column `cumul` do not exist here: a refusal naming the real sheet and columns. (The formula-refusal scenario of the original catalogue needs a different workbook; see 02-results, Q20) |
| Q21 | invoices  | `Combien de factures pour Alfa ?`             | `Alfa` matches nothing; no value is within edit distance 2, so no close value can be suggested                                                                                                              |




### VI - Documents and tables selected together, data-only question (session 15)


| Q   | Selection               | Question                            | Expected                                                                                                            |
| --- | ----------------------- | ----------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Q22 | any document + invoices | `Quelle est la somme des montant ?` | 2 215 from the engine alone, line "the selected documents were not needed", zero gateway call. **Not run in HAP-1** |




### VII - Documents and tables selected together, the mixed tier (session 16)


| Q   | Selection                                                     | Question                                                                                          | Expected                                                                                                                                        |
| --- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Q23 | quote + invoices                                              | `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans le devis signé ?` | Table 1 450 versus quote 1 200,00 HT: they differ, and the quote says later orders are invoiced separately. A combined answer citing both sides |
| Q24 | quote + invoices                                              | `Quel est le montant total pour le fournisseur mentionné dans cette lettre ?`                     | The supplier named in the document (MedSupply) is linked to the real column value and the total is 1 450                                        |
| Q25 | -                                                             | (A statement in the catalogue, not a question: a model correcting a wrong figure)                 | Cannot be triggered by the user; covered by `tests/mixed_answer.rs`                                                                             |
| Q26 | quote + invoices, **gateway stopped** (`docker compose down`) | Q23 again                                                                                         | The table part answers; a line says the document part could not be written                                                                      |




### VIII - Nothing selected, bounded general assistance


| Q   | Selection | Question                                                                            | Expected                                                                                                                                                                                                                                           |
| --- | --------- | ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Q27 | nothing   | `Rédige un e-mail de relance pour un fournisseur dont la facture n'est pas réglée.` | A drafted e-mail, labelled "Réponse sans vos documents"                                                                                                                                                                                            |
| Q28 | nothing   | `Quel est le délai légal de conservation des dossiers médicaux en France ?`         | A cautious general answer from the model, labelled as written without documents. The exact legal period must be checked against an authoritative source, the point of the test is that the product reaches the model and does not invent precision |




## 5. How to score a re-run

Copy the table of [02-results.md](02-results.md) and fill one new row per question and model. A change is a
**regression** when a question that was PASS becomes anything else, or when a FAIL becomes an answer that
is wrong in a new way. A change is **progress** when the verdict improves with no PASS lost. Always re-run
Q8 (both models, with and without the quote, with and without memory) and Q14/Q16 after any change to the
tabular classifier, because those are the cases that exposed silent wrong answers.