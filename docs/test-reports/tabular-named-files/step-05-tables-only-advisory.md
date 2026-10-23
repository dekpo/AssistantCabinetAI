# TABULAR step 05 - Tables-only advisory answers (23 October 2026)

Branch `feat/tables-only-advisory`, from `feat/tabular-integration` at the merge of step 04 (`67536c3`). The choices are in `docs/DECISIONS.md`, "Taken while building TABULAR step 05" (rows 1 to 14).

Goal: with **only spreadsheets** ticked, a question that is not a calculation ("explain this formula", "how do I link column B of X to column D of Y", "describe X.xlsx", "what is wrong with this file?") used to reach session 14's hidden interpreter (a query plan from the schema), fail, and end in a nudge listing columns. The model never wrote about the tables in this tier. It now gets the writer of the mixed tier, with the cards (and the sample) and no document block, and the user no longer waits for a useless query-plan call.

## What changed

- **`tabular/question.rs`:** two new pack lists (`advisory_intents`, `advisory_intents_with_a_name`, both `#[serde(default)]`, both packs) and `pub fn is_advisory(question, locale, named_count, route)`: true when a phrase of the first list is in the question, or a phrase of the second beside a named workbook, or two workbooks are named, **and** the route is `NotRecognised` or `HasFormulas`. Read from the question's own locale pack only.
- **`tabular_answer.rs`:** `advisory_target(question, data, selection, locale, entity_candidates) -> Option<AdvisoryTarget>` (inventory only, no workbook load: the workbooks that would get a card, the route checked on each, on the text the engine reads); `advisory_skips_documents(..)`; `without_the_model(PendingAnswer)`; `no_table_part()` is `pub`. `prepare_if_data_only` returns `None` and `prepare_for_mixed` returns the no-table-part nudge for an advisory question.
- **`commands.rs`:** `advisory_tier` (async, owns `index` and `folder`, takes the table side as `pending` for step 06): cards and sample as in `mixed_tier`, `mixed_answer::answer` with an empty document list and `NotSelected`, the same stream events, the usage trace through `workbook_use::trace(.., Answered::Mixed { .. })`; on a failure or an empty text it returns the engine's nudge (`Answered::Engine`). `tabular_tier` takes `history` and `on_event` and routes an advisory question to it before `prepare`. `mixed_tier` skips the embedding and the search for an advisory question that designates no document.
- **`mixed_answer.rs`:** `MixedPartUnavailable::NotSelected`.
- **Interface:** `not_selected` in the `MixedPartUnavailable` union (`ipc.ts`), the sentence in both catalogues, `annotations.ts` treats it like `not_asked_about` (informative, not read aloud).

## Routes measured before coding (inline workbooks `factures.csv` and `mixte.xlsx`, throwaway test deleted)

| Question | Locale | Route on the workbook |
|---|---|---|
| explain the formula of column cumul / explique la formule de la colonne cumul | en / fr | `HasFormulas` |
| how do I link column .. of A to column .. of B using a formula | en | `HasFormulas` |
| write a formula that adds .. / ecris une formule .. / formula for the total per supplier | en / fr | `HasFormulas` |
| does the sheet contain formulas / la feuille contient-elle des formules ? | en / fr | `SheetNames` (the word *sheet* wins) |
| quelles formules utilise mixte.xlsx | fr | `HasFormulas` |
| describe X / decris X, what is in X / que contient X, summarise this spreadsheet / resume ce tableur, que peux-tu me dire de ce tableur ? | both | `NotRecognised` |
| how to do a vlookup between A and B, compare A and B, join A and B, help me write a sumif | both | `NotRecognised` |
| what is wrong with this file, quelle est la difference entre ces deux fichiers | both | `NotRecognised` |
| total of the invoices / total des factures, how are the amounts distributed by supplier / comment sont repartis .. | both | `NotRecognised` (not advisory: no phrase) |
| which columns does X have / quelles colonnes a X, how many rows / combien de lignes | both | `ColumnNames`, `RowCount` |
| sum of montant / la somme du montant | both | `Operation` (on the workbook that has the column) |

Findings that changed the list: the owner's question « Que peux-tu me dire de ce tableur ? », « qu'est-ce qui ne va pas dans ce fichier ? » and « ce tableur contient » held none of the words of the prompt; « what is in » / « que contient » are lookups when no file is named (`row_lookup`), hence the second list. « Does the sheet contain formulas » is `SheetNames`, not `HasFormulas`, but is not advisory either way.

## Tests

- `question.rs`: 3 new unit tests (`is_advisory` on the two routes, the plain formula and figure questions, the name-only phrases and two names).
- `tests/tabular_pack_parity.rs`: the two advisory lists exist in both packs, are non-empty and share no phrase.
- `tests/mixed_routing_baseline.rs`: case K (both halves), case M and an assertion of case C flipped on purpose; 7 new `step_05_*` tests: advisory detection per locale, positive and negative; two named workbooks and the names that are not advisory; no table part in doors 3 and 4; the writer asked once with the cards and no document block (counting gateway: one call, `Document excerpts: none.`, no cell value in the request); a server that accepts and hangs up gives an error from the writer and then the engine's plain nudge, and nothing dials the server again; `without_the_model`; a question that designates a document keeps its retrieval.
- The hidden interpreter for a non-advisory question is covered by case M (exactly one call) and by the 16 tests of `tests/tabular_query_plan.rs`, unchanged.
- `src/lib/annotations.test.ts` and `src/speech/output/readingParts.test.ts`: the `not_selected` line is shown and copied, not read aloud.
- Not testable below the Tauri command: `advisory_tier` itself and the skip of the embedding in `mixed_tier` need an `AppHandle`. Their decisions are the pure functions above, tested; the human test plays the whole chain.

## Results

- Pre-check before the first edit (targeted): `tabular::question` unit tests 24 passed, `tabular_pack_parity` 4, `mixed_routing_baseline` 20, `tabular_query_plan` 16; nothing failing.
- Final pass (once, after the last edit of the work): `cargo test -j 2 -- --test-threads=1` about 1 874 passed, 0 failed (one failure in the first run, my own new test of the "with a formula" phrases, compiled before the packs were edited; `tabular::question`, `tabular_pack_parity` and `mixed_routing_baseline` were then run again: 27, 5 and 27 passed). `cargo clippy --all-targets`: no error; the warnings listed in the files of this step (doc list items, two functions with too many arguments, a `for` over one element) are all on code this step did not write. `pnpm run build` green; `pnpm run test` 115 files, 2 881 tests passed.
- `rustfmt`: `commands.rs` 0 blocks (was 0), `tabular_answer.rs` 48 (48), `mixed_answer.rs` 10 (10), `question.rs` 21 (21), `mixed_routing_baseline.rs` 0 (0), `tabular_pack_parity.rs` 0 (0).

## Found by the walk on her real index (copy of `index.sqlite3`, the Data folder read only)

The throwaway test (deleted) ran the questions of the human test on a copy of her analysed index, with `settings.json` read first (Documents `AssistantCabinetAI-test\Docs`, Tableaux `AssistantCabinetAI\Data`, server `http://localhost:8080`, `qwen3:1.7b`, rows mode `sample`):

- `factures-test.csv` ticked alone: « Que peux-tu me dire de ce tableur ? » is advisory (`implied_single`, one card); « Quel est le total du montant de factures-test.csv ? » is not, and the engine gives **450**.
- `factures-test.csv` + `kb-lot09-data/rdv.csv` ticked: « Comment relier la colonne fournisseur de factures-test.csv à la colonne patient de rdv.csv avec une formule ? » is advisory with **two named cards**; « Décris rdv.csv » is advisory with one named card; « Comment sont répartis les montants par fournisseur dans factures-test.csv ? » is **not** advisory and stays `TryModel` (the hidden interpreter).
- On `revenue_sub_agency.csv` (35 854 rows): « Explique la formule de la colonne Total » is advisory (the yes/no `HasFormulas` is no longer given); « Quelles colonnes a .. ? », « Combien de lignes ? », « La feuille contient-elle des formules ? » are the engine's.
- `donnees_publipostage.csv` (root and `kb-lot05/`, step 04's ambiguous name) was avoided in every question.

## Corrections made to later prompts

- `SESSION-TABULAR-06`: new section "What step 05 delivered": the real signature of `advisory_tier` and its `pending` parameter (Analyse in tables only passes a `Value`), its degrade branch that would drop a figure and must be changed, the interface entry of a tables-only advisory answer (a `mixedAnswer`, no `deterministic`, one "Regenerer" button), `MixedContext` built in six places; the Analyse item now names `advisory_tier`.
- `SESSION-TABULAR-07`: what step 05 settled and left (the formula phrases, the yes/no left on purpose, the phrase to add to both packs if another wording still ends in the yes/no); the advisory writer is `mixed_answer::answer`, so the wiring is done once; the structured cards are needed in `MixedContext`, set in all six places.
- `SESSION-TABULAR-08`: `advisory_target` returns `None` for an ambiguous or unticked name, never captures case L, makes two named workbooks advisory in a free writing request, and `advisory_skips_documents` must keep receiving the mixed tier's `names_a_document` (item 3).
- `SESSION-TABULAR-10`: the advisory intent is `tabular_answer::advisory_target`; `advisory_tier` is where a tables-only turn gets its rows.
- Plan: a paragraph after the table of cases (K and M fixed for advisory questions, what remains).

## Not done, by choice

- No change to the buttons under a figure (step 06) and none to what the writer says about formulas (step 07).
- A formula question with no advisory phrase keeps the yes/no answer.
- No Rust formatting pass of the crate (`AGENTS.md`).

## Owner's test (23 October 2026): results

Played on the development PC, model `qwen3:1.7b` on localhost (the tower was off). Parts A to E as expected.

- **B1** `Que peux-tu me dire de ce tableur ?` (only `factures-test.csv` ticked): a text written by the model in 35 s, with the nine rows and the total 450 (the sample was sent: "structure et 9 lignes sur 9, identifiants masqués"); under it « Vous n'avez sélectionné aucun document… » then « factures-test.csv : seul tableur coché, l'IA en a reçu la structure. » No nudge, no « sans l'IA » line. **B2** the total: 450, computed, no document line.
- **C1** two named workbooks: the model wrote about both files and a formula; two « cité dans votre question » lines. **C2** `Décris rdv.csv`: one named card plus « 1 tableur coché n'a pas servi… ». **C3** the non-advisory question kept the hidden interpreter (6 s): a computed answer, « La question elle-même a été comprise par le modèle… », no « aucun document » line.
- **D** the copy ends with the same lines. **E** with the AI stopped: no banner; the nudge of before for `kb-lot09-data/rdv.csv`, « Réponse calculée… sans l'IA » and « 1 tableur coché n'a pas servi… », no « interrogé pendant N s » line.

Findings, none caused by this step:

| # | Finding | Status |
| --- | --- | --- |
| N-5 | The 1.7b model writes franglais: in C1 and C2 it copies the English card and profile text ("Extrait du workbook", "5 data rows", "Table profile (computed by the program…)") into its answer instead of describing the table. The cards and the instruction are English by design (model-facing text); step 03 introduced the cards. | Not investigated. Candidate remedy for step 07, which rewrites the instruction: one sentence telling the model to describe the sources in the answer language and not to copy them (needs a neutrality guard; measure it on the three models). |
| N-6 | C3: « Comment sont répartis les montants par fournisseur » was understood by the hidden interpreter as a **count of rows per supplier** (3, 3, 3), not as totals of `montant` per supplier (the question says so, with a line telling the user to rephrase). | Pre-existing behaviour of the interpreter, kept on purpose here; not investigated. |
| N-7 | C1: the formula written (`=JOIN(A2:B2, " ")`) does not exist as a spreadsheet function. | Step 07's checker is built for exactly this case (real formulas of this kind are its test cases). |
