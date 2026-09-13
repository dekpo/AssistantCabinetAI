# Model licence register

A weight without a row in this file is not loaded on a practice machine.

Review date: 2026-09-13.

## First proposed model

| Field | Value |
| --- | --- |
| Ollama name | `mistral` (alias of `mistral:7b-instruct`, v0.3) |
| Hugging Face source | [mistralai/Mistral-7B-Instruct-v0.3](https://huggingface.co/mistralai/Mistral-7B-Instruct-v0.3) |
| Licence | Apache License 2.0 |
| Commercial use | Allowed under Apache 2.0 |
| Why this one | Small enough for a first localhost trial, writes usable French, permissive licence |
| Health / legal limits | Not a medical device. Admin drafting only. A human must review every letter and certificate. Do not use for diagnosis, prescriptions, or any send (mail, MSSanté, DMP). |
| Next review | Before any real patient text, and again before a Mac mini deploy |

Pull (after `docker compose up`):

```text
docker compose exec ollama ollama pull mistral
```

If CPU-only inference is too slow, try `llama3.2` (Llama 3.2 Community License — read it before any commercial offer) and keep `mistral` as the quality check. On the Mac mini, prefer a more capable instruct model and add a new row here first.

Do not pull a weight that has no row. Do not send weights back to Hugging Face from a practice machine.
