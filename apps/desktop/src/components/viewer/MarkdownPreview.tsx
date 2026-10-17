import { useMemo } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { prepareViewerText } from "../../lib/viewerText";
import { Markdown } from "../Markdown";

/**
 * A Markdown file read as formatting, through the application's own `Markdown` component: nothing a
 * file contains can become markup, and a link or an image is reduced to its text, so the preview
 * can neither navigate the window nor fetch anything. It takes the same bounded text as the raw
 * view (same decoding, same limit, same notice), so a very large file cannot freeze the parser.
 */
export function MarkdownPreview({ bytes }: { bytes: ArrayBuffer | Uint8Array }) {
  const { t } = useTranslation();
  const view = useMemo(() => prepareViewerText(bytes), [bytes]);

  return (
    <div className="viewer__textview">
      {view.truncated ? (
        <p className="viewer__notice" role="status">
          {t("viewer.textTruncated", { shown: view.shownCharacters.toLocaleString() })}
        </p>
      ) : null}
      <div className="viewer__preview">
        <div className="message__body--markdown">
          <Markdown text={view.text} />
        </div>
      </div>
    </div>
  );
}
