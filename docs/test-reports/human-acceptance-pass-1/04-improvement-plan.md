# 04 - Improvement plan, from the cheapest fix to the most expensive

Source: [03-bug-register.md](03-bug-register.md). Nothing here has been implemented. Each step names what
must be re-tested; the reference for "no regression" is this folder's catalogue plus the existing suites
(`cargo test`, `pnpm test`, `tabular_reference_cases.json`, `work_folder_inventory`).

Ordering rule: ascending effort and risk, except that **BUG-01 (a crash) goes first** whatever its size, and
that anything touching a model-facing string waits for the owner's explicit approval
(`AGENTS.md`; the 27 September 2026 hallucination regression).

## Rank table

| Rank | ID | Effort | Risk of regression | Why it sits here |
| --- | --- | --- | --- | --- |
| 1 | BUG-01 | XS | Very low | One `unreachable!` becomes a refusal; a crash on a plausible question outranks everything |
| 2 | BUG-04 | XS | Very low | One branch on `skip_deterministic`; gives users an escape from BUG-03/05 immediately |
| 3 | BUG-11 | XS | None | Message and documentation only; tells the user what really failed |
| 4 | BUG-13 | XS-S | None | Catalogue wording in both languages (`fr-FR.json` and `en-US.json` together) |
| 5 | UX-2 | XS | None | Layout of the sidebar/banner |
| 6 | BUG-05 | S | Low | Prefer the numeric column when several are named; reference case added |
| 7 | BUG-15 | S | Low | Pick the workbook that has the named column |
| 8 | UX-1 | S | Low | Clickable choices that resend a rewritten question (design in 05, section 5) |
| 9 | BUG-02 | S then M | **Medium** | Silent wrong numbers; the cheap part (consume date tokens, stop-words, de-duplicate, 0-row refusal) first, the policy part (numeric equality, plausible years) after a decision |
| 10 | BUG-12 | S | Low | Do not consult the model when the data already settles the refusal |
| 11 | BUG-10 | S + approval + measurement | **Medium** | Model-facing; needs an A/B over Q1-Q8 on both models before and after |
| 12 | BUG-08 | S-M | Low | Needs a reproduction with instrumentation first (cause unproven) |
| 13 | BUG-07 | S-M | Medium | Routing vocabulary is a delicate balance (`docs/DECISIONS.md`); needs negative and positive cases |
| 14 | BUG-09 | M | Medium | Scope-aware memory plus a tier-1 numeric safety net; touches what the model reads |
| 15 | BUG-03 + BUG-06 | M | Medium | Redesign the entry gate of the mixed selection; add end-to-end tests through the real classifier |
| 16 | BUG-14 | L | Medium | New engine capability (per-group threshold) or an honest guided refusal |
| 17 | Publipostage issue (row lookup, template filling) | L | Medium-High | New capability, see `docs/SESSION-DATA-17-Publipostage-Issue.md` (local) |

## Batches (suggested sessions)

**Batch A - safety and honesty (ranks 1-5), one short session.**
Fix BUG-01 with a unit test that replays the plan from HAP-1; wire `skip_deterministic` for mixed
selections; make the timeout and the nudges say what happened. Exit criteria: Q18 on rdv-mars with both
models does not hang; Q23 "Demander à l'IA" reaches the model; Q8-b message mentions the timeout.

**Batch B - the tabular engine tells the truth (ranks 6-10).**
BUG-05, BUG-15, BUG-02 (cheap part), BUG-12, UX-1. Exit criteria: Q14 = 4, Q16 = Salle 3/50, Q19c no longer
answers "date en 5000", Q24 = 1 450 or an explicit refusal that names the column, Q9 resolves to the invoices
workbook without asking, Q21/Q20 return the nudge with no model wait. Add the HAP-1 appointment fixture to
the reference suite so these stay green.

**Batch C - model-facing and interface behaviour (ranks 11-14).**
Owner decisions required first (see "Decisions to take"). Exit criteria: ten repeated runs of Q8-d show no
`WORK_FOLDER_CONTEXT`; Q8-e behaves like Q8-f; Q28 reaches the model; the end of every answer is visible.

**Batch D - the mixed selection does what it says (rank 15).**
Re-open sessions 15/16: tighten the data-only precondition, entity linking on referential phrases, remove
the `not_asked_about` shortcut, and run Q23, Q24, Q26 as automated tests through the real router. Exit
criteria: Q23 gives a combined answer that cites the table and the quote and shows the difference between
1 450 and 1 200,00; with the gateway stopped it shows the table value and the "document part unavailable"
line.

**Batch E - new capabilities (ranks 16-17) and the hardware reality check.**
Per-group threshold; row lookup by identifier; template filling. Re-run the whole HAP on the Mac mini with
larger models (7-14B class) before judging model-related findings (OBS-1..6) as product defects.

## Decisions to take (owner)

1. Whether `a`/`est`/`de`... may be added to the French filler list of the tabular pack (BUG-02), or whether
   the guard lives in code (recommended: both a code guard for one-letter tokens and a decision row).
2. Whether the work-folder block is renamed, made conditional, or both (BUG-10), and approval to change that
   model-facing text.
3. How the conversation memory should depend on the selection (BUG-09), and whether to send fewer exchanges to
   models of 3B parameters and below.
4. Whether, with both kinds of source selected, the default is the data-only router first (today) or the mixed
   tier first with a model-free partial answer (BUG-03).
5. Whether Q19-style questions are in scope (BUG-14) or should be answered by a guided refusal.
6. The minimum model class for the pilot: `gemma2:2b` is not reliable enough for document questions on this
   evidence; `ministral-3:3b` is better but slow on CPU.

## Automation to add so that this pass can be repeated cheaply

- Turn Q1-Q4 and Q9-Q18 into automated cases against a command-level harness (the deterministic ones need
  no model): the tabular reference file already does this for the engine; add the HAP-1 fixtures and the
  appointment/invoice questions, including Q14 and Q16.
- Add a classifier-level test of "mixed selection" routing that uses the real router, so BUG-03/05/06 cannot
  hide behind tests that call `mixed_answer::answer` directly (what `tests/mixed_answer.rs` does today).
- Keep a short list of model-dependent questions (Q5-Q8, Q27) for the measuring script
  (`apps/server/scripts`) so that a model or prompt change is checked on the same questions every time; log
  model name, duration and a manual verdict.

## Definition of "no regression" for this baseline

All PASS entries of [02-results.md](02-results.md) stay PASS. FAIL entries may change, but never to a
different wrong answer. Any new "Compris comme" line must describe only conditions that are in the question.

---

## Triage after the lot B retest (4 October 2026)

Source: [07-retest-lot-b.md](07-retest-lot-b.md). The question asked of every open finding is: **does a later
fix stack on it?** If yes, fixing it later means building on a wrong result, so it is fixed now; if no, it stays
in its planned lot.

| Finding | Do later work rely on it? | Decision |
| --- | --- | --- |
| BUG-17 French "plus de N" dropped (Q19) | **Yes.** Lot D tightens the data-only precondition on the idea that a recognised question with nothing left over is "clearly about the data": an unfiltered count wrongly passes that test, also with documents ticked. Lot E (per-group threshold, row lookup) extends the very filter code that does not work in French. Every reference case added from now on would encode the bug | **Fix now, as lot B2** (same branch or a stacked one), S effort, S2 severity |
| BUG-18 silent forced model attempt | No. It is local to the tier 2 "Demander à l'IA" path; the mixed tier has its own degrade lines. It shares its cause with BUG-12 (the model is consulted for questions the data settles) | Lot C, with BUG-12 and the interface fixes; copy and one field only, no model-facing text |
| BUG-06 entity linking, BUG-16 hidden computed figure (Q24, Q23 conv 1) | Lot D is the lot that fixes them. Lot E's template filling needs the document entity linked to the table row, so D must precede E. Nothing in B2 or C depends on them | Stay in lot D. Do not add more mixed-tier behaviour before D |
| Q24 gate (document-reference phrases) | Lot D will redesign the gate (decision 4, still open). The phrase list is data and can be replaced | Keep as is; revisit in D |
| OBS-8 structural route ignores leftover words | Lot D's precondition. The same class as BUG-17, but it was coherent in the one case seen | Stays open; re-examine inside lot D together with the precondition |
| BUG-15 remainder, UX-1 (Q14 with both ticked) | No. Asking which workbook is the safe behaviour; UX-1 only removes the retyping | Lot C or later; no dependency |
| BUG-12 | Interacts with BUG-18. Contradicts D6 (the model is the interpreter of last resort) | Owner decision first |
| Not verified live: 0-row note, BUG-11, BUG-05 on a live question | Nothing stacks on them | Replay with the questions of the replay list in 07 and in 06 |

Conclusion: **only BUG-17 must be fixed before moving on.** The mixed-tier failures of Q24 are real but are the
reason lot D exists; fixing them earlier would mean patching the same entry gate twice.

---

## Triage after the lot B2 replay (4 October 2026)

Source: [07-retest-lot-b.md](07-retest-lot-b.md), replay of 4 October. Fifteen checks, no regression, all lot B
and B2 targets confirmed live except BUG-18's new line (unit-tested only). Open items and whether a later fix
stacks on them:

| Open item | Does later work rely on it? | Decision |
| --- | --- | --- |
| Q24 gives the quote's 1 200,00 instead of 1 450 | Lot D owns it; lots C and E do not read the table block. Lot E (template filling) needs the same entity linking, so D precedes E | Stays in D |
| BUG-19 spelled-out threshold | No | Backlog |
| UX-5 single-day wording | No | Lot C-a, copy only |
| BUG-18 line not seen live | No | Lot C-a replay |
| BUG-12 missing sheet (Q20) | No | Backlog |
| BUG-16 hidden computed figure | Lot D | Stays in D |

**Nothing blocks lot C.** Lot C is split into C-a (no model-facing change) and C-b (conversation memory, needs
approval); C-b precedes lot D. Plan: [08-lot-c-plan.md](08-lot-c-plan.md).

## Owner decisions of 4 October 2026

These close the "Decisions to take" list above. Recorded in `docs/DECISIONS.md` too.

| # | Decision |
| --- | --- |
| 1. BUG-10 | An internal name shown to the user is replaced by the name the application already uses: "Dossier des documents" / "Documents folder" for `WORK_FOLDER_CONTEXT`, and the equivalent for data-side labels, **on display only; the code identifiers do not change**. The audit asked for on the data folder is in 08 (C-a1) |
| 2. BUG-09 | The memory of the conversation does not depend on which documents are ticked. Sources (document texts, table data) **outrank** the memory, formally, for every model and above all the smallest: a small model remembers the sources first, then at most the previous question. Limit what is sent to small-context models. **Any change to prompts or to what the model reads is announced with its plan before it is tried** |
| 3. Router order | With documents and tables both ticked, "data only first" is not the default; the mixed tier is. Whether the order in which files are cited in the sentence can drive the router: see 08, "Looking ahead" |
| 4. Models | Development must not depend on the capabilities of the models on this machine ("who can do more can do less"): get acceptable results with very small models first. The pilot server will be much more powerful. `llama3.2:3b` is installed here and untested: to be tried at the end of the fixes. Other models, installable thanks to the modular configuration, will be tested in later phases |
| BUG-12 | (4 October, earlier) Refuse at once when the data settles it (value with no close match); D6 narrowed |
| Q19 | (4 October, earlier) Refuse a threshold on a group's total while the capability does not exist |
