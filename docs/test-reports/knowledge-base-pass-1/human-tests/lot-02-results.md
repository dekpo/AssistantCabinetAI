# Human test, lot 2 - results

9 October 2026 · branch `feat/kb-normalize-phonetic` (pull request #22) · run by the owner · verdicts written by the agent from the conversations the owner pasted · English, French quoted as data.

Protocol: `lot-02-normalize-phonetic.md`. Verbatim conversations: `docs/test-reports/small-model-comparison-1/transcripts.md`. Model comparison and recommendation: `docs/test-reports/small-model-comparison-1/README.md`.

## What was run, what was not

- **Part B, the essential subset: all 26 starred steps were run** (D1, D4, D5, D6, D8, D9, C1, C3, C6, C7, C9, C10, T1, T2, T4, T5, T7, T9, T10, T12, T15, T16, M1, M2, M3, N1). The unstarred steps D2, D3, D7, D10, D11, C2, C4, C5, C8, T3, T6, T8, T11, T13 and T14 were **not run**. P1 and P2 were not reported separately; D4 shows the eight documents as analysed.
- **Part A (A1 to A8): not reported yet.** The table of sounds was read before; `git status` shows `phonetic-fr.json` unmodified, which is the state A6 asks for. The owner confirms A1 to A8 in one line before the merge.
- **The Sources disclosure was not pasted** for any answer. The criterion "the only source is X" is therefore **not verified** anywhere below; the content of each answer comes from the expected file in every case.
- The default alias that day was `qwen2.5:1.5b`. Block C, D5 to D9 and N1 were also asked to other models; see the comparison.

## Verdict on the lot

**No regression attributable to lot 2.** The only code the lot touched that the product uses is `fold_text`, which decides that `BAIL-CABINET-2024` means `bail-cabinet-2024.pdf` (steps D5 to D11). D5, D6, D8 and D9 each reached the right file (the facts quoted come from that file only), and the ambiguity question of D5 offered exactly the two `neurologie.pdf`. Everything found below exists independently of the lot and is recorded in `defects-register.md` (KBD-04 to KBD-07), none of which is a reason to hold the merge (rule of the protocol, Findings).

## Part B results

Two verdicts are given where an AI wrote the answer: the **product** (routing, engine, labels) and the **model** (facts, judged on the rubric of the comparison). A step answered by the engine has one verdict.

| Step | Observed | Product | Model |
| --- | --- | --- | --- |
| D1 | "Votre dossier de documents contient 8 fichiers." with "sans l'IA" | PASS | n/a |
| D4 | The 8 paths, each "analysé", "sans l'IA" | PASS | n/a |
| D5 | Asked which of the two `neurologie.pdf`, never picked; after the click the January letter: M. Hugo Exemple, 14 January 2026, tension headaches, no imaging needed | PASS (source not shown) | PASS for `gemma4:e2b`, `granite3.1-moe:3b`, `:1b`, `ministral-3:3b`; PASS-WITH-ISSUES `qwen2.5:1.5b`, `qwen3:1.7b`; **FAIL** `qwen3:0.6b` (English, invented subject), `qwen3.5:2b` (invented sentences) |
| D6 | Mme Alice Exemple, 9 March 2026, paraesthesias, electromyogram | PASS (source not shown) | PASS `gemma4:e2b`, `ministral-3:3b`; PASS-WITH-ISSUES `granite3.1-moe:1b`, `:3b`, `qwen3:0.6b`, `qwen3:1.7b`; **FAIL `qwen2.5:1.5b`** ("électroencéphalogramme") |
| D8 | An answer from the lease only: 14 rue des Tilleuls, 85 m², Jean Fontaine, 18 400 euros, 1 April 2024 to 31 March 2033 | PASS | **FAIL** (`qwen2.5:1.5b`): "dix-neuf ans" instead of nine |
| D9 | M. Hugo Exemple struck off from 1 February 2026, new employer; the product printed a warning on the correct social security number | PASS-WITH-ISSUES (KBD-05) | PASS |
| C1 | Hugo Exemple, 1 February 2026, new employer | PASS | PASS |
| C3 | "NEUF ANS", 1 April 2024 to 31 March 2033 | PASS | PASS |
| C6 | "montant total du devis … 1 200,00 euros HT"; no invented price; does not say the printer alone is unpriced | PASS | PASS-WITH-ISSUES |
| C7 | Refusal, no price ("je ne suis pas en mesure de répondre … liste des fichiers") | PASS | PASS-WITH-ISSUES (names "la liste des fichiers", not the selected documents) |
| C9 | "La consultation de Mme Alice Exemple a eu lieu le 9 mars 2026." | PASS | PASS-WITH-ISSUES (right and empty) |
| C10 | No amount given, but opens with an invented claim: "Le cabinet est facturé à la Sécurité Sociale pour son assurance responsabilité civile", then six lines of "sources say nothing" | PASS | **FAIL** (invented fact) |
| T1 | 2 215 on 8 lines | PASS | n/a |
| T2 | 620 | PASS | n/a |
| T4 | MedSupply 1 450, Fournitures Dupont 550, Papeterie Lefevre 215 | PASS | n/a |
| T5 | 3, "Compris comme : fournisseur = MedSupply" | PASS | n/a |
| T7 | Refusal: no value "Alfa", lists `date`, `fournisseur`, `montant`; no number, no suggested supplier | PASS | n/a |
| T9 | 3, "Compris comme : date un lundi" | PASS | n/a |
| T10 | 4, "Compris comme : date entre 2026-03-09 et 2026-03-15" (the old defect, 0, is gone) | PASS | n/a |
| T12 | "Salle 3, avec 50", with Salle 2 = 95 and Salle 1 = 100 | PASS | n/a |
| T15 | 2 215 at once, no question about the workbook | PASS | n/a |
| T16 | Asked which workbook; the three buttons gave 8, 5 and 10, as expected, **but** with spurious clauses "Compris comme : date en 2026" and "date en mars et date en 2026" (and none for the CSV) | **PASS-WITH-ISSUES** (KBD-04) | n/a |
| M1 | The engine block "Somme de montant : 1 450, 3 lignes, fournisseur = MedSupply" under every answer | PASS | PASS `gemma4:e2b`, `ministral-3:3b`, `qwen3:1.7b`; PASS-WITH-ISSUES `qwen3:0.6b`; **FAIL** `granite3.1-moe:1b`, `:3b`, `qwen2.5:1.5b` (say 1 200,00 was billed, or "Oui") |
| M2 | Supplier linked, the block gives 1 450 on 3 lines in all seven answers, never 2 215, never "la colonne n'est pas numérique" | PASS | PASS-WITH-ISSUES `gemma4:e2b`, `ministral-3:3b`, `qwen2.5:1.5b`; **FAIL** `granite3.1-moe:1b`, `:3b`, `qwen3:0.6b`, `qwen3:1.7b` (contradict or dismiss the block, KBD-06) |
| M3 | 2 215 with no model, and the line "les documents sélectionnés n'étaient pas nécessaires" | PASS | n/a |
| N1 | The label "Réponse sans vos documents." under all seven answers | PASS | PASS `gemma4:e2b` (5m34), `ministral-3:3b`; PASS-WITH-ISSUES `granite3.1-moe:3b`, `qwen2.5:1.5b`, `qwen3:1.7b`; **FAIL** `granite3.1-moe:1b` (the draft repeated three times, with stray lines), `qwen3:0.6b` (no draft) |

## Results table of the protocol, filled

| Step | Model | Expected (see protocol) | Observed | Verdict |
| --- | --- | --- | --- | --- |
| A1 to A8 | n/a | see Part A | not reported | **NOT-RUN** (owner to confirm) |
| P1, P2 | n/a | 8 files; 3 tables + 2 unreadable | D4 lists 8 analysed; the unreadable pair not shown | NOT-RUN (P1 implied PASS) |
| D1 to D11 | various | as above | D1, D4, D5, D6, D8, D9 run | PASS, with KBD-05 on D9; D2, D3, D7, D10, D11 NOT-RUN |
| C1 to C10 | `qwen2.5:1.5b` | as above | C1, C3, C6, C7, C9, C10 run | C1 and C3 PASS; C6, C7, C9 PASS-WITH-ISSUES; **C10 FAIL** (model); C2, C4, C5, C8 NOT-RUN |
| T1 to T16 | none | as above | T1, T2, T4, T5, T7, T9, T10, T12, T15, T16 run | PASS, T16 PASS-WITH-ISSUES (KBD-04); T3, T6, T8, T11, T13, T14 NOT-RUN |
| M1 to M3 | seven models | as above | all three run | product PASS; model verdict varies (KBD-06) |
| N1 | seven models | as above | run | product PASS; model verdict varies |

## Findings

| # | What was seen | Bug, pre-existing defect or idea | Register row / fixed in |
| --- | --- | --- | --- |
| 1 | The file name appended by the "which workbook?" button is read as filter words | Pre-existing defect (not lot 2) | KBD-04, deferred |
| 2 | A warning on a correct social security number split by a line break | Pre-existing defect (not lot 2) | KBD-05, deferred |
| 3 | The model's sentence contradicts the engine block on 7 of 14 mixed answers | Model quality, product mitigation to consider | KBD-06, deferred |
| 4 | CI has never been green (0 of 40 runs): `pnpm` 9 in the workflow against the project's `pnpm-workspace.yaml`, and 3 `ruff` findings in a script | Pre-existing, process | KBD-07, open |
| 5 | The default model `qwen2.5:1.5b` gives a wrong or invented fact in 2 of its 8 wider answers (D8, C10) and in 2 of the 5 shared questions (D6, M1) | Idea: change `DEFAULT_MODEL_ALIAS` for human tests | comparison README, section 9 |
| 6 | A model returning an empty answer (`qwen3.5:2b`): what the product showed was not reported | Question to the owner | comparison README, section 8 |
| 7 | A name the encoder gets wrong | None seen: Part A not reported | to add to `phonetic-fr.json` if any |
