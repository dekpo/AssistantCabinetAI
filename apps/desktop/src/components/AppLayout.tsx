import { useEffect, useRef, useState, type ReactNode } from "react";
import { Group, Panel, Separator, useGroupRef } from "react-resizable-panels";
import { useTranslation } from "../i18n/I18nProvider";
import type { PanelLayouts } from "../lib/ipc";
import {
  CHAT_ID,
  NO_LAYOUTS,
  RESOURCES_ID,
  WORKSPACE_ID,
  isCentreOpen,
  layoutToApply,
  storeLayout,
} from "../lib/panelLayout";
import type { WorkspaceView } from "../lib/workspaceView";
import { WorkspacePanel } from "./WorkspacePanel";

/* Pixels, not percentages: below these the content of the zone stops being usable (the two folder
   cards, a readable page, the model selector beside the composer). A number is pixels and a string
   is a percentage in this library's v4 API. */
const RESOURCES_MIN_PX = 240;
const WORKSPACE_MIN_PX = 320;
const CHAT_MIN_PX = 340;

/**
 * The three zones of the window, left to right: the resources (both folder cards), the workspace
 * and the chat. The workspace panel is **always rendered**: closed, it is collapsed to nothing, so
 * the group always holds three panels and the chat keeps its place in the tree (the conversation
 * lives in it and must never be remounted). The workspace is visible exactly while `workspace` is
 * not `null`.
 */
export function AppLayout({
  resources,
  chat,
  workspace,
  layouts,
  onLayoutsChange,
  onWorkspaceCollapsed,
  onCloseWorkspace,
  onFileMissing,
  restoreNonce,
}: {
  resources: ReactNode;
  chat: ReactNode;
  workspace: WorkspaceView | null;
  /** The saved widths, read once at launch and when the user asks for the default layout. */
  layouts: PanelLayouts;
  /** Asks for the saved widths to be changed; the change is made on the settings of its turn. */
  onLayoutsChange: (change: (current: PanelLayouts) => PanelLayouts) => void;
  /** The user dragged the workspace closed: `App` forgets its view. */
  onWorkspaceCollapsed: () => void;
  /** The close button of what the workspace shows. */
  onCloseWorkspace: () => void;
  /** The file shown is no longer where it was (stable function). */
  onFileMissing: () => void;
  /** Changes whenever the default layout must be put back on screen at once. */
  restoreNonce: number;
}) {
  const { t } = useTranslation();
  const groupRef = useGroupRef();
  const centreOpen = workspace !== null;
  const centreOpenRef = useRef(centreOpen);
  centreOpenRef.current = centreOpen;
  /* The widths of this window session. They answer "reopen at the size it had" even when saving
     failed, and they are what the layouts are first read from: the settings are read at launch. */
  const session = useRef<PanelLayouts>(layouts);
  /* Nothing is open at launch (nothing has been chosen to show), so the group starts collapsed. */
  const [initialLayout] = useState(() => layoutToApply(layouts, false));

  /* Opening or closing from anywhere (the button, later a viewer) puts the widths of that state on
     screen. A close the user made by dragging already is that layout, so nothing is moved. */
  useEffect(() => {
    /* One frame later: the panel's own limits change with `centreOpen` and the library applies them
       after this effect, so a layout set at once is cut back to the old limits (a closed centre
       cannot grow). */
    const frame = requestAnimationFrame(() => {
      const group = groupRef.current;
      if (group === null || isCentreOpen(group.getLayout()) === centreOpen) {
        return;
      }
      group.setLayout(layoutToApply(session.current, centreOpen));
    });
    return () => cancelAnimationFrame(frame);
  }, [centreOpen, groupRef]);

  useEffect(() => {
    if (restoreNonce === 0) {
      return;
    }
    session.current = NO_LAYOUTS;
    groupRef.current?.setLayout(layoutToApply(NO_LAYOUTS, centreOpenRef.current));
  }, [restoreNonce, groupRef]);

  return (
    <Group
      id="app-layout"
      className="app__group"
      orientation="horizontal"
      groupRef={groupRef}
      defaultLayout={initialLayout}
      onLayoutChanged={(layout, meta) => {
        /* Only what the user did counts: a layout the code applied (opening, restoring, the
           library correcting itself after a window resize) is neither saved nor allowed to close
           what was just opened. */
        if (!meta.isUserInteraction) {
          return;
        }
        const shown = isCentreOpen(layout);
        if (!shown && centreOpenRef.current) {
          onWorkspaceCollapsed();
        }
        if (shown && !centreOpenRef.current) {
          // A closed centre is never opened by a drag on the line beside it.
          groupRef.current?.setLayout(layoutToApply(session.current, false));
          return;
        }
        const stored = storeLayout(session.current, layout);
        if (stored !== null) {
          session.current = stored;
          onLayoutsChange((current) => storeLayout(current, layout) ?? current);
        }
      }}
    >
      <Panel
        id={RESOURCES_ID}
        className="pane"
        defaultSize="22%"
        minSize={RESOURCES_MIN_PX}
        maxSize="35%"
      >
        <div className="pane__content">{resources}</div>
      </Panel>
      <Separator
        id="separator-resources"
        className={centreOpen ? "pane-separator" : "pane-separator pane-separator--hidden"}
        disabled={!centreOpen}
        aria-label={t("workspace.separatorResources")}
      />
      <Panel
        id={WORKSPACE_ID}
        className="pane"
        defaultSize="38%"
        collapsible
        collapsedSize={0}
        minSize={centreOpen ? WORKSPACE_MIN_PX : 0}
        maxSize={centreOpen ? "70%" : 0}
      >
        <div className="pane__content">
          {workspace === null ? null : (
            <WorkspacePanel
              view={workspace}
              onClose={onCloseWorkspace}
              onFileMissing={onFileMissing}
            />
          )}
        </div>
      </Panel>
      <Separator
        id="separator-chat"
        className="pane-separator"
        aria-label={centreOpen ? t("workspace.separatorChat") : t("workspace.separatorBetween")}
      />
      <Panel id={CHAT_ID} className="pane" defaultSize="40%" minSize={CHAT_MIN_PX}>
        <div className="pane__content">{chat}</div>
      </Panel>
    </Group>
  );
}
