import { useEffect, useRef } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { selectionOf, tickedPaths, toggleAll, toggleScopeFile } from "../lib/analysisScope";
import type { AnalysisScope, FileRecord } from "../lib/ipc";
import type { IndexingState } from "../state/useIndexing";

/*
 * The pieces both folder cards are built from: the Documents Folder card and the Data Folder card
 * are one markup (`docs/DECISIONS.md`, "the folder card in two places, one markup"), so what they
 * share lives here once rather than drifting apart in two copies.
 */

/**
 * The one control that starts a pass. Filled with the accent colour only while a pass would still
 * change something - once every file has been read it is an ordinary button beside "change the
 * folder", because by then it is a thing she may do, not the thing she must do first.
 */
export function AnalyseButton({
  indexing,
  emphasised,
  pendingCount,
}: {
  indexing: IndexingState;
  emphasised: boolean;
  /** How many files the pass would still change. Shown only while it is worth acting on. */
  pendingCount: number;
}) {
  const { t } = useTranslation();

  return (
    <button
      type="button"
      className={
        emphasised
          ? "button button--compact button--primary work-folder__analyse"
          : "button button--compact work-folder__analyse"
      }
      disabled={indexing.running}
      onClick={() => void indexing.run()}
    >
      {indexing.running ? (
        <span className="message__pending">
          <span className="spinner" aria-hidden="true" />
          {t("actions.analyzing")}
        </span>
      ) : pendingCount > 0 ? (
        /* "Analyser (3)". The count rides inside the label rather than in a separate badge: the
           row already carries three controls, and a badge would be a fourth thing competing for
           the same few pixels. */
        `${t("actions.analyze")} (${pendingCount})`
      ) : (
        t("actions.analyze")
      )}
    </button>
  );
}

/** A hairline that fills as the pass walks the folder. Gone the moment the pass ends. */
export function AnalysisProgress({ fraction, label }: { fraction: number; label: string }) {
  return (
    <div
      className="progress"
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(fraction * 100)}
    >
      <div className="progress__fill" style={{ width: `${fraction * 100}%` }} />
    </div>
  );
}

/** Only a file whose analysis is current and usable can be selected: a document whose passages
 * match its content, or a workbook holding a real table (green). A red or orange file is listed,
 * with its state visible, and offered no checkbox. */
export function isSelectable(file: FileRecord): boolean {
  return file.processingStatus === "indexed";
}

/**
 * The box above the listing: every file, or none. Checked for "tous", partly filled while some
 * files are ticked, empty for "aucun" - the same three states the line above it names.
 */
export function SelectAll({
  scope,
  onScopeChange,
  disabled,
  label,
}: {
  scope: AnalysisScope;
  onScopeChange: (scope: AnalysisScope) => void;
  disabled: boolean;
  label: string;
}) {
  const box = useRef<HTMLInputElement>(null);
  const selection = selectionOf(scope);

  /* "Partly filled" has no attribute, only a DOM property, so it is set after each render. */
  useEffect(() => {
    if (box.current !== null) {
      box.current.indeterminate = selection === "some";
    }
  }, [selection]);

  return (
    <label className="inventory__choice scope-picker__all">
      <input
        ref={box}
        type="checkbox"
        disabled={disabled}
        checked={selection === "all"}
        onChange={() => onScopeChange(toggleAll(scope, Date.now()))}
      />
      <span>{label}</span>
    </label>
  );
}

type Tone = "ok" | "wait" | "fail";

/** What a file's state looks like at a glance: usable, waiting for an analysis, or unreadable.
 * The words stay on hover and for screen readers - a colour alone is not an answer. */
const TONE_OF_STATUS: Record<FileRecord["processingStatus"], Tone> = {
  indexed: "ok",
  discovered: "wait",
  pending: "wait",
  processing: "wait",
  failed: "fail",
};

export function StatusDot({ tone, label }: { tone: Tone; label: string }) {
  return <span className={`status-dot status-dot--${tone}`} role="img" aria-label={label} title={label} />;
}

/** One file per row: a dot for its state, then its name, so the listing is one short line per
 * file in the sidebar and in the settings panel alike. The state's words are on the dot's hover:
 * `statusLabel` says them, in the words that folder's answers use. */
export function FileList({
  files,
  statusLabel,
  scope,
  onScopeChange,
  scopeLocked = false,
  onReveal,
}: {
  files: FileRecord[];
  statusLabel: (file: FileRecord) => string;
  /** When given, each row ends with a "See" button that shows the file in the file manager. */
  onReveal?: (relativePath: string) => void;
  /** When given, each usable file gets a checkbox: the listing doubles as the choice of the files
   * the conversation may rely on. Choosing one copies nothing. */
  scope?: AnalysisScope;
  onScopeChange?: (scope: AnalysisScope) => void;
  scopeLocked?: boolean;
}) {
  const { t } = useTranslation();
  const analysed = files.filter(isSelectable);
  const ticked = scope === undefined ? [] : tickedPaths(scope, analysed);
  const selectable = scope !== undefined && onScopeChange !== undefined;

  return (
    <ul className="inventory">
      {files.map((file) => {
        const dot = <StatusDot tone={TONE_OF_STATUS[file.processingStatus]} label={statusLabel(file)} />;
        /* One line, whatever the path costs: a name broken across two lines is harder to scan
           than one that ends in an ellipsis, and the full path is on hover for the rare name long
           enough to need it. */
        const name = (
          <span className="inventory__path" title={file.relativePath}>
            {file.relativePath}
          </span>
        );
        return (
          <li key={file.id + file.relativePath} className="inventory__file">
            {selectable ? (
              <label className="inventory__choice">
                <input
                  type="checkbox"
                  disabled={scopeLocked || !isSelectable(file)}
                  checked={ticked.includes(file.relativePath)}
                  onChange={() =>
                    onScopeChange(toggleScopeFile(scope, file, analysed, Date.now()))
                  }
                />
                {dot}
                {name}
              </label>
            ) : (
              <>
                {dot}
                {name}
              </>
            )}
            {onReveal === undefined ? null : (
              <button
                type="button"
                className="button button--compact inventory__see"
                title={t("workFolder.revealFileHint")}
                onClick={() => onReveal(file.relativePath)}
              >
                {t("workFolder.reveal")}
              </button>
            )}
          </li>
        );
      })}
    </ul>
  );
}
