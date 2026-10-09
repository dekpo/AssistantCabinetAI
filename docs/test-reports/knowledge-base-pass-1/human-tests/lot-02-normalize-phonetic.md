# Human test - lot 2, Names, accents and sounds

Written for the owner, who runs it. English, with the French strings of the interface quoted as data.

| | |
| --- | --- |
| Branch to be on | `feat/kb-normalize-phonetic` (pull request #22) |
| Build | Part A: none (`cargo test` only). Part B: `cd apps\desktop` then `pnpm tauri dev` |
| Services | Part A: none. Part B: `docker compose up -d` from the repository root, chat and embedding models loaded, as for any pass |
| Duration | Part A about 15 minutes. Part B about 60 to 75 minutes in full (about 30 minutes for the steps marked with a star, the essential subset), most of it waiting for the model |
| Visible in the application? | **No.** Lot 2 is pure functions: nothing reads them yet (the extractors of lot 4 and the resolver of lot 3 will). There is no screen, no setting and no command to click. Part A lets you **read and edit the table of names the encoder treats as the same sound**. Part B replays the whole existing product on your two folders, because the one existing function lot 2 touched (`fold_text`) is used by the file-name matching |

## What this proves, and what it does not

It proves two things. First, that the way the product has always lowercased and de-accented a file name (`fold_text`) is now the same code in a new home and gives the same answer on every file name of the fixture folders (a unit test compares the old body with the new one), and that the product, replayed on your own folders, still gives the answers it gave before. Second, that the **French phonetic key** puts "Dupont", "Dupond" and "Dupon" on one key and keeps "Dupont" and "Dumont", "Martin" and "Martine", "Simon" and "Simone" apart, as you decide in `phonetic-fr.json`.

It does not prove that names are found in your documents (lot 4), that two spellings are linked to one person (lot 3 and later), or anything about speed. **The agent ran every `cargo test` of Part A and read the table. It did not open the application window**, so Part B is yours to confirm.

## Part A - the table of sounds (15 minutes, no application, no model)

**Where you work.** Two things open at once:

- **Cursor**, with the file `apps\desktop\src-tauri\tests\fixtures\knowledge\phonetic-fr.json` open (it is versioned; it is the only file you edit in this part).
- **A PowerShell window** in the folder `C:\Users\elise\Documents\CURSOR\AssistantCabinetAI\apps\desktop\src-tauri` (in Cursor: Terminal, New Terminal, then `cd apps\desktop\src-tauri`). Every command below is typed there.

**What the file looks like.** Near the top you will see `"collide": [` followed by one line per group of names that must share one sound, for example `["Dupont", "Dupond", "Dupon"],`; further down `"differ": [` (pairs that must **not** share a sound) and `"observe": [` (names that are only printed, never checked). Each line ends with a comma except the last of its list.

### A1 - Run the test and read the table

Type: `cargo test --test knowledge_names -- --nocapture`

Expected (you did this one, it matched): `test result: ok. 22 passed; 0 failed`, and in the middle a table starting `French phonetic key (fr-rules:1), 72 names, 50 keys`.

### A2 - Read the first block ("names that sound alike")

Nothing to type. In the output of A1, find the first block. Expected lines, among others: `DYPo  Dupon = Dupond = Dupont`, `LEFEVR  Lefebvre = Lefevre = Lefèvre`, `TIBO  Thibault = Thibaut = Tibo`, `FILI  Filip = Philippe`, `DYRa  Durand = Durant`, `DYBUA  Dubois = Duboit`, `MIXEL  Michel = Michelle`, `aRI  Henri = Henry`, `FRaSUA  Francois = François`.
**Your decision, not a bug:** for each line, would you accept that the product offers the second name as "did you mean?" when it meets the first? If not, write down the line; we move it to `"differ"` (see A5).

### A3 - Read the second block ("names with a key of their own")

Nothing to type. Expected: `DYMo  Dumont` is not on the line of Dupont; `MARTe  Martin` and `MARTIN  Martine` are two lines; `SIMo  Simon` and `SIMON  Simone` are two lines; `LORa  Laurent` and `LORaS  Laurence` are two lines; `FRaSUAZ  Francoise = Françoise` is a different line from `FRaSUA  Francois = François`.

### A4 - Add a row that must pass

1. In Cursor, in `phonetic-fr.json`, click at the end of the line `"collide": [`, press Enter, and type this line exactly (4 spaces first): `    ["Marchand", "Marchant"],`
2. Save (Ctrl+S).
3. In PowerShell type: `cargo test --test knowledge_names -- --nocapture`

Expected: still `22 passed; 0 failed`; the title line now says `74 names`; and in the first block of the table a new line with `Marchand = Marchant` (the key is `MARXa`; what matters is that both names are on the same line). It does not recompile the program, only the tests: it is quick.

### A5 - Add a row that must fail, and see the test name it

1. In `phonetic-fr.json`, click at the end of the line you added in A4, press Enter, type: `    ["Dupont", "Dumont"],`
2. Save.
3. In PowerShell type: `cargo test --test knowledge_names the_phonetic_contract_holds`

Expected: `test result: FAILED. 0 passed; 1 failed` and the message `phonetic-fr.json is not respected:` followed by `should collide: Dupont = DYPo, Dumont = DYMo`. That is the test naming your wrong row: the contract is read from the file. If the test passes instead, that is a bug.

### A6 - Put the file back

1. In PowerShell type: `git restore tests/fixtures/knowledge/phonetic-fr.json` (this throws away your two edits and puts the committed file back; nothing else is touched).
2. Type: `git status` - expected: `phonetic-fr.json` is **not** listed as modified (the documents of this lot are, that is normal).
3. Type: `cargo test --test knowledge_names` - expected: `22 passed; 0 failed`.

### A7 - Try three names of your own

1. In `phonetic-fr.json`, click at the end of the line `"observe": [`, press Enter, type: `    "Marchetti", "Delmas", "Vidal",`
2. Save.
3. In PowerShell type: `cargo test --test knowledge_names -- --nocapture`

Expected: the title line now says `75 names`; `Marchetti`, `Delmas` and `Vidal` each appear in the table, each with a key made of letters (not empty, no accented letter in the key). If two of them share a key they are on a line of the first block. Invent others if you want; use invented or very common names only, never the name of a real person you know.
4. Put the file back as in A6 (`git restore tests/fixtures/knowledge/phonetic-fr.json`).

### A8 - The small tests beside the code

In PowerShell type: `cargo test --lib -- knowledge::normalize knowledge::phonetic fold_text`

Expected: `test result: ok. 16 passed; 0 failed`. They include the check that the old `fold_text` and the new one give the same output on every file and folder name of the fixture folders, plus accents written in both ways.

## Part B - non-regression battery on your two test folders

**Why this battery exists.** `file_reference::fold_text` decides that "BAIL-CABINET-2024" means the file `bail-cabinet-2024.pdf`. A test proves its output is unchanged, but the owner wants the whole product replayed on her own fictional folders, with every result written down, good or bad. Every expected value below was **checked by the agent against the files themselves** (the full text of every file is in `docs/test-reports/human-acceptance-pass-1/fixtures/README.md`) **and, for every question answered without the AI, by running the product's own engine and folder router on a copy of your two folders** (nothing written, nothing sent to a model). The answers of the model (blocks C, M1, M2, N1) cannot be fixed to the word: for those the criteria are the facts and the sources.

### Your two folders

| Folder | Content | Counts |
| --- | --- | --- |
| `C:\Users\elise\AssistantCabinetAI\Docs\Test` | `bail-cabinet-2024.pdf`, `courrier-cpam-radiation.pdf`, `devis-imprimante-medsupply.pdf`, `2026\janvier\neurologie.pdf`, `2026\mars\neurologie.pdf`, `convention-remplacement-dr-martin.docx`, `modele_lettre.docx`, `modele_lettre.txt` | 8 files: 5 PDF, 2 DOCX, 1 TXT |
| `C:\Users\elise\AssistantCabinetAI\Data\Test` | `factures-fournisseurs-2026.xlsx` (sheet `Factures`, 8 rows), `rdv-mars-2026.xlsx` (sheet `RDV`, 10 rows), `publipostage\donnees_publipostage.csv` (5 rows), `publipostage\modele_lettre.docx`, `publipostage\modele_lettre.txt` | 5 files: 3 tables, **2 unreadable** (the template) |

### How to select files (left panel) - read once, used by every step

- The **Documents** card has a line `Documents utilisés : ...`. Click that line to open the list: a box **Tous les documents** at the top, then one box per file. The **Data** card works the same with `Données utilisées : ...`, a box **Tous les tableurs**, then one box per table.
- The box at the top: if everything is ticked, clicking it unticks everything (the line then reads `aucun` / `aucune`); in any other state, clicking it ticks everything (`tous` / `toutes`).
- To select **exactly some files**: first bring the list to *none* (click the top box until the line reads `Documents utilisés : aucun`, or `Données utilisées : aucune`), then tick the files wanted.
- **Check the line after every selection.** It must read exactly what the step says: `Documents utilisés : tous`, `Documents utilisés : aucun`, `Documents utilisés : 1 fichier`, `Documents utilisés : 7 fichiers`; for data `Données utilisées : toutes`, `Données utilisées : aucune`, `Données utilisées : 1 fichier`.
- **A selection stays until you change it.** The selection of the previous step is still there when you start the next one. Every step therefore gives *both* lines (documents and data); change only what differs.
- **Before every step: click "Effacer la conversation"** (in the chat) and confirm. The conversation is sent back to the model with each question; an earlier answer has already contaminated a later one once (HAP-1, BUG-09). The only exception is a step that says "continue".

### Rules of the battery

1. **One model for the whole battery.** Write its name at the top of what you send back (selector at the bottom right of the chat).
2. Take nothing for granted from "the application answered": compare the number or the fact to **Expected**.
3. Verdicts: **PASS** (matches); **PASS-WITH-ISSUES** (right facts, a defect of wording or display); **FAIL** (a wrong number, a wrong file, an invented fact); **NOT-RUN**. A step answered without the AI (blocks D, T and M3) must match **exactly**. A model answer passes on its facts and sources; its phrasing is free.
4. **A step is a regression of lot 2 only if it worked before.** If a step fails, do not conclude anything: send it to me and I will say whether lot 2 can be the cause (only D5 to D11, the file-name steps, can). To settle it you can re-run that single step on `kb/integration` (`git switch kb/integration`, back up and restore your two files as in the preparation, `pnpm tauri dev`) and compare.
5. Steps marked with a star are the essential subset (about 30 minutes). Run all of them if you have the time.
6. **Defects found are recorded** in `docs/test-reports/knowledge-base-pass-1/defects-register.md` (rule of 9 October 2026): good or bad, nothing is lost.

### How to send me the results

Paste the conversations **straight into the chat**, block by block (block D, then block C, ...). For each step, put the step number (`D5`) before the pasted text. The text must contain: the question, the answer, the line `Généré par ... en ...` when there is one, and the **sources** shown under the answer (open the "Sources" disclosure; if the copy does not include them, write them under the paste or send a screenshot of the answer, which can also be pasted into the chat). For the blocks D and T, add, if you can read it, the sentence under the answer such as `sans l'IA` or `Calculé dans vos données`. Say the model's name once. I check each step against this table and write the verdicts, with the defects, into `human-tests/lot-02-results.md`.

### Preparation of Part B

1. Close the application. Back up (PowerShell):

```text
$backup = "$env:USERPROFILE\kb-backup-lot-02"
New-Item -ItemType Directory -Force $backup
Copy-Item "$env:LOCALAPPDATA\com.assistantcabinetai.desktop\index.sqlite3" $backup
Copy-Item "$env:APPDATA\com.assistantcabinetai.desktop\settings.json" $backup
```

2. `docker compose up -d` from the repository root, then `cd apps\desktop` and `pnpm tauri dev`.
3. Settings: Documents folder = `C:\Users\elise\AssistantCabinetAI\Docs\Test`, Data folder = `C:\Users\elise\AssistantCabinetAI\Data\Test`.
4. Press **Analyser** on the Documents card, then on the Data card.

| # | Expected | If you see ... it is a bug |
| --- | --- | --- |
| P1 | Documents card: 8 files analysed, nothing unreadable | an error; a file listed unreadable |
| P2 | Data card: 3 tables analysed (the invoices, the appointments, `donnees_publipostage.csv`) and 2 files unreadable (the template `.docx` and `.txt`, as before the lot) | the invoices or the appointments unreadable |

### Block D - the folder of documents, answered without the AI

For D1 to D4 there is no `Généré par` line and the answer says `sans l'IA`.

**D1 ★**
- Documents: **tous** (line to see: `Documents utilisés : tous`). Data: **aucune** (`Données utilisées : aucune`).
- Question: `Combien de fichiers au total ?`
- Expected: **8** fichiers, "sans l'IA".
- Bug if: 6, or a sentence written by a model.

**D2**
- Documents: tous. Data: aucune.
- Question: `Combien de fichiers PDF ?`
- Expected: **5** fichiers avec l'extension .pdf.
- Bug if: any other number.

**D3**
- Documents: tous. Data: aucune.
- Question: `Combien de fichiers docx ?`
- Expected: **2**.
- Bug if: any other number.

**D4 ★**
- Documents: tous. Data: aucune.
- Question: `Peux-tu me donner la liste des fichiers ?`
- Expected: the **8** paths, each marked analysé: `2026/janvier/neurologie.pdf`, `2026/mars/neurologie.pdf`, `bail-cabinet-2024.pdf`, `convention-remplacement-dr-martin.docx`, `courrier-cpam-radiation.pdf`, `devis-imprimante-medsupply.pdf`, `modele_lettre.docx`, `modele_lettre.txt`.
- Bug if: a file is missing or not marked analysé.

**D5 ★** (this step continues after a click)
- Documents: tous. Data: aucune.
- Question: `Que dit neurologie.pdf ?`
- Expected: the product **asks which one**: two choices, `2026/janvier/neurologie.pdf` and `2026/mars/neurologie.pdf`. It never picks one. **Then click the January one**: an answer about **M. Hugo Exemple**, consultation of **14 January 2026**, tension headaches, no imaging needed. The only source is the January file.
- Bug if: it answers without asking; the March content appears; a source from another file.

**D6 ★**
- Documents: tous. Data: aucune.
- Question: `Que dit 2026/mars/neurologie.pdf ?`
- Expected: an answer about **Mme Alice Exemple**, consultation of **9 March 2026**, paraesthesias of the upper limbs for three months, an electromyogram proposed. The only source is `2026/mars/neurologie.pdf`.
- Bug if: the January content; two sources.

**D7**
- Documents: tous. Data: aucune.
- Question: `Que dit modele_lettre ?`
- Expected: the product **asks which one**: `modele_lettre.docx` or `modele_lettre.txt`.
- Bug if: it picks one silently.

**D8 ★**
- Documents: tous. Data: aucune.
- Question: `Que dit BAIL-CABINET-2024 ?` (capitals, no extension)
- Expected: an answer from **`bail-cabinet-2024.pdf` only**: nine years from 1 April 2024 to 31 March 2033, annual rent 18 400 euros, 14 rue des Tilleuls, Lyon. The only source is that file.
- Bug if: sources from other documents; "plusieurs fichiers possibles"; "aucun fichier".

**D9 ★**
- Documents: tous. Data: aucune.
- Question: `Que dit courrier-cpam-radiation ?` (no extension)
- Expected: an answer from **`courrier-cpam-radiation.pdf` only**: M. Hugo Exemple struck off the general scheme from 1 February 2026. Only that source.
- Bug if: another file as source.

**D10**
- Documents: tous. Data: aucune.
- Question: `Que dit le fichier convention-remplacement-dr-martin.docx ?`
- Expected: an answer from **the convention only**: Dr Antoine Martin replaces Dr Camille Exemple from 6 to 31 July 2026; 80 % retrocession; signed 2 June 2026. Only that source.
- Bug if: a mention of the lease; two sources.

**D11**
- Documents: tous. Data: aucune.
- Question: `Que dit modele_lettre.txt ?`
- Expected: an answer from **`modele_lettre.txt` only**: a model letter confirming an order, with fields such as `[No_Commande]`, `[Civilite]`, `[Prenom]`, `[Nom]`, `[Article]`. Only that source.
- Bug if: the `.docx` as source.

### Block C - content questions through the AI

All of block C: **Data: aucune** (`Données utilisées : aucune`). A real answer from the model, with sources. Pass on **facts and sources**. A refusal in C7 and C10 is the correct behaviour.

**C1 ★**
- Documents: tous. Data: aucune.
- Question: `Que dit le courrier de la CPAM concernant la radiation ?`
- Expected: **Hugo Exemple** struck off the general scheme **from 1 February 2026**, new employer; later paper forms go to the new fund. Source `courrier-cpam-radiation.pdf`, page 1.
- Bug if: another file as the main source; another date.

**C2**
- Documents: tous. Data: aucune.
- Question: `Résume la convention de remplacement signée avec le Dr Martin.`
- Expected: replacement **6 to 31 July 2026**; retrocession **80 %**; signed **2 June 2026**; source the `.docx`. The remaining **20 %** to the titular is expected too, but a small model sometimes forgets it: PASS-WITH-ISSUES, not FAIL.
- Bug if: any statement about the lease or its duration.

**C3 ★**
- Documents: tous. Data: aucune.
- Question: `Quelle est la durée du bail du cabinet d'après le contrat ?`
- Expected: **nine years**, from **1 April 2024 to 31 March 2033**. Source `bail-cabinet-2024.pdf`.
- Bug if: another duration; the convention as source.

**C4**
- Documents: tous. Data: aucune.
- Question: `Quel est le loyer annuel du bail du cabinet ?`
- Expected: **18 400 euros** per year, payable monthly. Source `bail-cabinet-2024.pdf`.
- Bug if: another amount.

**C5**
- Documents: tous. Data: aucune.
- Question: `Qui est le bailleur du cabinet ?`
- Expected: **M. Jean Fontaine** (bailleur); the tenant is Mme Camille Exemple. Source the lease.
- Bug if: another name.

**C6 ★**
- Documents: tous. Data: aucune.
- Question: `Quel est le prix de l'imprimante dans le devis MedSupply ?`
- Expected: the quote gives **one total, 1 200,00 euros HT**, for printer + cartridges + paper + consumables; **the printer alone is not priced**. Source `devis-imprimante-medsupply.pdf`.
- Bug if: **1 000** or any invented printer price.

**C7 ★**
- Documents: **7 fichiers**: bring the list to *aucun*, then tick everything **except** `devis-imprimante-medsupply.pdf` (line to see: `Documents utilisés : 7 fichiers`). Data: aucune.
- Question: `Quel est le prix de l'imprimante dans le devis MedSupply ?` (the same as C6; clear the conversation first)
- Expected: an honest "not found in the selected documents". No price.
- Bug if: any price. An unrelated excerpt pasted before the refusal is PASS-WITH-ISSUES.

**C8**
- Documents: tous (tick the box at the top again). Data: aucune.
- Question: `Quel est le numéro du devis de MedSupply ?`
- Expected: **DV-2026-0117**, dated 8 January 2026. Source the quote.
- Bug if: another number.

**C9 ★**
- Documents: tous. Data: aucune.
- Question: `Quelle consultation a eu lieu le 9 mars 2026 ?`
- Expected: the neurology letter about **Mme Alice Exemple** (paraesthesias, electromyogram). Source **`2026/mars/neurologie.pdf` only**.
- Bug if: the January letter; Hugo Exemple.

**C10 ★**
- Documents: tous. Data: aucune.
- Question: `Quel est le montant de l'assurance responsabilité civile du cabinet ?`
- Expected: **not in the documents**: the product says the documents do not carry the answer. No amount.
- Bug if: any invented amount.

### Block T - the tables, answered by the engine without the AI

All of block T: **Documents: aucun** (`Documents utilisés : aucun`). No `Généré par` line: the answer is computed by the engine, with the number of rows and, when a filter was understood, a "Compris comme" clause. Tick only the table named; the CSV is listed as `publipostage/donnees_publipostage.csv`.

**T1 ★**
- Documents: aucun. Data: **1 fichier** = `factures-fournisseurs-2026.xlsx` (line to see: `Données utilisées : 1 fichier`).
- Question: `Quelle est la somme des montant ?`
- Expected: **2 215**, on **8** lignes.
- Bug if: any other number.

**T2 ★**
- Documents: aucun. Data: invoices only.
- Question: `Quel est le maximum de montant ?`
- Expected: **620**.
- Bug if: any other number.

**T3**
- Documents: aucun. Data: invoices only.
- Question: `Quel fournisseur a le plus de montant ?`
- Expected: **MedSupply**, **1 450** (a total per supplier, not the largest line).
- Bug if: 620; another supplier.

**T4 ★**
- Documents: aucun. Data: invoices only.
- Question: `Quel est le total de montant par fournisseur ?`
- Expected: Fournitures Dupont **550**, MedSupply **1 450**, Papeterie Lefevre **215**.
- Bug if: any other figure.

**T5 ★**
- Documents: aucun. Data: invoices only.
- Question: `Combien de factures pour le fournisseur MedSupply ?`
- Expected: **3**, with the clause fournisseur = MedSupply.
- Bug if: 8; any other number.

**T6**
- Documents: aucun. Data: invoices only.
- Question: `Combien de factures le 22/01/2026 ?`
- Expected: **1** (the invoice of 22 January).
- Bug if: any other number.

**T7 ★**
- Documents: aucun. Data: invoices only.
- Question: `Combien de factures pour Alfa ?`
- Expected: a **refusal**: no supplier called Alfa; it lists the real columns `date`, `fournisseur`, `montant`. **No number.**
- Bug if: a count; a suggested supplier.

**T8**
- Documents: aucun. Data: invoices only.
- Question: `Quelle est la somme de fournisseur ?`
- Expected: a refusal: the column `fournisseur` is not numeric; it suggests `montant`.
- Bug if: a sum.

**T9 ★**
- Documents: aucun. Data: **1 fichier** = `rdv-mars-2026.xlsx` only (untick the invoices; line to see: `Données utilisées : 1 fichier`).
- Question: `Combien de rendez-vous le lundi ?`
- Expected: **3** (2, 9 and 16 March).
- Bug if: any other number.

**T10 ★**
- Documents: aucun. Data: appointments only.
- Question: `Combien de rendez-vous entre le 09/03/2026 et le 15/03/2026 ?`
- Expected: **4**, with "Compris comme : date entre 2026-03-09 et 2026-03-15" (9, 10, 11, 12 March).
- Bug if: **0** (the old defect) or any other number.

**T11**
- Documents: aucun. Data: appointments only.
- Question: `Quelle est la moyenne de duree_min ?`
- Expected: **24,5**, on **10** lignes.
- Bug if: any other number.

**T12 ★**
- Documents: aucun. Data: appointments only.
- Question: `Quelle salle a le moins de duree_min ?`
- Expected: **Salle 3, 50** (ranking: Salle 3 = 50, Salle 2 = 95, Salle 1 = 100).
- Bug if: "Salle 2, 30" (the old defect).

**T13**
- Documents: aucun. Data: appointments only.
- Question: `Quelle salle a le plus de duree_min ?`
- Expected: **Salle 1, 100**.
- Bug if: another room.

**T14**
- Documents: aucun. Data: appointments only.
- Question: `Combien de rendez-vous par salle ?`
- Expected: Salle 1 **5**, Salle 2 **3**, Salle 3 **2**.
- Bug if: any other figure.

**T15 ★**
- Documents: aucun. Data: **toutes** (tick the box **Tous les tableurs**: the three tables; line to see: `Données utilisées : toutes`).
- Question: `Quelle est la somme des montant ?`
- Expected: **2 215** straight away, from `factures-fournisseurs-2026.xlsx` (the only table with a `montant` column); no question about which table.
- Bug if: a question "which workbook?"; another number.

**T16 ★** (this step continues after clicks)
- Documents: aucun. Data: toutes (as in T15).
- Question: `Combien de lignes ?`
- Expected: the product **asks which workbook**, one button per table (3). Click `factures-fournisseurs-2026.xlsx`: **8** lignes. Ask the same question again and click `rdv-mars-2026.xlsx`: **10**. Ask again and click `donnees_publipostage.csv`: **5**.
- Bug if: no question asked; a wrong count.

### Block M - documents and tables together

**M1 ★**
- Documents: **1 fichier** = `devis-imprimante-medsupply.pdf` (line to see: `Documents utilisés : 1 fichier`). Data: **1 fichier** = `factures-fournisseurs-2026.xlsx`.
- Question: `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans le devis signé ?`
- Expected: a written answer that sets the tables' **1 450** (3 lignes, fournisseur = MedSupply) against the quote's **1 200,00 euros HT**, citing both sides; under it a block "Calculé dans vos données (par le moteur, pas par l'IA)" showing 1 450.
- Bug if: only one side; an invented figure; 2 215.

**M2 ★**
- Documents: the quote only. Data: invoices only (same as M1; clear the conversation).
- Question: `Quel est le montant total pour le fournisseur mentionné dans cette lettre ?`
- Expected: the supplier named in the quote (**MedSupply**) is linked to the table and the total is **1 450** ("Somme de montant : 1 450. Calculé sur 3 lignes. Compris comme : fournisseur = MedSupply").
- Bug if: **2 215**; "la colonne n'est pas numérique".

**M3 ★**
- Documents: **1 fichier** = `bail-cabinet-2024.pdf` only. Data: invoices only.
- Question: `Quelle est la somme des montant ?`
- Expected: **2 215** at once, with no model (`Généré par` absent), and a line saying the selected documents were not needed.
- Bug if: a model-written answer; another number.

### Block N - nothing selected

**N1 ★**
- Documents: **aucun** (`Documents utilisés : aucun`). Data: **aucune** (`Données utilisées : aucune`).
- Question: `Rédige un e-mail de relance pour un fournisseur dont la facture n'est pas réglée.`
- Expected: a drafted e-mail, labelled "Réponse sans vos documents".
- Bug if: "Aucun document sélectionné" and no draft.

### Not in this battery, on purpose

- The aggregates of `donnees_publipostage.csv` on the column `Total_Ligne`: defect KBD-01 of the defects register (a sum or a maximum of that column is answered as a row count of 5). It is recorded and deferred; do not spend a step on it.
- `Que dit mars/neurologie.pdf ?` (a partial path): KBD-02, recorded and deferred.
- Names, targeting, speech: they do not exist yet (lots 4 onwards).

## Results table

Filled by the agent from what you paste, in `human-tests/lot-02-results.md`. This table is your checklist.

| Step | Model | Expected (see above) | Observed | Verdict |
| --- | --- | --- | --- | --- |
| A1 to A8 | n/a | see Part A | | |
| P1, P2 | n/a | 8 files; 3 tables + 2 unreadable | | |
| D1 to D11 | | | | |
| C1 to C10 | | | | |
| T1 to T16 | | | | |
| M1 to M3 | | | | |
| N1 | | | | |

## Findings

A bug found here is fixed on `feat/kb-normalize-phonetic` (a new commit, the pull request updated) before the merge block is used. A defect that exists before lot 2 (it also happens on `kb/integration`) is not a reason to hold the merge: it goes into `defects-register.md`, with its effect on the lots to come. A name the encoder gets wrong is not a bug of the lot: add it to `phonetic-fr.json`.

| # | What was seen | Bug, pre-existing defect or idea | Register row / fixed in |
| --- | --- | --- | --- |
| | | | |
