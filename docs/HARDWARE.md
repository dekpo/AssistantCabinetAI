# Hardware — Mac mini, 64 GB

Engineering sizing for the inference machine. The v0 plan puts this Mac mini **at the GP's practice**
as the AI server, reached by the workstation through the gateway (`docs/ARCHITECTURE.md`,
`docs/ROADMAP.md`). Purchase, pricing, retail and financing logistics live in
`docs/private/HARDWARE-PURCHASE.md`.

## Verdict

Buy the **new Mac mini M5 Pro**, **64 GB** unified memory, **1 TB** SSD, **stock Ethernet** (not the
10 Gbit option). Memory cannot be added later, so 64 GB is the bet to make; 24 or 32 GB is a ceiling
reached too soon.

Do **not** buy the Mac mini **M6**: it caps at **32 GB**, which is not enough for the model bake-off
(several large weights, plus Docker and the system).

Do **not** fall back to a leftover **M4 Pro** without a serious discount. The M5 Pro (announced
25 August 2026) has AI accelerators in the GPU and about **307 GB/s** of memory bandwidth, against
about **273 GB/s** on the M4 Pro.

This is **not** a cluster. One 64 GB mini serves a practice of a few seats. It does not run ten heavy
agents in parallel.

## Configuration

| Item | Choice | Why |
| --- | --- | --- |
| Chip | **M5 Pro, 18 CPU cores, 20 GPU cores** | Not the 15 / 16-core variant. Inference plus Docker. |
| Memory | **64 GB** (the mini's maximum) | The only part that cannot be added later. |
| Storage | **1 TB SSD** | Apple's floor at this memory tier; see below. |
| Ethernet | **Stock 1 Gbit**, RJ45 on the back | Included on every Mac mini. No dongle. Enough for localhost and a small practice. |
| Display / keyboard / mouse | **Reuse existing ones** | The screen must be switched on for Docker and Terminal, but no Apple accessory is required. |

## Storage: 1 TB, not 2 TB, not 512 GB

This machine is not a document store. Business documents stay on the workstation. The mini's SSD holds
**model weights**, macOS, Docker and the request register — never patient files.

Less than 1 TB does not work either:

1. Apple **couples** 64 GB with **1 TB** on the high M5 Pro, so it is often not a real choice.
2. Even if 512 GB existed: macOS and Docker, then `gpt-oss:120b` (~65 GB) plus three or four other
   weights (14–43 GB each), already fills it. The intended bake-off spans several model families.
3. 1 TB holds **6–8 weights**, with deletion after testing. That is the right tier.
4. 2 TB only makes sense if large weights are never deleted. Not now.

Weight sizes (Ollama, 2026): `gpt-oss:20b` ~14 GB; `gpt-oss:120b` ~65 GB; `gemma3:27b` ~17 GB;
`qwen3:30b` ~19 GB; `mistral-small` ~14 GB; `llama3.3:70b` ~43 GB. Full list: `docs/MODELS.md`.

## Why 64 GB and not 128

The Mac mini **caps at 64 GB**. 128 GB means a Mac Studio: another budget, out of scope for v0.

| Load | 64 GB M5 Pro |
| --- | --- |
| `gpt-oss:20b` (~14 GB) + macOS + Docker | **Comfortable** — the daily target |
| `mistral-small`, `qwen3:30b`, `gemma3:27b` | **Yes**, one at a time (Ollama unloads the previous one) |
| `llama3.3:70b` (~43 GB) | **Yes**, just barely; little room for a second weight |
| `gpt-oss:120b` (~65 GB) | **At the limit**: quality trial on an otherwise empty machine, not daily use |
| Two large models **loaded together** | **No** |

Ollama keeps **one** weight in memory. Size the machine in **time slots**, not in licences.

## Sizing by seats

Size on **concurrent inference slots**, not on the number of licences: how many generations really run
at the same time. In a practice, 15 to 30 % of seats hit the AI at any one instant; the rest are in
Word, on the phone, or at reception.

| Unit | Definition |
| --- | --- |
| Seat | A workstation where the client is installed |
| Slot | One generation in flight (chat or agent step) |
| Heavy job | OCR plus a large PDF plus several tool calls (filing a whole case) |
| Short chat | Question or drafting, context under 8k tokens, no batch of files |

A 64 GB M5 Pro holds roughly **2 comfortable dense-27B slots**, or **3–4 slots** if the main model is
an MoE (few active parameters) and retrieval and OCR stay **on the clients**. A quantised 70B takes
almost the whole machine: **1 slot**. These are orders of magnitude, to be confirmed by benchmark on
the machine once received.

| Seats | Target slots | Typical mix | Configuration | Why |
| --- | --- | --- | --- | --- |
| 1–3 | 2 | Chat plus one case at a time | **1× Mac mini 64 GB** | Enough. Simple queue. Entry offer. |
| 4–8 | 3–4 | Daily chats, **one** heavy job at a time | **1× Mac mini 64 GB**, interactive MoE model, queue. OCR and retrieval on the workstations | 64 GB holds if neither the index nor OCR is centralised. Beyond that it feels slow. |
| 9–15 | 4–6 | Mixed, morning peaks | **2 nodes**: inference (Mac mini 64/128 GB **or** one 24–48 GB GPU) plus a batch worker (OCR, queues). Single gateway | One mini saturates at two filing jobs plus three chats. |
| 16–30 | 8–12 | Open space, several services | **1 GPU server** (48–80 GB VRAM) **or** 2 Mac Studio / 2 GPUs. Gateway plus workers | Apple memory bandwidth does not multiply slots. |
| 30+ | 12+ | Multi-site or large practice | Cluster, model routing, dedicated OCR workers | Outside the "one Mac mini" envelope. |

Method for a new practice:

1. Count **seats** and the trade (secretarial work means more file jobs; a lawyer means more chat plus
   attachments).
2. Estimate slots: `seats × 0.2` (quiet) to `seats × 0.35` (a filing day).
3. Separate **interactive** work (chat, case panel) from **batches** (OCR, renaming 200 files):
   batches go into a queue, off-hours if needed.
4. Never make a 70B the default multi-user model.
5. Recompute after the pilot: real tokens/s, P95 wait time, share of heavy jobs.

```text
slots_min = ceil(seats * 0.25)
if slots_min <= 2 and parallel_heavy_jobs <= 1  ->  1x Mac mini 64 GB
if slots_min <= 4 and retrieval/OCR on clients  ->  1x Mac mini 64 GB + strict queue
if slots_min > 4 or 2+ heavy jobs               ->  second node or GPU
```

## What client-side retrieval changes

If the index and OCR live on each workstation (`docs/RETRIEVAL.md`), the server never loads the
practice's embeddings or its scans, and a 64 GB mini carries more seats. A server-side index plus
server-side OCR **halves** the usable capacity of the same machine. **Delegated** OCR, for
workstations too weak to do their own, counts as a **heavy job**: one queue, not several silent
batches in parallel.

## First boot — HDMI screen and USB keyboard

A screen and a keyboard are required: macOS account, FileVault, Docker Desktop, `docker compose up`.
"No Apple accessories" means no Magic Keyboard and no Studio Display, not a blind install.

- **A PC screen over HDMI works.** The Mac mini has **HDMI** on the back. A 1080p or 1440p monitor and
  an HDMI cable are enough; select the HDMI input. No DisplayPort adapter if the monitor really has
  an HDMI socket.
- **An old USB keyboard works**, with a cheap part. The current Mac mini has **no USB-A**: two USB-C
  on the front, Thunderbolt (same USB-C shape) plus HDMI and Ethernet on the back. Use a **USB-C to
  USB-A adapter**, or a **small USB-C hub** if a mouse is needed too. Aluminium wired Apple keyboards
  often carry two USB sockets on the back: adapter into the mini, mouse into the keyboard. A USB-A
  mouse from the Windows PC also works.
- A mouse or trackpad is worth having: the macOS setup assistant and Docker Desktop are painful with
  a keyboard alone.

Remote desktop from the PC comes later, **after** screen sharing has been enabled once. Keep screen
and keyboard plugged in for the first evenings.

## Ethernet, cables, switch

There **is** an Ethernet port (RJ45) on the back, as standard. The paid option only upgrades that same
port to 10 Gbit, and it is skipped.

To install Docker and test Compose on the mini, Wi-Fi is enough, or one cable from the router. No
switch is needed on unboxing day.

A **gigabit switch** (5 ports) plus two or three **Cat 5e or Cat 6** cables becomes useful afterwards,
to put the Windows workstation and the Mac mini on the same wired network (practice simulation),
especially if the router has no free socket. No 10 Gbit switch. No "Cat 8" cables.

Ollama stays on `127.0.0.1` (`docs/OPERATIONS.md`). The wired link serves the **workstation**; it does
not expose inference to the whole network.

## On receipt, before any real file

1. Update macOS. **FileVault** (disk encryption) is mandatory before the real pilot
   (`docs/PRIVACY-AND-SECURITY.md`).
2. Restricted admin account. No screen sharing open to the internet.
3. Docker Desktop, the **same** `compose.yaml` as on Windows (`docs/OPERATIONS.md`). Do not copy
   `data/` from the PC.
4. Pull **registered** models only (`models/LICENSES.md` plus `docs/MODELS.md`).
5. Inference ports on `127.0.0.1` only. No Ollama on the practice network during the prototype.

## If the mini saturates later

- A second machine (batches, OCR) rather than a bigger disk.
- A Mac Studio 128 GB **only** if `gpt-oss:120b` becomes the daily model, which is not the v0 need.
- A server with a discrete GPU: more throughput, more heat, more operations. After measurement, not
  before.

## Developing without the Mac mini

The Windows PC with a small model (Mistral, already registered) is enough to iterate on the flow. The
mini is for **quality** and for the multi-model bake-off.
