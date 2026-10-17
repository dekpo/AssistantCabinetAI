import { useMemo, useRef } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { prepareViewerText } from "../../lib/viewerText";
import { useScrollMemory, type ScrollMemory } from "./useScrollMemory";

/** The element is the scroller itself: nothing to look for inside it. */
const ITSELF = (root: HTMLElement): HTMLElement => root;

/**
 * A text file as stored, line by line, in the application's own element rather than the library's
 * text engine (which joins single line breaks). Monospace, wrapping, selectable, no Markdown
 * rendering. Small and separate on purpose: the Raw view of a Markdown file reuses it. Its position
 * is remembered through `memory` (the tab's view state).
 */
export function TextView({
  bytes,
  memory,
}: {
  bytes: ArrayBuffer | Uint8Array;
  memory: ScrollMemory;
}) {
  const { t } = useTranslation();
  const view = useMemo(() => prepareViewerText(bytes), [bytes]);
  const scroller = useRef<HTMLPreElement>(null);
  useScrollMemory(scroller, true, memory, ITSELF);

  return (
    <div className="viewer__textview">
      {view.truncated ? (
        <p className="viewer__notice" role="status">
          {t("viewer.textTruncated", { shown: view.shownCharacters.toLocaleString() })}
        </p>
      ) : null}
      <pre ref={scroller} className="viewer__text">
        {view.text}
      </pre>
    </div>
  );
}
