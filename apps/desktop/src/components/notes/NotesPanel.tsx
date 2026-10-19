import { useState } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { normaliseError, type AppError } from "../../lib/errors";
import {
  activeDraft,
  draftTitle,
  hasContent,
  stampName,
  unsavedDrafts,
  type DraftPatch,
} from "../../lib/notesDrafts";
import type { NotesSnapshot, SaveScope } from "../../lib/notesSession";
import { pluralise } from "../../lib/plural";
import type { PreviewMode } from "../../lib/viewerFormats";
import type { NoteFormat } from "../../speech/ipc";
import { ErrorBanner } from "../ErrorBanner";
import { Markdown } from "../Markdown";
import { PreviewModeSwitch } from "../viewer/PreviewModeSwitch";
import { RepairChips } from "../RepairChips";
import { DictationStatus, LivePill, type NotesDictation } from "./LiveDictation";

/** What the Notes section can ask of `App`. Each call is the user's own click. */
export interface NotesActions {
  select: (id: string) => void;
  add: () => void;
  edit: (id: string, patch: DraftPatch) => void;
  /** Throws one note away; `App` asks first when it has text. */
  discard: (id: string) => void;
  /** Builds the plan (the exact files and names). Writes nothing. */
  review: (scope: SaveScope) => void;
  /** The click on the plan: writes the files, then analyses. */
  confirm: () => void;
  cancelPlan: () => void;
  dismissReport: () => void;
  /** Shows a saved note in the file manager. */
  reveal(relativePath: string): Promise<void>;
}

/** Everything `App` hands to the Notes section, in one bundle. */
export interface NotesBundle {
  snapshot: NotesSnapshot;
  actions: NotesActions;
  /** A documents folder is chosen: without it nothing can be written. */
  hasFolder: boolean;
  /** `null` while the voice is off: the pill is then not drawn at all. */
  dictation: NotesDictation | null;
}

const MODES: readonly PreviewMode[] = ["raw", "preview"];
const FORMATS: ReadonlyArray<{ value: NoteFormat; label: string }> = [
  { value: "md", label: "notes.formatMarkdown" },
  { value: "txt", label: "notes.formatText" },
];

/**
 * The Notes section of the centre zone, laid out like a plain writing page: the note fills the pane
 * and is editable, Raw or rendered as Markdown; once there is text, a save block sits under it.
 * Saving is two steps: the plan lists the exact files with their names, and only the click on the
 * plan writes them. The drafts live in `App` (memory only), so this component can unmount freely.
 */
export function NotesPanel({ bundle }: { bundle: NotesBundle }) {
  const { t } = useTranslation();
  const { snapshot, actions, hasFolder, dictation } = bundle;
  const [mode, setMode] = useState<PreviewMode>("raw");
  const [revealError, setRevealError] = useState<AppError | null>(null);

  const active = activeDraft(snapshot.notes);
  const savable = unsavedDrafts(snapshot.notes).length;
  const busy = snapshot.phase !== "idle";
  /* A dictation is writing into the note: it cannot be saved or thrown away under it. */
  const dictating = dictation !== null && dictation.phase !== "idle";
  const canRender = active.format === "md";
  const shownMode: PreviewMode = canRender ? mode : "raw";
  const label = (number: number, name: string | null) =>
    name ?? t("notes.draftLabel", { number });

  const reveal = async (relativePath: string) => {
    setRevealError(null);
    try {
      await actions.reveal(relativePath);
    } catch (raw: unknown) {
      setRevealError(normaliseError(raw));
    }
  };

  const report = snapshot.report;
  const error = snapshot.error ?? revealError;

  return (
    <section className="notes" aria-label={t("notes.title")}>
      <div className="notes__bar">
        <div className="notes__drafts" role="group" aria-label={t("notes.draftsLabel")}>
          {snapshot.notes.drafts.map((draft) => (
            <button
              key={draft.id}
              type="button"
              className="button button--compact notes__draft"
              aria-pressed={draft.id === active.id}
              onClick={() => actions.select(draft.id)}
            >
              {label(draft.number, draftTitle(draft))}
            </button>
          ))}
        </div>
        <button
          type="button"
          className="button button--compact"
          disabled={busy || dictating}
          onClick={actions.add}
        >
          {t("notes.new")}
        </button>
        <button
          type="button"
          className="button button--compact"
          disabled={busy || dictating}
          onClick={() => actions.discard(active.id)}
        >
          {t("notes.discard")}
        </button>
      </div>

      {canRender ? (
        <PreviewModeSwitch modes={MODES} mode={mode} kind="markdown" onChange={setMode} />
      ) : null}

      <div className="notes__page">
        {shownMode === "raw" ? (
          <textarea
            className="notes__text"
            value={active.text}
            readOnly={busy}
            placeholder={t("notes.placeholder")}
            aria-label={t("notes.textLabel")}
            onChange={(event) => actions.edit(active.id, { text: event.target.value })}
          />
        ) : (
          <div className="notes__preview message__body--markdown">
            {hasContent(active) ? (
              <Markdown text={active.text} />
            ) : (
              <p className="notes__empty">{t("notes.previewEmpty")}</p>
            )}
          </div>
        )}
        {dictation === null ? null : <LivePill dictation={dictation} />}
      </div>

      {dictation === null ? null : (
        <>
          <DictationStatus dictation={dictation} />
          {dictation.repair === undefined ? null : <RepairChips repair={dictation.repair} />}
        </>
      )}

      {hasContent(active) ? (
        <div className="notes__save">
          <div className="notes__fields">
            <label className="field notes__name">
              <span className="field__label">{t("notes.nameLabel")}</span>
              <input
                className="field__control"
                type="text"
                value={active.name}
                disabled={busy}
                placeholder={stampName(Date.now())}
                onChange={(event) => actions.edit(active.id, { name: event.target.value })}
              />
            </label>
            <label className="field">
              <span className="field__label">{t("notes.formatLabel")}</span>
              <select
                className="field__control"
                value={active.format}
                disabled={busy}
                onChange={(event) =>
                  actions.edit(active.id, { format: event.target.value as NoteFormat })
                }
              >
                {FORMATS.map((format) => (
                  <option key={format.value} value={format.value}>
                    {t(format.label)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <div className="notes__buttons">
            <button
              type="button"
              className="button button--compact button--primary"
              disabled={busy || dictating || !hasFolder}
              onClick={() => actions.review("active")}
            >
              {t("notes.save")}
            </button>
            {savable > 1 ? (
              <button
                type="button"
                className="button button--compact"
                disabled={busy || dictating || !hasFolder}
                onClick={() => actions.review("all")}
              >
                {t("notes.saveAll", { count: savable })}
              </button>
            ) : null}
          </div>
          {hasFolder ? null : (
            <p className="notes__hint">{t("errors.notes_folder_unavailable")}</p>
          )}
        </div>
      ) : null}

      {snapshot.phase === "plan" && snapshot.plan !== null ? (
        <div className="notes__plan" role="group" aria-label={t("notes.plan.title")}>
          <p className="notes__plan-title">{t("notes.plan.title")}</p>
          <ul className="notes__files">
            {snapshot.plan.files.map((file) => (
              <li key={file.relativePath} className="notes__file">
                {file.relativePath}
              </li>
            ))}
          </ul>
          <p className="notes__hint">{t("notes.plan.hint")}</p>
          <div className="notes__buttons">
            <button type="button" className="button button--compact" onClick={actions.cancelPlan}>
              {t("actions.cancel")}
            </button>
            <button
              type="button"
              className="button button--compact button--primary"
              onClick={actions.confirm}
            >
              {pluralise(
                snapshot.plan.files.length,
                t("notes.plan.writeOne"),
                t("notes.plan.writeMany", { count: snapshot.plan.files.length }),
              )}
            </button>
          </div>
        </div>
      ) : null}

      {snapshot.phase === "previewing" ||
      snapshot.phase === "saving" ||
      snapshot.phase === "asking" ||
      snapshot.phase === "analysing" ? (
        <p className="notes__status" role="status">
          <span className="spinner" aria-hidden="true" />
          {t(`notes.phase.${snapshot.phase}`)}
        </p>
      ) : null}

      {error === null ? null : (
        <ErrorBanner
          error={error}
          onDismiss={() => (snapshot.error === null ? setRevealError(null) : actions.dismissReport())}
        />
      )}

      {report === null ? null : (
        <div className="notes__report">
          <p className="notes__plan-title">
            {pluralise(
              report.written.length,
              t("notes.report.savedOne"),
              t("notes.report.savedMany", { count: report.written.length }),
            )}
          </p>
          <ul className="notes__files">
            {report.written.map((file) => (
              <li key={file.relativePath} className="notes__file">
                <span className="notes__path">{file.relativePath}</span>
                <button
                  type="button"
                  className="button button--compact button--quiet"
                  onClick={() => void reveal(file.relativePath)}
                >
                  {t("notes.report.reveal")}
                </button>
              </li>
            ))}
          </ul>
          {report.analysis === "failed" ? (
            <ErrorBanner error={{ code: "notes_analysis_failed", data: {} }} />
          ) : report.analysis === "pending" ? null : (
            <p className="notes__hint">{t(`notes.report.analysis.${report.analysis}`)}</p>
          )}
          {busy ? null : (
            <button
              type="button"
              className="button button--compact"
              onClick={actions.dismissReport}
            >
              {t("actions.close")}
            </button>
          )}
        </div>
      )}
    </section>
  );
}
