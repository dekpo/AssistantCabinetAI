import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { normaliseError, errorMessage, type AppError } from "../../lib/errors";
import {
  readDataFileForViewer,
  readWorkFileForViewer,
  revealDataFile,
  revealWorkFile,
} from "../../lib/ipc";
import { FRESH_VIEW_STATE, type TabViewState } from "../../lib/viewState";
import {
  previewKind,
  previewModes,
  viewerRendererOf,
  type PreviewMode,
} from "../../lib/viewerFormats";
import type { OpenFile } from "../../lib/workspaceView";
import { ErrorBanner } from "../ErrorBanner";
import { DocumentViewer } from "./DocumentViewer";
import { MarkdownPreview } from "./MarkdownPreview";
import { PreviewModeSwitch } from "./PreviewModeSwitch";
import { ReadableTextView } from "./ReadableTextView";
import { TextView } from "./TextView";
import type { ScrollMemory } from "./useScrollMemory";

type Load =
  | { state: "loading" }
  | { state: "ready"; bytes: ArrayBuffer }
  | { state: "failed"; error: AppError };

/**
 * The tab being shown: a header with its path, and under it the renderer its extension asks for
 * (`viewerRendererOf`). Reading is display-only: nothing here writes, selects or analyses
 * anything. A file that is gone ends in `onMissing`, which closes this tab and shows the one modal;
 * any other failure stays on screen as a sentence.
 *
 * Only the tab on screen is mounted (`WorkspacePanel` keys this component by the tab), so coming
 * back to a tab reads the file again. What the user did in it comes back through `initialState`:
 * the view keeps the latest state in a ref while she works, and hands it to `onSave` when it is
 * left, but only if the view had finished restoring the previous one. A view that is torn down
 * before that point (React runs every effect twice in development) saves nothing, so the state
 * kept is never overwritten by a fresh, unscrolled one.
 */
export function FileViewer({
  tab,
  initialState,
  onSave,
  onMissing,
}: {
  tab: OpenFile;
  /** What the tab remembered, or `undefined` for a tab opened for the first time; read once. */
  initialState: TabViewState | undefined;
  /** Receives the state when the tab is left or closed. */
  onSave: (state: TabViewState) => void;
  onMissing: () => void;
}) {
  const { t } = useTranslation();
  const { folder, record } = tab;
  const renderer = useMemo(() => viewerRendererOf(record.extension), [record.extension]);
  const modes = useMemo(() => previewModes(record.extension), [record.extension]);
  const kind = useMemo(() => previewKind(record.extension), [record.extension]);
  const start = useRef<TabViewState>({
    ...(initialState ?? FRESH_VIEW_STATE),
    mode:
      initialState !== undefined && modes.includes(initialState.mode)
        ? initialState.mode
        : FRESH_VIEW_STATE.mode,
  }).current;
  /* The latest state, written as the user works and never read from the screen. */
  const live = useRef<TabViewState>(start);
  const restored = useRef(false);
  const [mode, setMode] = useState<PreviewMode>(start.mode);
  /* Where a view starts: the remembered position at first, the top once the mode was switched. */
  const [restoreRatio, setRestoreRatio] = useState(start.scrollRatio);
  const [load, setLoad] = useState<Load>({ state: "loading" });
  const [revealError, setRevealError] = useState<AppError | null>(null);
  const readable = renderer.renderer !== "unsupported";
  const onMissingRef = useRef(onMissing);
  onMissingRef.current = onMissing;
  const onSaveRef = useRef(onSave);
  onSaveRef.current = onSave;

  useEffect(
    () => () => {
      if (restored.current) {
        onSaveRef.current(live.current);
      }
    },
    [],
  );

  const report = useCallback((patch: Partial<TabViewState>) => {
    live.current = { ...live.current, ...patch };
  }, []);
  const memory = useMemo<ScrollMemory>(
    () => ({
      ratio: restoreRatio,
      onRatio: (ratio) => report({ scrollRatio: ratio }),
      onRestored: () => {
        restored.current = true;
      },
    }),
    [restoreRatio, report],
  );
  const changeMode = (next: PreviewMode) => {
    report({ mode: next, scrollRatio: 0 });
    setRestoreRatio(0);
    setMode(next);
  };

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
          onMissingRef.current();
          return;
        }
        setLoad({ state: "failed", error });
      });
    return () => {
      cancelled = true;
    };
    /* The content identifies the file: the same path with another hash is another file. */
  }, [folder, record.relativePath, record.sha256, readable]);

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
    if (mode === "preview" && kind === "markdown") {
      body = <MarkdownPreview bytes={load.bytes} memory={memory} />;
    } else if (mode === "preview" && kind === "readable") {
      body = (
        <ReadableTextView
          tab={tab}
          memory={memory}
          onMissing={onMissing}
          revealButton={revealButton}
        />
      );
    } else {
      body = <TextView bytes={load.bytes} memory={memory} />;
    }
  } else {
    body = (
      <DocumentViewer
        bytes={load.bytes}
        format={renderer.format}
        initial={start}
        memory={memory}
        onChange={report}
      />
    );
  }

  return (
    <div className="viewer__file">
      <header className="viewer__header">
        <h2 className="viewer__name" title={record.relativePath}>
          {record.relativePath}
        </h2>
      </header>
      {revealError === null ? null : <ErrorBanner error={revealError} />}
      {kind !== null && load.state === "ready" ? (
        <PreviewModeSwitch modes={modes} mode={mode} kind={kind} onChange={changeMode} />
      ) : null}
      <div className="viewer__content">{body}</div>
    </div>
  );
}
