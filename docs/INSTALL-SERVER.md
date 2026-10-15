# Installing the AI machine (server device): a reproducible recipe

Written 15 October 2026 from the real state of the practice's test machine, **the tower at `192.168.1.83`**, read on that day. The aim: any other installation of the complete solution (another practice, the Mac mini, a rebuilt tower) is made by following this file top to bottom and ends in the same state. Every value below is either read from the tower (marked **read**) or a step that has been run there (marked **run**); a step that has not been run is marked **not verified** and must be checked the first time it is used. English, like all of `docs/`.

This file is the recipe. `docs/OPERATIONS.md` explains why the stack is Compose, and `docs/DEPLOYMENT.md` describes the target shape (a server device apart from each user device, per-person keys in sprint 4). The desktop application is installed separately and only needs the server address (`docs/CLIENT.md`).

## 1. What the tower is (read, 15 October 2026)

| Item | Value |
| --- | --- |
| Operating system | Windows 11 Home (French), build 26300 |
| Memory | 16 GB; Docker Desktop's virtual machine gets about 11.5 GB |
| Graphics card | NVIDIA GeForce RTX 5060, 8 GB, driver 591.59 (CUDA 13.1 reported), compute capability 12.0 (Blackwell) |
| Docker | Engine 29.8.1, Compose 5.5.1, Docker Desktop |
| Address | `192.168.1.83` (must be fixed: a router reservation or a static address; **not verified**) |
| Project folder | `C:\Users\dwyol\Documents\CURSOR\AssistantCabinetAI` (a clone of the repository) |
| Services | `ollama` (published on `127.0.0.1:11434`), `open-webui` (`127.0.0.1:3000`), `server`, the gateway (`192.168.1.83:8080`) |
| Models pulled | the ten tags of section 6 |

Until 15 October 2026 the tower ran **on the processor only** (`ollama ps` said `100% CPU`) with its `compose.yaml` edited by hand on one line (the gateway's address). Sections 4 and 5 replace both with settings that live in `.env`, so the repository file is never edited on a machine.

## 2. Prerequisites (once per machine)

1. **A recent NVIDIA driver** from nvidia.com (a driver of the 570 series or later is needed for the RTX 50 family; the tower has 591.59). Check: `nvidia-smi` in PowerShell lists the card.
2. **Docker Desktop with the WSL 2 backend.** `winget install -e --id Docker.DockerDesktop`, then start it once and accept the WSL 2 option (**not verified**: the tower was installed by hand). In Docker Desktop's settings, limit memory to about 11.5 GB on a 16 GB machine so that Windows keeps room.
3. **Git.** `winget install -e --id Git.Git`.
4. **A fixed address** for the machine on the practice network.

## 3. Get the code

```powershell
cd $HOME\Documents\CURSOR
git clone <the repository address> AssistantCabinetAI
cd AssistantCabinetAI
git checkout search/integration
```

Never edit `compose.yaml` on a machine. If `git status` shows it modified, the machine has drifted from this recipe.

## 4. The `.env` of the tower

Copy `.env.example` to `.env` and set the lines below (the others keep their default). The two secrets are generated on the machine and never written in the repository.

```text
# Network: only the gateway leaves the machine, on the machine's own address
SERVER_BIND_ADDRESS=192.168.1.83
SERVER_HOST_PORT=8080
OLLAMA_HOST_PORT=11434
OPEN_WEBUI_HOST_PORT=3000

# The GPU (section 5)
COMPOSE_FILE=compose.yaml;compose.gpu.yaml

# Models: the gateway's allow-list (alias=model). The embedding alias is the one the desktop asks for.
MODEL_ALIASES=granite3.1-moe:1b=granite3.1-moe:1b,granite3.1-moe:3b=granite3.1-moe:3b,ministral-3:3b=ministral-3:3b,qwen2.5:1.5b=qwen2.5:1.5b,qwen3:0.6b=qwen3:0.6b,qwen3:1.7b=qwen3:1.7b,gemma4:e2b=gemma4:e2b,assistant-embed=nomic-embed-text,qwen3-embedding:0.6b=qwen3-embedding:0.6b,bge-m3=bge-m3
DEFAULT_MODEL_ALIAS=ministral-3:3b
DEFAULT_EMBEDDING_ALIAS=assistant-embed
DEFAULT_OUTPUT_LOCALE=fr-FR
OLLAMA_MAX_LOADED_MODELS=1
MAX_OUTPUT_TOKENS=2048
LLM_REQUEST_TIMEOUT_SECONDS=300
MODEL_CONTEXT_WINDOWS=granite3.1-moe:1b=8192,granite3.1-moe:3b=12288,ministral-3:3b=16384,qwen2.5:1.5b=4096,qwen3:0.6b=4096,qwen3:1.7b=4096,gemma4:e2b=12288

# Open WebUI (owner workbench)
ENABLE_PERSISTENT_CONFIG=false
ENABLE_FOLLOW_UP_GENERATION=false
ENABLE_TAGS_GENERATION=false
ENABLE_SIGNUP=true            # set to false once the first account exists
WEBUI_SECRET_KEY=<a long random string, generated here>
GATEWAY_API_KEY=local-owner-key
```

Values read from the tower's `.env` on 15 October 2026. `MODEL_CONTEXT_WINDOWS` for `ministral-3:3b` is 16384 (`docs/CONTEXT-BUDGET.md` recommends 12288 to 16384 on this machine; it will be measured again on the GPU, section 5). Changing `.env` means `docker compose up -d` (a plain `restart` does not read it again).

To generate the secret: `-join ((48..57)+(65..90)+(97..122) | Get-Random -Count 48 | ForEach-Object {[char]$_})`.

## 5. The GPU

Docker Desktop on Windows gives containers the card through WSL 2; Ollama's image carries its own CUDA libraries, so nothing is installed inside the container. `compose.gpu.yaml` asks for the card; `COMPOSE_FILE` in `.env` (section 4) adds it to every `docker compose` command run in the folder, so a later `docker compose up -d server` cannot start an Ollama without it.

1. Test that Docker sees the card, before touching the stack: `docker run --rm --gpus all ubuntu nvidia-smi` should list the RTX 5060 (**not verified**; if it fails, update Docker Desktop and the NVIDIA driver, restart Windows).
2. Add the `COMPOSE_FILE` line to `.env` and run `docker compose config | Select-String nvidia` (it must print the device reservation).
3. `docker compose up -d`: Compose recreates the `ollama` container with the card.
4. Check, in this order: `docker exec assistant-cabinet-ollama nvidia-smi` lists the card; ask one question from the desktop (or `docker exec assistant-cabinet-ollama ollama run ministral-3:3b "ok"`); then `docker exec assistant-cabinet-ollama ollama ps` shows `100% GPU` in the `PROCESSOR` column. A split such as `40%/60% CPU/GPU` means the model plus its cache do not fit in the 8 GB (the browser and the desktop use about 1.3 GB of it): lower `MODEL_CONTEXT_WINDOWS` for that alias.
5. Memory: the card has 8 GB, of which about 1.35 GB is used by the Windows session. `ministral-3:3b` with a 16384-token window is 4.2 GB. Keep `OLLAMA_MAX_LOADED_MODELS=1` until a measure shows that the chat model and the embedding model fit together.

On a machine with **no** GPU (the dev PC, the Mac mini) leave `COMPOSE_FILE` unset: the default stack is unchanged.

## 6. Start the stack and pull the models

```powershell
docker compose up -d --build
$models = 'ministral-3:3b','granite3.1-moe:1b','granite3.1-moe:3b','qwen2.5:1.5b','qwen3:0.6b','qwen3:1.7b','gemma4:e2b','nomic-embed-text','qwen3-embedding:0.6b','bge-m3'
foreach ($m in $models) { docker exec assistant-cabinet-ollama ollama pull $m }
```

The licence of each tag is in `models/LICENSES.md` (free licences only, `docs/MODELS.md`); **read the card of an exact tag and add its row before pulling a new one.** Weights are stored in `data/ollama` (not in git).

## 7. The firewall

The gateway is the only port that leaves the machine, and it has no access key yet (per-person keys are sprint 4). Until then, allow only the practice computers to reach it. Replace the address with the one of the computer that runs the desktop application:

```powershell
New-NetFirewallRule -DisplayName "Assistant Cabinet gateway" -Direction Inbound -Protocol TCP -LocalPort 8080 -RemoteAddress 192.168.1.50 -Action Allow
```

**Not verified:** Docker Desktop publishes ports through its own process; the rule must be tested from a computer that is not in the list (it must fail) and from one that is (it must answer). The ports 11434 and 3000 are bound to `127.0.0.1` and are never reachable from the network.

## 8. Check that the installation is right

From the computer that runs the desktop (not from the tower):

```powershell
curl.exe -s http://192.168.1.83:8080/health
```

The answer must list the aliases of section 4, `"embedding_alias":"assistant-embed"`, the context windows and `"max_context_chars":200000`, with `"reachable":true`. On the tower: `docker compose ps` shows three healthy services; `git status --short` shows nothing modified; `docker exec assistant-cabinet-ollama ollama ps` shows `GPU` after a question.

## 9. Updating

```powershell
git fetch origin
git checkout search/integration
git pull
docker compose up -d --build server
```

A change to `.env` needs `docker compose up -d`. A change to the embedding model (`assistant-embed=...`) means **every user device must press « Analyser » again**: the vectors of two models cannot be compared, and the desktop recomputes them all (`docs/RETRIEVAL-QUALITY.md`). Back up first (`scripts/backup-local-data.ps1`).

## 10. Not yet in this recipe

- The reranker service (added with the reranker, `docs/RETRIEVAL-QUALITY.md` lot D).
- A per-person access key, and what replaces the firewall rule (sprint 4).
- The Mac mini variant: Ollama natively or in Docker without a GPU reservation, the larger context windows listed in `.env.example`, and its own measures (`apps/server/scripts/measure_context.py`).
- Open WebUI first-account steps and the recreation of the instructions from `prompts/` (`docs/OPERATIONS.md`).
