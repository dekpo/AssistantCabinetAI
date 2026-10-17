import type { FileRecord } from "./ipc";

/** The two roots a file can come from: a path is only meaningful with the folder it is relative to. */
export type ViewerFolder = "documents" | "data";

/** One tab of the centre: a file and the folder it is relative to. The whole record is kept: the
 * outside-change guard needs its `relativePath`, `sha256` and `processingStatus`. `id` is
 * `tabId(folder, relativePath)` (`viewerTabs.ts`). */
export interface OpenFile {
  id: string;
  folder: ViewerFolder;
  record: FileRecord;
}

/**
 * What the centre zone of the window shows. A discriminated union so that each later use of the
 * zone (voice notes, a calendar...) adds a kind here and a case in `WorkspacePanel`, without
 * touching the layout. The zone is open exactly while `App` holds one of these. `files` never holds
 * an empty list: closing the last tab closes the zone.
 */
export type WorkspaceView =
  | { kind: "welcome" }
  | { kind: "files"; tabs: OpenFile[]; activeId: string | null };

/** Opens the centre zone on a view, or replaces what it shows. */
export type OpenWorkspace = (view: WorkspaceView) => void;
