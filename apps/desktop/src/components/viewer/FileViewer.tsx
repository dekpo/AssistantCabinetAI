import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { normaliseError, errorMessage, type AppError } from "../../lib/errors";
import {
  readDataFileForViewer,
  readWorkFileForViewer,
  revealDataFile,
  revealWorkFile,
} from "../../lib/ipc";
import { viewerRendererOf } from "../../lib/viewerFormats";
import type { FileView } from "../../lib/workspaceView";
import { ErrorBanner } from "../ErrorBanner";
import { DocumentViewer } from "./DocumentViewer";
import { TextView } from "./TextView";

type Load =
  | { state: "loading" }
  | { state: "ready"; bytes: ArrayBuffer }
  | { state: "failed"; error: AppError };

/**
 * The centre zone when it shows a file: a header with its name and the close button, and under it
 * the renderer its extension asks for (`viewerRendererOf`). Reading is display-only: nothing here
 * writes, selects or analyses anything. A file that is gone ends in `onMissing`, which closes the
 * zone and shows the one modal; any other failure stays on screen as a sentence.
 */
export function FileViewer({
  view,
  onClose,
  onMissing,
}: {
  view: FileView;
  onClose: () => void;
  /** Must be stable: a new function would read the file again. */
  onMissing: () => void;
}) {
  const { t } = useTranslation();
  const { folder, record } = view;
  const renderer = useMemo(() => viewerRendererOf(record.extension), [record.extension]);
  const [load, setLoad] = useState<Load>({ state: "loading" });
  const [revealError, setRevealError] = useState<AppError | null>(null);
  const readable = renderer.renderer !== "unsupported";

  useEffect(() => {
    setRevealError(null);
    if (!readable) {
      return;
    }
    let cancelled = false;
    setLoad({ state: "loading" });
    const read = folder === "documents" ? readWorkFileForViewer : readDataFileForViewer;
    read(record.relativePath)
      .then((bytes) => {
        if (!cancelled) {
          setLoad({ state: "ready", bytes });
        }
      })
      .catch((raw: unknown) => {
        if (cancelled) {
          return;
        }
        const error = normaliseError(raw);
        if (error.code === "viewer_file_missing") {
          onMissing();
          return;
        }
        setLoad({ state: "failed", error });
      });
    return () => {
      cancelled = true;
    };
    /* The content identifies the file: the same path with another hash is another file. */
  }, [folder, record.relativePath, record.sha256, readable, onMissing]);

  const reveal = () => {
    setRevealError(null);
    const show = folder === "documents" ? revealWorkFile : revealDataFile;
    show(record.relativePath).catch((raw: unknown) => setRevealError(normaliseError(raw)));
  };

  const revealButton = (
    <button type="button" className="button button--compact" onClick={reveal}>
      {t("viewer.reveal")}
    </button>
  );

  let body;
  if (renderer.renderer === "unsupported") {
    body = (
      <div className="viewer__message viewer__message--block" role="status">
        <p>{t("viewer.unsupported")}</p>
        {revealButton}
      </div>
    );
  } else if (load.state === "loading") {
    body = (
      <p className="viewer__message" role="status">
        {t("viewer.loading")}
      </p>
    );
  } else if (load.state === "failed") {
    body = (
      <div className="viewer__message viewer__message--block" role="alert">
        <p>{errorMessage(t, load.error)}</p>
        {revealButton}
      </div>
    );
  } else if (renderer.renderer === "text") {
    body = <TextView bytes={load.bytes} />;
  } else {
    body = <DocumentViewer bytes={load.bytes} format={renderer.format} />;
  }

  return (
    <section className="workspace workspace--file">
      <header className="viewer__header">
        <h2 className="viewer__name" title={record.relativePath}>
          {record.relativePath}
        </h2>
        <button type="button" className="button button--compact" onClick={onClose}>
          {t("viewer.close")}
        </button>
      </header>
      {revealError === null ? null : <ErrorBanner error={revealError} />}
      <div className="viewer__content">{body}</div>
    </section>
  );
}
