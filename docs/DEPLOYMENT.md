# Deployment — from one pilot machine to a cabinet, and from one cabinet to many

This is the step-by-step runbook `docs/ROADMAP.md` Sprint 4 calls for ("a step-by-step runbook for
deploying the server side... written for any such device rather than for one Mac mini") and that
`docs/OPERATIONS.md` has carried as "Pending" since 27 September 2026. Written early, ahead of
Sprint 4's 8–14 October window, because the first real beta — the pilot GP's own 2019 PC as a
client, a Mac mini as the server — needs it now.

**Read first:** `docs/CLIENT.md` ("Deployment shape"), `docs/OPERATIONS.md` (the Compose commands
this runbook assumes), `docs/HARDWARE.md` (server sizing method and the pilot's own purchase
decision), `docs/PRIVACY-AND-SECURITY.md` (what a LAN deployment must still refuse),
`docs/PLATFORM-VISION.md` (the long-term multi-client, multi-engine shape this stays compatible
with).

**What this document is.** An operational map: what a device needs to run which half of the
product, how to carry the software from this repository onto a cabinet's own machines, and a
checklist to follow each time. It records facts and decisions, the same as every other file in
`docs/`.

**What this document is not.** A new feature, a new architecture, or a change to the v0 scope in
`AGENTS.md`. Every section below is either already built (and cited to the code or doc that proves
it), or explicitly marked **not yet built** — a plan is not a claim that something works today.

## 1. The shape of one deployment

One **cabinet** (a practice, an office) is one deployment. It has exactly one server and any number
of clients:

```text
                              Cabinet LAN
                                  │
   ┌───────────────┐             │             ┌───────────────┐
   │  Workstation A │◄────────────┼────────────►│  Workstation B │
   │  Assistant      │            │             │  Assistant      │
   │  Cabinet AI     │            │             │  Cabinet AI     │
   │  (Tauri client) │            │             │  (Tauri client) │
   └───────┬───────┘             │             └───────┬───────┘
           │          ┌──────────┴──────────┐          │
           └─────────►│   Server device     │◄─────────┘
                       │  Docker Compose:    │
                       │   gateway (FastAPI) │
                       │   Ollama            │
                       │   Open WebUI        │
                       └──────────────────────┘
```

- **The server holds no business documents, ever.** It is model weights, the gateway, Open WebUI
  (the owner's workbench), and a metadata register — never a work folder, never a tabular cache.
  `docs/PRIVACY-AND-SECURITY.md`, "Who stores what".
- **Each client holds its own corpus**, its own local index (`IndexStore`, `%LOCALAPPDATA%`), its
  own typed tabular cache (`docs/SESSION-DATA-13-Column-Cache.md`), and its own work folders. Two
  workstations in the same cabinet do not share an index: `docs/VISION.md`, "Workspaces stay
  partitioned by practice"; `docs/PRIVACY-AND-SECURITY.md` item 6.
- **Cabinets never share a server.** A new cabinet is a new server device, a new `.env`, a new
  `data/` folder. Nothing from one cabinet's Compose stack is copied into another's — not the
  account, not the chat history, not the model cache (pulling again is cheap; copying is how a
  partition breaks).
- **Today's actual state (1 October 2026): not yet split.** The gateway, Ollama and Open WebUI run
  on the same machine as the desktop app in development, exactly as `docs/OPERATIONS.md` says. The
  LAN split below is Sprint 4's target, now becoming real with the Mac mini's arrival. Treat every
  "not yet built" marker in this document as literal, not as modesty.

## 2. Client device — what it takes to run the desktop app

| | Status |
| --- | --- |
| Windows | **The pilot's platform.** Built, tested, `cargo test --lib` green in CI on `windows-latest` (`.github/workflows/ci.yml`). This is where the first beta runs. |
| macOS | **Mandatory by decision** (`AGENTS.md`, 19 September 2026), **architecturally portable, not yet physically run.** CI now compiles and runs `cargo test --lib` on `macos-latest` (added after `docs/SPRINT-2.5-ASSESSMENT.md` section O flagged that nothing had ever been built there), which proves the Rust code builds and its unit tests pass on macOS. **Nobody has run the actual Tauri window on a real Mac yet**, there is no `.icns` in `apps/desktop/src-tauri/icons/` and no `bundle.targets` entry in `tauri.conf.json` — `pnpm tauri build` has never produced a `.app`/`.dmg` to test. Section 5 below has this as a checklist item, not an assumption. |

**Hardware.** No GPU, no model weights, no server role — inference happens on the server device
over the LAN. What the client actually spends CPU and disk on, all local:

- **Extraction and OCR** of the Documents Folder. A native-text PDF is cheap; a scanned page costs
  roughly 0.5–2 seconds on the pilot's 2019 CPU (`docs/CLIENT.md`, "Thin client on a weak
  workstation"), measured, not assumed, and only ever on a changed file.
- **The local index** (SQLite, full-text plus brute-force cosine): sized for "tens of files, not
  millions" (`docs/RETRIEVAL.md`).
- **The tabular pipeline** (`docs/SESSION-DATA-13-Column-Cache.md`): on this session's development
  machine (Intel Core 7 150U, 10 cores, 23.6 GB RAM, NVMe SSD), a single question against an
  **uncached** 100,000-row, 8-column CSV costs about 820 ms, and a 50,000-row XLSX about 1.5 s
  (`docs/HARDWARE.md`, "Client workstation: tabular column cache benchmark"). The typed workbook
  cache added that session removes this cost for every question after the first on an unchanged
  file. **The 2019 PC's own number is not yet measured** — see the checklist in section 5. Until it
  is, do not promise a GP-sized spreadsheet (a few thousand rows) is instant on 2019-era hardware;
  assume it is fine, but confirm it before a beta tester sees a frozen window.
- **No minimum RAM or disk figure is recorded yet for the client**, because nothing has measured
  one. The pilot's 2019 PC is the first real data point (checklist item below); record whatever is
  found here, in this table, not only in a private note.

**Network.** The client must reach the server device's gateway over the cabinet's LAN — today that
means the gateway's published port (`SERVER_HOST_PORT`, default `8080`, `docs/OPERATIONS.md`) is
reachable from the workstation. **Not yet built:** the gateway is published on `127.0.0.1` only
today; publishing it on the LAN, behind a firewall rule and a per-person access key, is Sprint 4
work (`docs/ROADMAP.md`, `docs/PRIVACY-AND-SECURITY.md` "Network and access"). Ollama and Open
WebUI must never be reachable from the LAN, only from the server device itself.

**Storage location.** The work folder must not sit under a OneDrive- or iCloud-synced path on
either OS — this is enforced in code (`work_folder.rs`) and does not need a manual check, but a
fresh machine should still have its OneDrive/iCloud sync state looked at once during setup
(`docs/PRIVACY-AND-SECURITY.md`, "The workstation is not automatically local").

## 3. Server device — what it takes to run the Compose stack

The server is **Docker Compose**: `server` (the FastAPI gateway) + `ollama` + `open-webui`, one
file, the same on every platform (`docs/OPERATIONS.md`). Sizing is about **one thing**: enough
unified or VRAM memory to hold one model's weights plus Docker and the OS, with enough slots for
however many of the cabinet's seats hit the AI at once. The method is hardware-agnostic and already
written down in full in `docs/HARDWARE.md` ("Sizing by seats", "Why 64 GB and not 128"); this
section only widens *which box* fills that role.

### The pilot's own decision stands

**The Mac mini M5 Pro, 64 GB, is the purchased and documented choice for the pilot cabinet**
(`docs/HARDWARE.md`). Nothing below changes it. It is listed here as option 1 because later
cabinets need a comparison, not because the pilot's decision is being reopened.

### Options for a future cabinet

| | Mac mini M5 Pro, 64 GB | AMD Strix Halo mini-PC, 128 GB | Tower, RTX 3090 (24 GB VRAM) + 128 GB DDR5 |
| --- | --- | --- | --- |
| Role | Current pilot server (`docs/HARDWARE.md`) | Candidate, cheaper, more unified memory headroom | Candidate, highest raw inference throughput |
| Memory for models | 64 GB unified, one weight loaded at a time (Ollama) | 128 GB unified (CPU and GPU share it, like the Mac) — room for a larger daily model or two mid-size ones with deletion between trials | 24 GB VRAM for the active model (fast), 128 GB system RAM for everything else; a weight bigger than ~20–24 GB spills out of VRAM and slows down a lot |
| Inference engine maturity | Ollama on Apple Silicon: mature, the one already in production in this project | Ollama on Linux/Windows + ROCm or CPU-only, depending on the exact SKU and driver stack: **check ROCm/driver support for the specific model before buying**, this is less uniformly mature than CUDA or Apple Silicon | Ollama + CUDA on Linux or Windows: the most mature, best-documented GPU path for Ollama today |
| Form factor, noise, power | Small, silent, low power — sits on a shelf in a practice unnoticed | Small, usually quiet, moderate power — same "appliance" feel as the Mac mini | A full tower: louder, needs real airflow and a dedicated power budget, looks and sounds like a workstation, not an appliance |
| Platform for Compose | macOS + Docker Desktop | Linux (native Docker) or Windows + Docker Desktop | Linux (native Docker) or Windows + Docker Desktop |
| Good fit when | The office already favours Apple hardware, or a quiet appliance matters most | Budget matters and 128 GB of headroom is worth more than CUDA's maturity | The office already runs IT infrastructure, noise and a tower's footprint are acceptable, and the largest/fastest model matters most |
| Not yet validated | — | **Never bought or benchmarked by this project.** Every number above is vendor-class reasoning, not a measurement. Benchmark before committing a second cabinet to it. | **Never bought or benchmarked by this project.** Same caveat. |

Whichever box is chosen, size it with `docs/HARDWARE.md`'s own formula:

```text
slots_min = ceil(seats * 0.25)
if slots_min <= 2 and parallel_heavy_jobs <= 1  ->  one box, the daily model comfortably
if slots_min <= 4 and retrieval/OCR on clients  ->  one box + a strict queue
if slots_min > 4 or 2+ heavy jobs               ->  a second node, independent of which box this is
```

**Not yet built, any option:** a step that pulls the cabinet's chosen model set on a fresh server
and verifies `/health` reports every alias — today this is the manual steps in
`docs/user/local-compose.md` and `docs/OPERATIONS.md`, not a script. Worth a `scripts/` entry once
a second real server is actually being set up.

## 4. The vision this stays compatible with

`docs/PLATFORM-VISION.md` describes a platform with several clients and several inference engines.
Nothing in this document gets ahead of that — it only makes concrete what the **v0 shape already
supports without a rewrite**, because the boundaries `docs/PLATFORM-VISION.md` lists ("A second
client", "A second inference runtime") are already in place:

- **One cabinet, several seats.** The desktop app installs per workstation (`docs/CLIENT.md`,
  "Deployment shape"); nothing about the client assumes it is the only one talking to the gateway.
  Multiple workstations pointed at the same server's LAN address is the v0 architecture already,
  not a v1 feature — only the LAN exposure and per-person keys (section 2, "Network") are not
  built yet.
- **Several cabinets, never one shared server.** Each cabinet is an independent Compose stack and
  an independent fleet of clients. A second cabinet does not touch the first cabinet's `data/`
  folder, `.env`, or Mac mini — it is a second, separate instance of everything in section 1's
  diagram, possibly on different server hardware (section 3).
- **What is genuinely not built yet, so it is not implied by this document:** authentication beyond
  a single Open WebUI admin account, per-person API keys, a model registry, device pairing, mobile
  or web clients. These are `docs/PLATFORM-VISION.md`'s V1 and later, and nothing here asks for
  them early.

## 5. Beta rollout checklist

Two versions of the same checklist: the one running now (the pilot's own 2019 PC and the project
owner's Mac mini), and the reusable template for every cabinet after it. Check items off in
`docs/private/` or wherever the owner tracks progress — this file stays the recipe, not the log.

### 5a. The first beta: the pilot's 2019 PC + the Mac mini

**Server side (Mac mini), once it arrives:**

- [ ] Unbox per `docs/HARDWARE.md`, "First boot — HDMI screen and USB keyboard".
- [ ] Update macOS; turn on FileVault before anything else (`docs/PRIVACY-AND-SECURITY.md`,
      "Disk encryption... is mandatory before any real pilot").
- [ ] Create a restricted admin account; confirm screen sharing is off or locked down.
- [ ] Install Docker Desktop.
- [ ] Clone this repository onto the Mac mini (or copy it — not the Windows PC's `data/` folder,
      `docs/OPERATIONS.md`, "A new machine (Mac mini, later)").
- [ ] `cp .env.example .env`, set `WEBUI_SECRET_KEY`, `docker compose up -d`
      (`docs/user/local-compose.md`).
- [ ] Pull the registered chat model and `nomic-embed-text` (`docs/OPERATIONS.md`, "The gateway
      service"); confirm `/health` reports `ok`.
- [ ] Run `apps/server/scripts/measure_context.py` on the Mac mini itself and set
      `MODEL_CONTEXT_WINDOWS` from what it measures (`docs/OPERATIONS.md`).
- [ ] **Not yet built — build it here first:** publish the gateway (and only the gateway) on the
      practice's wired LAN, behind a firewall rule; confirm Ollama and Open WebUI stay unreachable
      from any other device on that network (`docs/PRIVACY-AND-SECURITY.md`, "Network and access").
      This is the one piece of section 2/3 that genuinely does not exist yet and gates everything
      below it.

**Client side (the pilot's 2019 PC):**

- [ ] Record the 2019 PC's actual specification (CPU, RAM, disk, free space, Windows version) —
      `docs/PILOT-GP.md` notes this is still unchecked as of this writing. Put it in
      `docs/HARDWARE.md`'s "2019 practice PC" table once known, next to the client benchmark below.
- [ ] Build and install the desktop app on that machine (or an equivalent development machine
      first, per `docs/ROADMAP.md` Sprint 4's fallback: "installer tested on an equivalent Windows
      machine" if the real PC is not yet available).
- [ ] Point the client at the Mac mini's LAN address once the gateway is published there.
- [ ] Run the typed-column-cache benchmark on the real 2019 PC and record it in `docs/HARDWARE.md`:
      ```text
      cargo test --release --test tabular_column_cache_bench -- --ignored --nocapture
      ```
      This is the one measurement `docs/SESSION-DATA-13-Column-Cache.md`'s own gate asked for and
      could not be taken on a development machine — do this before calling that session's cache
      design final.
- [ ] Confirm disk encryption (BitLocker) is on.
- [ ] Walk the three GP flows on `fixtures/gp-sandbox/` (fictional data) end to end, client talking
      to the LAN server, not `127.0.0.1` — this is milestone A's acceptance, repeated over a real
      network instead of localhost.
- [ ] **Before her own documents ever touch it:** the written DPIA draft, the named data controller,
      and the legal gate `AGENTS.md` rule 6 and `docs/PRIVACY-AND-SECURITY.md` require. Milestone B
      (14 October 2026) is explicitly gated on this — do not skip it because the hardware is ready.

### 5b. Template for every cabinet after this one

1. **Choose and size the server** (section 3): pick a box, apply the seats→slots formula, confirm
   the inference engine stack (CUDA, ROCm, or Apple Silicon) is actually set up and working on that
   specific machine before relying on it.
2. **Stand up the server**: same Compose file, a fresh `.env`, a fresh `data/` folder — never copy
   another cabinet's. Pull the cabinet's chosen models, confirm `/health`.
3. **Publish the gateway on that cabinet's LAN only**, behind a firewall rule and per-person access
   keys once that exists (section 2/3's "not yet built" item — build it once, reuse the recipe).
4. **Install the client on every workstation** that cabinet wants it on. Each one gets its own work
   folder, its own index, its own tabular cache — nothing is shared between workstations in the
   same cabinet, let alone between cabinets.
5. **Measure before trusting**: run the tabular cache benchmark (and anything `docs/HARDWARE.md`
   adds later) on that cabinet's actual hardware, not just the development machine's. Record the
   numbers in `docs/HARDWARE.md` or a cabinet-specific private note, whichever this project is using
   by then.
6. **The legal gate, every time**: a DPIA draft and a named controller for *that* cabinet before any
   of its real documents are processed, independently of whether another cabinet already cleared
   this step — the controller and the risk are per practice, not inherited.
7. **Encrypt both ends**: disk encryption on every client and on the server device, confirmed, not
   assumed, before anything real is processed.

## 6. What this document deliberately leaves open

- **Signed Windows installer, or not.** `docs/ROADMAP.md` Sprint 4: "signed if possible, otherwise
  with a written install procedure." Not decided here.
- **Which server hardware a specific future cabinet gets.** Section 3 is a comparison, not a
  purchase order — that decision is made per cabinet, the way the Mac mini was made for this one
  (`docs/HARDWARE.md`), including a real benchmark before money is spent on an unvalidated option.
- **Per-person authentication, device pairing, a model registry.** `docs/PLATFORM-VISION.md`'s
  later stages; nothing here builds toward them early.
