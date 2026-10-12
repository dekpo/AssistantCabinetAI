# 03 - The release gate: may « Automatique » become the default?

Written for the owner, who decides, and for the developer or agent who reads the decision later. English; the French sentences are what is typed or read on screen.
12 October 2026 · KB lot 11 · branch `feat/kb-validation`. The measures are in `measurements/` (one file per run) and summed up in `04-final-report.md`, sections 6 to 8.

## Verdict

**Owner's decision, 12 October 2026: the verdict is accepted, gate item 2 is rewritten as proposed in section 2 (the exact wording is in `docs/DECISIONS.md`, « Settled after KB lot 11 »), the fix battery is applied before the big human test, Switzerland first, and `kb/integration` is not merged into `main`.** Proposals P-1 to P-4 below are therefore accepted; P-5 is the rewritten human test.

**The gate is not met. « Automatique » stays optional and « Suggestions seulement » stays the default.** This is the answer the owner expected; it is now an answer
the figures give, not an impression, and the list of what would have to change before the question is asked again is section 4.

Three reasons carry the verdict, each measured:

1. **Recall is below 98 %.** On the real stack (real embeddings, real text recognition) the files that hold the answer are kept in **91.6 %** (tier 30), **95.9 %** (100) and **91.5 %** (200) of the cases where the base would
   narrow the search (`measurements/real-tier-*.md`). The questions about **a person** reach 95.2 %, 98.7 % and 98.0 %; the questions about **a practitioner** fall to 65.7 % at 200 documents, mostly through the Swiss title `Dre` (KBD-26), and
   the questions about an organisation or a place lose the files the cap of 15 cuts (KBD-29).
2. **A wrong confident entity was chosen** on 3 of 205 questions at 200 documents and 8 of 225 at 1 000 (KBD-36, KBD-26, KBD-29, KBD-27). The right file was still kept in most of them; the condition « no wrong confident entity choice » is not met.
3. **There is no gain on the wait the user feels.** The base shortens the search by milliseconds (median search 18.9 ms to 9.1 ms at 200 documents, one embedding call of about 300 ms unchanged) and the prompt by about 45 % in a sample of 8 questions, but the answer took
   10.7 s with « Suggestions seulement » and 11.1 s with « Automatique » (median of 7 warm questions; the differences per question, -3 s to +12 s, are noise). The condition « a real gain in time, or the report says so and `auto` stays opt-in » (11.7 item 4) is therefore answered by the second half.

What the base **does well**, and why it stays shipped as an option: it never read a file outside the selection (0 in 2 198 replays), it created nothing from a question, it keeps the answer's files for the questions that name a person who is written in full (100 % in the
categories `alias`, `homonym`, `identifier`, `leak`, `term` at 100 documents), it narrows 56 % of the questions of a 200-document library by a median 98 % of the files, and its own cost is 3 ms (p95 9 ms) per question and 1 to 3 % of an Analyse.

## 1. The grid (SESSION-KB-05, 11.7)

| # | Condition | Threshold | Measured | Status |
| --- | --- | --- | --- | --- |
| 1 | Invariants I1 to I14 green on both platforms | green | Windows: **1 248 tests pass, 0 fail** across 41 test targets (`cargo test`), including every invariant test; the sandbox asserts I1 (nothing outside the selection) on 2 198 replays and I3 (a question creates nothing) after each of them. **macOS: not run by the agent.** CI runs only `cargo test --lib`, which does not contain the `tests/knowledge_*.rs` targets (proposal P-4 below) | **partly**: Windows yes, macOS not shown |
| 2 | At least 100 real questions recorded in `suggest` on the owner's library, recall of the candidate set ≥ 98 % on would-apply questions, every miss classified and fixed or accepted in writing | 100 questions, 98 % | **Not yet played on her library** (Part E of the protocol). On the sandbox, same definition, files that hold the answer and that the base-off search cites: 91.6 / 95.9 / 91.5 % (real stack, tiers 30 / 100 / 200). Every miss is classified (`not_linked_to_the_name`, `capped`, `no_mention_in_file`, `weak_form_only`, `carry_over`, `entity_not_resolved`) and each cause has a row of the register | **not met** |
| 3 | Manual protocol complete; no wrong confident entity choice; no cross-scope leak; answers and citations at least as good as `off` | all | Protocol written, walked by the agent on a copy, **not yet played by the owner**. Wrong confident entity: 3 of 205 (200 documents), 8 of 225 (1 000). Leaks: 0. Answers and citations: at least one right source in 165 of 174 questions with a known answer in `auto` against 160 with the base off; the cited files that really hold the answer are 59.9 % in `auto` against 43.0 % with the base off; same first source in 86 of 112 reduced questions; the model found the expected fact in 8 of 8 generated answers in each mode | **not met** (wrong entity) / **pending** (her protocol) |
| 4 | Median reduction on selections of 30 or more files, and median end-to-end time improvement, reported honestly | reported | Median reduction **96 %** (30 documents), **98 %** (100 and 200), **99 %** (500 and 1 000). Time: see verdict 3: **no measurable improvement** of the answer; the search is 2x faster in milliseconds | **reported: no gain, so `auto` stays opt-in** |
| 5 | Knowledge lookup p95 ≤ 150 ms at 1 000 files, ≤ 30 ms at 50 files | 150 ms / 30 ms | **22 ms** at 1 000 documents (878 read; 24 ms in the benchmark); **6 to 8 ms** at 50 documents (41 read) | **met** |
| 6 | Documentation complete (11.8) | complete | `docs/KNOWLEDGE-BASE.md`, `docs/OPERATIONS.md`, `docs/DECISIONS.md`, the pointers in the existing documents, this folder | **met** |

## 2. How the figure is read (and why the raw one is low)

- The base-off search **always cites six passages** (`retrieval::MAX_EVIDENCE_CHUNKS`). For a person who has one or two files, four to five of the six are filler. A reduced set drops the filler on purpose.
  The **raw** figure (« of the files cited with the base off, how many does the reduced set keep ») is therefore low by construction: 31.3 %, 38.0 % and 41.6 % at tiers 30, 100 and 200 on the real stack, while the same reduced sets keep 91.6 %, 95.9 % and 91.5 % of
  the cited files that **really hold the answer**. The reduction dropped 231, 360 and 365 filler files and kept 22, 40 and 39.
- The application's own diagnostic gives the same pair on the 77 questions of the recording test: **raw 31.7 %** and **adjusted 100.0 %** (`lot11_helper.py report`: « a cited file counts only when it names an entity of the question »).
  The adjusted figure cannot see a relevant file that mentions none of the question's names (an extraction gap), which is the loss of KBD-26, KBD-27, KBD-31: it is an upper bound, not a proof.
- **Recommendation to the owner (the amendment of defect KBD-34):** replace the wording of 11.7 item 2 by « of the cited files that hold the answer, at least 98 % are in the candidate set », measured on the sandbox by its ground truth and on the owner's library by the protocol
  (« un fichier pertinent raté »). Under the old wording no reduction can ever pass; under the new one the sandbox says 91.5 to 95.9 %.

## 3. What decided each point, and what it is not

- The 98 % threshold is not missed by a small margin on the questions the base was built for: a patient named in full is found at 98 to 100 %. It is missed by **families of names the extractor does not read**:
  Swiss women doctors (`Dre`), hospitals and insurers without a marker word (`CHUV`, `HUG`, `SUVA`), a recipient alone on a line, a name split by a line break, a surname with an apostrophe, a file that writes only `M. Dupont`.
  The French-speaking Swiss market is a stated target, and these are its names.
- The widening rule cannot be calibrated by a score (KBD-17), and a short follow-up carries the previous person (KBD-19): both are measured in the real run (the search widened by itself on 31 of 112 narrowed questions at 200 documents, and 7 relevant files were lost to the carry-over).
- Nothing here says the knowledge base is not worth having: in « Suggestions seulement » it costs about 3 ms per question, changes nothing the user sees, and fills the record that Part E of the protocol reads.

## 4. What would reopen the question

In this order (each is a row of the fix battery, `04-final-report.md` section 9):

1. F-02 (`Dre` and the other Swiss titles), F-03 (a place never drives the reduction, and the cap does not cut a lexical-only set), F-16 (an AVS number or an acronym is not a file name), F-06 to F-08 (organisations and names the extractor does not read), F-09 (apostrophes, swapped names), F-17 and F-18 (the follow-up rule, the widening rule).
2. The sandbox re-run: the recall of the files that hold the answer must reach 98 % at 200 and 1 000 documents with zero wrong-entity questions.
3. The owner's Part E: more than 100 questions recorded in « Suggestions seulement » on her own library, `Release gate, adjusted` at 98 % or more and every `miss` classified.
4. A measured gain: the base shortens the search but not the answer. Unless a model that is slower on long prompts ships, or the evidence cap is raised, there is nothing to gain on the wait; the case for `auto` is then quality (fewer unrelated passages in the prompt: 59.9 % of the cited files hold the answer against 43.0 %), not speed.

If the owner prefers to decide earlier, the one-line change is `knowledge_mode` default in `settings.rs`, with an entry in `docs/DECISIONS.md`; **this report recommends against it today**.

## 5. Proposals for the owner

| # | Proposal | Why |
| --- | --- | --- |
| P-1 | Accept the verdict: `Suggestions seulement` stays the default, `Automatique` stays available in « Réglages » | sections 1 to 3 |
| P-2 | Amend 11.7 item 2 as in section 2 | the old wording cannot be met by any reduction (KBD-34) |
| P-3 | Order the fix battery (section 9 of the final report) and cut it into branches `fix/kb-<topic>` from `main` after the merge of `kb/integration` | every `deferred` row has a proposal |
| P-4 | Make CI run `cargo test` for all targets on Windows and macOS (today `cargo test --lib`) | the `knowledge_*` integration targets and the sandbox invariants are the KB's safety net; they ran only on the agent's Windows machine |
| P-5 | Play Parts 0 to R of `human-tests/lot-11-validation.md`; Part E needs several days of use | gate item 2 and 3 are only complete with her library |
