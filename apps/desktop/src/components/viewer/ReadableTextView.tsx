import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { normaliseError, errorMessage, type AppError } from "../../lib/errors";
import { readDataFileReadableText, readWorkFileReadableText } from "../../lib/ipc";
import type { OpenFile } from "../../lib/workspaceView";
import { TextView } from "./TextView";
import type { ScrollMemory } from "./useScrollMemory";

type Load =
  | { state: "loading" }
  | { state: "ready"; bytes: ArrayBuffer }
  | { state: "failed"; error: AppError };

/**
 * A mail or a subtitle file as the assistant reads it: the text Rust's readers produce for the
 * index (a mail's header fields and decoded body, one line per cue), shown as plain text by the
 * same `TextView` as the raw mode, so the bound, the line breaks and the remembered position are
 * shared. It is read only when this mode is chosen (a single file, on the user's click) and kept
 * while the view stays mounted, so going back and forth reads nothing again. Nothing is written.
 */
export function ReadableTextView({
  tab,
  memory,
  onMissing,
  revealButton,
}: {
  tab: OpenFile;
  memory: ScrollMemory;
  /** The file is gone: the caller closes the tab and shows the one modal. */
  onMissing: () => void;
  /** Offered beside a failure, as the raw read does. */
  revealButton: ReactNode;
}) {
  const { t } = useTranslation();
  const { folder, record } = tab;
  const [load, setLoad] = useState<Load>({ state: "loading" });
  const onMissingRef = useRef(onMissing);
  onMissingRef.current = onMissing;
  const onRestoredRef = useRef(memory.onRestored);
  onRestoredRef.current = memory.onRestored;
  const empty = load.state === "ready" && load.bytes.byteLength === 0;

  useEffect(() => {
    let cancelled = false;
    setLoad({ state: "loading" });
    const read = folder === "documents" ? readWorkFileReadableText : readDataFileReadableText;
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
          onMissingRef.current();
          return;
        }
        setLoad({ state: "failed", error });
      });
    return () => {
      cancelled = true;
    };
    /* The content identifies the file: the same path with another hash is another file. */
  }, [folder, record.relativePath, record.sha256]);

  /* An empty reading shows no text view, which is what tells the tab its view is ready: without
     this the mode would never be remembered. */
  useEffect(() => {
    if (empty) {
      onRestoredRef.current();
    }
  }, [empty]);

  if (load.state === "loading") {
    return (
      <p className="viewer__message" role="status">
        {t("viewer.loading")}
      </p>
    );
  }
  if (load.state === "failed") {
    return (
      <div className="viewer__message viewer__message--block" role="alert">
        <p>{errorMessage(t, load.error)}</p>
        {revealButton}
      </div>
    );
  }
  if (empty) {
    return (
      <p className="viewer__message" role="status">
        {t("viewer.readableEmpty")}
      </p>
    );
  }
  return <TextView bytes={load.bytes} memory={memory} />;
}
