import { useTranslation } from "../i18n/I18nProvider";
import type { ViewStateStore } from "../lib/viewState";
import type { WorkspaceView } from "../lib/workspaceView";
import type { CentreSection } from "../lib/centreSection";
import { CentreSwitch, centrePanelId, centreTabId } from "./notes/CentreSwitch";
import { NotesPanel, type NotesBundle } from "./notes/NotesPanel";
import { FileViewer } from "./viewer/FileViewer";
import { TAB_PANEL_ID, TabStrip, tabDomId } from "./viewer/TabStrip";

/** What the tabs of the centre can ask of `App`. */
export interface TabActions {
  activate: (id: string) => void;
  close: (id: string) => void;
  /** The file of this tab is no longer where it was. */
  missing: (id: string) => void;
}

/** The Notes section of the centre: its bundle, the section on screen and the way to change it. */
export interface CentreNotes {
  section: CentreSection;
  onSection: (section: CentreSection) => void;
  /** A recording is running (red dot on the Notes segment). */
  recording: boolean;
  bundle: NotesBundle;
}

/**
 * The content of the centre zone. With the Notes switch off it is the files' own content, as
 * always. With it on, a `Files | Notes` switch sits on top and both sections stay mounted, the
 * inactive one hidden, so open tabs keep their scroll and zoom while a note is written, and a note
 * (and later a recording) survives a switch.
 */
export function WorkspacePanel({
  view,
  viewStates,
  actions,
  notes,
}: {
  view: WorkspaceView;
  /** What each tab remembers of its view; owned by `App`, so it outlives a tab switch. */
  viewStates: ViewStateStore;
  actions: TabActions;
  notes?: CentreNotes;
}) {
  const files = <FilesSection view={view} viewStates={viewStates} actions={actions} />;
  if (notes === undefined) {
    return files;
  }
  return (
    <div className="centre">
      <CentreSwitch
        section={notes.section}
        recording={notes.recording}
        onChange={notes.onSection}
      />
      <div
        className="centre__section"
        role="tabpanel"
        id={centrePanelId("files")}
        aria-labelledby={centreTabId("files")}
        hidden={notes.section !== "files"}
      >
        {files}
      </div>
      <div
        className="centre__section"
        role="tabpanel"
        id={centrePanelId("notes")}
        aria-labelledby={centreTabId("notes")}
        hidden={notes.section !== "notes"}
      >
        <NotesPanel bundle={notes.bundle} />
      </div>
    </div>
  );
}

/** The Files section: one case per kind of `WorkspaceView`. */
function FilesSection({
  view,
  viewStates,
  actions,
}: {
  view: WorkspaceView;
  viewStates: ViewStateStore;
  actions: TabActions;
}) {
  const { t } = useTranslation();

  switch (view.kind) {
    case "welcome":
      return (
        <section className="workspace">
          <h2 className="workspace__title">{t("workspace.title")}</h2>
          <p className="workspace__text">{t("workspace.welcome")}</p>
        </section>
      );
    case "files": {
      const activeIndex = view.tabs.findIndex((tab) => tab.id === view.activeId);
      const active = view.tabs[activeIndex] ?? null;
      return (
        <section className="workspace workspace--file">
          <TabStrip
            tabs={view.tabs}
            activeId={view.activeId}
            onActivate={actions.activate}
            onClose={actions.close}
          />
          <div
            className="viewer__panel"
            role="tabpanel"
            id={TAB_PANEL_ID}
            aria-labelledby={active === null ? undefined : tabDomId(activeIndex)}
          >
            {active === null ? null : (
              /* Keyed by the tab and its content: another tab, or the same path with another hash,
                 starts from a clean viewer, and only this one is ever mounted. */
              <FileViewer
                key={`${active.id}\u0001${active.record.sha256}`}
                tab={active}
                initialState={viewStates.recall(active.id, active.record.sha256)}
                onSave={(state) => viewStates.save(active.id, active.record.sha256, state)}
                onMissing={() => actions.missing(active.id)}
              />
            )}
          </div>
        </section>
      );
    }
  }
}
