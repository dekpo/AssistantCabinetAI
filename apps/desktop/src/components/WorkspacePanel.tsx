import { useTranslation } from "../i18n/I18nProvider";
import type { ViewStateStore } from "../lib/viewState";
import type { WorkspaceView } from "../lib/workspaceView";
import { offeredSections, type CentreSection } from "../lib/centreSection";
import type { OverviewPage as OverviewPageModel } from "../lib/overviewModel";
import { CentreSwitch, centrePanelId, centreTabId } from "./notes/CentreSwitch";
import { NotesPanel, type NotesBundle } from "./notes/NotesPanel";
import { OverviewPage, type OverviewActions } from "./OverviewPage";
import { FileViewer } from "./viewer/FileViewer";
import { TAB_PANEL_ID, TabStrip, tabDomId } from "./viewer/TabStrip";

/** What the tabs of the centre can ask of `App`. */
export interface TabActions {
  activate: (id: string) => void;
  close: (id: string) => void;
  /** The file of this tab is no longer where it was. */
  missing: (id: string) => void;
}

/** What the sections of the centre need: the section on screen, the way to change it, and the
 * content of the two sections that are not the files. */
export interface Centre {
  section: CentreSection;
  onSection: (section: CentreSection) => void;
  /** A recording is running (red dot on the Notes segment). */
  recording: boolean;
  /** The Notes section; absent while its switch is off. */
  notes?: NotesBundle;
  /** The overview page; `null` while it is not on screen, so it is neither worked out nor drawn. */
  overview: { page: OverviewPageModel | null; actions: OverviewActions };
}

/**
 * The content of the centre zone: a switch on top (Aperçu, Fichiers and, while its switch is on,
 * Notes), and the sections under it. Files and Notes stay mounted, the inactive one hidden, so open
 * tabs keep their scroll and zoom while a note is written, and a note (and later a recording)
 * survives a switch. The overview is drawn only while it is on screen: it holds nothing to keep, and
 * a long report is not worked out on every tick of a running pass.
 */
export function WorkspacePanel({
  view,
  viewStates,
  actions,
  centre,
}: {
  view: WorkspaceView;
  /** What each tab remembers of its view; owned by `App`, so it outlives a tab switch. */
  viewStates: ViewStateStore;
  actions: TabActions;
  centre: Centre;
}) {
  return (
    <div className="centre">
      <CentreSwitch
        sections={offeredSections(centre.notes !== undefined)}
        section={centre.section}
        recording={centre.recording}
        onChange={centre.onSection}
      />
      <div
        className="centre__section"
        role="tabpanel"
        id={centrePanelId("overview")}
        aria-labelledby={centreTabId("overview")}
        hidden={centre.section !== "overview"}
      >
        {centre.section === "overview" && centre.overview.page !== null ? (
          <OverviewPage page={centre.overview.page} actions={centre.overview.actions} />
        ) : null}
      </div>
      <div
        className="centre__section"
        role="tabpanel"
        id={centrePanelId("files")}
        aria-labelledby={centreTabId("files")}
        hidden={centre.section !== "files"}
      >
        <FilesSection view={view} viewStates={viewStates} actions={actions} />
      </div>
      {centre.notes === undefined ? null : (
        <div
          className="centre__section"
          role="tabpanel"
          id={centrePanelId("notes")}
          aria-labelledby={centreTabId("notes")}
          hidden={centre.section !== "notes"}
        >
          <NotesPanel bundle={centre.notes} />
        </div>
      )}
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
