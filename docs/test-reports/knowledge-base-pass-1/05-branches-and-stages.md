# 05 - Branches, tags and stages of the Knowledge Base programme

Written for the owner, who wants to study and test the programme stage by stage and to compare the product with and without the Knowledge Base (KB), and for any developer or agent who has to find a stage again. English.
12 October 2026, read from the repository (local branches, `git ls-remote origin`, tags, merge commits). **Nothing was deleted, moved or merged to write it.**

## 1. Owner decisions that shape this report (12 October 2026)

1. **`kb/integration` is not merged into `main`.** `main` stays the product **without** the KB (tip `f27505f`, tag `kb-baseline`), so that the value of the KB can be compared with the product without it. `main` receives the KB only if the owner decides it later, after the comparison.
2. **Every branch and tag of the programme is kept**, local and remote, so that the product can be tested at each stage of the integration and unnecessary complexity can be spotted. Nothing in this report asks for a deletion.
3. The fix battery of the final report is applied **before** the big human test (it is a new branch, section 6).

## 2. What exists

All local branches have a remote twin at the same commit (verified: every `heads/*` of `origin` equals the local tip). There is no unpushed local branch. The old branch `feat/document-folder-plus-data` does not exist locally or remotely.

| Branch (local and `origin`) | Tip | Kind | In `kb/integration`? | What it holds |
| --- | --- | --- | --- | --- |
| `main` | `f27505f` | product | - | The product **without the KB**: the last merge is PR #19 (embedding batches and timeout). Not touched by the programme |
| `kb/integration` | `5d18c55` | programme trunk | - | `main` + the 45 commits (16 merged pull requests) of the programme, lots 0 to 9, the fixes, the withdrawal of lot 10 |
| `feat/kb-validation` | `5d18c55` + this lot's uncommitted work | lot 11 | not yet | The validation lot (library with ground truth, tests, protocol, reports). The tip equals `kb/integration` until the owner commits |
| `fix/kb-swiss-first` | `d2e0786` (merged by `fc1a425`, #37) | fix 4, batch 1 | yes (#37) | F-00, F-01, F-02, F-03, F-11, F-14, F-16, F-24 and the summary line of F-20 (`lots/fix-04-batch-1-swiss-first.md`). Tag: `kb-after-fix-04-batch-1` |
| `fix/kb-names-and-organisations` | `fc1a425` + batch 2 | fix 4, batch 2 | yes (#38) | F-06, F-07, F-08, F-09, F-10, F-04 (`lots/fix-04-batch-2-names-and-organisations.md`). Tag: `kb-after-fix-04-batch-2` |
| `fix/kb-retrieval-rules` | `15088d5` + batch 3 | fix 4, batch 3 | not yet | F-12, F-13, F-15, F-22, F-23 only (the owner left F-17, F-18, F-19 out: the setting stays `suggest`) (`lots/fix-04-batch-3-table-router-and-check-defects.md`). Tag to come: `kb-after-fix-04-batch-3` |
| `feat/kb-shared-vocabulary` | `2852694` | lot 9 | yes (#34) | Entity suggestions, transcript repair, outbound guard |
| `feat/kb-management-dialog` | `ffd7222` | lot 8 | yes (#33) | The « Gérer… » dialog |
| `feat/kb-targeting-ui` | `f67407b` | lot 7 | yes (#32) | The targeting line, chips, `@` picker, widen button |
| `feat/kb-targeted-scope` | `84c7747` | lot 6 | yes (#31) | The reduction rules, the lexical net, the widening |
| `feat/kb-table-semantics` | `d1e81fe` | lot 5 | yes (#29) | Entities from workbooks |
| `feat/kb-document-ingestion` | `cba9131` | lot 4 | yes (#28) | Entities from documents |
| `feat/kb-ports-and-packs` | `83b2e7b` | lot 3 | yes (#26) | Extractor and resolver ports, lexicon packs |
| `feat/kb-names-calibration` | `7653e31` | lot 2 bis | yes (#24) | Name keys aligned with the owner's review of 88 pairs |
| `fix/kb-surname-initial` | `15e387a` | fix 3 (KBD-15) | yes (#30) | `Dupont J` form |
| `fix/gateway-thinking-and-keep-alive` | `b86fe6c` | fix 2 (KBD-08) | yes (#25) | Thinking switch, empty answer code, keep-alive (server) |
| `fix/ci-pnpm-and-ruff` | `0c6e1ca` | fix 1 (KBD-07, KBD-09) | yes (#23) | CI made green |
| `docs/kb-lot-3-answers` | `6eb2ee3` | documentation | yes (#27) | The owner's answers after lot 3 |
| `docs/kb-lot-10-withdrawn` | `1b2069d` | documentation | yes (#35) | The withdrawal of lot 10 |

The branches of lots 0, 1 and 2 (`feat/kb-measure-and-unfreeze`, `feat/kb-store`, `feat/kb-normalize-phonetic`) were deleted by the owner after their merge (#20, #21, #22): **their stages are reachable by tag only** (section 3). No branch exists for lot 10, which was withdrawn the day it was built; its archive is in `data/withdrawn-lot-10/` (not in git).

## 3. The stages, in order (tags on `kb/integration`)

A tag marks the state of the whole product right after a lot was merged. `size` is the product source that stage added (Rust `src-tauri/src`, client `src`, server `src`, tests and docs excluded), measured with `git diff --numstat <previous tag> <tag>`. **A large number is a place to look for unnecessary complexity, not a verdict.**

| # | Tag | Merge | What the user can do or see at this stage | Size (lines added) |
| --- | --- | --- | --- | --- |
| baseline | `kb-baseline` = `main` | #19 | The product without the KB | - |
| 0 | `kb-after-lot-00` | #20 | Nothing new on screen: per-stage timings of a question and an Analyse (« Noter les durées dans un journal »), the lexical search held to the selection | +1 693 |
| 1 | `kb-after-lot-01` | #21 | Nothing on screen: the `kb_*` tables in `index.sqlite3`, migrations, the keyed hash for personal identifiers | +3 706 |
| 2 | `kb-after-lot-02` | #22 + docs | Nothing on screen: accents, titles, particles, French and English phonetic keys | +1 080 |
| fix 1 | (merge #23, no tag) | #23 | CI green (pnpm version, three long lines) | small |
| 2 bis | `kb-after-lot-02-bis` | #24 | Nothing on screen: the name keys calibrated on real public lists (88 pairs reviewed by the owner) | +376 |
| fix 2 | (merge #25, no tag) | #25 | The gateway: a thinking switch, `empty_answer`, keep-alive (server side) | +318 (server) |
| 3 | `kb-after-lot-03` | #26 | Nothing on screen: extractor and resolver ports, packs `health-fr`, `health-ch` | +4 688 |
| 4 | `kb-after-lot-04` | #28 | **The Analyse summary now says how many names were found**; names are read from documents | +4 362 |
| 5 | `kb-after-lot-05` | #29 | The same for workbooks (columns recognised, relations read) | +2 803 |
| fix 3 | `kb-after-fix-03` | #30 | `Dupont J` is a possible match of `Jean Dupont` | +679 |
| 6 | `kb-after-lot-06` | #31 | Behind the setting: the reduction rules, the lexical net, the widening (`knowledgeMode` in the settings file) | +3 710 |
| 7 | `kb-after-lot-07` | #32 | **« Ciblage intelligent » in « Réglages »**, the line under the answer, « Élargir à ma sélection », chips, `@` | +4 234 |
| 8 | `kb-after-lot-08` | #33 | **« Gérer… »**: the dialog (entities, edit, merge, split, delete, restore, reset) | +9 127 |
| 9 | `kb-after-lot-09` | #34 | Suggested questions, the sheet of a name; read-only services for speech, transcript repair, an outbound guard (no caller yet) | +4 261 |
| 10 | - | #35 | **Withdrawn**: no code; the decision is recorded | 0 |
| 11 | (to come) | - | Validation only: no product code | 0 |

Total, `main` to `kb/integration`: `src-tauri/src/knowledge/` 26 files, **+25 712 lines** (a large part is `#[cfg(test)]` modules); other Rust files 16 files, +3 394 -175; client `apps/desktop/src` 57 files, +11 248 -48; server 9 files, +318 -11; tests 30 files, +19 179; docs 55 files, +8 457.

**Where it touches code that existed before the KB** (the places to compare with `main` if the question is « did the KB complicate the product »): `commands.rs` +1 217 -69, `index_store.rs` +717 -20, `retrieval.rs` +387 -6, `indexing.rs` +300 -32, `settings.rs` +255, `tabular_answer.rs` +140, `data_folder.rs` +126 -11, `file_reference.rs` +47 -6, `tabular/csv_adapter.rs` +42 -4.
The biggest modules of the KB itself: `store.rs` 3 981 lines, `manage.rs` 2 143, `resolve.rs` 1 977, `ingest.rs` 1 695, `extract/text.rs` 1 484, `extract/table.rs` 1 413, `packs.rs` 1 278.

**Candidates for « complexity that is not needed yet »** (my reading, to confirm or reject by testing): the read-only services of lot 9 for speech, transcript repair and the outbound guard (`surface.rs` 446, `repair.rs` 343, `guard.rs` 379 lines) have **no caller in the product today**; the management dialog (lot 8) is the largest single stage (+9 127) and is used only to correct the base; `phonetic.rs` (820 lines) and `normalize.rs` (662) serve name matching whose benefit the lot 11 measure shows only for a few forms. The lot 11 measure says the recall limit is in the extractor, not in these.

## 4. How to test a stage (without breaking your installation)

The application reads and writes **one** settings file and **one** index (`com.assistantcabinetai.desktop`), whatever the build. An older build on an index migrated by a newer one is not guaranteed to work. So:

1. **Keep your own state safe, once:** close the application, then `python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py backup` (index, settings, key; a copy in `C:\Users\elise\kb-backup-lot-11`).
2. **Before every stage:** close the application, then `python docs\test-reports\knowledge-base-pass-1\human-tests\lot11_helper.py restore` (puts back the index and settings of step 1, so each stage migrates forward from the same starting point).
3. **Never run two builds at once.** One working folder, one stage at a time. Do not use `git checkout` on a branch with uncommitted work: commit first (the commands are in the answer that goes with this report).
4. **Go to a stage and build it** (from the repository root; replace the tag):

```text
git switch --detach kb-after-lot-04
cd apps\desktop
pnpm install --frozen-lockfile
pnpm tauri dev
```

The first Rust build of a stage takes several minutes. To come back to the current work: `git switch feat/kb-validation` (or the fix branch).
5. **What to look at at each stage** is the « what the user can do or see » column of section 3: stages 0 to 3 have no screen (use the timing log and the tests), stage 4 is the first visible one (the Analyse summary), stage 7 the first with the setting and the line under the answer.

**Comparing the same stage on the same sentences** is what Part A of `human-tests/lot-11-validation.md` does between the three modes.

## 5. Comparing with and without the KB

Three levels, from cheapest to most faithful:

1. **Inside one build, mode « Désactivé »** (`knowledgeMode` = `off`): names are not read at the Analyse, nothing is looked up at question time, the search is the one that has always run. This is the comparison of Part A, and the only one that holds the model, the library, the index and the machine constant.
2. **`main` against `kb/integration`**: `main` has none of the programme, **including the two non-KB fixes** (the thinking switch of the gateway and the CI), so a difference in an answer can come from them. For a clean build of `main`: commit your work, then `git switch --detach main`, build as in section 4, and come back. Use `restore` before each.
3. **A control branch** (optional, proposed): `main` plus only the two non-KB fixes, so that the only difference with `kb/integration` is the KB itself. It is three cherry-picks (`d0125c5` and `f536b9f` for the CI, `b86fe6c` for the gateway); the owner decides whether it is worth a branch. Not created.

Expected size of the effect, from lot 11 (`03-release-gate.md`): in the same build, `Désactivé` against `Automatique` differ by milliseconds in the search and by which files are cited (43.0 % of the cited files hold the answer without the KB, 59.9 % with it, 200 documents); the answer takes the same time.

## 6. The fix branches that follow

The fix battery of the final report (24 fixes, section 9) is applied on **new branches cut from the lot 11 commit**, in batches, each merged into `kb/integration` and never into `main`. The plan, the order and the proof required for each fix are in `docs/SESSION-KB-FIX-04-battery.md` (a local file, like the other launchers). New branches will appear in the table of section 2 as they are created.

## 7. Facts to remember

- `kb/integration` and `main` share the commit `f27505f`; `git diff --stat main...kb/integration` is the whole programme; `git diff --stat kb-after-lot-04..kb-after-lot-05` is one stage.
- A tag is a state of the **whole** product, not of one lot's files: always compare two tags, not a tag and a branch.
- The remote and the local copies agree today. If a branch is ever deleted, its tag (lots 0 to 9) and its merge commit (`git log --first-parent main..kb/integration`) still hold the stage.
- No AI mention appears in any commit, branch name or pull request of the programme (`AGENTS.md`).
