# Human acceptance pass 1 (HAP-1) - 3 October 2026

First end-to-end manual test of Assistant Cabinet AI on a real workstation, run by the owner right after
the sprint 2b data work (sessions 6-16) was coded. It is the baseline every later pass is compared
against: **same fixtures, same questions, same models, same checks**, so that a change can be classified
as progress or as a regression.

This folder is written for a developer or an AI agent who was not in the session. Everything needed to
re-run the pass or to investigate a finding is here: the fixtures, the question catalogue, the results,
the screenshots, the bug register and the improvement plan. The French sentences quoted in these files
are test data (what a user typed, what the product answered), not documentation.

Reference name: **HAP-1**. Use it in chat, in commit messages and in `docs/DECISIONS.md`
("found in HAP-1, BUG-04").

## Files

| File | What it holds |
| --- | --- |
| [01-protocol.md](01-protocol.md) | Environment, fixtures with their verified ground truth, the 28 questions of the catalogue (Q1-Q28) with the expected behaviour, how to re-run the pass |
| [02-results.md](02-results.md) | Every question as it was actually asked and answered, which screenshot shows it, and a verdict checked against the fixture contents |
| [03-bug-register.md](03-bug-register.md) | BUG-01..BUG-15 and UX-1..UX-3: evidence, root cause found by reading the code (read-only), candidate fixes. **No code was changed in this pass** |
| [04-improvement-plan.md](04-improvement-plan.md) | The same findings ordered from the cheapest and safest fix to the most expensive, with what to re-test after each |
| [05-analysis-notes.md](05-analysis-notes.md) | Answers to the owner's open questions: model size and family, memory versus scope, the leaked internal label, the gateway timeout, the clickable-choices proposal, what the mixed tier did and did not do |
| [06-retest-lot-a.md](06-retest-lot-a.md) | Replay of five checks after the lot A fixes (4 October 2026): verdicts, the four Q23 conversations, a verification of the "history" hypothesis, new findings BUG-16, OBS-7, UX-4, updated bug status, a re-run recipe |
| [07-retest-lot-b.md](07-retest-lot-b.md) | Replay of the lot B checks (4 October 2026): verdicts verified against the fixtures, the Q19 root cause (BUG-17), BUG-18, OBS-8, updated bug status, replay list |
| [08-lot-c-plan.md](08-lot-c-plan.md) | The plan for lot C (C-a interface and routing, C-b conversation memory), written from the owner decisions of 4 October 2026, with the BUG-10 audit of internal names, the BUG-09 proposal awaiting approval, exit criteria and replay list |
| [09-retest-lot-c-a.md](09-retest-lot-c-a.md) | Replay of lot C-a (4 October 2026) on four models, the Q28 comparison, BUG-20 and BUG-21, the BUG-11 recipe now that the variable is forwarded, the C-b change and its measurement protocol |
| [screenshots/INDEX.md](screenshots/INDEX.md) | The 51 screenshots (`S01.png`..`S51.png`, chronological), what each one shows, and the concordance with the numbers the owner wrote in the original notes |
| [fixtures/README.md](fixtures/README.md) | The 6 documents and 2 workbooks used, copied from `C:\Users\elise\AssistantCabinetAI\{Docs,Data}\Test`, with their full text and the numbers computed by hand |

## Headline result

28 questions were proposed; 25 were run (Q22 was not run, Q25 is not a question, Q8 was repeated six
times, several others twice). Scored on the answer shown to the user:

| Layer | Result |
| --- | --- |
| Folder questions answered from the filesystem, no model (Q1-Q3) | Correct |
| Disambiguation instead of a guess (Q4, Q9) | Correct, but the user must retype the whole question |
| Tabular engine, plain aggregates (Q9-Q13, Q15, Q17, Q18) | Correct, checked against the workbooks |
| Tabular engine, filters read from the question (Q14, Q16, Q19 with "5000") | **Wrong numbers, presented as computed** (BUG-02) |
| Tabular model-assisted path (Q19, Q20, Q21) | Never produced an answer; one run crashed the app (BUG-01) |
| Documents + model, with the source selected (Q5-Q7, Q8-c) | Mostly correct with `ministral-3:3b`; `gemma2:2b` invented figures (Q8-a, Q6) |
| Documents + model, source *not* selected (Q8-d, e, f) | Correct refusal only when the conversation memory was empty (BUG-09); a leaked internal label (BUG-10) |
| Documents and tables selected together (Q23, Q24, Q26) | **The mixed tier was never reached.** The data-only router answered or refused first (BUG-03, 04, 05, 06) |
| Nothing selected (Q27, Q28) | Q27 depends on the model; Q28 was answered "no document selected" instead of a general answer (BUG-07) |
| Interface | The view does not scroll to the end of an answer (BUG-08) |

The tabular engine is trustworthy for what it recognises and wrong in specific, reproducible ways; the
model-written parts are only as good as the model; and the session 15/16 mixed selection does not yet
do what its documentation says.

## Conventions

- **Sxx** = screenshot file `screenshots/Sxx.png`.
- **Verdict** values: PASS, PASS-WITH-ISSUES (answer right, behaviour or text flawed), FAIL (wrong or
  missing answer), ERROR (the product reported a failure), NOT-REACHED (the code path under test was
  never executed), NOT-RUN.
- "Ground truth" always means a value computed by hand from the fixture files in `fixtures/`.
- Dates are written dd-mm-yyyy in the screenshots (the application's own format) and ISO in the text.

## Status of the code under test

Branch `feat/mixed-selection-data-routing`, working tree containing the uncommitted session 16 mixed tier
(`mixed_answer.rs`, `commands::mixed_tier`) and an unrelated uncommitted change lowering
`USABLE_MIN_ROWS` from 8 to 1 (`docs/DECISIONS.md`, "a short real export stays usable"). Both fixture
workbooks (8 and 10 data rows) were already above the old floor, so that change did not influence this
pass; both workbooks were green in the interface throughout.
