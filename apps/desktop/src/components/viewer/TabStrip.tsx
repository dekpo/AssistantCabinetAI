import { useEffect, useRef, type KeyboardEvent } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { tabLabel } from "../../lib/viewerTabs";
import type { OpenFile } from "../../lib/workspaceView";
import { DocumentGlyph } from "../DocumentGlyph";
import { TableGlyph } from "../TableGlyph";

/** The id of the panel the active tab controls. */
export const TAB_PANEL_ID = "workspace-tabpanel";

/** The id of the tab at `index`, for the panel's `aria-labelledby`. */
export function tabDomId(index: number): string {
  return `workspace-tab-${index}`;
}

/**
 * The tabs of the centre: one per open file, in the order they were opened. A `tablist` with the
 * usual keys (Left and Right move to the neighbour and wrap, Home and End go to the ends; moving
 * focus activates, since switching is cheap: only the active tab is ever mounted). Only the active
 * tab and its close button are in the Tab order (a roving tabindex), so a long strip is crossed in
 * one stop. The strip scrolls sideways when it overflows and keeps the active tab in view.
 */
export function TabStrip({
  tabs,
  activeId,
  onActivate,
  onClose,
}: {
  tabs: readonly OpenFile[];
  activeId: string | null;
  onActivate: (id: string) => void;
  onClose: (id: string) => void;
}) {
  const { t } = useTranslation();
  const buttons = useRef(new Map<string, HTMLButtonElement>());
  /* Set when focus has to follow the active tab once it has changed (after a close). */
  const focusActive = useRef(false);

  useEffect(() => {
    if (activeId === null) {
      return;
    }
    const button = buttons.current.get(activeId);
    if (button === undefined) {
      return;
    }
    if (typeof button.scrollIntoView === "function") {
      button.scrollIntoView({ block: "nearest", inline: "nearest" });
    }
    if (focusActive.current) {
      focusActive.current = false;
      button.focus();
    }
  }, [activeId, tabs.length]);

  const move = (event: KeyboardEvent, index: number) => {
    let target = -1;
    if (event.key === "ArrowRight") {
      target = (index + 1) % tabs.length;
    } else if (event.key === "ArrowLeft") {
      target = (index + tabs.length - 1) % tabs.length;
    } else if (event.key === "Home") {
      target = 0;
    } else if (event.key === "End") {
      target = tabs.length - 1;
    }
    const next = tabs[target];
    if (next === undefined) {
      return;
    }
    event.preventDefault();
    buttons.current.get(next.id)?.focus();
    onActivate(next.id);
  };

  return (
    <div className="tabs" role="tablist" aria-label={t("viewer.tabsLabel")}>
      {tabs.map((tab, index) => {
        const active = tab.id === activeId;
        const name = tabLabel(tab.record.relativePath);
        const folder = t(tab.folder === "documents" ? "viewer.folderDocuments" : "viewer.folderData");
        return (
          <div key={tab.id} role="presentation" className={active ? "tab tab--active" : "tab"}>
            <button
              type="button"
              role="tab"
              id={tabDomId(index)}
              ref={(element) => {
                if (element === null) {
                  buttons.current.delete(tab.id);
                } else {
                  buttons.current.set(tab.id, element);
                }
              }}
              className="tab__label"
              aria-selected={active}
              aria-controls={active ? TAB_PANEL_ID : undefined}
              tabIndex={active ? 0 : -1}
              title={t("viewer.tabTitle", { path: tab.record.relativePath, folder })}
              onClick={() => onActivate(tab.id)}
              onKeyDown={(event) => move(event, index)}
            >
              {tab.folder === "documents" ? <DocumentGlyph /> : <TableGlyph />}
              <span className="tab__name">{name}</span>
            </button>
            <button
              type="button"
              className="tab__close"
              aria-label={t("viewer.closeTab", { name })}
              title={t("viewer.closeTab", { name })}
              tabIndex={active ? 0 : -1}
              onClick={(event) => {
                /* Focus follows only a close made with the keyboard (a click made with Enter or
                   Space has no pointer position: `detail` is 0). */
                focusActive.current = event.detail === 0;
                onClose(tab.id);
              }}
            >
              <svg viewBox="0 0 12 12" aria-hidden="true">
                <path
                  d="M3 3 L9 9 M9 3 L3 9"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.4"
                  strokeLinecap="round"
                />
              </svg>
            </button>
          </div>
        );
      })}
    </div>
  );
}
