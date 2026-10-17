import { useTranslation } from "../i18n/I18nProvider";
import type { ViewStateStore } from "../lib/viewState";
import type { WorkspaceView } from "../lib/workspaceView";
import { FileViewer } from "./viewer/FileViewer";
import { TAB_PANEL_ID, TabStrip, tabDomId } from "./viewer/TabStrip";

/** What the tabs of the centre can ask of `App`. */
export interface TabActions {
  activate: (id: string) => void;
  close: (id: string) => void;
  /** The file of this tab is no longer where it was. */
  missing: (id: string) => void;
}

/** The content of the centre zone: one case per kind of `WorkspaceView`. */
export function WorkspacePanel({
  view,
  viewStates,
  actions,
}: {
  view: WorkspaceView;
  /** What each tab remembers of its view; owned by `App`, so it outlives a tab switch. */
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
