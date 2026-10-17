import { useMemo, useRef } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { prepareViewerText } from "../../lib/viewerText";
import { Markdown } from "../Markdown";
import { useScrollMemory, type ScrollMemory } from "./useScrollMemory";

/** The element is the scroller itself: nothing to look for inside it. */
const ITSELF = (root: HTMLElement): HTMLElement => root;

/**
 * A Markdown file read as formatting, through the application's own `Markdown` component: nothing a
 * file contains can become markup, and a link or an image is reduced to its text, so the preview
 * can neither navigate the window nor fetch anything. It takes the same bounded text as the raw
 * view (same decoding, same limit, same notice), so a very large file cannot freeze the parser.
 * Its position is remembered through `memory` (the tab's view state).
 */
export function MarkdownPreview({
  bytes,
  memory,
}: {
  bytes: ArrayBuffer | Uint8Array;
  memory: ScrollMemory;
}) {
  const { t } = useTranslation();
  const view = useMemo(() => prepareViewerText(bytes), [bytes]);
  const scroller = useRef<HTMLDivElement>(null);
  useScrollMemory(scroller, true, memory, ITSELF);

  return (
    <div className="viewer__textview">
      {view.truncated ? (
        <p className="viewer__notice" role="status">
          {t("viewer.textTruncated", { shown: view.shownCharacters.toLocaleString() })}
        </p>
      ) : null}
      <div ref={scroller} className="viewer__preview">
        <div className="message__body--markdown">
          <Markdown text={view.text} />
        </div>
      </div>
    </div>
  );
}
