import { useMemo } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { prepareViewerText } from "../../lib/viewerText";

/**
 * A text file as stored, line by line, in the application's own element rather than the library's
 * text engine (which joins single line breaks). Monospace, wrapping, selectable, no Markdown
 * rendering. Small and separate on purpose: the Raw view of a Markdown file reuses it.
 */
export function TextView({ bytes }: { bytes: ArrayBuffer | Uint8Array }) {
  const { t } = useTranslation();
  const view = useMemo(() => prepareViewerText(bytes), [bytes]);

  return (
    <div className="viewer__textview">
      {view.truncated ? (
        <p className="viewer__notice" role="status">
          {t("viewer.textTruncated", { shown: view.shownCharacters.toLocaleString() })}
        </p>
      ) : null}
      <pre className="viewer__text">{view.text}</pre>
    </div>
  );
}
