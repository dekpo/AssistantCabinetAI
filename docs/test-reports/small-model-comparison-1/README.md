# Small-model comparison on the owner's PC (pass 1)

9 October 2026 · written for the owner and for any agent choosing a model · English, with the French answers quoted as data.

## What this is, and what it is not

A by-product of the lot 2 human test (`docs/test-reports/knowledge-base-pass-1/human-tests/lot-02-normalize-phonetic.md`, Part B). The owner replayed the battery on the default model, then asked the same five questions to every small model she had pulled, clearing the conversation before each answer so no memory could interfere. The raw conversations are in [transcripts.md](transcripts.md); this file is the analysis.

It is **one run per cell**, five shared questions, a rubric written by the agent, on one CPU-only machine. It ranks failure modes well and ranks speed and quality only roughly. Do not read a decimal into any number below. Section 8 lists what a second pass must change.

## 1. Setup

| Item | Value |
| --- | --- |
| Machine | The owner's development PC. Docker VM of about 11.5 GiB, **CPU only** (`.env` comment of 7 October), `OLLAMA_MAX_LOADED_MODELS=1`: a model is loaded when it is asked for, and **unloaded after 5 minutes idle** (`OLLAMA_KEEP_ALIVE` is not set, the Ollama log says `5m0s`). **Measured afterwards (section 10): loading takes 36 to 210 seconds on this PC, so the times below probably include model swaps; the order the owner asked in was not recorded.** Read the speed columns as orders of magnitude, not as inference speed |
| Gateway | `MAX_OUTPUT_TOKENS=2048`, `LLM_REQUEST_TIMEOUT_SECONDS=300`, `DEFAULT_OUTPUT_LOCALE=fr-FR`; no setting that turns a model's "thinking" on or off (no `think` handling in `apps/server`) |
| Time | The `Généré par <model> en <time>` line of each answer |
| Conversation | Cleared before every question |
| Documents | The owner's fictional `Docs\Test` and `Data\Test` folders (README of the fixtures: `docs/test-reports/human-acceptance-pass-1/fixtures/README.md`) |
| Questions | JAN `Que dit 2026/janvier/neurologie.pdf ?` · MAR `Que dit 2026/mars/neurologie.pdf ?` · M1 `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans le devis signé ?` · M2 `Quel est le montant total pour le fournisseur mentionné dans cette lettre ?` · N1 `Rédige un e-mail de relance pour un fournisseur dont la facture n'est pas réglée.` |

## 2. The models and their licences

Licence checked on 9 October 2026 on the upstream model card (Hugging Face), because the Ollama page of `gemma4`, `qwen3` and `qwen3.5` states no licence. The Ollama conversion's own licence file was not read. **All eight are Apache 2.0, so none is excluded for its licence.** Rows are in `models/LICENSES.md`.

| Tag | Disk | Window set in `.env` | Model's own limit | Thinking by default | Licence | Status after this pass |
| --- | --- | --- | --- | --- | --- | --- |
| `ministral-3:3b` | 3.0 GB | 8192 | 256K | not stated | Apache 2.0 | **candidate** |
| `granite3.1-moe:3b` | 2.0 GB | 8192 | 128K | no | Apache 2.0 | fast alternative, weak facts |
| `granite3.1-moe:1b` | 1.4 GB | 4096 | 128K | no | Apache 2.0 | dropped |
| `qwen2.5:1.5b` | 986 MB | 4096 | 32K | no | Apache 2.0 (the card says all sizes except 3B and 72B) | **current default; not fit for factual answers** |
| `qwen3:1.7b` | 1.4 GB | 4096 | 32K (Ollama: 40K) | yes (card) | Apache 2.0 | fast challenger |
| `qwen3:0.6b` | 522 MB | 4096 | 32K | yes (card) | Apache 2.0 | dropped |
| `gemma4:e2b` | 4.6 GB | 4096 | 128K | a thinking mode exists (card) | Apache 2.0 (card metadata) | accurate, too slow here; retest on the Mac mini |
| `qwen3.5:2b` | 2.7 to 3.1 GB | 4096 | 256K | no (card) | Apache 2.0 | removed by the owner: empty answers; not measured |

## 3. Rubric

Written before reading the scores. **2 (PASS):** the facts are right and complete enough, grounded in the file, in French. **1 (PWI):** the facts are right but incomplete, padded, garbled or loosely worded. **0 (FAIL):** a wrong or invented fact, a wrong conclusion, a sentence that contradicts the engine block printed under it, a wrong language, no draft where one was asked, or a degenerate repetition. M2's engine block (1 450 over 3 lines) was right in all seven answers: the score is for the **model's sentence** above it.

## 4. Results: five shared questions

Cell = score, then the time. The facts behind each score are in section 6.

| Model | JAN | MAR | M1 | M2 | N1 | Score /10 | Mean time |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `ministral-3:3b` | 2 (1m37) | 2 (1m30) | 2 (1m25) | 1 (2m13) | 2 (1m59) | **9** | 1m45 |
| `gemma4:e2b` | 2 (2m20) | 2 (2m14) | 2 (2m28) | 1 (2m59) | 2 (5m34) | **9** | 3m07 |
| `qwen3:1.7b` | 1 (50 s) | 1 (59 s) | 2 (1m10) | 0 (57 s) | 1 (1m06) | 5 | 1m00 |
| `granite3.1-moe:3b` | 2 (45 s) | 1 (46 s) | 0 (37 s) | 0 (54 s) | 1 (1m01) | 4 | 49 s |
| `qwen2.5:1.5b` | 1 (42 s) | 0 (39 s) | 0 (48 s) | 1 (29 s) | 1 (41 s) | 3 | 40 s |
| `granite3.1-moe:1b` | 2 (32 s) | 1 (29 s) | 0 (26 s) | 0 (37 s) | 0 (47 s) | 3 | 34 s |
| `qwen3:0.6b` | 0 (31 s) | 1 (28 s) | 1 (35 s) | 0 (40 s) | 0 (28 s) | 2 | 32 s |
| `qwen3.5:2b` | 0 (1m41) | not run | not run | not run | not run | excluded | n/a |

By kind of question (mean time, seconds): one file (JAN, MAR) · documents plus table (M1, M2) · nothing selected (N1).

| Model | One file | Documents plus table | Nothing selected |
| --- | --- | --- | --- |
| `gemma4:e2b` | 137 | 164 | **334** |
| `ministral-3:3b` | 94 | 109 | 119 |
| `qwen3:1.7b` | 55 | 64 | 66 |
| `granite3.1-moe:3b` | 46 | 46 | 61 |
| `qwen2.5:1.5b` | 41 | 39 | 41 |
| `granite3.1-moe:1b` | 31 | 32 | 47 |
| `qwen3:0.6b` | 30 | 38 | 28 |

Reading: the file does not change the time much for the small models (an excerpt of one page is short). A longer prompt (documents plus a table) adds about 15 to 20 % on `gemma4:e2b`, `ministral-3:3b` and `qwen3:1.7b`, and nothing visible on the others. The one outlier is `gemma4:e2b` writing an e-mail with nothing selected: 5m34, more than twice its other answers, with the shortest prompt. Explained in part by section 10: loading `gemma4:e2b` took 210 s on its own, and on a real prompt it also writes about 1 600 characters of hidden reasoning before answering.

## 5. The wider battery on `qwen2.5:1.5b` (the default alias)

Only this model answered the whole starred battery. Facts below were checked against the fixtures' text (`fixtures/README.md`).

| Step | Question (short) | Answer, in substance | Score | Time |
| --- | --- | --- | --- | --- |
| D8 | `Que dit BAIL-CABINET-2024 ?` | The right file, address, 85 m², landlord, 18 400 euros, and the dates 1 April 2024 to 31 March 2033, **but "dix-neuf ans"**: the lease says nine | 0 | 34 s |
| D9 | `Que dit courrier-cpam-radiation ?` | Struck off from 1 February 2026, new employer; copies the social security number correctly; the product printed "Le chiffre « 69 123 456 » ne figure dans aucun des extraits" under it: a **false warning** (KBD-05) | 1 | 30 s |
| C1 | CPAM letter, radiation | Hugo Exemple, 1 February 2026, new employer | 2 | 43 s |
| C3 | Lease duration | "NEUF ANS", 1 April 2024 to 31 March 2033. Right; D8 gave the opposite number for the same lease | 2 | 45 s |
| C6 | Printer price, MedSupply quote | "montant total du devis … 1 200,00 euros HT". Does not say the printer alone is unpriced | 1 | 44 s |
| C7 | Same, quote not selected | Refuses, no price. Wording says "la liste des fichiers" instead of the selected documents | 2 | 43 s |
| C9 | Consultation of 9 March 2026 | "La consultation de Mme Alice Exemple a eu lieu le 9 mars 2026": right and empty | 1 | 41 s |
| C10 | Civil-liability insurance amount | Says no amount is documented (right), **but opens with "Le cabinet est facturé à la Sécurité Sociale pour son assurance responsabilité civile"**, an invented claim, then six lines of "sources say nothing" | 0 | 58 s |

Five of eight rows lose points; two of them (D8, C10) on a wrong or invented fact, and section 4 adds two more (MAR, M1). **A model that answers "nine years" in C3 and "dix-neuf ans" in D8 for the same lease, in the same battery, is not fit to be the default** for questions about the owner's documents.

## 6. What went wrong, per model

| Model | Failure modes seen |
| --- | --- |
| `qwen3:0.6b` | JAN answered in **English** and invented the subject ("anxiety-related sleep disorders" for tension headaches). N1: no draft ("Aucun document n'a été attaché…"). M1: an English fragment ("computed from 3 rows"). Fastest, unusable |
| `granite3.1-moe:1b` | M1 states that MedSupply billed 1 200,00 (the quote's figure) and answers "Non". MAR: a garbled last sentence. N1: the whole draft repeated three times, with stray "[Réponse en français]" and "Réponse:" lines, and the sender's role reversed ("nous avons commis une erreur") |
| `granite3.1-moe:3b` | M1: "est égal à 1 200,00 … correspond à ce que le devis propose" while the engine block says 1 450. MAR: facts right, then a long invented-sounding disclaimer. N1: a draft followed by a meta sentence ("Dans ce cas, aucune facture n'est sélectionnée…") and "Subject :" in English. Verbatim excerpts (JAN) are good |
| `qwen2.5:1.5b` | MAR: "**électroencéphalogramme**" for électromyogramme, a wrong medical fact. M1: "**Oui**, le montant … correspond", directly above an engine block that says 1 450 against 1 200. See section 5 for the rest |
| `qwen3:1.7b` | JAN: "sans examen clinique d'alerte" misreads "sans signe d'alarme à l'examen clinique", adds "Mr.", and the meta remark "Il est indexé, lisible". M2: tells the reader the table figure "ne doit pas être utilisé pour établir le montant réel", which contradicts the product's rule that the engine's number is the computed truth. M1 is right and clear |
| `gemma4:e2b` | Accurate and close to the text everywhere (it quotes the excerpt). M2 answers with the quote's total only. The cost is time: 2m14 to 5m34 on this PC |
| `ministral-3:3b` | Never contradicts the engine block (nor does `gemma4:e2b`), cites the quote number (DV-2026-0117) in M1, and writes a clean e-mail. One slip: in M2 it says the supplier's total "n'est pas précisé dans le devis", while the quote does give 1 200,00. One and a half to two minutes per answer here |
| `qwen3.5:2b` | JAN: adds "Il ne faut pas juger un diagnostic immédiat à ce stade" and "pour le médecin traitant", neither in the file. The owner removed it for empty answers elsewhere; cause not established (section 7) |

## 7. Observations on the product visible in these answers

Recorded in `docs/test-reports/knowledge-base-pass-1/defects-register.md` with their effect on the KB lots:

- **KBD-04** the file name appended by the "which workbook?" button is read as filter words (`Compris comme : date en 2026`, `date en mars et date en 2026`). The counts were right only because every row of both files matches.
- **KBD-05** the number guard warns about a correct social security number because the source splits it across a line.
- **KBD-06** on 7 of 14 mixed answers (M1 and M2, seven models), the model's sentence contradicts or dismisses the engine block printed under it.
- **KBD-08** a model that returns no text (`qwen3.5:2b`) is shown as a blank message, with no error.
- **KBD-07** the continuous integration has never been green (0 of 40 runs since 21 September). Not a model matter; found while checking whether lot 2 could be merged.

One thing is **not** a window effect: the Ollama log (`docker logs assistant-cabinet-ollama`, 108 completed requests, chat and embedding, since the container started on 9 October at 15:55 UTC) has **no `truncated = 1`**; the largest prompt-plus-answer was 2 941 tokens, under the 4 096 window; the windows applied were 4096 and 8192 (2048 is the embedding model's). So the 4 096 windows did not cut these prompts. They would on a long document: the desktop may send up to about 8 000 tokens of excerpts plus a 2 048-token answer (the `.env` comment).

The log covers one container run, not necessarily every question of the afternoon, and says nothing about the earlier runs of `qwen3.5:2b`.

## 8. Limits, and what a second pass must change

1. One run per cell; a model's answer varies from one run to the next. Repeat each question three times.
2. Five shared questions, three of them on facts. Add the refusals (C7, C10: "the documents do not say"), which matter most for this product, D8 and D9, and a table question the model must phrase.
3. **Two window sizes are mixed** (8192 for `ministral-3:3b` and `granite3.1-moe:3b`, 4096 for the others). `granite3.1-moe:3b` scores low at 8192, so the window alone does not make a model good, but the data cannot separate the two effects. Run every model at the same window (8192) before concluding that ministral's lead is the model and not the window.
4. Thinking: **measured in part on the same day (section 10)**: it exists, it is hidden, and `think: false` removes it. Still to do: score the answers of `qwen3:1.7b` and `gemma4:e2b` with thinking off, since the quality may change.
5. Warm and cold timings separately: **measured in part (section 10)**. Still to do: ask every model its questions in a row (one model at a time, not one question across models) so that only the first answer pays the load.
6. Open the Sources disclosure and paste it: no answer here was judged on its sources.
7. Write the machine down (CPU, RAM, Docker memory). **None of the times transfers to the Mac mini M5 Pro**; the order of the models by quality probably does, their order by speed does not.
8. What the product showed for the `qwen3.5:2b` empty answers: the owner reports `Assistant: (vide)`, then Sources, then `Généré par qwen3.5 en 1m25s`. A blank message with no error: KBD-08. The probable cause is the hidden reasoning of section 10 using up the 2 048-token budget; for this model it is not measured, it was removed from disk.

## 9. Recommendation

- **Most precise and, of the two that are as precise, the faster: `ministral-3:3b`** (9 of 10 at about 1m45 here; `gemma4:e2b` scores the same at 3m07). With `gemma4:e2b` it is the only one that never contradicts the engine block. It is the model to use for the human tests of the lots that follow, and the reference to beat.
- **Fastest model that stays usable: `qwen3:1.7b`** (5 of 10 at about one minute). Keep it as the speed challenger for the second pass **with thinking off**, which made it four times faster on a 2 000-token prompt (section 10); its score may change either way.
- **`gemma4:e2b`:** keep for the Mac mini, where its time may collapse.
- **Drop for factual use:** `qwen3:0.6b`, `granite3.1-moe:1b`, `qwen2.5:1.5b`, `qwen3.5:2b`. `granite3.1-moe:3b` has good excerpts and fails on mixed questions.
- **`DEFAULT_MODEL_ALIAS=qwen2.5:1.5b` should change** (to `ministral-3:3b`) for any human test of a lot that depends on a model's sentence. That is the owner's setting and was not changed. The `models/LICENSES.md` note "latency witness, not a pilot quality candidate" already said as much.

No single model is both the fastest and the most precise on this PC: the fast ones invent, the precise ones take one to three minutes. That gap is what the Mac mini and the thinking test have to close.

## 10. Measured afterwards: load time and hidden reasoning (same day, same PC)

The agent called Ollama directly (`/api/chat`, the call the gateway makes) with a prompt of about 2 000 tokens (six copies of the January excerpt and the JAN question), using the counters Ollama returns. Cold means another model was resident just before.

| Model | Cold load | Cold, reading the prompt | Warm answer | Hidden reasoning (characters) | Warm with `think: false` |
| --- | --- | --- | --- | --- | --- |
| `ministral-3:3b` | 137 s | 98 s | 30 s (148 tokens) | none | not applicable |
| `qwen3:1.7b` | 64 s | 41 s | 44.8 s (398 tokens) | about 1 100 | **10.9 s (101 tokens)** |
| `gemma4:e2b` | **210 s** | 51 s | 54.6 s (548 tokens) | about 1 650 | not measured on this prompt |
| `qwen3:0.6b` (one short question) | n/a | n/a | 4.6 s (157 tokens) | about 500 | 0.8 s (12 tokens) |

Cold load on a one-line question: `granite3.1-moe:1b` 36 s, `qwen2.5:1.5b` 49 s, `ministral-3:3b` 154 s.

What this changes:

1. **The speed column of section 4 mixes loading and answering.** `ministral-3:3b` answered in 30 s warm and 264 s cold; the owner's 85 to 133 s sit between the two. The precision ranking is not affected; the speed ranking is an order of magnitude only.
2. **The thinking hypothesis is confirmed** for `qwen3` and for `gemma4:e2b` on a real prompt (not on a one-line question, where `gemma4:e2b` did not think). The reasoning is generated, takes time, counts against the 2 048-token answer budget, and is thrown away by the gateway, which reads only `message.content` (`providers/ollama.py`). With thinking off, `qwen3:1.7b` was four times faster on this prompt; the visible answer of the one-line test was identical on `qwen3:0.6b`. Whether the quality holds on the real questions is the second pass's job.
3. **`think: false` is accepted by every model tried** (`ministral-3:3b`, `granite3.1-moe:1b`, `qwen2.5:1.5b`, `qwen3:*`, `gemma4:e2b`): no error on a model without thinking. Asking for thinking on such a model (`think: true`) is refused with HTTP 400 (`"ministral-3:3b" does not support thinking`), so the setting should only ever send "off".
4. **Idle time costs a reload.** With the default 5 minutes, the question asked after a quiet moment in a practice waits for the load: 36 to 210 s here. The Mac mini has the memory to keep a model resident for hours; this is the setting `OLLAMA_KEEP_ALIVE`, not set in `compose.yaml` today.

The probe prompt is longer than the product's typical 700 to 1 100 tokens, and a second identical prompt is read from Ollama's cache (0.2 s), so warm figures understate the reading cost.

## 11. The deterministic steps

Not a model matter. Every step answered without the AI matched its expected value: D1, D4, T1, T2, T4, T5, T7, T9, T10, T12, T15, M3 (the totals 2 215, 620, 550 / 1 450 / 215, 3, 4, "Salle 3, 50"). Details: `docs/test-reports/knowledge-base-pass-1/human-tests/lot-02-results.md`.
