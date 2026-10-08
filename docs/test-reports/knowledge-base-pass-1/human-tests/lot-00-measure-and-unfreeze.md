# Human test - lot 0, Measure and unfreeze

Written for the owner, who runs it. English, with the French strings of the interface quoted as data.

| | |
| --- | --- |
| Branch to be on | `feat/kb-measure-and-unfreeze` |
| Build | `cd apps/desktop` then `pnpm tauri dev` |
| Services | `docker compose up -d` from the repository root (gateway, Ollama) with the chat and embedding models loaded, as for any pass |
| Duration | Parts A and B: about 20 minutes. Part C is the long measurement and is a session of its own (1 to 1.5 hours) |
| What to expect to **see** | One new thing: a group **Avancé** at the bottom of the settings panel, with two checkboxes. Nothing else changes on screen. Behind it: the questions and the Analyse passes are timed, the lexical search is held to the selection, and two optional log files exist. If you see a timing or a number under an answer, that is a bug |

## What this proves, and what it does not

Part A proves the existing product still behaves the same on the same fixtures (the lot touched the search and the
answer path). Part B proves the new timing log exists only when you switch it on (from the new checkbox) and holds numbers and codes only.
Part C produces the baseline the whole programme is judged against. The agent ran the automated suites and an
automated test of every point of Part B at the file level; **it did not drive the window and did not run a
gateway**, so every step below is yours to confirm.

## Preparation

1. Close the application.
2. Back up what the test may alter (PowerShell):

```text
$backup = "$env:USERPROFILE\kb-backup-lot-00"
New-Item -ItemType Directory -Force $backup
Copy-Item "$env:LOCALAPPDATA\com.assistantcabinetai.desktop\index.sqlite3" $backup
Copy-Item "$env:APPDATA\com.assistantcabinetai.desktop\settings.json" $backup
```

   (To go back later: copy them back with the application closed.)
3. The fictional fixtures of the first human pass: copy `docs/test-reports/human-acceptance-pass-1/fixtures/documents`
   to `C:\Users\<you>\AssistantCabinetAI\Docs\Test` and `.../fixtures/data` to `C:\Users\<you>\AssistantCabinetAI\Data\Test`
   (6 documents, 2 workbooks). Do not use real files.
4. Check that no timing log exists yet:

```text
Test-Path "$env:LOCALAPPDATA\com.assistantcabinetai.desktop\retrieval-timings.jsonl"
Test-Path "$env:LOCALAPPDATA\com.assistantcabinetai.desktop\analysis-timings.jsonl"
```

   Both must print `False` (if a previous run left them, delete them now).
5. `docker compose up -d`, then `pnpm tauri dev`.

## Part A - the existing product, unchanged (non-regression smoke test)

Interface in French. Settings: Documents folder `...\Docs\Test`, Data folder `...\Data\Test`. The expected answers are
those of `docs/test-reports/human-acceptance-pass-1/01-protocol.md`.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| A1 | Press **Analyser** on the Documents card | The bar moves; the summary says 6 files analysed, nothing unreadable | An error, or a file listed as unreadable that was fine before |
| A2 | Press **Analyser** on the Data card | 2 workbooks analysed | An error |
| A3 | Select all documents. Ask `Combien de fichiers au total ?` | 6, answered "sans l'IA" | A different count, or a model-written sentence |
| A4 | Select all documents. Ask `Que dit le courrier de la CPAM concernant la radiation ?` | Hugo Exemple struck off the general scheme from 1 February 2026; the sources list `courrier-cpam-radiation.pdf`, page 1 | Another file as the main source, or no source |
| A5 | Ask `Quelle est la durée du bail du cabinet d'après le contrat ?` | Nine years, 1 April 2024 to 31 March 2033 | A different duration |
| A6 | Untick the documents; tick only the invoices workbook. Ask `Quel est le maximum de montant ?` | 620 | Another number |
| A7 | Tick the quote (`devis-imprimante-medsupply.pdf`) **and** the invoices. Ask `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans le devis signé ?` | An answer that sets 1 450 (table) against 1 200,00 HT (quote), citing both sides | Only one side, or an invented figure |
| A8 | Select a single document named in the question: ask `Que dit bail-cabinet-2024.pdf ?` | An answer from that file only | Excerpts from another file |
| A9 | **Reset** the Documents card (confirm), then **Analyser** again | The index empties, then the 6 files are analysed again as in A1 | Anything left behind, or an error |

## Part B - the timing log

Commands used below (PowerShell, copy as they are):

```text
# B5 - the questions' log, one line per question
Get-Content "$env:LOCALAPPDATA\com.assistantcabinetai.desktop\retrieval-timings.jsonl" | ForEach-Object { $_ | ConvertFrom-Json } | Format-Table path,plan,selectedDocuments,chunksConsidered,chunksSelected,embeddingMs,searchMs,generationFirstTokenMs,totalMs

# B6 - look for any word of a question or any file name in both logs: there must be no output
Select-String -Path "$env:LOCALAPPDATA\com.assistantcabinetai.desktop\*-timings.jsonl" -Pattern "CPAM|radiation|MedSupply|Exemple|neurologie|bail|\.pdf|\.xlsx|Dupont|e-mail"

# B7 - the Analyse log
Get-Content "$env:LOCALAPPDATA\com.assistantcabinetai.desktop\analysis-timings.jsonl"
```

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| B1 | After Part A, with the switch never touched, run the two `Test-Path` lines of the preparation again | Both `False`: asking and analysing with the switch off writes nothing | A file exists |
| B2 | Open the settings panel (**Réglages**). A new group **Avancé** holds two checkboxes: **Noter les durées dans un journal** (off) and **Préparer l'affichage des durées** (off, and says it changes nothing yet). Tick the first one. Check that `%APPDATA%\com.assistantcabinetai.desktop\settings.json` now contains `"writeTimingLog": true` | The box stays ticked after closing and reopening the panel; the file holds the value | The box unticks itself, or the file is unchanged |
| B3 | Press **Analyser** on the Documents card, then on the Data card | Normal. The Documents files are unchanged since A9, so that pass mostly skips them: its stages are small, which is expected | |
| B4 | Ask five questions in this order, clearing the conversation between them: `Combien de fichiers au total ?` (all documents) · `Que dit le courrier de la CPAM concernant la radiation ?` (all documents) · `Quel est le maximum de montant ?` (invoices only, no document) · the A7 question (quote + invoices) · `Rédige un e-mail de relance pour un fournisseur dont la facture n'est pas réglée.` (nothing selected) | Each behaves as in Part A, and the last one is labelled "Réponse sans vos documents" | |
| B5 | Run the B5 command | Five lines, with `path` in this order: `folder_answer`, `documents`, `tabular`, `mixed`, `without_documents`. Only the second has a `plan` (`whole_folder` or `one_file`, depending on how the question was read) and a `chunksSelected` above 0. `generationFirstTokenMs` is empty for the first and third and a number for the others | A missing line, or a `path` out of order |
| B6 | Run the B6 command | **No output at all** | Any match: a name or a word of a question reached the log. Stop and tell the agent which |
| B7 | Run the B7 command | Two lines (documents, then data), each a flat object of numbers plus `"pass"` and a `slowestFiles` list of `position` and milliseconds. No file name | A file name, or a line missing |
| B8 | Look at the whole screen after each answer and at the settings panel | The only new thing anywhere is the **Avancé** group. No timing under any answer | A number or a duration on screen |
| B9 | Untick **Noter les durées dans un journal**; ask one question; run the B5 command again | No new line appears. Then press **Réinitialiser** in the settings (confirm): both boxes are unticked | A line is added with the box unticked, or a box stays ticked after the reset |
| B10 | Delete the two `*-timings.jsonl` files if you do not want them | | |

## Part C - the baseline measurement (separate session)

After A and B are accepted, follow `docs/test-reports/knowledge-base-pass-1/00-baseline.md` from its section 3. It
generates the 10, 30 and 50 letters and the three tables (`fixtures/kb-baseline/make_corpus.py`), tells you what to
type and which numbers to copy from the logs, and ends with the question this lot exists to answer: which stage
dominates the 30-file case. It is the measurement the whole programme is judged against, so run it on the machine
pair that matters (the workstation and the Mac mini at the practice) and write the machine down.

## Results table

| Step | Expected | Observed | OK? | Screenshot |
| --- | --- | --- | --- | --- |
| A1 | | | | |
| A2 | | | | |
| A3 | | | | |
| A4 | | | | |
| A5 | | | | |
| A6 | | | | |
| A7 | | | | |
| A8 | | | | |
| A9 | | | | |
| B1 | | | | |
| B2 | | | | |
| B3 | | | | |
| B4 | | | | |
| B5 | | | | |
| B6 | | | | |
| B7 | | | | |
| B8 | | | | |
| B9 | | | | |
| B10 | | | | |
| C (baseline file filled) | | | | |

## Findings

A bug found here is fixed on `feat/kb-measure-and-unfreeze` (a new commit, the pull request updated) before the merge
block is used. An idea goes to the open questions of `lots/lot-00-measure-and-unfreeze.md`.

| # | What was seen | Bug or idea | Fixed in |
| --- | --- | --- | --- |
| | | | |
