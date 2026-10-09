# Model licence register

A weight without a row in this file is not loaded on a practice machine.

How to add or remove a model without restarting Compose: `models/README.md`.

Review date: 2026-09-21. **Licence policy of 7 October 2026: only the free-licence families (Apache 2.0, MIT) listed in `docs/MODELS.md`, "Licence policy", are used from now on.** A row for each model the owner pulls from that list is still required before it is loaded on a practice machine; the register below predates the policy and still holds rows for families that are now out (Llama, Gemma 3): they are kept as history, not as candidates.

**Currently loaded** (matches `.env`'s `MODEL_ALIASES` on this machine, 9 October 2026): `ministral-3:3b`,
`granite3.1-moe:3b`, `granite3.1-moe:1b`, `qwen2.5:1.5b`, `qwen3:1.7b`, `qwen3:0.6b`, `gemma4:e2b`, and the
embedding weight `nomic-embed-text` (`bge-m3` is also on disk, not the active embedding alias). Everything else
below is a past or future bench candidate, kept for its licence review, not necessarily on disk right now —
check `docker compose exec ollama ollama list` for what is actually pulled.

**Licence check of 9 October 2026 (agent):** the eight chat tags above, plus `qwen3.5:2b` which the owner pulled
and removed, are all **Apache License 2.0**. Source of each: the upstream Hugging Face model card. The Ollama
library page states the licence for `ministral-3`, `granite3.1-moe` and `qwen2.5` only; for `gemma4`, `qwen3`
and `qwen3.5` it states none, so the card of the upstream weight is the evidence. The licence file inside each
Ollama conversion was not read. Measured comparison of these tags:
`docs/test-reports/small-model-comparison-1/README.md`.

## mistral (witness)

| Field | Value |
| --- | --- |
| Ollama name | `mistral` (alias of `mistral:7b-instruct`, v0.3) |
| Hugging Face source | [mistralai/Mistral-7B-Instruct-v0.3](https://huggingface.co/mistralai/Mistral-7B-Instruct-v0.3) |
| Licence | Apache License 2.0 |
| Commercial use | Allowed under Apache 2.0 |
| Why this one | First localhost trial, usable French, permissive licence |
| Health / legal limits | Not a medical device. Admin drafting only. A human must review every letter. Do not use for diagnosis, prescriptions, or any send (mail, MSSanté, DMP). |
| Next review | Before any real patient text, and again before a Mac mini deploy |

```text
docker compose exec ollama ollama pull mistral
```

## nomic-embed-text (embedding, currently loaded)

| Field | Value |
| --- | --- |
| Ollama name | `nomic-embed-text` (library pull) |
| Hugging Face source | [nomic-ai/nomic-embed-text-v1.5](https://huggingface.co/nomic-ai/nomic-embed-text-v1.5) |
| Licence | Apache License 2.0 |
| Commercial use | Allowed under Apache 2.0 |
| Why this one | Small (~274 MB), fast, multilingual embedding weight behind the `assistant-embed` alias. Not a chat model: it builds the retrieval vectors, never answers (`docs/RETRIEVAL.md`) |
| Health / legal limits | Not a medical device. Never surfaced to the GP: the embedding alias is hidden from the chat-profile picker (`docs/MODELS.md`, "Alias policy") |
| Next review | Before a Mac mini deploy. Re-bench against `bge-m3` on French retrieval quality first |

```text
docker compose exec ollama ollama pull nomic-embed-text
```

## qwen2.5:1.5b (fast-profile bench, currently loaded)

| Field | Value |
| --- | --- |
| Ollama name | `qwen2.5:1.5b` (library pull) |
| Hugging Face source | [Qwen/Qwen2.5-1.5B-Instruct](https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct) |
| Licence | Apache License 2.0 |
| Commercial use | Allowed under Apache 2.0 |
| Why this one | Fast and reasonably accurate on this low-resource dev machine behind `assistant-turbo`; the current `DEFAULT_MODEL_ALIAS`. Not a pilot quality candidate at this size — a latency witness while waiting for the Mac mini. Measured on 9 October 2026 (`docs/test-reports/small-model-comparison-1/README.md`): a wrong or invented fact in 4 of its 13 answers, so it should not stay the default for human tests |
| Health / legal limits | Not a medical device. Admin drafting only. A human must review every letter. Do not use for diagnosis, prescriptions, or any send (mail, MSSanté, DMP). |
| Next review | After the bake-off on `fixtures/gp-sandbox/`. Before a Mac mini deploy and before any sale |

```text
docker compose exec ollama ollama pull qwen2.5:1.5b
```

## ministral-3:3b (quality candidate of the comparison of 9 October 2026, currently loaded)

| Field | Value |
| --- | --- |
| Ollama name | `ministral-3:3b` (library pull, 3.0 GB) |
| Hugging Face source | [mistralai/Ministral-3-3B-Instruct-2512](https://huggingface.co/mistralai/Ministral-3-3B-Instruct-2512) |
| Licence | Apache License 2.0 (stated on the Ollama page and on the card) |
| Commercial use | Allowed under Apache 2.0 |
| Why this one | Best of the eight small tags tried on the owner's PC: no contradiction of the engine's figures, clean drafts, about 1m45 per answer on CPU. 256K native window; 8192 set in `.env`. Accepts images (not used) |
| Health / legal limits | Not a medical device. Admin drafting only. A human must review every letter. Do not use for diagnosis, prescriptions, or any send (mail, MSSanté, DMP). |
| Next review | Second pass of the comparison at one window size, thinking switched off where possible; before a Mac mini deploy |

```text
docker compose exec ollama ollama pull ministral-3:3b
```

## granite3.1-moe:3b and granite3.1-moe:1b (IBM Granite MoE, currently loaded)

| Field | Value |
| --- | --- |
| Ollama name | `granite3.1-moe:3b` (2.0 GB), `granite3.1-moe:1b` (1.4 GB) |
| Hugging Face source | [ibm-granite/granite-3.1-3b-a800m-instruct](https://huggingface.co/ibm-granite/granite-3.1-3b-a800m-instruct) (3.3B total, 0.8B active), [ibm-granite/granite-3.1-1b-a400m-instruct](https://huggingface.co/ibm-granite/granite-3.1-1b-a400m-instruct) (1.3B total, 0.4B active) |
| Licence | Apache License 2.0 (Ollama page and cards) |
| Commercial use | Allowed under Apache 2.0 |
| Why these | Fast mixture-of-experts witnesses, 128K native window (8192 for the 3b and 4096 for the 1b in `.env`). The 3b copies excerpts well but contradicts the engine's figures on mixed questions; the 1b invents and repeats. Neither is a quality candidate |
| Health / legal limits | Not a medical device. Admin drafting only. A human must review every letter. Do not use for diagnosis, prescriptions, or any send (mail, MSSanté, DMP). |
| Next review | Dropped after the comparison of 9 October 2026 unless a second pass changes the picture |

```text
docker compose exec ollama ollama pull granite3.1-moe:3b
docker compose exec ollama ollama pull granite3.1-moe:1b
```

## qwen3:1.7b and qwen3:0.6b (Qwen 3, currently loaded)

| Field | Value |
| --- | --- |
| Ollama name | `qwen3:1.7b` (1.4 GB), `qwen3:0.6b` (522 MB) |
| Hugging Face source | [Qwen/Qwen3-1.7B](https://huggingface.co/Qwen/Qwen3-1.7B), [Qwen/Qwen3-0.6B](https://huggingface.co/Qwen/Qwen3-0.6B) |
| Licence | Apache License 2.0 (card metadata `apache-2.0`; the Ollama page states none) |
| Commercial use | Allowed under Apache 2.0 |
| Why these | Fastest usable small models (1.7b about one minute per answer, 0.6b about half a minute). Native window 32 768. **Thinking is on by default on the card**; the gateway does not switch it off. Measured on 9 October 2026: 44.8 s with it, 10.9 s with `think: false`, on a 2 000-token prompt (`small-model-comparison-1`, section 10) |
| Health / legal limits | Not a medical device. Admin drafting only. A human must review every letter. Do not use for diagnosis, prescriptions, or any send (mail, MSSanté, DMP). |
| Next review | The 1.7b is the speed challenger of the second comparison pass; the 0.6b answered in English and invented a subject, dropped |

```text
docker compose exec ollama ollama pull qwen3:1.7b
docker compose exec ollama ollama pull qwen3:0.6b
```

## gemma4:e2b (Gemma 4, currently loaded)

| Field | Value |
| --- | --- |
| Ollama name | `gemma4:e2b` (4.6 GB on disk; 2.3B effective, 5.1B with embeddings) |
| Hugging Face source | [google/gemma-4-E2B-it](https://huggingface.co/google/gemma-4-E2B-it) |
| Licence | **Apache License 2.0** per the card metadata. The card's text also links "Gemma 4's license terms": the Gemma 2 and Gemma 3 weights were under the Gemma Terms of Use, so **read that link before any sale** |
| Commercial use | Allowed under Apache 2.0 |
| Why this one | Accurate, quotes the file faithfully. Too slow on this CPU (2m14 to 5m34 per answer). 128K native window; 4096 set in `.env`. The card lists a configurable thinking mode |
| Health / legal limits | Not a medical device. Not MedGemma. Admin drafting only. A human must review every letter. |
| Next review | Retest on the Mac mini; read the linked licence terms before any paid offer |

```text
docker compose exec ollama ollama pull gemma4:e2b
```

## qwen3.5:2b (tried, removed from `MODEL_ALIASES` on 9 October 2026)

| Field | Value |
| --- | --- |
| Ollama name | `qwen3.5:2b` (2.7 to 3.1 GB) |
| Hugging Face source | [Qwen/Qwen3.5-2B](https://huggingface.co/Qwen/Qwen3.5-2B) |
| Licence | Apache License 2.0 (card metadata `apache-2.0`; the Ollama page states none) |
| Commercial use | Allowed under Apache 2.0 |
| Why it was tried | Newer Qwen generation, 256K native window, non-thinking by default on the card |
| Health / legal limits | Same as the other small models |
| Disk | **Removed** by the owner: it returned empty answers; its one answer that was kept (JAN) added two sentences that are not in the file. Cause of the empty answers not established |

```text
docker compose exec ollama ollama pull qwen3.5:2b
```

## bge-m3 (embedding candidate, pulled, not the active alias)

| Field | Value |
| --- | --- |
| Ollama name | `bge-m3` (library pull) |
| Hugging Face source | [BAAI/bge-m3](https://huggingface.co/BAAI/bge-m3) |
| Licence | MIT |
| Commercial use | Allowed under MIT |
| Why this one | Stronger multilingual/French retrieval than `nomic-embed-text` in independent benchmarks, at roughly 2x the size (~1.2 GB vs ~274 MB). Candidate to repoint `assistant-embed` at, to compare retrieval quality on `fixtures/gp-sandbox/` |
| Health / legal limits | Not a medical device. Never surfaced to the GP, same as `nomic-embed-text` above |
| Next review | Bench against `nomic-embed-text` before adopting; switching means a **full re-index** — vectors from two embedding models are not comparable (`docs/RETRIEVAL.md`) |

```text
docker compose exec ollama ollama pull bge-m3
```

## gemma2-9b-it (Windows bench, removed from disk 21 September 2026)

| Field | Value |
| --- | --- |
| Ollama name | `gemma2-9b-it` (local `ollama create`, not a library pull) |
| Weight file | `gemma-2-9b-it-Q4_K_M-fp16.gguf` in `data/ollama/import/` |
| Hugging Face source | [google/gemma-2-9b-it](https://huggingface.co/google/gemma-2-9b-it) |
| Licence | [Gemma Terms of Use](https://ai.google.dev/gemma/terms) (not OSI open source) |
| Commercial use | Allowed under those terms if the prohibited-use policy is followed; re-read before any paid offer. Circulate the policy. |
| Why this one | Existing local GGUF on the Windows PC (~6.4 GB). Compare instruction-following and hallucinations with `mistral` on the same fictional lot. |
| Health / legal limits | Same as `mistral`. Not MedGemma. Not clinical care. |
| Next review | After the bake-off on `fixtures/gp-sandbox/`. Before a Mac mini deploy and before any sale. |
| Disk | **Removed** 21 September 2026 (not in the current `MODEL_ALIASES`). Row kept so a re-pull needs no new licence check — re-run the command below. |

```text
docker compose exec ollama ollama create gemma2-9b-it -f /root/.ollama/import/Modelfile.gemma2
```

## llama3.1-8b-instruct (Windows bench)

| Field | Value |
| --- | --- |
| Ollama name | `llama3.1-8b-instruct` (local `ollama create`, not a library pull) |
| Weight file | `Meta-Llama-3.1-8B-Instruct-Q6_K_L.gguf` in `data/ollama/import/` |
| Hugging Face source | [meta-llama/Llama-3.1-8B-Instruct](https://huggingface.co/meta-llama/Llama-3.1-8B-Instruct) |
| Licence | [Llama 3.1 Community License](https://www.llama.com/llama3_1/license/) |
| Commercial use | Often allowed under the community licence below Meta’s user-count threshold, **but** a paid product using this weight needs the “Built with Llama” notice and acceptable-use policy. Avoid for a single-brand practice screen until that is a written decision. |
| Why this one | Existing local GGUF on the Windows PC (~6.4 GB). Same bake-off lot as Gemma 2. |
| Health / legal limits | Same as `mistral`. Admin drafting only. |
| Next review | After the bake-off. Before a Mac mini deploy and before any sale. |

```text
docker compose exec ollama ollama create llama3.1-8b-instruct -f /root/.ollama/import/Modelfile.llama31
```

## qwen3:8b (fast-profile bench, removed from disk 21 September 2026)

| Field | Value |
| --- | --- |
| Ollama name | `qwen3:8b` (library pull) |
| Hugging Face source | [Qwen/Qwen3-8B](https://huggingface.co/Qwen/Qwen3-8B) |
| Licence | Apache License 2.0 |
| Commercial use | Allowed under Apache 2.0 |
| Why this one | Smaller, faster witness for a low-resource dev machine while waiting for the Mac mini; `docs/MODELS.md` flags `qwen3:8b` as the "Fast" profile candidate |
| Health / legal limits | Not a medical device. Admin drafting only. A human must review every letter. Do not use for diagnosis, prescriptions, or any send (mail, MSSanté, DMP). |
| Next review | After the bake-off on `fixtures/gp-sandbox/`. Before a Mac mini deploy and before any sale. |
| Disk | **Removed** 21 September 2026, superseded by `qwen2.5:1.5b` as the fast witness on this machine. Row kept for a future re-pull. |

```text
docker compose exec ollama ollama pull qwen3:8b
```

## smollm2:1.7b (fast-profile bench, low-resource witness, removed from disk 21 September 2026)

| Field | Value |
| --- | --- |
| Ollama name | `smollm2:1.7b` (library pull) |
| Hugging Face source | [HuggingFaceTB/SmolLM2-1.7B-Instruct](https://huggingface.co/HuggingFaceTB/SmolLM2-1.7B-Instruct) |
| Licence | Apache License 2.0 |
| Commercial use | Allowed under Apache 2.0 |
| Why this one | Very small (~1.8 GB), fast on a low-resource dev PC, latency witness for UI testing while waiting for the Mac mini. Not a quality candidate for the pilot catalogue — French output quality expected to be weak at this size |
| Health / legal limits | Not a medical device. Admin drafting only. A human must review every letter. Do not use for diagnosis, prescriptions, or any send (mail, MSSanté, DMP). |
| Next review | After the bake-off on `fixtures/gp-sandbox/`. Before a Mac mini deploy and before any sale — likely dropped once bigger weights are available |
| Disk | **Removed** 21 September 2026, superseded by `qwen2.5:1.5b` as the fast witness on this machine. Row kept for a future re-pull. |

```text
docker compose exec ollama ollama pull smollm2:1.7b
```

Do not pull or create a weight that has no row. Do not send weights back to Hugging Face from a practice machine. Do not use Ollama `*-cloud` tags (remote inference).
