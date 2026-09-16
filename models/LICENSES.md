# Model licence register

A weight without a row in this file is not loaded on a practice machine.

How to add or remove a model without restarting Compose: `models/README.md`.

Review date: 2026-09-16.

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

## gemma2-9b-it (Windows bench)

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

Do not pull or create a weight that has no row. Do not send weights back to Hugging Face from a practice machine. Do not use Ollama `*-cloud` tags (remote inference).
