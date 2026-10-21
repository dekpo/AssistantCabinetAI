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

- **left (Resources):** one file explorer: the two families of files, Documents then Tables (see "The left panel" below);
- **centre (Workspace):** an extensible zone, closed unless something is shown in it (see "Layout" below);
- **right:** chat, the proposed plan, and Approve / Cancel;
- **top:** Assistant Cabinet AI;
- **settings:** light / dark / follow the system, and the language, defaulting to the system choice.

### Layout

The three zones sit in one `react-resizable-panels` v4 group (`Group`, `Panel`, `Separator`; a number is pixels,
a string is a percentage), in `src/components/AppLayout.tsx`. Pixel minimums: Resources 320 (the explorer's family header is one line and never wraps),
Workspace 320, Chat 340; Resources never exceeds 35 % with the centre open, and 50 % in two columns (centre closed; owner request, 20 October 2026, so the explorer's line of times has room). The window `minWidth` is 1080 (320 + 320 + 340, two
separators and the padding come to about 1040). Each zone scrolls inside itself. **Default widths** (`lib/panelLayout.ts`,
explorer step 4): `[28, 34, 38]` with the centre open and `[30, 0, 70]` with it closed, so the left zone starts at about
330 px in the default window. A layout saved before the minimum went to 320 px (for example `[22, 38, 40]`) is read back
untouched and shown clamped to 320 px by the panel library; it is never rewritten by that clamp, because only a gesture of the
user is saved (`isUserInteraction`).

**The centre is a slot, not a viewer.** `App` holds `workspace: WorkspaceView | null` (`src/lib/workspaceView.ts`,
a discriminated union with two kinds today, `welcome` (the empty state of the Files section: no file open) and `files` (a list
of tabs, see "The document viewer")). **The centre is visible exactly while `shownCentre(...)` is not `null`**
(`lib/centreSection.ts`, pure, tested over every combination): the open tabs, or the Overview section, or the Notes section while
it holds a note. Nothing is open at launch (the section is Files), so the user sees two columns. A new use of the zone (a
calendar, say) adds a kind to the union and a case to `WorkspacePanel`, and touches nothing else. **Overview and voice notes are
not kinds of view:** they are *sections* of the zone, reached by a switch **Aperçu · Fichiers · Notes** that is drawn whenever
the zone is open (`Notes` only while enabled; see "The overview of the centre" below and "Voice in the window" at the end of this
file). A button in the title bar (`aria-pressed`) opens the zone, on the Overview when no file is open, or closes it; the user
can also close it by dragging it below its minimum or by pressing Enter on a separator beside it.

**The workspace panel is always rendered**, collapsed to size 0 when closed, so the group always holds three
panels and `ChatPanel` keeps its place in the tree: the conversation is never remounted by a resize, an open or a
close. A closed centre has no limits to grow (`maxSize` 0) and its separator is hidden, so the line that remains
resizes Resources against Chat.

**Remembered widths.** `settings.json` holds `layout`, two triples `[resources, workspace, chat]` in percent:
`open` (centre above 0, saved while the centre is open) and `collapsed` (centre exactly 0, saved while it is
closed), so each state keeps its proportions across closing, reopening and restarting. They are saved only at the
end of a gesture made by the user (`onLayoutChanged` with `isUserInteraction`), never on each pixel, and never for
a layout the code applied (opening, restoring, the library correcting itself after a window resize). Rust keeps
only usable triples (`PanelLayouts::sanitised`, on load and on save). A failed layout save is silent. Settings has
**Restaurer la disposition**; the settings reset resets the layout with the rest. Both put the default widths on
screen at once.

**Saves are queued.** `save_settings` replaces the whole file, so `useAppSettings` serialises its saves
(`src/lib/settingsQueue.ts`): each change is merged into the settings as they stand when its turn comes, a failed
save does not block the next, and `reset` and the initial load go through the same queue.

Under the folder's buttons sits a **Documents used** disclosure (sprint 2a.7, moved into the sidebar and
reworked by sprint 2a.8, `docs/SELECTION-AND-MEMORY.md`). Closed, it says **aucun** ⚠, **tous** ⚠ or how many files
she ticked; open, it lists every file with a coloured state dot (green analysed, amber waiting, red unreadable), a
checkbox on each analysed file, and a header checkbox that selects all documents or none. The selection starts
empty at startup and whenever the folder changes, survives Analyse, and applies to the current conversation only:
conversations are not saved yet, and there is no list of them. With nothing selected, questions are answered
without documents and say so under the answer ("Réponse sans vos documents."). The composer is disabled until a
documents folder is chosen. Under an answer, **Sources** lists each file once with its pages. Under the Analyse
button, the summary names any file that was renamed to a clean name, old and new. When the knowledge base took
part (`knowledge_mode` is not `off`, KB lot 4) it also says, in counts only and never a name, how many people and
organisations the pass found and how many were already known, how many identifiers (an e-mail address, an invoice
number, an IBAN), how many documents analysed earlier were read again for names without asking the AI, and, first
because it asks her to act, how many documents could not be fully analysed and are retried by the next Analyse.
The Data card's summary says the same for the spreadsheets (KB lot 5): the people and organisations the columns
named, the identifiers, how many columns of which spreadsheets were recognised (a patient, a supplier, an e-mail
address...), how many links were read between two columns, and the documents read again because the spreadsheets
taught the base new names. While those documents are read again the progress bar is labelled with the same
sentence as the Documents card's (reading the names in documents analysed earlier).
Nothing is read in the background: documents analysed before the knowledge base existed are read for names, from the text the index already holds, only when she presses Analyse. The settings panel shows the same listing with a **See** button on each file, which shows it selected in
the file manager without opening it.

**Knowledge base, in the window (KB lot 7).** Four things are added, and nothing changes while the mode is `off`.
(1) Under an answer whose search the base **narrowed**, a small line says how many files were searched out of
how many and for which name, with **Élargir à ma sélection** (the question is asked again on everything she
selected) and **Voir les fichiers**; the same line says when the base widened the search by itself, and names the
workbook it chose with **Choisir un autre classeur**. Nothing is shown when the base only measured. (2) When a name
fits several people, the question is answered anyway and buttons below ask which one was meant, each with its number
of files; a click asks again with that name. (3) In the box, typing `@` and the first letters of a name (or pressing
the **@** button) lists the names of the current selection with their number of files; arrows and Enter choose,
Escape closes. The chosen name appears as a chip above the box and narrows that question's search; the question
text is untouched. If the name exists only in files she did not tick, one clearly labelled line offers
**Ajouter à ma sélection**. (4) The settings dialog has a « Connaissances » group (**Ciblage intelligent**, the
health modules, hiding names in suggestions) and, under **Avancé**, the thresholds and a **diagnostics panel** for
developers that appears under the last answer. Design and rules: `docs/DECISIONS.md`, "Settled by KB lot 7".

**Knowledge management dialog (KB lot 8).** Settings → « Connaissances » → **Gérer…**, or the link **Voir les connaissances** under the analysis summary of either folder card, opens a full-window administrative dialog; nothing in the conversation opens it. Tabs: **Entités** (a searchable, filterable list of 50 per page and the sheet of the chosen entity: its names, where each mention was found with the passage, the method and the confidence, the links, the details, the pairs, how the spreadsheet columns were read; actions Modifier, Ajouter un nom, Fusionner avec…, Séparer…, Supprimer, Restaurer, Défaire la fusion), **À vérifier** (possible matches to merge or dismiss, and library-health counts), **Types et rôles** (read-only) and **Réinitialiser**. A strip under the title counts what the base holds, says how many documents the next Analyse will read again (from the stored text, without the AI) and holds **Annuler : <operation>**. Delete, merge, split, removing a name and both resets ask first through the confirmation dialog. Design and rules: `docs/DECISIONS.md`, "Settled by KB lot 8".

**The search field of a folder card (explorer step 1, 19 October 2026).** A field above the list in each folder card searches that folder's file names, its file profile and, for documents, the text, with no model. It **only filters the list**: the selection never changes because of it. Under the result line, « Cocher les N » adds the analysed results that are not yet selected (N counts only those, the button is hidden at 0), and the line says how many results are not analysed and cannot be ticked. « Tous les fichiers » shows the whole folder again and « Réinitialiser » ends the search; neither touches the selection. A typed word that is a file extension (`.docx`, `xlsx`) filters by format, and a text that is only a date (`2026`, `2026-02`, `2026-02-06`, `06/02/2026`) finds the files dated so. The search runs again by itself when an Analyse finishes or the list is read again on window focus, and the text is cleared when the folder changes. Rules: `docs/DECISIONS.md`, « the explorer's search step »; code: `lib/finderSearchState.ts` (tested), `state/useFinderSearch.ts`, `components/FinderControls.tsx`, `finder/typed.rs`.

**Suggestions, and the sheet of a name (KB lot 9).** Two more things, and nothing changes while the mode is `off`. (1) In the empty conversation (and nowhere else: the folder cards no longer draw them, 17 October 2026), a short block titled **« Suggestions »** (« Questions suggérées à partir de vos fichiers ») is drawn as buttons: each was checked on the selected files before it was shown (a few operations run on the cached workbooks, nothing on the AI) and a click sends it as it stands. Nothing says it will have an answer. Names are **hidden by default** (the setting « Masquer les noms dans les suggestions »): only questions that name nobody are shown, with **Afficher les suggestions avec des noms**, which the user clicks and which lasts the session, and **Masquer à nouveau les noms**. Nothing is looked up at launch or while an Analyse runs. (2) A small **i** beside a name, on its chip above the box and in the `@` list, opens the **sheet of the name** inside the selection: its other spellings, its roles, the documents with their pages and the tables with the sheet and the column, a count (never a list) for what lies outside the selection, and the links the files state. It only reads. Design and rules: `docs/DECISIONS.md`, "Settled by KB lot 9".


The conversation remembers its recent exchanges, as many as the chosen model's context window allows; past
excerpts are never resent. Design and budget: `docs/SELECTION-AND-MEMORY.md`.

Full-page chat in the style of a consumer assistant is excluded from the practice screen. Native fonts,
lists, and margins instead. If a large PDF preview is added later it is still Assistant Cabinet AI, a second
window of the same product, never Open WebUI.

Only the name **Assistant Cabinet AI** appears: icon, window title, menus. Not the default Tauri icon, not
the word Ollama, no Open WebUI banner. Licence notices go in About → Licences. See `docs/MODELS.md` for the
trademark constraints, which also forbid rebadging Open WebUI as our product.

### The document viewer

An analysed file opens in the centre, inside the window, fully offline. It is display-only: opening a file never
ticks or unticks a file, never starts an analysis, and touches no extraction, OCR, index, retrieval or provider.

**How a file is opened.** Each row of the two lists in the sidebar (the Documents list and the Tables list)
has an **Ouvrir** button, only on an analysed file (`isOpenable`, the green dot), independent of its checkbox,
**wherever the file sits** (explorer step 9, 21 October 2026): a workbook found in the Documents folder or a document
found in the Data folder opens like any other, once its family's pass has read it there. A file that is not analysed
or is unreadable has no button (but keeps **Voir**). The tab is keyed by the **physical folder** (`entry.root`) and the
plain path, never by the family, and the viewer reads the file through the command of that folder; the renderer is
chosen by the extension. The viewer guard compares a tab with the inventory of its own folder, which joins a file to
what its own family made of it, so a foreign file is judged by its own state. The old cards of the Settings still use
their own gate (`isSelectable`) until step 10. Only analysed files open because an Analyse renames files to clean
names, so a path known before it can change after it. `WorkspaceView` carries
`{ kind: "files", tabs: OpenFile[], activeId }` (each `OpenFile` is `{ id, folder, record }`: the whole record, with
the folder it is relative to). Every opening goes through one function in `App`, `openFile(folder, record)`, which
adds a tab or activates the one that exists (see "Tabs" below). The centre shows the tab strip, then a header (the
file's relative path) and the content; closing the last tab collapses the zone. The title-bar button and dragging
the centre closed only **hide** it: the tabs stay open, nothing is closed, and the button (or opening any file)
shows them again, each tab coming back as it was left (the viewers are unmounted while hidden and restored from the
tab's remembered view). With nothing open, the button shows the Overview.

**Tabs.** Several files can be open at once, one tab each (`src/components/viewer/TabStrip.tsx`, pure logic in
`src/lib/viewerTabs.ts`). A tab's identity is the folder plus the relative path (`tabId`, joined by a NUL
character, which cannot occur in either), so the same name in the Documents Folder and in the Tables Folder gives
two tabs, told apart by the folder's glyph (decorative) and by the folder's name in the tooltip. Opening a file
that is already open activates its tab and never adds a duplicate; if its content changed meanwhile (another
hash, after an Analyse) the tab takes the fresh record and forgets its remembered view. The strip is a `tablist`:
Left and Right move to the neighbour and wrap, Home and End go to the ends, and moving focus activates; only the
active tab and its close button are in the Tab order. The label is the file name, the tooltip the full path and the
folder; the strip scrolls sideways and keeps the active tab in view. Closing the active tab activates its right
neighbour, else the left one; closing the last one collapses the centre. There is **no cap** on the number of tabs
(open decision), no shortcut, no reordering, no pinning. **Only the active tab is mounted**: no other viewer, canvas
or PDFium instance exists, which keeps memory flat for a library of thousands of files; the price is that coming
back to a tab reads the file again (no byte cache).

**What a tab remembers** (`src/lib/viewState.ts`, in memory only, never stored, dropped with the tab or when the
folder is replaced): the scroll position as a 0 to 1 ratio of the scrollable height (robust to a zoom or a
resized zone), the zoom only if the user chose one (otherwise the usual fit to width runs again), the sheet of a
workbook, the slide of a presentation (restored by stepping, the handle has no direct setter) and the Markdown
mode. The state is kept in a ref while the user works and handed to the store when the tab is left or closed, only if
the view had finished restoring the previous one; the restore (`useScrollMemory`) sets the position again on every
frame until the scrollable height has stopped changing, for at most two seconds, and gives way at the first wheel,
key or pointer input. The element that scrolls is found by what the browser reports (it overflows and its style
lets it scroll), not by a class of the library.

**Two renderers, chosen from the extension, never from the content** (`src/lib/viewerFormats.ts`):

| Extensions | Renderer |
| --- | --- |
| `pdf docx xlsx pptx odt ods odp rtf csv` and `png jpg jpeg gif bmp webp svg` | the library `shrimp-doc-viewer`, behind `src/lib/viewerEngine.ts`, its format forced |
| `txt md srt vtt eml` | the application's own plain text view (`TextView`), line by line as stored, monospace, wrapping, selectable, no Markdown rendering (a Markdown file can also be switched to a rendered preview, below) |
| everything else (`doc xls ppt tiff heic`...) | a message in the user's language and the button **Afficher dans l'explorateur de fichiers** (the existing reveal command) |

The library's text engine is not used: it joins single line breaks, so a subtitle's index, timestamp and text, or
a mail's headers, would run together. The plain text view decodes UTF-8 with `TextDecoder`, turns CRLF into one
line break, and shows at most **500 000 characters** (cut at a line end) with a notice, so a very large text never
freezes the window. Only the first part of a large file is decoded. The Raw view of a Markdown file reuses it.

**Raw | Preview (Markdown).** `previewModes(extension)` in `viewerFormats.ts` lists the modes of a file, raw first:
`md` gives `["raw", "preview"]`, every other extension `["raw"]`, and the two-button switch
(`PreviewModeSwitch`, **Brut** | **Aperçu**, `aria-pressed`) is drawn under the header only when there are two. A
file opens raw. The mode is a typed value (`PreviewMode`) in the tab's view state, so a tab comes back in the mode it
was left in and another file starts raw; the scroll position is not carried across the switch (each mode has its own
scroll box, a switch starts at the top). The preview
(`MarkdownPreview`) is the application's own `Markdown` component, the one the chat answers use (`react-markdown`
and `remark-gfm`, no `rehype-raw`): nothing a file contains can become markup, and a link or an image is reduced to
its text (the alt text for an image), so the preview can neither navigate the window nor fetch anything. It is
fed the same bounded text as the raw view (`prepareViewerText`: same decoding, same 500 000-character limit, same
notice), from the bytes already read, and sits in its own scroll container. `Markdown.tsx` is unchanged.

**The sheet viewer at a fractional display scale (21 October 2026).** `shrimp-doc-viewer` draws a sheet on a canvas
sized to the scrolling element's `clientWidth` x `clientHeight`, which are whole numbers. At a Windows scale of 125 %
the real box is fractional, so the canvas overflowed by a fraction of a pixel: a bar appeared, the box shrank, the
library redrew the canvas smaller, the bar went, and the loop never stopped (the bars flickered, only on the 125 %
screen, only at zooms where the sheet fits). `styles.css` now clamps that one canvas to its containing block
(`.viewer__host > div:not(:first-child) > canvas[style*="will-change"]`, `max-width` and `max-height` 100 %). Only the
sheet canvas carries `will-change`; a large sheet still scrolls. Measured on a replica of the library's own DOM and
resize loop in Edge: 17 of 40 sizes flickered at 125 %, none at 100 %, none after the rule.

**Text read by the assistant (`eml`, `srt`, `vtt`).** These three also give `["raw", "preview"]`
(`previewKind(extension)`: `markdown` for `md`, `readable` for them), and the second button is named for what it
shows: **Texte lu par l'assistant** instead of **Aperçu**, so it cannot be taken for the original. Raw stays the default and
stays complete (every header, every timestamp, an encoded body as stored). The second mode shows exactly what the
assistant indexes, produced by the Rust readers (`readers::eml`, `srt`, `vtt`, unchanged) through two read-only
commands, `read_work_file_readable_text` and `read_data_file_readable_text` (`viewer_source::read_readable_text`: the
raw read's path checks, 128 MiB cap and codes; any other extension is refused as `viewer_file_unreadable`). The text
comes back as UTF-8 bytes and is shown by the same `TextView` (`ReadableTextView`), so the 500 000-character bound,
the notice and the remembered position are shared. It is read only when the mode is chosen, kept while the view
stays mounted, and read again when a tab is returned to. A mail is given to its reader as stored (it decodes the
character set the message declares); subtitles go through the raw view's UTF-8 conversion, UTF-16 included. What
the view leaves out is what the readers leave out: for a mail, everything but `From`, `To`, `Cc`, `Date`,
`Subject` and the body (plain text; the HTML with its tags removed only when there is no plain text; attachments are
not read); for subtitles, the cue number, the end time, formatting tags and cue settings (one line per cue with its
start time, a WebVTT voice tag shown as `Name:`). A file the reader finds empty shows a sentence, not a blank area.

**Controls are the application's own, in the interface language.** The library draws a toolbar of its own with
labels in one fixed language (Chinese); it is hidden by one CSS rule (`display: none !important`, because the
library sets `display` on the element itself) and these controls replace it. A control exists only where the
library handle has the matching member, measured in the real window: PDF, DOCX, ODT, ODP, RTF: zoom out, the zoom
level, zoom in, **Ajuster à la largeur** and the page count (« 1 page », or « x / N » once there are several,
language-neutral, following the scroll: the library does not report the page, so it is read from where its page
elements sit against a line a quarter of the way down the zone, `visiblePage` in the adapter); images: the same, with a zoom step of the
application's own through `setZoom` (a picture grows up to 150 % and not beyond); XLSX and ODS: zoom and a
**Feuille** selector when there are several sheets (the library's `setSheet`, no remount; a CSV goes through the
same code and its handle was not probed: the Tables Folder one is in the human test); PPTX: **Précédent**,
**Suivant** and « Diapositive n sur N », no zoom. Pages and pictures are fitted to the width on opening and after a
resize (debounced) unless the user chose a zoom; a grid is not fitted (it would open at 200 % and more); the PDF
refits itself. The viewer is mounted only once the zone has a width (the centre is 0 px wide at its first render),
destroyed on every replace and on unmount, and given a copy of the bytes (the library empties the buffer it
receives).

**Accents.** The file is read by Rust (`read_work_file_for_viewer`, `read_data_file_for_viewer`, 128 MiB cap) and
text-like files (`txt md srt vtt eml csv`) arrive as UTF-8: UTF-16 with a byte-order mark is decoded first,
otherwise UTF-8 or Windows-1252. The conversion is in memory; the file on disk is never rewritten.

**When the file changed outside the application** (renamed, moved, deleted or edited in the file manager): while a
file is open, the window's `focus` re-reads the inventory of that file's folder (`useOutsideChangeGuard`, cheap
thanks to the hash cache) and compares it with the opened record (`outsideChange`: missing, no longer analysed, or
a different hash). On a difference the centre closes and **one modal** says so, with one button **Compris**
(`ConfirmDialog` in its `singleAction` variant; Escape and a click outside acknowledge it too). The read command's
`viewer_file_missing` ends in the same modal. No watcher and no timer: the check happens when the user comes back.

**Limits, accepted.** Italic is never drawn (shown upright) and bold is synthesised; serif and other families fall
back to one sans font; Chinese, Cyrillic, Arabic and every script outside the Latin subset render blank. Legacy
binary formats (`doc xls ppt`) are not shown and not converted. An EML shows its raw source, bodies still
encoded. The PPTX renderer puts one letter per line on the tested files, and the XLSX one truncates long values
in narrow columns (`10/1/202`): both are the library's, not the application's.

### The left panel: one file explorer (explorer programme, step 2, 19 October 2026)

The two folder cards of the sidebar are replaced by one compact box, `FileExplorer`
(`src/components/FileExplorer.tsx`). The two physical folders and the two analysis pipelines are unchanged; only
the way they are shown is. Decisions: `docs/DECISIONS.md`, "Settled by the owner: the left panel becomes one file
explorer" and "... the explorer's layout, first start, progress and centre". Plan: `docs/SESSION-EXPLORER/` (git-ignored).

- **Two families, sorted by extension** (`lib/explorerModel.ts`, `familyOf`): **Documents** (`pdf docx txt md jpg jpeg
  png vtt srt rtf eml odt odp pptx`, the document pipeline's list) and **Tables** (`csv xlsx xls xlsm ods`). A file no
  pipeline reads is set apart and **not listed**: the bottom bar says how many there are. No new inventory command: the box
  merges the two existing reports (`work_folder_inventory`, `data_folder_inventory`), which already list every file
  of their folder (`docs/WORK-FOLDER-INVENTORY.md`, "The merged view").
- **One definition of the states for the screen:** analysed (filled disc), to analyse (hollow ring: not analysed yet,
  **or changed since**, or being analysed), unreadable (triangle). The shape carries the state, the colour repeats it, and
  every glyph has a text for screen readers (`StateGlyph`, a small SVG component; no emoji, no text character).
- **A file of the other family** (a workbook in the Documents folder, a document in the Data folder) is shown under its
  family with a tag « depuis Docs » / « depuis Data », in its real state (to analyse until its family's pass has read it
  there, then analysed or unreadable), and is **counted** by its family's **Analyser (n)**, which reads it in place
  (explorer step 8). Once analysed it can be ticked, **opened** (Ouvrir) and **shown** (Voir) exactly like any other file
  (explorer step 9): the actions go to the folder the file really sits in (`entry.root`, `entry.path`), and Voir is offered
  for every file, whatever its state or its folder. The search, the facts and
  the selection are by family and place each file by the folder its key names (`lib/indexKey.ts`).
- **Any two folders (explorer step 8B, 21 October 2026).** The Documents folder and the Tables folder can be separate (the
  suggested default), **the same folder**, a Tables folder **inside** the Documents folder, or the reverse: choosing one
  is never refused (« ⋯ » then « Changer de dossier »). A file reachable from both is listed **once**, with the same
  state in both listings, and the tag « depuis … » shows only on a file really outside its family's folder, so never
  in the same-folder layout. Counters, the typed search, « Select. auto » and the saved views count and place each
  physical file once. After a change of folder the new family's files are « À analyser » until its **Analyser** has run;
  that pass forgets what the old folder left and reads the files natively. Rule: `docs/WORK-FOLDER-INVENTORY.md`, "The
  key of a file".
- **Per family:** a two-line header (the family's folder icon, its name, the three counters; **Analyser (n)**; a fold
  button), the analysis gauge while a pass runs (below), and
  a flat list with **its own scroll bar**. A click anywhere on a tickable row ticks it (`rowClickTicks`). **« Mes notes (N) »** (under the segmented control, only while the Notes folder holds a file) is a one-click origin-folder choice; while a text search is active the lists follow the finder's ranking, not the alphabet (`listSort`). The folder's short name (« …\Docs ») sits in grey after the family's name and gives way first; the whole path is the header's tooltip. The long text of a pass is **not** in the
  box any more (step 6): after a pass the box keeps its one end-of-pass line and, on its own line under it, **« Voir le
  rapport »**, which opens the centre on the Overview; the lines themselves (`lib/passSummary.ts`) stay for the two old cards
  of the Settings panel. Choosing, changing and clearing a folder are in the
  box itself since step 4 (below); the Settings panel still draws the two old cards untouched until step 10.
- **The gauge (step 5).** While a family's pass runs, **Analyser** is replaced by `AnalysisGauge`: a spinner, « Lecture 45 % »
  then « Sens 63 % » (« Noms n % » while the stored documents are read again for new names), a fill that follows the
  percentage and a square **Stop** of its own. It is not a disabled button (the text stays at full contrast) and is a
  `role="status"`. The « ⋯ » menu is absent, the fold button stays. **Each family stops on its own**: the Documents pass and the
  Tables pass can run together, and `cancel_analysis(domain)` raises only that family's flag. Under the header, one line
  (`lib/analysisLines.ts`): « Étape 1 sur 2 · Lecture des documents · 761 / 1 245 · 3 min 48 s écoulées · environ 2 min
  restantes », the stage and the counts first and the times last, so that an ellipsis cuts the times first (the whole line is
  the tooltip); the Tables pass has one stage and names no step; stage 2 adds « La recherche par mots fonctionne déjà. ». A thin
  2 px line under the header (`PROGRESS_HAIRLINE` in `ExplorerFamilySection.tsx`) fills with the stage's percentage. When the pass
  ends the same place says « Terminé · 1 245 analysés, 3 illisibles · 6 min 49 s », or « Analyse arrêtée · 3 min 48 s · ce qui
  est lu est conservé »; « Voir le rapport » comes with step 6. The numbers come from `lib/analysisProgress.ts` (pure: the clock
  is an input): the remaining time is shown only after 10 % of the stage and 8 s in it, from the **recent rate** (the last 15 s of
  messages, at least two), smoothed (30 % of each new estimate of the finishing moment), and it is the time left **in the stage
  being run**. `useIndexing` keeps the trace and the pass's duration; `useNow` ticks once a second only while a pass runs. A pass
  started from the Notes section shows the same gauge. The old cards of the Settings panel keep `AnalysisProgress` and their Stop
  until step 10.
- **Height budget** (`lib/explorerWindow.ts`): both lists open, six rows each; one open, twelve; both folded, the headers
  alone. A short list is not padded. Rows have a **fixed height** (24 px `compact`, 32 px `comfortable`), so a **hand-written
  window** draws only the rows in view plus four above and below, the rest being padding: a thousand files are about
  a dozen rows. Keyboard: the scroll area takes the focus when it scrolls, and the arrow keys scroll it; Tab walks the rows drawn.
- **Remembered across launches** (settings file, `settings.json`, key `explorer`): `density` (`compact` default, `comfortable`),
  `documentsFolded`, `tablesFolded` (open by default). Rust reads each field **leniently** (`ExplorerSettings`, a corrupted
  value is the default, the file never becomes unreadable); the density is chosen in the **Settings panel**, never in the box.
  The folds are written through `saveExplorer`, in the settings queue.
- **Search and selection.** One field. The finder works per folder, so the same text is asked of each and the two answers
  are joined by `(folder, path)`; a search that fails shows everything and says so. The search only filters the view and
  never changes the selection. The segmented control **Tout / Documents / Tableaux** filters the lists and shows counts
  (computed on the search results). The **bottom bar** says « Utilisés : n doc. · n tab. », offers **Cocher les N** (adds the
  analysed files the view shows, both folders, with `addScopeFiles` through the existing rules) and **Décocher**, and a
  second line only when there is something to say (nothing used, a whole family used, files of unread formats). The scope
  state (`scope`, `dataScope`, `combineScopes`) and the chat contract are unchanged.
- **The filter panel (explorer step 3, 20 October 2026).** The « Filtres » button opens a panel **in the flow of the box**, under the
  search line (the box clips what overflows it, and nothing may be drawn below the box). It closes on Escape (the focus returns to the
  button), on « Terminé » (same) and on a click outside it (the focus stays where the click put it). It has **no scroll area**: the
  format pills wrap. **Format** and **state** are pills with the number of files each leaves; the **origin folder** is a typed field
  that offers **at most five** suggestions (the folders, each ancestor included, with the files in it or under it) **only while it
  has the focus**, then « N autres : tapez pour affiner ». The segmented control stays outside. The choices in force appear as
  **removable pills** under the segmented control (a « Tout effacer » link from two), and as a badge on the button. A filter only
  narrows the view; it never changes the selection, and « Cocher les N » ticks what all the filters show. The numbers beside every
  choice, and the segmented control's, are counted over what **all the other filters** leave (the choice's own kind left out); a
  choice that would empty the list says 0. Rules and code: `lib/explorerFilters.ts` (pure, tested), `ExplorerFilter` in
  `lib/explorerModel.ts` (the filter type, with the origin folder), `components/ExplorerFilterPanel.tsx`,
  `components/ExplorerActivePills.tsx`. The two roots can hold a folder of the same name: a choice is identified by root and folder,
  and says its root (« 2025 (Docs) ») only when both hold that name.
- **A typed extension and the reasons of the hits.** The finder's answer now echoes the extensions it took out of the text
  (`FinderResult.typedFormats`, set by `find_files` from `finder::typed`): the matching format pill is lit and pressing it takes the
  extension out of the text. A typed extension **narrows** like any other filter (see `docs/DECISIONS.md`, « the explorer's filter
  panel »). With a text search, each row shows beside its name **why it was found** (« dans le nom », « dans le contenu », « dans une
  cellule »...), the first reason in the row and all of them in its tooltip, from `FinderHit.reasons`, which the paths-only request
  already carries.
- **The three situations of the box (explorer step 4, 20 October 2026).** `lib/explorerStates.ts` (pure, tested) decides from
  the two inventories and the settings: a family's folder is `none` (not chosen), `empty` (chosen, its inventory read, no file
  in the folder and none listed under the family) or `filled` (anything else, **including a folder not read yet or whose read
  failed**, so the box never flashes « empty »); the box is `firstStart` (both `none`), `empty` (no family `filled`) or `use`.
  While the box is not in use the search field and « Filtres » are disabled, and the segmented control and the bottom bar are
  not drawn: there is nothing to search or to select.
  - **First start**: a « Pour commencer » block (three steps, and « Créer les deux dossiers suggérés », which makes the two
    suggested folders one after the other and keeps the first if the second fails; the failure is said). Each family without a
    folder is a setup block (`ExplorerFamilySetup`): the type line (« Documents : PDF, DOCX, ODT… »), the suggested folder in its
    short form (the whole path and the reason it sits outside the user's own Documents are the **tooltip only**), and « Créer et
    utiliser » beside « Choisir un dossier ». A family alone without a folder (the other one is chosen) shows its setup block
    only, without the steps.
  - **Folder created, still empty**: the family's header says the formats it reads in place of the counters, **Analyser** is
    greyed with a tooltip, and the body is « Vide. Déposez-y vos fichiers. » with « Voir le dossier » (the existing reveal
    command) and « Changer ». The box re-reads the folder when the window gets the focus back (existing behaviour).
  - **The « ⋯ » menu** (`ExplorerFamilyMenu`): « Voir le dossier », « Changer de dossier », « Effacer l'analyse » (the confirmation
    dialog of the old cards, same texts, its button now says « Effacer l'analyse »). It opens in **fixed coordinates**
    (`lib/explorerMenu.ts`: under the button, above it when there is no room), because the box clips its overflow; it closes on a
    click elsewhere, on Escape (focus back to the button), on Tab, and on a resize or a scroll; the arrows, Home and End move
    between the entries. It is **not drawn while that family is being analysed**. The folder actions live in
    `state/useExplorerFolders.ts` (the commands of the old cards, the same guard for the Documents folder).
- **The filter is held by `App`** (step 6): `ExplorerView { family, choices }` (`lib/explorerView.ts`) is the segmented
  control's family and the filter panel's three sets, passed to the box as `view` / `onViewChange`, so the Overview's link sets
  exactly what the panel sets. The typed search stays in the box (`searchClears` asks it to empty). The box reports what the
  Overview needs from its inventories (`ExplorerFacts`: the merged entries, the situation of each folder, the share of the meaning
  search, the profile note) through `onFacts`.
- **Knowledge-base filters, views and « Select. auto »** (step 7, 21 October 2026): see the next section.

### The knowledge-base filters, the views and « Select. auto » (explorer step 7, 21 October 2026)

- **Filters.** The « Filtres » panel gains four blocks, drawn between the states and the origin folder: **Type de document** (chips,
  labelled from the catalogue key `businessType.<code>`; a type the build has no word for keeps its code), **Année** (chips,
  newest first), **Personnes** and **Organisations** (typed fields: at most five suggestions, the most frequent first, only while
  the field has the focus, « N autres : tapez pour affiner », no scroll bar). A chosen name is a chip; when it is linked to the
  files in two or more labelled roles it gets the **role step**: « Tous les rôles » then one chip per role with its count (author,
  subject, recipient, sender, issuer, « Cité »; a name met in the text with no role is not a role). Counts follow the step 3 rule:
  over what **all the other filters** leave, the kind's own choices left out; a chosen name or type the files no longer carry stays
  listed with 0 (a name the base no longer knows says « Nom introuvable »). The choices of one kind combine as « this or that », the
  kinds as « this and that », and everything combines with the family, the typed text and the other filters.
- **One read, no call per filter change.** `file_knowledge_facts(domain)` (Rust, `finder/facts.rs`) returns once per folder, for
  every analysed file, its business type, its years and its people and organisations with their roles, plus the names of those
  entities. `lib/explorerFacts.ts` attaches them to the entries (`ExplorerEntry.facts`), and **every count and every filter is worked
  out in TypeScript** (`filterEntries`, `typeChoices`, `yearChoices`, `entityOptions`, `suggestEntities`). It is read only when
  something needs it (the panel open, the « Vues » side, or a choice of the knowledge base in force), again after each pass and each
  time the panel opens, and never at launch (`state/useKnowledgeFacts.ts`). A person is a person and an organisation an
  organisation whatever packs are active: the type comes from the base. The engine facets were **not** used: they are counted
  over one result (the kind's own choices cannot be left out) and list ten entities at most.
- **Pills.** Each choice is a pill (`ActivePill` kinds `person`, `organisation`, `type`, `year`, plus `conversation` and `recent`);
  a person pill reads « Name · Role »; a folder that no longer exists says « (introuvable) ».
- **Vues.** The switch « Fichiers | Vues » of the header is enabled. « Vues » shows the three views of the application (**Récents**:
  newest first; **À analyser**: the state filter; **Dans la conversation**: only the files the conversation uses now), the user's
  own (open, rename, delete with a confirmation) and the form « Enregistrer les filtres actuels » (name unique ignoring case and
  accents, at most 60 characters, at most 50 views, nothing to save without a filter). A view stores **filters only** (the family and
  the panel's choices; never a file, a selection or the typed text), in `settings.json` under `explorerViews` (`SavedView` in
  `settings.rs`, read leniently entry by entry); a person or an organisation is kept **by id, never by name**. Opening a view
  replaces the filters through `onViewChange` and goes back to « Fichiers »; it **never touches the selection, the lists' folds or
  the typed text** (the text keeps narrowing; it stays visible in the field). A view or a filter that names someone the files no
  longer link to says so (« Un nom de ce filtre ne figure plus dans vos fichiers. ») instead of showing nothing.
- **« Select. auto ».** A `role="switch"` under the search line with its hint; off by default and at every launch (nothing is
  stored). Pure rules in `lib/autoSelect.ts`, held by `state/useAutoSelect.ts`; see `docs/SELECTION-AND-MEMORY.md`, « Automatic
  selection ». The bottom bar always says « Mode : automatique (suit les filtres) » or « Mode : manuel », shows
  « Rétablir la sélection précédente (n) » while a snapshot exists, says how many results cannot be ticked, and « Cocher les N
  résultats » is hidden while the switch is on.
- **Not built:** a per-person automatic view, a « Tout analyser » button, any change to how entities are extracted.

### The overview of the centre (explorer step 6, 20 October 2026)

The **Aperçu** section is a page that each analysis updates; it is never opened by force (a finished pass changes what it says,
nothing more). Rules and code: `lib/overviewModel.ts` (pure, tested: the inventories and the two passes' summaries in, an ordered
list of blocks out), `components/OverviewPage.tsx` (draws the blocks through the catalogues; its own page scroll is the only
scroll bar), `lib/centreSection.ts` (the section and when the zone is open).

- **Four phases.** No folder at all: the first-start guidance, pointing at the left. Folders chosen, nothing analysed in either:
  the three steps (folders chosen, files found, « Appuyez sur Analyser » with one sentence on what the analysis does) and a card per
  family (what it reads, how many files wait). Otherwise the report: per family the three counters, the date and length of the last
  pass of this session (« Analysé avant cette session. » when none ran), the meaning-search share and the profile note; then
  **À vérifier**, the files dropped from the index because they left the folder (a count, as the old cards say it), the cleaned names **in full**, the files read by text recognition, the unread formats, the knowledge-base lines with
  « Gérer », the capabilities that did not start, and where the time went.
- **À vérifier** lists the unreadable files of their own folder and the files the last pass could not read (a pass retries those,
  so the inventory still shows them as to analyse), with the reason from the error catalogue (or « Aucun texte lisible. », « Trop peu
  lisible pour être indexé. », « La reconnaissance de texte n'a trouvé aucun texte lisible. », « Aucun texte trouvé dans ce fichier. », « Aucun tableau exploitable. »; the index alone gives the last four, so no pass is needed), the first twenty and a count of the rest. **Voir** shows the file in
  the file manager, wherever the file sits (explorer step 9); **Ouvrir** appears only where the explorer's own rows open a file
  (analysed, wherever it sits), so it is not there for an unreadable one. « Afficher les N dans la liste de gauche » sets the explorer's state filter to
  unreadable for both families, empties the typed search and unfolds the folded lists that hold some of those files; N is what that filter shows. Each family's card shows its folder's **whole path**.
- **The unread formats are text, not filters:** a file no pipeline reads is not an entry of the explorer, so a chip cannot filter the
  list (its tooltip says so).
- The report is held for the session, like the summaries it comes from; a new pass clears its family's report while it runs.

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

- The suggested folder is `~/AssistantCabinetAI/Docs`, shown before anything is chosen. `Docs` is a
  dedicated subfolder of `~/AssistantCabinetAI` rather than the root itself, so a sibling `Data`
  folder for spreadsheet work (CSV/XLSX) can sit beside it later without the two kinds of file
  sharing one directory. A primary button **creates it if needed and uses it** (both the root and
  `Docs` together); **Choose a folder...** still opens the system dialog. Nothing is created until
  she asks.
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

## The data folder

A second, sibling folder for tabular files (CSV, XLSX): `~/AssistantCabinetAI/Data`, beside
`~/AssistantCabinetAI/Docs`. The documents folder feeds retrieval — extract, index, answer with
sources. The data folder is not a second documents folder and does not feed that pipeline: it
exists so CSV/XLSX files never share a directory with PDF/DOCX/scans, and it feeds the deterministic
tabular engine (`tabular::engine`, `docs/ARCHITECTURE.md`'s "Tabular analysis" row) instead.
**Unlike the documents folder, it is never required**: the conversation works without one.

Its card is the documents folder card, cloned (tabular UI session, 28 September 2026): one
component, `DataFolderCard`, in the sidebar and again in the settings panel, built from the same
parts as `WorkFolderCard` (`components/FolderCardParts.tsx`) — the same buttons (Voir, Changer,
Reset, **Analyser**), the same summary line ("X fichiers, Y analysés, Z illisibles"), the same dots,
and, in the sidebar, the same selection list ("Données utilisées : aucune ⚠ / N fichiers /
toutes ⚠").

- **Analyser** (`index_data_folder`) cleans file names exactly as the documents pass does (same
  rule, same `renamed-files.jsonl`), then parses every CSV/XLS/XLSX/XLSM file on this computer and
  caches its `TabularInventory`. No embedding and no gateway call, so it is over in a moment; the
  line under the card says so, because an Analyse that finishes before the spinner is seen would
  otherwise read as a button that did nothing.
- **Three states per file**, computed by `data_folder::DataFolder` from the filesystem
  (`WorkFolderInventory`, unchanged, built with no index) joined with the cached inventories
  (`IndexStore::all_tabular_inventories`): orange, not analysed yet or changed since; green, at
  least one sheet looks like a real table (`TabularInventory::has_a_usable_sheet`: 8 rows, 2
  columns, formulas under 5 % of the rows); red, analysed and nothing usable in it — a note renamed
  `.csv`, a three-row table, a file that is not a workbook — or not a spreadsheet at all.
- **Only a green file can be ticked**, as only an analysed document can be. Unticking the last file
  returns to "aucune", never to "toutes". Changing the data folder resets the data selection; the
  next Analyse drops the inventories of files no longer present.
- **Reset** (`reset_data_index`) forgets the workbook analyses only. The documents index is never
  touched by it, nor the workbooks by the documents card's Reset.

A conversation with workbooks selected and no document is answered by the tabular engine alone —
tier 2 of the grounding priority chain (`docs/SELECTION-AND-MEMORY.md`): a computed value or a
structural fact, cited by **sheet** (and column) where a document answer cites its page, with no
model involved; or, when the engine cannot answer exactly, a sentence naming the workbook's real
columns and the operations that work. Since session 11 (30 September 2026) a filter anchored on
real data — a value, a weekday, a month, a year, or a comparison beside a number — is computed, not
just named: the answer shows an **"Understood as"** line under it, written from the filter the
engine actually ran, never from the question's own words. A value found in two reachable columns
is asked about rather than guessed at (`WhichColumn`); a value found in none gets a nudge naming
close real values instead of a silent zero. Documents and workbooks selected together: a question
that is clearly and only about the data is routed straight to the tabular engine, exactly as above
(session 15, D7 in `docs/DECISIONS.md`); every other question is still refused with
`documents_and_tables_together` — combining both sides in one answer is a later session's design.

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

The full runbook — client and server hardware requirements, the LAN topology, the beta rollout checklist for
this cabinet and the template for the next one — is `docs/DEPLOYMENT.md`.

### The knowledge base in the window (KB programme, lot 11)

« Réglages » → « Connaissances » has the mode « Ciblage intelligent » (« Suggestions seulement » by default; « Désactivé »; « Automatique »), the health modules and « Gérer… » (the dialog). In « Automatique » a line under the answer says « Recherche ciblée : N documents sur M — nom » with « Voir les fichiers » and « Élargir à ma sélection ». The script the owner plays to decide whether « Automatique » may become the default is `docs/test-reports/knowledge-base-pass-1/human-tests/lot-11-validation.md`; the verdict of lot 11 is in `03-release-gate.md` (not met: it stays optional). The canonical description is `docs/KNOWLEDGE-BASE.md`.

### Voice in the window (speech programme, decided 17 and 18 October 2026)

Not built yet; this is what the phases will build, so that the layout rules above stay true. Spec: `docs/SPEECH.md`.

- **Notes: a section of the centre, not a kind of view.** A title-bar button beside Settings (its own switch `notesEnabled`, on by
  default, independent of the voice master switch) opens the centre on the **Notes** section, or hides the zone when it is already
  there. A switch **Aperçu · Fichiers · Notes** (role `tablist`) sits at the top of the centre zone and is drawn whenever the
  zone is open since the overview (step 6); `Notes` is in it only while `notesEnabled` is on. The file tabs are kept and
  `WorkspaceView` is not changed: `App` holds the section (`"overview" | "files" | "notes"`) and the drafts, and passes `AppLayout` a value derived from both, in one
  pure function, so that the rule "the centre is visible exactly while there is something to show" keeps working while a note holds
  a draft or a recording. Both sections stay mounted while the zone is open (the inactive one is hidden), so open file tabs keep
  their scroll, zoom and sheet while she writes, and an unsaved note and a recording in progress survive a switch. Opening a file
  from a list shows the Files section. A red dot marks the Notes segment while recording. The welcome view is the empty state of the
  Files section (« Aucun fichier n'est ouvert. »). A note saved from here starts the application's own Analyse; its report is read
  in the Overview like any other, and the zone is not opened for it.
- **The Notes pane.** The text fills the pane and is editable, with the viewer's Raw / Preview switch (Markdown rendered by the
  application's own component); a dark pill button centred at the bottom (microphone at rest, an animated waveform driven by the input
  level while recording, "click again to finish"); under the text, a save block (a name field whose placeholder shows the timestamp,
  a format choice, `.md` by default, and **Save**); a switch between several drafts and a "new note" button. It works at the 320 px
  minimum of the centre. Save shows a preview of the exact files, writes them to `<Documents folder>/Notes`, then runs the Analyse
  (with a confirmation when other files are also pending). A modal with Save, Discard and Cancel waits before an unsaved note can be
  lost (another folder, closing the window, turning notes or voice off, Reset); drafts are never written to disk.
- **The microphone button** in the Composer, between the **@** button and the model selector; with voice off the Composer renders
  exactly as today. The transcript fills the text box; the user edits and presses Send. Names the knowledge base can suggest appear as
  "heard to display" chips, selection only, collapsed while names are hidden.
- **Read aloud** is one button in the action row under each finished answer, after Copy and Regenerate: Read aloud / Stop, nothing else.
- **Settings** gets one block of its own, beside the Advanced one: master switch, per-feature switches, model status and the
  install action, the language override, the caps, the voice and rate, the notes folder name and the voice-commands switch.

