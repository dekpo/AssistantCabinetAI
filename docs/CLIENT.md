# Native client and the work folder

Merges the earlier French notes on the native client and on the work-folder design.

## Why a native window

A window with **no address bar** is the first proof that this is not the internet. The GP double-clicks
**Assistant Cabinet AI** like Word or Medilink. She never opens a browser and never sees `127.0.0.1:3000`.

**Tauri 2**, not Electron. Tauri reuses the web engine already present in Windows and in macOS, so the
application stays small; Electron would bundle a whole Chromium, and practice machines are often tight.
Electron remains a fallback only if the system engine refuses to start on a specific machine.

One Tauri project produces both a Windows installer (the pilot's 2019 PC) and a macOS installer (many French
liberal doctors use Macs). The UI is React and TypeScript. Rust is used only for native work: file access,
extraction, the local index, and applying approved plans.

Reference: [Tauri 2](https://v2.tauri.app/start/), [webview versions](https://v2.tauri.app/reference/webview-versions/),
[scope restriction](https://v2.tauri.app/security/scope/).

## The window the doctor sees

One window, one product:

- **left:** the work folder (files);
- **right:** chat, the proposed plan, and Approve / Cancel;
- **top:** Assistant Cabinet AI;
- **settings:** light / dark / follow the system, and the language, defaulting to the system choice.

Full-page chat in the style of a consumer assistant is excluded from the practice screen. Native fonts,
lists, and margins instead. If a large PDF preview is added later it is still Assistant Cabinet AI, a second
window of the same product, never Open WebUI.

Only the name **Assistant Cabinet AI** appears: icon, window title, menus. Not the default Tauri icon, not
the word Ollama, no Open WebUI banner. Licence notices go in About → Licences. See `docs/MODELS.md` for the
trademark constraints, which also forbid rebadging Open WebUI as our product.

## The work folder

Disk writes happen only inside one chosen folder, an allow-list, like a project root.

```text
C:\Users\<user>\AssistantCabinetAI\        (Windows)
/Users/<user>/AssistantCabinetAI/          (macOS)
  ├─ a-traiter\      she drops PDFs and scans here
  ├─ a-valider\      proposed names, awaiting her approval
  └─ corbeille\      after filing into the practice software; no permanent delete in v1
```

Folder names shown to the user follow her language, like everything else she reads
(`docs/LANGUAGE-AND-LOCALE.md`). The root folder keeps the product name, which is not translated.

**Not under Documents, on purpose.** This was corrected on 18 September 2026. Windows Known Folder Move
redirects `Documents` into `C:\Users\<user>\OneDrive\Documents`, and macOS does the same through iCloud's
"Desktop & Documents Folders". Either one turns the work folder into a folder Microsoft or Apple copies
off the machine, silently, for a corpus of medical letters. A folder directly in the home is reached by
neither: OneDrive Backup only ever covers Desktop, Documents, Pictures, Music and Videos, and iCloud only
Desktop and Documents. Detail and the checks in code: `docs/PRIVACY-AND-SECURITY.md`.

Rules:

- The suggested folder is `~/AssistantCabinetAI`, shown before anything is chosen. A primary
  button **creates it if needed and uses it**; **Choose a folder...** still opens the system dialog.
  Nothing is created until she asks.
- Refuse the drive root, system folders, the practice software's own store, and the whole of Documents as
  an allow-list. Today the pilot's downloads and scans land directly in My Documents; the work folder is a
  dedicated folder she **copies into**, not all of Documents.
- Refuse any folder a cloud client mirrors, with a message naming the product. The rule is re-applied at
  every launch, because OneDrive can be switched on after the folder was chosen; a folder that no longer
  passes is dropped rather than written into.
- Exactly one folder is writable.
- `apply` refuses paths outside the allow-list, symbolic links, and unexpected UNC paths.

Because the folder is not in Explorer's sidebar, the window needs an **Open the work folder** button. That
is cheaper than the alternative, which is putting patient letters back in a synchronised tree.

Product flow: she designates the folder; the client lists and extracts locally; the server receives excerpts,
never the disk; the output is a plan (rename and move **inside** the folder, plus a summary to paste into the
practice software, plus a probable-duplicate flag); she adjusts and approves; nothing is written before that.
After she has filed the documents in her own software, the work folder empties into the dedicated trash.

**Undo the batch** works only while the files are still in the work folder. What she typed into Medilink is
outside our reach, and the screen says so. If she empties the dedicated trash herself, we do not restore —
same as Explorer or the Finder.

## Why not Open WebUI Computer

Computer gives the logged-in session the disk, the desktop and a terminal. Restricting it "to the Documents
folder" by instructing the agent is not a boundary: text is not a security frontier, a hostile PDF can ask to
leave it, and a bug can see everything. On a 2019 practice PC, Documents is not a safe area either — it holds
personal files, exports and scans.

Decision: **never on a practice machine.** Possible evaluation later on a disposable machine, away from
business files. The real need, pointing the assistant at a folder, is met by an allow-list plus an approved
plan — a policy in the code rather than in a prompt.

Three Open WebUI file-browsing options were examined (its server-side file browser, the Computer workspace,
community file plugins). All three either browse the **server's** disk or require mounting her folder into
Docker, and both break the contract. Customising Open WebUI for this would be heavier and more fragile than
drawing our own window, and its licence does not allow rebadging it as our product anyway.

## Thin client on a weak workstation

| Always light, on the workstation | Measured at install | Never on the practice machine |
| --- | --- | --- |
| The application window | OCR of scanned images (heavy) | Ollama or model weights |
| Already-text PDFs (simple extraction) | Local indexing if the machine keeps up | Open WebUI Computer |
| Showing the plan, approving, renaming inside the work folder | — | Any cloud upload |

A real text PDF needs no OCR, and the pipeline decides that per page so a born-digital page is never
rasterised. Scans are the expensive case: roughly 0.5 to 2 seconds per page on her 2019 CPU, against a
measured volume of 10 to 20 scans a week, and an unchanged file is never read twice. OCR runs **on the
workstation**, in memory, behind `OcrProvider` (`docs/SPRINT-2.5-ASSESSMENT.md`). If OCR cannot read a
page, report it and refuse to classify rather than guess — the rule that applied when scan OCR was out
of v0 still applies now that it is in.

At first launch, measure extraction time on a **fictional** page shipped with the software, never a patient
scan, and record a local / delegated / automatic preference. Delegated mode would have the Mac mini return
text and discard the pages (`/v1/jobs`); if the mini is unreachable, say so and send nothing silently.
Delegating a real scan is still processing health data, so it waits for the DPIA
(`docs/PRIVACY-AND-SECURITY.md`).

## Deployment shape

The Mac mini is the practice's model server. Assistant Cabinet AI installs on **each** workstation. They are
not the same machine, and patient files are never copied to the mini to "use its power" — that would turn it
into a health-record store.
