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
