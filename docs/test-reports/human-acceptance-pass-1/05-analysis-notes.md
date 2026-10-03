# 05 - Analysis notes (answers to the owner's open questions)

Read-only analysis; no code changed. Claims about models are general knowledge of the model families and
about the observations of HAP-1; they should be re-checked on the Mac mini with the real measuring script
(`apps/server/scripts`) before they drive a decision.

## 1. What is the difference between a 2B and a 3B model, and between Gemma and Ministral?

**"2B" and "3B" are numbers of parameters, not tokens.** A parameter is one learned weight of the neural
network; `gemma2:2b` has roughly 2.6 billion of them, `ministral-3:3b` roughly 3 billion. Tokens are the
pieces of text (about three or four characters of French) the model reads and writes; the number of tokens
a model can read at once is its *context window* (8 192 tokens by default in this product, set on the
server, `MODEL_CONTEXT_WINDOWS`). Size and context are independent.

What more parameters buy: more capacity to store language patterns, to keep several constraints in mind at
once and to follow instructions precisely. What they cost: memory, and time per generated token, because
each token needs a pass over all the weights (on this CPU: `ministral-3:3b` took 1.5 to 3 times as long as
`gemma2:2b` for comparable answers, e.g. 3m05s versus 1m22s on Q8, 1m22s versus 35s on Q27).

It is **not** "thinking capacity" in the sense of reasoning models. Both are plain instruction-following
models that produce the answer token by token; neither deliberates before answering.

**The family and generation matter as much as the size.** What decides quality at equal size: the training
data and its language mix, how the model was distilled or instruction-tuned, how well it keeps to a
"answer only from these excerpts" rule, and the quantisation used to run it locally (Ollama's default
4-bit quantisation costs some accuracy, more on small models). `gemma2:2b` (Google, 2024) is a very small,
distilled general model. `ministral-3:3b` (Mistral, a more recent generation, a French company with a
strong French corpus, designed for edge use) is slightly larger and newer. In HAP-1 the difference was
visible exactly where the product needs reliability:

| Task | `gemma2:2b` | `ministral-3:3b` |
| --- | --- | --- |
| Print the price from a quote that has no printer price (Q8) | invented 1 000,00 | exact figure and the caveat that the printer alone is not priced |
| Summarise a convention (Q6, gemma only) | added "durée du bail" | not run |
| Draft an e-mail with no document (Q27) | refused (misapplied a rule) | drafted a template |
| Refuse when the document is not selected (Q8-d..f) | right only without memory | right (leaked an internal label) |

A model is not "right" because it is larger; a 3B can still hallucinate, and a 7B-14B model is the realistic
target for a document assistant that must refuse reliably (the Mac mini is meant for that class). The pilot
conclusion from this evidence: `gemma2:2b` is not dependable for document questions; `ministral-3:3b` is a
better floor but slow on CPU and exposed to the gateway timeout on first load (BUG-11).

## 2. Does conversation memory override the document scope? Yes, on this evidence

Experiment (Q8-e versus Q8-f, S16 versus S17): the same model, the same selection (quote unticked), the same
question. The only difference is whether the earlier answer "1 000,00 euros HT" is in the conversation.

Mechanism, read from the code:

1. `apps/desktop/src/lib/history.ts::conversationHistory` returns every question/answer pair that was answered,
   excluding only software messages (`needsIndexing`, `tabularNudge`). It is **not aware of the selection** that was
   active when each answer was written; a deterministic folder answer or a tabular result is sent back like any
   other turn.
2. `commands.rs::Writer::write` places the grounding material (retrieval instruction, excerpts, the question) in the
   **last** turn, after the remembered exchanges, precisely so that it is read last
   (`docs/SELECTION-AND-MEMORY.md`, "Grounding outranks memory").
3. Even so, a 2B model gives more weight to a previous assistant turn that already looks like the answer than to
   an instruction saying the information may not be there. With the scope narrowed (quote unticked) the excerpts do
   not contain the figure, the history does.

Consequences: any wrong answer becomes sticky; a conversation that changed its selection is the worst case; and
the first wrong answer (Q8-a) was itself a hallucination with the right source selected. See BUG-09 for the
candidate mitigations. None changes what is sent to the model until the owner approves it.

## 3. Why did the model say `WORK_FOLDER_CONTEXT`? And is the Data folder exposed in the same way?

For every tier-1 question `work_folder_context::build_system_turn` sends the retrieval instruction, a
"WORK FOLDER KNOWLEDGE CONTRACT" and a block that starts with the literal word `WORK_FOLDER_CONTEXT`
(`work_folder_context.rs`, lines 116-121 and 172-184). The contract names the block explicitly, so a model asked
to say where it looked reuses that word. The block carries counts, relative paths and states only (no text, no
hash), so it is a naming problem, not a privacy one.

Data folder: the tabular tier sends no such block. The mixed tier sends `Document excerpts` and `Table results`
(plain words). The hidden interpreter sends a `Schema:` JSON that the user never sees. Nothing else was observed
in HAP-1, but the check was only by reading: a guard test that fails when an upper-case internal identifier
appears in a model-facing constant would make it systematic. Options and risks: BUG-10. Do not edit the prompt
before the owner agrees; measure with ten repetitions of Q8-d before and after.

## 4. Why did the second `ministral-3:3b` call fail? (Q8-b, S11)

Timeline: `gemma2:2b` answered Q8-a at 15:35 (1m22s) and again at 15:40:36 the user switched to
`ministral-3:3b` and asked the same question. Ollama had to unload one model and load the other, then read
the prompt, before producing a first token. The gateway talks to Ollama with an `httpx` client whose timeout
is 180 s (`LLM_REQUEST_TIMEOUT_SECONDS`, default 180); when nothing arrives for that long it answers the client
with `provider_error` (HTTP 504, `reason: "timeout"`). The client shows the generic sentence for that code
("Le modèle n'a pas répondu correctement. Réessayez."), which hides the reason. The third attempt found the
model already loaded and succeeded in 3m05s, because tokens kept arriving.

The client's own setting (`answerIdleTimeoutSeconds`, raised to 600 s) was not the limit. The owner's intuition
that a timeout was too short was right, but the server-side one. Remedies in BUG-11.

## 5. Making the choices clickable (Q4, Q9): recommendation

Today the product answers "Lequel voulez-vous ?" with a bullet list, and the user must retype the question with
the file name. The data already exists in machine form (`FolderAnswer::AmbiguousReference { query, candidates }`
and the tabular `ambiguous_reference` / `which_workbook` answers carry `candidates`).

Two designs, both reusing code that exists:

- **A. Append a new question below** ("Que dit 2026/mars/neurologie.pdf ?"): reuses `send`; the conversation keeps
  the clarification step, which is honest but long; the clarification answer stays in the memory sent to the model.
- **B. Replace in place (recommended)**: clicking a choice calls the existing `resend(questionEntryId, newText)`
  that "truncates and replaces" (the edit-and-repost feature of `MessageList`). The clarification turn
  disappears and the conversation reads as if the user had been specific from the start. The clarification is
  software-written and carries no information worth remembering, so nothing useful is lost, and the memory stays
  clean.

Both need the same three small pieces: (1) the entry carries `choices` as data (not only a Markdown string);
(2) a pure function that rewrites the previous question: replace `query` by the chosen path when it occurs in the
text (case and accent folded), otherwise append ` (fichier : <path>)`; for "which workbook" append
` dans <file>`, which the classifier already resolves ("Nommez-le dans la question"); (3) a button per
choice rendered under the answer. Effort S, plus tests for the rewrite function in both locales. Cheapest
fallback if the rewrite is not wanted: A with the original question reused unchanged and the path appended.
Related quick win independent of the UI: BUG-15 would remove the Q9 question entirely when only one workbook
holds the named column.

## 6. Q19: was the question too ambitious? Reformulations that work today

Yes for the current engine. "Has any single supplier cost more than 5 000 this quarter?" is a group total
compared with a threshold; the engine can total per group (Q12) and rank groups (Q11) but cannot filter groups by
total (BUG-14). The model cannot compensate because it may only produce a plan in the closed vocabulary. Two
questions that work today and let the user conclude (supplier totals are 1 450, 550 and 215, all under 5 000):

```text
Quel fournisseur a le plus de montant ?              (S20, S23: MedSupply, 1 450)
Quel est le total de montant par fournisseur ?       (S24)
```

With "5000" typed as digits the classifier answers a meaningless "date en 5000" (BUG-02); with "cinq mille" no plan
could be produced. The nudge also contradicts the question by listing `fournisseur` as a column the user "should
name" (BUG-13).

## 7. What the mixed tier did and did not do in this pass

- `mixed_answer::answer` (session 16) was **not executed by any of the three mixed-selection questions** of the
  catalogue. The session 15 router answered or refused first (Q23, Q24, Q26).
- Therefore the behaviours verified only by `tests/mixed_answer.rs` (correction appended, rejected citation,
  entity ambiguity, partial answer with the gateway down) have **no human confirmation yet**.
- The automated tests call `mixed_answer::answer` directly with hand-built evidence; they bypass the router and
  the classifier, which is why they were green while the product path was blocked. The missing test is an
  end-to-end one through the real routing (improvement plan, "Automation").
- What *did* work with the gateway stopped (Q26): the deterministic half kept answering and nothing hung
  (S43). That is a real, desirable property.

## 8. Reading the "Compris comme" line

The line under a tabular answer ("Compris comme : patient = Martin A.") is the product's own audit trail and was
what made BUG-02 diagnosable without a debugger. It should stay, and it suggests a cheap safety rule: if the
understood conditions contain a word or number that is not clearly in the question as a condition, or if they leave
0 rows, the product should say so instead of presenting the number as the answer.
