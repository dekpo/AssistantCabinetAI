# Human test - lot 2, Names, accents and sounds

Written for the owner, who runs it. English, with the French strings of the interface quoted as data.

| | |
| --- | --- |
| Branch to be on | `feat/kb-normalize-phonetic` |
| Build | Part A: none (`cargo test` only). Part B: `cd apps/desktop` then `pnpm tauri dev` |
| Services | Part A: none. Part B: `docker compose up -d` from the repository root, chat and embedding models loaded, as for any pass |
| Duration | Part A about 10 minutes, Part B about 10 minutes |
| Visible in the application? | **No.** Lot 2 is pure functions: nothing reads them yet (the extractors of lot 4 and the resolver of lot 3 will). There is no screen, no setting and no command to click. What you can do is **read and edit the table of names the encoder treats as the same sound**, and check that the one existing behaviour lot 2 touched, the matching of file names, did not move |

## What this proves, and what it does not

It proves two things. First, that the way the product has always lowercased and de-accented a file name (`fold_text`) is
now the same code in a new home and gives the same answer on every file name of the fixture folders (a unit test
compares the old body with the new one). Second, that the **French phonetic key** puts "Dupont", "Dupond" and "Dupon" on
one key and keeps "Dupont" and "Dumont", "Martin" and "Martine", "Simon" and "Simone" apart, as you decide in
`phonetic-fr.json`.

It does not prove that names are found in your documents (lot 4), that two spellings are linked to one person (lot 3 and
later), or anything about speed. **The agent ran every `cargo test` below and read the table printed in Part A. It did not
open the application window**, so Part B is yours to confirm.

## Preparation

1. Nothing to back up for Part A: it reads one file of the repository and writes nothing.
2. For Part B, close the application and copy `index.sqlite3` and `settings.json` aside, as in the lot 1 test, if you want
   to be able to go back (this lot changes neither, but the smoke test analyses files).
3. Part B uses the fictional fixtures of the first human pass, as in lots 0 and 1: `docs/test-reports/human-acceptance-pass-1/fixtures/documents`
   in your Documents test folder. Do not use real files.

## Part A - the table of sounds (developer-level, 10 minutes)

All commands from the repository root, in PowerShell.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| A1 | `cd apps\desktop\src-tauri` then `cargo test --test knowledge_names -- --nocapture` | `test result: ok. 22 passed; 0 failed`. Between the test lines, a table titled `French phonetic key (fr-rules:1), 72 names, 50 keys` | Any failed test. The failing test prints which row of `phonetic-fr.json` is not respected |
| A2 | Read the first block of the table, "names that sound alike" | One line per key shared by several names, for example `DYPo  Dupon = Dupond = Dupont`, `LEFEVR  Lefebvre = Lefevre = Lefèvre`, `TIBO  Thibault = Thibaut = Tibo`, `FILI  Filip = Philippe`, `DYRa  Durand = Durant`, `DYBUA  Dubois = Duboit`, `MIXEL  Michel = Michelle`, `aRI  Henri = Henry` | A name you consider different from its neighbours on a line (that is a decision for you, see A4) |
| A3 | Read the second block, "names with a key of their own" | Pairs you asked to keep apart are in different blocks or on different lines: `DYMo  Dumont` (not with `DYPo  Dupont`), `MARTe  Martin` and `MARTIN  Martine`, `SIMo  Simon` and `SIMON  Simone`, `LORa  Laurent` and `LORaS  Laurence`. And `FRaSUA  Francois = François` is together while `FRaSUAZ  Francoise = Françoise` is a different line (the man and the woman are not mixed up) | Two names you consider the same sitting on separate lines, or two you consider different on one line |
| A4 | **Change the contract yourself.** Open `apps\desktop\src-tauri\tests\fixtures\knowledge\phonetic-fr.json` in the editor. In `"collide"` add a row, for example `["Marchand", "Marchant"]`. Save, re-run the command of A1 | Still 22 passed, and the table now has `Marchand = Marchant` on a shared line (no rebuild of the program, only of the test, and no Rust edited) | A failure on a row you just added correctly |
| A5 | Add a row you expect to **fail**: in `"collide"` add `["Dupont", "Dumont"]`, save, re-run A1 | `the_phonetic_contract_holds` fails with `should collide: Dupont = DYPo, Dumont = DYMo`, which names the row | The test passes (the contract is not read) |
| A6 | Remove the two rows you added (the file is versioned: `git diff` shows what you changed) | Back to 22 passed | |
| A7 | Add to `"observe"` three surnames you want to check (invented or very common names only, never a real person's record) and re-run A1 | Each appears in the table with its key; a name that shares a key with another appears in the first block | An empty key, or a key with an accent in it |
| A8 | `cargo test --lib -- knowledge::normalize knowledge::phonetic fold_text` | 16 passed (the few ASCII unit tests beside the code, and the check that `fold_text` still gives the old answer on every fixture file name) | |

What to decide with the table, when you have read it: which sounds the product should offer as "did you mean?" later
(a key shared by two names is only ever a suggestion, never a merge), and which pairs you want kept apart. Anything you add
to `"collide"` or `"differ"` is a rule the next change to the encoder must still respect.

## Part B - smoke test: the product as before (10 minutes)

The only existing code lot 2 touched is `file_reference::fold_text`, which decides whether "Que dit BAIL-CABINET-2024 ?"
means the file `bail-cabinet-2024.pdf`. Same fixtures, same expected answers as `docs/test-reports/human-acceptance-pass-1/01-protocol.md`.
Interface in French.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| B1 | Start the application and press **Analyser** on the Documents card (fixtures chosen) | The summary reads as before; nothing unreadable | An error, a file listed as unreadable that was fine before |
| B2 | All documents selected. Ask `Combien de fichiers au total ?` | 6, answered "sans l'IA" | |
| B3 | All documents selected. Ask `Que dit le courrier de la CPAM concernant la radiation ?` | Hugo Exemple struck off the general scheme from 1 February 2026; the sources list `courrier-cpam-radiation.pdf`, page 1 | |
| B4 | All documents selected. Ask `Que dit BAIL-CABINET-2024 ?` (capitals, no extension) | An answer from `bail-cabinet-2024.pdf` **only**; the sources list that file and no other | Sources from other documents, or "plusieurs fichiers possibles" |
| B5 | All documents selected. Ask `Que dit courrier-cpam-radiation ?` | An answer from `courrier-cpam-radiation.pdf` only | |
| B6 | Only the invoices workbook selected, no document. Ask `Quel est le maximum de montant ?` | 620 | A different number |
| B7 | The quote `devis-imprimante-medsupply.pdf` and the invoices selected. Ask `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans le devis signé ?` | 1 450 (table) against 1 200,00 HT (quote), both sides cited | |
| B8 | Nothing selected. Ask `Rédige un e-mail de relance pour un fournisseur dont la facture n'est pas réglée.` | An answer labelled "Réponse sans vos documents" | |

Accented file names cannot be tried here: **Analyser** renames every file to a clean ASCII name before anything is
matched (`docs/DECISIONS.md`, "clean file names"), so the fixtures hold none. The accent and decomposed-accent cases
are covered by the unit test that compares the old and the new `fold_text` on composed and decomposed names.

If any answer differs from a run on `main`, that is a regression of this lot: note the question and the answer. Do not
guess at the cause.

## Results table

| Step | Expected | Observed | OK? | Screenshot |
| --- | --- | --- | --- | --- |
| A1 | 22 passed, table printed | | | |
| A2 | shared keys as listed | | | |
| A3 | distinct keys as listed | | | |
| A4 | added row appears | | | |
| A5 | wrong row fails and is named | | | |
| A6 | back to 22 passed | | | |
| A7 | own names appear with a key | | | |
| A8 | 16 passed | | | |
| B1 | | | | |
| B2 | | | | |
| B3 | | | | |
| B4 | | | | |
| B5 | | | | |
| B6 | | | | |
| B7 | | | | |
| B8 | | | | |

## Findings

A bug found here is fixed on `feat/kb-normalize-phonetic` (a new commit, the pull request updated) before the merge block
is used. A name the encoder gets wrong is not a bug of the lot: add it to `phonetic-fr.json` and tell the next agent, or
list it in the open questions of `lots/lot-02-normalize-phonetic.md`.

| # | What was seen | Bug or idea | Fixed in |
| --- | --- | --- | --- |
| | | | |
