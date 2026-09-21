# Local models (Ollama in Compose)

This folder holds **Modelfiles** and the **licence register**. It does **not** hold `.gguf` weights (gitignored). Weights live under `data/ollama/` (not git).

Do not restart Docker Compose to add or remove a model. The stack stays up. Command Prompt (`cmd`) on Windows; not PowerShell.

Register a weight in `LICENSES.md` **before** loading it on a practice machine.

## Where files go

| What | Where | Git |
| --- | --- | --- |
| Chat template (`Modelfile.*`) | `models/` | Yes |
| Your `.gguf` copy | `data/ollama/import/` | No |
| Ollama’s internal copy after `create` | `data/ollama/models/` | No |

`data/ollama` is already mounted at `/root/.ollama` in Compose. Cursor often hides `data/` because it is gitignored. Open it in File Explorer:

`C:\Users\elise\Documents\CURSOR\AssistantCabinetAI\data\ollama\import`

## Add a local `.gguf` (no download)

Compose must already be running (`docker compose up -d`). From the repository root in **cmd**:

1. Copy the `.gguf` into `data\ollama\import\` (File Explorer paste, or `copy /Y` from another folder on this PC). Do not leave the only copy in LM Studio if you plan to uninstall that app.
2. Put a `Modelfile` in `models/` with `FROM /root/.ollama/import/<exact-filename>.gguf` plus the family `TEMPLATE` and `PARAMETER stop` lines. Copy it next to the GGUF:

```text
copy /Y models\Modelfile.gemma2 data\ollama\import\Modelfile.gemma2
```

3. Register it (wait until it finishes before the next one):

```text
docker compose exec ollama ollama create gemma2-9b-it -f /root/.ollama/import/Modelfile.gemma2
docker compose exec ollama ollama list
```

4. Refresh Open WebUI (F5). Pick **one** model. This PC has about 24 GB RAM: do not load two large models at once.

Existing templates: `Modelfile.gemma2`, `Modelfile.llama31`. Names already created: `gemma2-9b-it`, `llama3.1-8b-instruct`. Witness: `mistral`.

## Add a library model (downloads)

Only after a `LICENSES.md` row:

```text
docker compose exec ollama ollama pull mistral
```

## Remove a model from Ollama

Does **not** delete your `.gguf` in `import\`. Does **not** restart Compose.

```text
docker compose exec ollama ollama rm gemma2-9b-it
docker compose exec ollama ollama list
```

After `create` succeeds, Ollama has its own blobs. You may delete the LM Studio copy. You may keep `import\*.gguf` as this project’s originals, or delete them later to free disk; chat still works from `data/ollama/models/`.

## Switching the embedding model (e.g. testing `bge-m3` against `nomic-embed-text`)

Only after a `LICENSES.md` row for the new weight. From the repository root, in **cmd**:

```text
docker compose exec ollama ollama pull bge-m3
```

Edit `.env`: change what `assistant-embed` (or whatever your `DEFAULT_EMBEDDING_ALIAS` is) points
to — **keep the alias name itself unchanged**, only the value after `=`:

```text
MODEL_ALIASES=assistant-chat=llama3.1-8b-instruct,assistant-rapide=mistral,assistant-embed=bge-m3,assistant-turbo=qwen2.5:1.5b
```

Restart only the server container so it re-reads `.env`:

```text
docker compose up -d server
```

Then **re-index every Work Folder**: vectors from two embedding models are not comparable
(`docs/RETRIEVAL.md`), so click "Analyser" again in the client after the switch. Reverting to
`nomic-embed-text` follows the same three steps, in reverse.

Renaming the alias itself (not just what it points to) is also safe — the desktop client self-heals
a renamed `modelAlias`/`embeddingAlias` on its next health check (`docs/MODELS.md`, "The embedding
alias is never a chat choice"; `docs/TROUBLESHOOTING.md`) — but changing only the value, as above, is
simpler and needs no client restart at all.

## What requires Compose recreate

Changing `compose.yaml` (ports, images, volumes). Not adding or removing models.
