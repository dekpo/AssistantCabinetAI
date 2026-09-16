# Models — catalogue, licences, EU AI Act, import

Framing, **not code**. This is **not** legal advice: re-read every official licence before the **first
sale**. Tests use **fictional** files only, as long as there is no DPIA draft.

**A weight with no row in `models/LICENSES.md` is not loaded on a practice machine.** That register is
public and English. Agent-facing procedure: `models/README.md`. Machine purchase: `docs/HARDWARE.md`.
Data rules: `docs/PRIVACY-AND-SECURITY.md`. Contracts: `docs/ARCHITECTURE.md`.

In one sentence: **yes** test several families on the Mac mini, at the owner's home, in Open WebUI, on the
sandbox. **No** model shop at the GP's practice, **no** "clinical care" weight, and **not** the 120-billion
`gpt-oss` as a daily driver on 64 GB.

## Two tiers, never mixed

| Tier | Where | How many weights | Who sees the technical name |
| --- | --- | --- | --- |
| **Bench** | Owner's Mac mini + Open WebUI | About ten, **one loaded at a time** | The owner |
| **Pilot catalogue** | Aliases in Assistant Cabinet AI (`cabinet-chat`, `cabinet-rapide`) | **2 or 3** that won the grid | The GP sees Fast / Quality / Careful French, **not** Ollama |

Changing the model at the practice is done by the owner (or a technician). No free download from the
internet onto the workstation.

## Alias policy

The gateway exposes **aliases** (`cabinet-chat`, `cabinet-rapide`): the screen never shows "Ollama" or a
Hugging Face catalogue. **Allow-list only**, licence row mandatory. For the beta:

- The owner loads 2 or 3 already-registered weights, not the whole zoo.
- Assistant Cabinet AI shows a short list — **Fast** / **Quality** / **Careful French** — plus the internal
  name, so the GP can report "the Quality profile invents less".
- Every profile is scored on the **same** five fictional files with the **same** instructions (grid below);
  the owner keeps the table.

Open WebUI, at the owner's home, serves that bench **before** the practice window exists. At the practice,
later: the short list **inside** Assistant Cabinet AI, not Open WebUI.

## EU AI Act and medical-device avoidance

[Regulation (EU) 2024/1689](https://eur-lex.europa.eu/legal-content/FR/TXT/?uri=CELEX:32024R1689) classifies
mainly by **use**, not by the word "health" on a model card.

- **Administrative assistance** (letter draft, summary **with citations**, naming plan), a **human who
  validates**, no send, no prescription: we stay outside the **medical device** class and aim to avoid
  **high risk**.
- A tool presented as "medical AI", "diagnostic support", "what to prescribe", or a model **sold to treat
  patients**, moves into **high risk** and usually the **medical device** class. That is the opposite of the
  pilot.
- Transparency: the user must know it is an AI and that she reviews the output. The banner is already in the
  prompts.

[CNIL — AI and health](https://www.cnil.fr/fr/ia-et-sante-developper-et-evaluer-des-systemes-ia-conformes):
evaluate, document, do not let the model treat patients.

So putting a "medical" model (trained on clinical literature) **into** Assistant Cabinet AI **increases**
legal risk **without** helping the real need: read a PDF, write French, name a file. That family is not
opened for the beta.

### Three families

| Family | For our product | Decision |
| --- | --- | --- |
| **French writing** (Mistral and successors, row in `models/LICENSES.md`) | Letters, summaries, naming | **Yes** — core of the catalogue |
| **"Medical"** (clinical corpus) | Care, diagnosis | **No** for the pilot. Wrong use + AI Act / device class |
| **"Calculation" / accounting** | Figures, URSSAF | **Not an LLM.** Amounts are **copied** from the document. A model "good at maths" still invents. Accounting is **out of the prototype**: at the pilot practice, e-invoicing and the accountant's software already cover it (September 2026). |

### Hallucinations: the operating rule

Perfection as "zero hallucination" does not exist. Perfection as a **product** means: mandatory citations,
`[à compléter]` rather than invention, rejection of a model that adds a date or a treatment **absent from
the letter**, and human validation **before** any file write.

1. The task is **copy and constrained rewriting**, not "internal medical knowledge".
2. Every date, name, examination, treatment: a **citation** from the file, or `[à compléter]`.
3. One invented fact = **failure** of that profile on that lot; it leaves the pilot catalogue. Hallucination
   is not banned by magic — the profile that fails too often **on our tasks** is refused.

`[à compléter]` is French because the model's visible output is French. It is a user-facing string, so it
belongs in the locale pack rather than hardcoded in a prompt — see `docs/LANGUAGE-AND-LOCALE.md`. It is not
a leftover from the French documentation.

## Weight licences

Distinct from the stack licences below. Each **model** has its own card.

| Family | Typical licence | Bench | Product for sale |
| --- | --- | --- | --- |
| **gpt-oss**, Mistral, much of Qwen 3 | Apache 2.0 | Yes | **Priority**: simple commercial terms, no on-screen "Built with …" badge |
| **Gemma 3** | Gemma Terms (Google) | Yes | Possible, **not** OSI open source: circulate the prohibited-use policy; re-read before selling |
| **Llama** 3.3 / 4 | Meta community licence | Yes | Commercial often OK below the user threshold, **but** requires the "Built with Llama" notice and the usage policy: we **avoid** it for a single-brand screen, unless there is a written decision |
| "Medical" weights, MedGemma, "uncensored" forks | Varies + care use | **No** | **No** |

Ollama the software is **MIT**: usable without its logo. Ollama `…-cloud` tags mean inference at a third
party: **forbidden** (no remote LLM).

## Bench list (64 GB, one at a time)

Pull **only** after a row in `models/LICENSES.md`. Sizes are approximate Ollama weights, 2026.
Administrative French is criterion number one, not a "medicine" score.

### Priority 1 — Apache 2.0, realistic on 64 GB

| Ollama name | Disk | Memory | Test role | Notes |
| --- | --- | --- | --- | --- |
| `mistral` | ~4 GB | Light | **Already registered.** Windows reference, first fictional letter | Keep as the witness |
| `gpt-oss:20b` | ~14 GB | Comfortable | **Daily candidate**: local OpenAI weights, tools, adjustable reasoning | Apache 2.0. This is the gpt-oss target, not the 120b |
| `mistral-small` (24b) | ~14 GB | Comfortable | French, instruction following, tool calls | Strong rival to the 20b for letters |
| `qwen3:8b` | ~5 GB | Fast | **Fast** profile (titles, filing) | Apache: **check the card** of the exact tag |
| `qwen3:30b` | ~19 GB | Comfortable | MoE, responsive chat, good quality / throughput trade-off | Often the daily driver on Apple if the French holds |
| `qwen3:32b` | ~20 GB | Comfortable | Dense, a notch steadier than the 30b MoE | Compare with the 30b on **the same** fictional lot |

### Priority 2 — worth testing, licence to re-read before any offer

| Ollama name | Disk | Memory | Test role | Notes |
| --- | --- | --- | --- | --- |
| `gemma3:12b` | ~8 GB | Comfortable | French + vision (image PDF) later | Gemma Terms |
| `gemma3:27b` | ~17 GB | Comfortable | Dense "daily" quality | Same licence. Not MedGemma. |
| `llama3.3:70b` | ~43 GB | Tight | Quality ceiling, French among the supported languages | "Built with Llama" notice if that weight is **sold**. One slot, machine otherwise idle. |
| `llama3.2` | small | Fast | Only if a light Meta witness is needed | Same licence family. Already mentioned in `models/LICENSES.md`. |

### Priority 3 — one trial, not the product

| Ollama name | Disk | Memory | Test role | Notes |
| --- | --- | --- | --- | --- |
| `gpt-oss:120b` | ~65 GB | **At the limit** on 64 GB | Quality control: is the 20b good enough? | Apache 2.0. macOS + Docker eat the rest. **Unload** the other weights. Slow and risky (memory saturated). Not a pilot profile. |
| `nomic-embed-text` (or a registered equivalent) | < 1 GB | Low | Later: index **on the workstation**, not on the practice mini | Do not centralise the case index (`docs/RETRIEVAL.md`) |

### Too large or out of scope — do not pull

| Name / family | Why |
| --- | --- |
| `qwen3:235b` (~142 GB) | Does not fit in 64 GB |
| `gpt-oss:20b-cloud`, `gpt-oss:120b-cloud` | Cloud. Forbidden. |
| **MedGemma**, any weight advertised as "clinical", "diagnostic", "medical care" | Wrong use + AI Act / device class. The need is **read a PDF and write**. |
| "Uncensored" or "abliterated" forks, weights without a card | Unclear licence, out of the practice product |
| In-house fine-tune | Out of scope until the pilot holds |

Llama 4, if a stable Ollama tag exists on test day: **same box** as Llama 3.3 — bench yes, product only after
re-reading the community licence and the on-screen notice.

## Pull order (once the mini is there)

Do not download everything the first evening. One weight, **the same** fictional lot, take notes, **then** the
next one.

1. Recreate the Compose stack (do not copy `data/` from the PC). Pull `mistral` — the known witness.
2. `gpt-oss:20b` — first real test on the target machine.
3. `mistral-small` — French duel against gpt-oss.
4. `qwen3:30b` then `qwen3:8b` — quality vs fast duel.
5. `gemma3:27b` — if administrative French holds.
6. `llama3.3:70b` — only if the 14–32B dense models hallucinate too much on **our** tasks.
7. `gpt-oss:120b` — one evening, machine cleared, to learn whether the 20b is enough. Then **delete** the
   weight if disk space matters.

Typical command, after `docker compose up`, **one** at a time:

```text
docker compose exec ollama ollama pull gpt-oss:20b
```

## Scoring grid (identical for every profile)

Same five sandbox files, same instructions from `prompts/`.

| Criterion | Failure if… |
| --- | --- |
| Letter French | English calque, informal address, ChatGPT tone |
| Facts | Invents a date, a treatment, an examination **absent** from the text |
| Role | Wants to diagnose, prescribe, send |
| Naming | Unusable plan; a duplicate **not** flagged although it is in the lot |
| Licence | No row in `models/LICENSES.md` |

## Disk: what we keep

Keep the **winners** locally, plus the `mistral` witness and one fast profile. That fits comfortably on
**1 TB**. Losers: `ollama rm` after the note. The 120b does not need to stay.

## Windows bench, 16 September 2026

About 24 GB of RAM, **one large model loaded at a time**; the others stay on disk. `docker compose exec
ollama ollama list` shows `mistral` (witness, downloaded), `gemma2-9b-it` and `llama3.1-8b-instruct` (local
`.gguf` copied into the project, then `ollama create`).

## Importing a local GGUF

Compose **stays up**: adding or removing a model never requires restarting it. Recreate the stack only when
`compose.yaml` changes (ports, image, volume). Windows commands run in Command Prompt (`cmd`), **not**
PowerShell. Full agent-facing procedure: `models/README.md`.

| What | Folder | Git |
| --- | --- | --- |
| Chat template (`Modelfile`) | `models/` | Yes |
| Your `.gguf` copy | `data/ollama/import/` | No |
| Ollama's internal copy after `create` | `data/ollama/models/` | No |

Cursor often hides `data/` because it is gitignored; in File Explorer it is
`C:\Users\elise\Documents\CURSOR\AssistantCabinetAI\data\ollama\import`. Do not leave the only copy of a
weight in LM Studio if you plan to uninstall that app.

1. Add the row in `models/LICENSES.md`.
2. Copy the `.gguf` into `data\ollama\import\` (File Explorer paste, or `copy /Y` in `cmd`).
3. Put a `Modelfile` in `models/` with `FROM /root/.ollama/import/<exact-name>.gguf` plus the family
   template, and copy it next to the weight.
4. Register it **without** `pull`, then refresh Open WebUI (F5) and pick **one** model.

```text
copy /Y models\Modelfile.gemma2 data\ollama\import\Modelfile.gemma2
docker compose exec ollama ollama create gemma2-9b-it -f /root/.ollama/import/Modelfile.gemma2
docker compose exec ollama ollama list
```

Removing a model (`docker compose exec ollama ollama rm gemma2-9b-it`) deletes neither your `.gguf` files in
`import\` nor the stack. Once `create` has succeeded the LM Studio copy can go. Keep `import\*.gguf` as the
project's originals, or delete them later to free disk: chat keeps working from `data\ollama\models\`.

## Stack licences and the visible name

Goal: the user sees only **Assistant Cabinet AI**. No Tauri, Ollama, Open WebUI or Edge logo. Model
licences stay in `models/LICENSES.md`.

### Tauri — fine for selling

| Point | Detail |
| --- | --- |
| Licence | **MIT** or **Apache 2.0**, at our choice ([repository](https://github.com/tauri-apps/tauri), [MIT](https://github.com/tauri-apps/tauri/blob/dev/LICENSE_MIT)) |
| Commercial use, modification, **resale** | **Allowed.** We are not required to open the Assistant Cabinet AI source. |
| Obligation | Ship the copyright notices in the installation package (a licence file, not the welcome screen). |
| Trademark | [Trademark rules](https://v2.tauri.app/about/trademark/): do **not** ship the default Tauri icon; do **not** suggest this is an official Tauri product. Hiding the Tauri logo in the interface is exactly what they ask for. |

Tauri does not block resale: the window can be branded Assistant Cabinet AI only, with legal notices in a
"Licences" section of the software rather than a banner.

### Webview engine on Windows and Mac

- **Windows**: a Microsoft component already present on most Windows 10/11 PCs, and Microsoft provides
  installer redistribution for apps. Prefer the mode that relies on the **already installed** component; the
  practice is often offline, so plan for the small offline installer if needed
  ([distribution](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)). The user
  does **not** see Edge as a browser.
- **Mac**: the component is **already in macOS**. No Tauri royalty.

The old Microsoft "no commercial use" clause was **corrected** after public discussion (Tauri included).
Re-check the Microsoft text when the installer is built.

### Ollama — yes, with no logo on screen

**MIT** software: use, copy, **sell**. Keep the MIT notice in the package. The
[site terms](https://ollama.com/terms) do **not** grant the right to use the Ollama **trademark** without
agreement, so there is **no** Ollama logo in Assistant Cabinet AI — which matches the goal anyway.

Each **model** has its own licence. Mistral 7B is Apache 2.0, commercial use OK. A Llama-type model can
require "Built with Llama" **on screen**: we avoid those weights for a single-brand product, unless there is
a written decision.

### Open WebUI — the trap if resold as our screen

Since **v0.6.6** the licence is a BSD-style in-house licence **plus** a clause: removing or hiding the Open
WebUI name or logo is **forbidden** beyond **50 users** over 30 days, without a written agreement or an
**enterprise licence**. The [licence page](https://docs.openwebui.com/license/) states it plainly:
white-labelling or reselling a rebranded interface requires an enterprise licence.

- The owner's bench, 1–2 people, Open WebUI **with its own name**: internal use is fine.
- The GP must **not** see Open WebUI. We do not repaint Open WebUI as Assistant Cabinet AI to sell it.
- If we ever ship Open WebUI in Compose **to practices** as an admin tool, we **leave their branding** on
  that window, or we do not ship it.

That is one more **legal** argument for: the practice screen is **our** window, not a restyled Open WebUI.

### Docker

Docker Desktop has commercial rules based on company size. The Compose engine on a mini: revisit at the
first sales contract. Not a screen issue, but not to be forgotten.

### Product rules

1. Visible name: **Assistant Cabinet AI** (window, icon, Start menu / Applications).
2. No tool logo or name in the practice interface (Tauri, Ollama, Open WebUI, Edge, Safari, Docker).
3. Legal notices: an **About → Licences** file or screen (text, not advertising).
4. Lawyer review before the first customer invoice.
5. Inventory of crates and packages (`cargo` or equivalent) before the sellable v1 — an aggressively
   copyleft dependency could block us; Tauri core is MIT/Apache, to be checked package by package on the
   day of the freeze.

## Commercial offer (to study, not frozen)

| Offer | Catalogue |
| --- | --- |
| Entry | One profile (the beta winner) |
| Practice | Two or three profiles (fast vs quality) |
| On quote | A larger weight on the Mac mini, still **admin**, never "medical care" |

The contract says: administrative assistance, mandatory review, not a medical device.

## Decisions

| Subject | Decision |
| --- | --- |
| Test gpt-oss, Gemma, Llama, Qwen, Mistral on the mini | **Yes**, fictional files, licence rows first |
| Daily gpt-oss | **`20b`**, not `120b` |
| Several preconfigured models | **Yes** (aliases + allow-list) |
| Beta: the GP compares and reports back | **Yes**, fictional files, written grid |
| A GP-facing catalogue like a model shop | **No** |
| "Medical care" weights | **No** for the pilot |
| An LLM for accounting or arithmetic | **No**; copy the figures |
| Llama / Gemma in the paid offer | **Not before** a licence re-read + the on-screen notice |
| Tiered offer (1 vs 3 profiles) | **Yes, to study** in the contract |
| Guaranteed zero hallucination | **No** — a product lie |
| Adding a weight | Row in `models/LICENSES.md` **then** `pull` or `create` (see `models/README.md`) |
