# Model licence register

A weight without a row in this file is not loaded on a practice machine.

How to add or remove a model without restarting Compose: `models/README.md`.

Review date: 2026-09-21.

**Currently loaded** (matches `.env`'s `MODEL_ALIASES` on this machine, 21 September 2026):
`mistral`, `llama3.1-8b-instruct`, `nomic-embed-text`, `qwen2.5:1.5b`. Everything else below is a
past or future bench candidate, kept for its licence review, not necessarily on disk right now —
check `docker compose exec ollama ollama list` for what is actually pulled.

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
| Why this one | Fast and reasonably accurate on this low-resource dev machine behind `assistant-turbo`; the current `DEFAULT_MODEL_ALIAS`. Not a pilot quality candidate at this size — a latency witness while waiting for the Mac mini |
| Health / legal limits | Not a medical device. Admin drafting only. A human must review every letter. Do not use for diagnosis, prescriptions, or any send (mail, MSSanté, DMP). |
| Next review | After the bake-off on `fixtures/gp-sandbox/`. Before a Mac mini deploy and before any sale |

```text
docker compose exec ollama ollama pull qwen2.5:1.5b
```

## bge-m3 (embedding candidate, not yet pulled)

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
