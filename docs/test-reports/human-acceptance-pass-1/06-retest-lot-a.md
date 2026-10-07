# 06 - Retest after lot A (4 October 2026)

Lot A of [04-improvement-plan.md](04-improvement-plan.md), branch `fix/lot-a-safety-and-honesty`, commit
`7a4f866`. Five checks were replayed by the owner on the real stack (`docker compose up -d --build server`
then `pnpm tauri dev`), then verified against the code and the fixtures by the agent. The owner labelled
the checks "Q8, Q23, Q8-b, Q21, Q26"; two labels were corrected below (A1 is Q18, A3 is the quote-unticked
variant of Q8), the conversations themselves are unchanged.

## What lot A changed

| Finding | Change |
| --- | --- |
| BUG-01 | `query_plan.rs`: an `eq`/`in` filter on a column the workbook lacks becomes an ordinary refusal (`ColumnNotFound`); the `unreachable!` is gone; unit test replays the HAP-1 plan on both ops |
| BUG-04 | `commands.rs`: with documents and tables both selected, "Demander à l'IA" (`skip_deterministic`) goes to the mixed tier instead of the data-only router |
| BUG-11 | `errors.ts` + catalogues: `provider_error` with `reason: timeout` has its own sentence; `docs/OPERATIONS.md` documents `LLM_REQUEST_TIMEOUT_SECONDS` for CPU servers |
| BUG-13 (part) | After a value that matches nothing, the repeated-failure advice says to check the spelling of the value instead of naming a column (`nudgeAgainValue`) |
| UX-2 | `styles.css`: the sidebar scrolls inside itself; the page no longer scrolls |

## Verdicts

| Check | Replays | Verdict | Evidence |
| --- | --- | --- | --- |
| A1 | Q18 with only the appointments workbook ticked, both models | **PASS** (no hang, no crash, Stop never needed) | Conversation A1 below. The fix is also covered by a unit test |
| A2 | Q23 then "Demander à l'IA" | **PASS for the button; the answers are not reliable** | The model is now reached and writes an answer. 2 of 4 runs are wrong (BUG-16) |
| A3 | Quote unticked, `gemma2:2b` then `ministral-3:3b` in one conversation | Both refuse correctly; **BUG-11 not exercised** | `ministral-3:3b` answered in 2m03s without a timeout, so the new message could not appear |
| A4 | Q21 twice, then in a fresh conversation | **PASS** | The new advice appears on the repeat, and only there (by design) |
| A5 | Q26: server stopped, both folder lists open | **PASS** | No page scrollbar; the sidebar has its own; the deterministic answer still works with the server down. Screens: `screenshots/retest-lot-a/A5-01.png`, `A5-02.png` |

### A1 - Q18 with `rdv-mars-2026.xlsx` only (both models)

```text
Vous: Combien de factures pour le fournisseur MedSupply ?
Assistant: « MedSupply » ne correspond à aucune valeur réelle de rdv-mars-2026.xlsx.
Ses colonnes sont : date, salle, patient, duree_min.
Essayez de demander une somme, un minimum, un maximum, un nombre de lignes, la liste des valeurs d'une colonne, ou le groupe qui a le plus grand total.
Par exemple : « Quelle est la somme de duree_min ? »
Ou : « Quel salle a le plus de duree_min ? »
(second ask, same text, plus:) Votre question précédente n'a pas pu être calculée non plus : vérifiez que la valeur est écrite exactement comme dans le fichier.
```

Notes:

- The path taken was the "value matches nothing" nudge, not the `ColumnNotFound` refusal the unit test
  exercises; the plan that crashed on 3 October was not reproduced live (models are not deterministic), so
  the live proof is "no hang on the same question and the same selection", and the unit test is the proof of
  the panic path itself.
- New minor copy defect: "Quel salle a le plus de duree_min ?" (should be "Quelle salle"). The example sentence
  is built from the column name without its gender; see BUG-13.

### A2 - Q23 and "Demander à l'IA" (four conversations, quote + invoices ticked)

Ground truth: table 1 450 (MedSupply, 3 rows) versus the quote's total 1 200,00 HT: **they differ by 250**.
The correct verdict is "no, it does not match".

| Conv | Model | How it was produced | History sent with the question | Answer | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | `gemma2:2b` | question asked twice, button pressed on the second answer | the first (deterministic) answer, "Somme de montant : 1 450 ..." | "Le montant total facturé par MedSupply correspond au montant du devis." | **Wrong verdict** |
| 2 | `ministral-3:3b` | same procedure | same | "Extrait [1] : montant total du devis signé = 1 200,00 € HT. Facture totale = 1 450 € (tableau). Non, le montant facturé ne correspond pas au devis signé." | **Correct** |
| 3 | `gemma2:2b` | question asked **once**, button pressed on that answer | **none** (see below) | "... correspond au montant indiqué dans le devis. 1 200,00 euros HT." | **Wrong verdict** |
| 4 | `gemma2:2b` | after restarting the `server` container and the frontend; question once, then the button | none | "Le montant total facturé par MedSupply est de 1 450,00 euros. Le montant total du devis est de 1 200,00 euros. La différence est de 250,00 euros. ... n'est pas égal au montant du devis." | **Correct** (and the subtraction is right) |

This is the first human confirmation that the session 16 generation path works end to end: the table figure
and the document figure reached the model and a model wrote a combined, correct comparison (conv 2, conv 4).

**Verification of the owner's reading** ("the small model is influenced by the conversation history, and answers
correctly when the memory is reset and the host resources are freed"):

- *Memory alone does not explain it.* A regenerate removes the answer **and its question** from the history it
  sends (`lib/turns.ts::truncateForRegenerate`: `base = entries.slice(0, index - 1)`). In conv 3 the question
  was asked once, so the history was empty, and `gemma2:2b` was still wrong. Conv 3 and conv 4 used the same
  procedure, the same model and an empty history, and gave opposite verdicts. History may still make things
  worse (conv 1; BUG-09 stands from HAP-1), but it is not the cause of conv 3.
- *Restarting is probably not the cause either.* `docker compose up -d --build server` rebuilds the gateway
  container only; Ollama, which holds the model and its memory, keeps running. Nothing in the prompt changes.
  "Resources freed" is plausible for latency, but there is no evidence it changes the verdict.
- *The most likely cause is sampling.* The client sends no temperature (`gateway.rs` payload: `model`,
  `messages`, `stream`, `output_locale`) and the gateway forwards none (`providers/ollama.py` sets
  `temperature` only when given), so Ollama's default sampling applies and the same prompt can produce a
  different answer on each run. Four single runs cannot separate "luck" from "state". What the four runs do
  show: `gemma2:2b` is *capable* of the right answer (conv 4) but not *reliable* (conv 1, 3); `ministral-3:3b`
  was right once.
- To settle it: repeat the identical procedure at least 10 times per model with and without history, record
  the verdict each time (see the re-run recipe below). Fixing the temperature would make the answer repeatable,
  but it is a request parameter the owner has not approved changing.

**New finding BUG-16 (S2): a wrong comparison in the prose is not checked and the computed figure is not shown.**
In conv 1 and conv 3 the model did not state the table figure at all (conv 1 states no number; conv 3 states only
the quote's figure). Numeric verification (`mixed_answer::verify_numbers`) flags only numbers that contradict the
table, and "200,00" of "1 200,00" appears verbatim in the excerpt, so nothing was flagged. Meanwhile the table value
that the engine computed is kept in `mixedTable` but is **not displayed** beside the model's prose. The user
therefore sees a confident, wrong sentence and no number to check it against. Candidate fixes, none applied:

1. Always render the computed value (the old deterministic text, "Somme de montant : 1 450, calculé sur 3 lignes,
   fournisseur = MedSupply") above or below the model's prose in a mixed answer.
2. When the model's text does not contain the table's figure, append a notice "the table figure 1 450 is not
   quoted in this answer".
3. Optionally, when the question asks whether two figures match, compute the comparison in code (equal or not,
   difference) and show it as data, leaving the model to write around it.

### A3 - quote unticked, model switch in one conversation

```text
Vous: Quel est le prix de l'imprimante dans le devis MedSupply ?
Assistant (gemma2:2b): No matching file found.
Vous: Quel est le prix de l'imprimante dans le devis MedSupply ?
Assistant (ministral-3:3b, 2m03s): Extrait non trouvé dans les documents fournis.
```

- Both are correct refusals (the quote is not selected). `gemma2:2b` answered **in English** although the
  interface, the question and the locale directive are French (OBS-7, a known weakness of small Gemma models
  in French; not a product defect, but it must be counted when choosing the pilot model).
- BUG-11 was **not** exercised: no timeout occurred, so the new sentence did not appear. The owner suggested
  the short history made the difference. The gateway limit is the **silence before and between chunks**
  (httpx read timeout, 180 s), not the total duration (2m03s is fine), and the silence is dominated by Ollama
  loading the model and reading the prompt. A shorter history helps marginally; whether `ministral-3:3b` was
  already resident in memory (Ollama keeps a model loaded for a few minutes) is the likelier difference, and
  it was not recorded. The fix is therefore shipped but unproven live.
- To prove it: start the server with `LLM_REQUEST_TIMEOUT_SECONDS=5`, ask any question with `ministral-3:3b`,
  and expect "Le modèle a mis trop de temps à démarrer, ..." instead of the generic sentence; then restore the
  value.

### A4 - Q21

Conversation 1 (`gemma2:2b`, then `ministral-3:3b`): two identical refusals "« Alfa » ne correspond à aucune valeur
réelle de factures-fournisseurs-2026.xlsx. ..." and, on the second, the new line "Votre question précédente n'a
pas pu être calculée non plus : vérifiez que la valeur est écrite exactement comme dans le fichier." Conversation 2
(fresh): the same refusal without that line.

- This is the designed behaviour: the advice is shown only when the previous answer was also a nudge. The
  owner expected a spelling hint on the first refusal; the first refusal already says the value matches nothing
  and lists the columns. If a hint on the first refusal is wanted, it is a copy change (decision for the owner).

### A5 - Q26 layout

Screens `A5-01.png` (sidebar scrolled to the top, banner cut at the bottom edge) and `A5-02.png` (scrolled down,
banner fully visible with "Réessayer"). The window shows only the sidebar's scrollbar. The red "IA Indisponible"
status at the top right is visible, and the deterministic answer of Q23 stayed on screen. Remaining remark: the
connection banner sits at the bottom of the scrollable sidebar and is below the fold when both folder lists are
open; pinning it (sticky) would keep a critical state visible (cosmetic, new UX-4).

## Updated bug status

| ID | Status after lot A |
| --- | --- |
| BUG-01 | Fixed in code, unit-tested; no hang observed live (A1) |
| BUG-04 | Fixed, confirmed live (A2) |
| BUG-11 | Shipped, **not yet verified live** (A3); recipe above |
| BUG-13 | Partly fixed (value advice, confirmed A4); still open: unnamed refused column (Q24), generic column list, wrong gender in the example ("Quel salle") |
| UX-2 | Fixed, confirmed live (A5); UX-4 (pin the banner) new |
| BUG-16 | **New**: mixed answer prose unchecked and the computed figure hidden |
| OBS-7 | **New**: `gemma2:2b` can answer in English to a French question |
| BUG-02, 03, 05..10, 12, 14, 15 | Open (lots B to E) |

## Re-run recipe for the model-dependent checks

Repeat each configuration N = 10 times and record: model, selection, history (empty or not), the verdict
(correct, wrong, refusal), the duration. A claim of the form "model X is influenced by history" needs the same
question with and without history, both repeated; a single pair of runs is not evidence (A2). Configurations to keep:

| Id | Selection | History | Models |
| --- | --- | --- | --- |
| R1 | quote + invoices | empty (question once, then the button) | `gemma2:2b`, `ministral-3:3b` |
| R2 | quote + invoices | the deterministic answer in history (question twice, button on the second) | both |
| R3 | all documents except the quote | empty, then with the earlier wrong answer in history (HAP-1 Q8-e versus Q8-f) | both |
