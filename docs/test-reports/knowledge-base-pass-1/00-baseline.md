# 00 - Baseline: where the time goes, before the Knowledge Base

Written for the owner, who fills it in, and for the developer or agent of a later lot who needs the numbers
the programme is judged against. English; the French questions are quoted as test data.

**State: template.** Section 2 holds numbers measured by the agent of lot 0 on a development machine. Every
other table is empty and is filled in by the owner on the machines that matter. Nothing below is an estimate
made up to look complete: an empty cell means "not measured yet".

## 1. What this measures, and why first

The owner saw slowness. The Knowledge Base was expected to help, but at question time the only embedding call is
the question's, the chunk vectors are computed at Analyse, and the prompt is capped (`docs/RETRIEVAL.md`). So the
slowness may live somewhere the KB cannot reach: in the model's first word, in reading a workbook, in embedding a
long document. Lot 0 therefore makes each stage a number, and this file is where those numbers are written down
before anything is optimised. Every later lot is compared with it.

Two things are timed, both by code that was added in lot 0 and that holds **numbers and machine codes only** (no
question, no file name, no passage):

- each **question**: `retrieval-timings.jsonl`, one line per question;
- each **Analyse pass**: `analysis-timings.jsonl`, one line per pass (Documents or Data).

Both are off by default and appear in the application's local data folder
(`%LOCALAPPDATA%\com.assistantcabinetai.desktop\` on Windows) when `writeTimingLog` is switched on.

## 2. Already measured (agent of lot 0, development machine)

Machine: Intel Core 7 150U, 12 threads, 23.6 GB RAM, Windows 11. Release build. No gateway running: these are the
local, no-network parts.

Reading a large workbook with a cold typed cache (what the first question on a workbook costs;
`cargo test --release --test tabular_column_cache_bench -- --ignored --nocapture`, 8 October 2026):

| Workbook | Rows | Size | First analysis (`build_inventory`) | One question, cache cold (`load_current` + execute) | Ten questions, average |
| --- | --- | --- | --- | --- | --- |
| CSV | 100 000 | 5.6 MB | 415 ms | 526 ms | 530 ms |
| XLSX | 50 000 | 17.0 MB | 438 ms | 807 ms | 809 ms |

These are the worst realistic sizes; a workbook of a few hundred rows costs milliseconds. With the cache warm, a
question skips the read entirely (`docs/DECISIONS.md`, session 13). The column "workbook load" of section 4 will
say what a real first question costs on the owner's machines.

Not measured here because it needs the real stack: everything with a gateway, a model or an embedding model
(Analyse time, embedding per chunk, first token, generation).

## 3. Before you start

1. Close the application. Copy `index.sqlite3` from the local data folder and `settings.json` from the config
   folder (`%APPDATA%\com.assistantcabinetai.desktop\`; the settings panel also shows its path) to a folder of your
   choice. The measurement analyses other folders, and the index is shared by every build of the app.
2. Generate the fictional corpus outside the repository:
   `python fixtures/kb-baseline/make_corpus.py C:\kb-baseline` (it refuses a folder that is not empty). It writes
   `docs-10`, `docs-30`, `docs-50`, `data` and `ground-truth.txt`. Do not analyse `ground-truth.txt`.
3. Start the application (`cd apps/desktop; pnpm tauri dev`, with `docker compose up -d` running, or with the
   address of the other machine in the settings, see below). In **Réglages**, group **Avancé**, tick **Noter les
   durées dans un journal**. Leave the second box unticked: no screen shows it yet.
4. Note the machine and the models in the table below.

| Item | Value |
| --- | --- |
| Date | |
| Workstation (CPU, RAM, disk) | |
| AI machine (this PC, or the GPU machine on the network, or the Mac mini?) and how it is connected | |
| Chat model alias and the model behind it | |
| Embedding alias and model | |
| Context window (`/health`, `contextWindows`) | |
| Build (branch, commit) | |

## 4. The questions

The same four types at every size. Type them exactly; the selection is "tous" (the whole of the chosen folder).

| Type | Question (as typed) | Why this type |
| --- | --- | --- |
| E - entity | `Que dit le courrier concernant Camille Vasseur ?` | What the KB is meant to help most: a named person among many files |
| D - each document | `Résume chaque document en une phrase.` | The "every document" plan: one excerpt per file, a character budget |
| F - entity-free | `Quels sont les horaires d'ouverture indiqués dans les courriers ?` | A question the KB cannot narrow |
| T - data | `Quelle est la somme des montant ?` (Data folder = `data`, only `factures-fournisseurs` ticked, no document selected) | The tabular engine alone; the workbook read. Same phrasing as question Q9 of the first human pass |

## 5. Procedure

For each size N in 10, 30, 50:

1. Point the Documents folder at `docs-N` (settings). Press **Analyse** and wait. This is one Analyse measurement.
2. Ask E, D and F, one after the other, **clearing the conversation between them**. For each, wait for the end of
   the answer.
3. After the 50-file size only: set the Data folder to `data`, press **Analyse** on the Data card (one more Analyse
   measurement), select nothing in Documents and `factures-fournisseurs` in Data, and ask T twice: the **first**
   time is the cold read, the second is the warm one.
4. **Mixed run (once, with the 50 documents):** select all the documents **and** the three tables, and ask
   `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans les devis ?`
   (the two quotes are in the generated letters; `ground-truth.txt` gives both figures). What you want is the
   timing line with `"path":"mixed"`, whatever the answer. If that line shows `chunksSelected` 0 and `embeddingMs`
   0, the engine answered alone and the documents were not needed: note it, and ask the same thing again with
   "Demander à l'IA" on the answer.

**Two AI machines, no Mac mini yet (owner, 8 October 2026).** Take the whole protocol once with the AI on this PC and
once with the AI on the other machine of the network (the one with the graphics card, gateway on port 8080): set its
address in **Réglages > Adresse de l'IA**. Keep the same workstation, the same documents and the same models, and
write the machine in section 3 for each run, so the two sets of rows are comparable. The stages that run on the
workstation (inventory, scope, routing, search, hash, extract, chunk, write) should not move between the two runs; the
ones that run on the AI machine (embedding, first token, generation) are expected to. Whatever moves, and by how much, is
the useful result. The two-in-flight embedding measurement (section 9.2) is worth running against both addresses too.

Then read the two files (PowerShell):

```text
Get-Content $env:LOCALAPPDATA\com.assistantcabinetai.desktop\analysis-timings.jsonl | ForEach-Object { $_ | ConvertFrom-Json } | Format-List
Get-Content $env:LOCALAPPDATA\com.assistantcabinetai.desktop\retrieval-timings.jsonl | ForEach-Object { $_ | ConvertFrom-Json } | Format-Table path,plan,selectedDocuments,chunksConsidered,chunksSelected,inventoryMs,scopeMs,routingMs,embeddingMs,searchMs,workbookLoadMs,generationFirstTokenMs,generationTotalMs,totalMs
```

A line holds, in order of the question, the stages below. Every `...Ms` is the wall time of one stage; a stage the
question did not go through is 0.

| Field | Means |
| --- | --- |
| `inventoryMs`, `scopeMs` | Looking at the folder and resolving the selection |
| `routingMs` | Deciding what the question is (documents: the folder router; tables: classification **and** computation) |
| `embeddingMs` | The one embedding call for the question |
| `searchMs` | Lexical and vector search and the cut to the caps |
| `workbookLoadMs`, `workbookCacheHit` | Reading the workbooks; whether the typed cache served them |
| `generationFirstTokenMs` | Prompt sent to first word of the answer |
| `generationTotalMs` | Prompt sent to last word (tables: the model-assist step, 0 when no model was asked) |
| `totalMs` | The whole question, including everything above and the small gaps between them |
| `chunksConsidered`, `chunksSelected`, `estimatedInputChars` | How much was scored, how many excerpts and how many characters reached the prompt |

## 6. Results - questions

One row per question. Copy the numbers from the line; do not round.

| Size | Type | Path | Plan | Docs | Chunks considered | Chunks sent | Input chars | Inventory+scope | Routing | Embedding | Search | Workbook load (hit?) | First token | Generation total | Total |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 10 | E | | | | | | | | | | | | | | |
| 10 | D | | | | | | | | | | | | | | |
| 10 | F | | | | | | | | | | | | | | |
| 30 | E | | | | | | | | | | | | | | |
| 30 | D | | | | | | | | | | | | | | |
| 30 | F | | | | | | | | | | | | | | |
| 50 | E | | | | | | | | | | | | | | |
| 50 | D | | | | | | | | | | | | | | |
| 50 | F | | | | | | | | | | | | | | |
| 50 | T (cold) | | | | | | | | | | | | | | |
| 50 | T (warm) | | | | | | | | | | | | | | |
| 50 + 3 tables | mixed | | | | | | | | | | | | | | |

## 7. Results - Analyse

Copy from `analysis-timings.jsonl`. `extractMs` excludes `ocrMs`. All values in milliseconds unless a count.

| Pass | Files | Processed | Hash | Extract | OCR | Chunk | Embed | Embed batches | Embedded chars | Write | Build inventory | Chunks | Chunks repeating earlier text | Total |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Documents, 10 | | | | | | | | | | | | | | |
| Documents, 30 | | | | | | | | | | | | | | |
| Documents, 50 | | | | | | | | | | | | | | |
| Data (3 tables) | | | | | | | | | | | | | | |

The slowest five files of a pass are in `slowestFiles`, by position in the pass, never by name. Position `n` is the
`n`-th file in the order the pass walks the folder.

## 8. Reading the numbers

**Which stage dominates the 30-file case?** Fill this in last, from the rows of section 6 for size 30, in this way:
take `totalMs` of each of E, D and F, and divide each stage by it. The dominating stage is the largest share, per
question type. Write it plainly:

| Question type | Dominating stage | Its share of the total | Second | Its share |
| --- | --- | --- | --- | --- |
| E (entity) | | | | |
| D (each document) | | | | |
| F (entity-free) | | | | |

Two rules for reading it, so that no one concludes the wrong thing:

- **If `generationFirstTokenMs` dominates**, the wait is the model reading its prompt, and neither retrieval nor a
  Knowledge Base shortens it except by sending fewer characters (`estimatedInputChars`). Say so, and give the
  chars-per-second this machine achieves.
- **If `embeddingMs` or `searchMs` dominates**, the KB's candidate discovery has something to cut. Only then is
  scope reduction a performance argument rather than a precision one.

## 9. Decide-by-measurement items (lot 0, section 0.4)

Both are decided from the numbers above. Lot 0 builds neither; it records what the numbers say.

### 9.1 Pre-warming the typed workbook cache at the end of Data Analyse

Read: the first-question column "Workbook load (hit?)" for T (cold) against T (warm), and the Data row of
section 7 for the pass itself.

| Measurement | Value |
| --- | --- |
| Workbook load, first question after Analyse (cold), per size of table | |
| Workbook load, second question (warm) | |
| Size of `workbook_json` in the index for the three tables (database size before/after the Data Analyse, or `SELECT length(workbook_json) FROM tabular_workbooks`) | |
| Data Analyse total with the three small tables | |

Rule recorded in advance: pre-warming doubles the data written by a pass and re-opens the residual-risk question of
`docs/DECISIONS.md` (session 13). It is built only if a cold first question costs a noticeable share of its total on a
table size the owner really uses. The development-machine worst case (section 2) is about half a second for 100 000
rows, once per workbook.

### 9.2 Analyse throughput

| Measurement | Value |
| --- | --- |
| Share of the 50-file Documents pass spent embedding (`embedMs / totalMs`) | |
| Share spent extracting / in OCR / writing | |
| Chunks repeating earlier text, as a share of all chunks (`chunksRepeatingEarlierText / chunksTotal`) | |
| Embedding per 1 000 characters on this machine (`embedMs / embeddedChars * 1000`) | |

Rules recorded in advance: a `chunk_text_sha256 -> vector` cache is **proposed, not built**, and only if the repeated
share is material (the letterheads of the generated corpus are repeated on purpose, so measure a few of the owner's
own fictional folders as well). Embedding is expected to dominate a Documents pass; if it does not, say what does.

**Two requests in flight.** On the machine that serves the AI (the Mac mini at the practice, from the workstation),
from the repository:

```text
cd apps/desktop/src-tauri
$env:ASSISTANT_CABINET_SERVER_URL = "http://<address of the AI machine>:8080"
cargo test --release --test embedding_parallelism_bench -- --ignored --nocapture
```

It sends synthetic filler text only and prints the time with one request in flight and with two. Paste the output:

```text
(paste here)
```

A ratio near 1.00 means the machine answers them one after the other and parallel batches would gain nothing; a
ratio near 0.5 means they would roughly halve an Analyse pass. Whatever it says, the pass stays sequential in lot 0.

## 10. After the run

Delete `retrieval-timings.jsonl` and `analysis-timings.jsonl` (or leave them: they hold numbers only), set
`writeTimingLog` back to false, restore your own `settings.json` and `index.sqlite3` if you want your folders back,
and write in the report of the next lot what the 30-file case showed.
