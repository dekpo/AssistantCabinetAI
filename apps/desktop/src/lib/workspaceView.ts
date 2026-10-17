/**
 * What the centre zone of the window shows. A discriminated union so that each later use of the
 * zone (a file viewer, voice notes, a calendar...) adds a kind here and a case in `WorkspacePanel`,
 * without touching the layout. The zone is open exactly while `App` holds one of these.
 */
export type WorkspaceView = { kind: "welcome" };

/** Opens the centre zone on a view, or replaces what it shows. */
export type OpenWorkspace = (view: WorkspaceView) => void;
