# Knowledge Base programme — reports, human tests and validation (pass 1)

Written for a developer or an AI agent who was not in the sessions. The Knowledge Base (KB) is built in twelve lots (0 to 11), each in its own chat and on its own branch, so that the result can be compared with the baseline (`main` at `f27505f`, tag `kb-baseline`). This folder is the **versioned memory of that programme**: what each lot did, how it was verified, and what the owner saw when she ran it.

The long-lived specification is `docs/KNOWLEDGE-BASE.md`, written in lot 11. The working plan (invariants, schema, decisions D1-D10, lot order) is local scaffolding (`docs/SESSION-KB-*.md`, not versioned); what matters from it ends up in `docs/DECISIONS.md` (lot 0) and in the lot reports below.

## Layout

| Path | Written by | Holds |
| --- | --- | --- |
| `lots/lot-NN-<slug>.md` | the agent of lot NN | summary, files, decisions, deviations, invariants proved, verification numbers, measurements, what was not done, open questions, git block |
| `human-tests/lot-NN-<slug>.md` | the agent of lot NN, results filled in by the owner | step-by-step test in the real application, expected result per step, results table, findings |
| `00-baseline.md` | lot 0 | timings of the product before the KB (owner fills in) |
| `01-protocol.md`, `02-results.md`, `03-release-gate.md`, final report | lot 11 | the full manual protocol and the decision on making `auto` the default |

## Branches and tags

```text
main                 untouched until the release gate
kb/integration       receives each accepted lot by pull request
feat/kb-<topic>      one branch per lot
kb-baseline          tag on main before the programme
kb-after-lot-NN      tag on kb/integration after lot NN is merged
```

Compare two states with `git diff --stat kb-after-lot-03..kb-after-lot-04`, or the whole programme with `git diff --stat main...kb/integration`. Compare behaviour inside one build with the knowledge mode setting (`off` is the pre-KB behaviour).

## Lots

| Lot | Title | Branch | Capability milestone |
| --- | --- | --- | --- |
| 0 | Measure and unfreeze | `feat/kb-measure-and-unfreeze` | K-A |
| 1 | Store and migrations | `feat/kb-store` | K-A |
| 2 | Names, accents and sounds | `feat/kb-normalize-phonetic` | K-A |
| 3 | Ports, packs, resolver | `feat/kb-ports-and-packs` | K-A |
| 4 | Entities from documents | `feat/kb-document-ingestion` | K-B |
| 5 | Entities from tables | `feat/kb-table-semantics` | K-B |
| 6 | Targeted scope | `feat/kb-targeted-scope` | K-C |
| 7 | Targeting in the interface | `feat/kb-targeting-ui` | K-C |
| 8 | Manage knowledge | `feat/kb-management-dialog` | K-D |
| 9 | Shared vocabulary | `feat/kb-shared-vocabulary` | K-D |
| 10 | Activity profiles | `feat/kb-activity-profiles` | K-D |
| 11 | Prove it | `feat/kb-validation` | K-E |

Rules that every lot report and human test follows are in the workflow file the agents receive (`docs/SESSION-KB-WORKFLOW.md`, local). In short: a lot is *code done* when its report is written and its suites are green, *accepted* when the owner has run its human test, *merged* when its pull request is in `kb/integration`.
