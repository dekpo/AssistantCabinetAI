import { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import {
  canGenerateOne,
  draftBytes,
  initialDecisions,
  isDrafted,
  MAX_DRAFT_BYTES,
  mergeDecisions,
  pendingConfirmations,
  requestOf,
  type CardState,
  type Decision,
  type FillPlan,
  type FillPreview,
  type FillReport,
} from "../lib/fill";
import { fillGenerate, fillPreview, revealWorkFile } from "../lib/ipc";
import { errorMessage, normaliseError, type AppError } from "../lib/errors";

/** How long a text being typed is left alone before the preview is read again. */
const DRAFT_PAUSE_MS = 400;

/**
 * The plan for a letter (HAP-1, lot E): what a question that asked for a letter gets instead of an
 * answer. It shows the template, the table row (or every row) it will be filled from, which column
 * fills which field, and the letter as it would be written. **Nothing is written until she presses
 * the button**, and a proposed match (a field `Prix_U` for a column `Prix_Unitaire`) is never used
 * until she confirms it. No AI reads the data: the program fills the letter, on this computer.
 *
 * When no template of her documents fits, the AI drafted one from the column names alone (TABULAR
 * step 09): the card then shows that text in a box she can edit, with an action to draft it again.
 * The preview follows the text; the text is kept as a new file only when she approves.
 */
export function FillPlanCard({ plan, onDraftAgain }: { plan: FillPlan; onDraftAgain?: () => void }) {
  const { t } = useTranslation();
  const drafted = isDrafted(plan);
  const [state, setState] = useState<CardState>(() => ({
    template: plan.template,
    draftText: plan.draftText,
    all: !canGenerateOne(plan),
    keyColumn: plan.keyColumn,
    decisions: initialDecisions(plan.placeholders),
  }));
  const [preview, setPreview] = useState<FillPreview>(plan.preview);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<FillReport | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const first = useRef(true);
  const draftId = useId();

  const pending = pendingConfirmations(state.decisions);
  const fields = Object.keys(state.decisions);

  // Whenever she changes a choice, the preview is recomputed from the same inputs Rust will use to
  // write. Not on first render: the plan already carries its own preview.
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    let cancelled = false;
    setError(null);
    // A text being typed is read again once she pauses; any other choice at once.
    const timer = window.setTimeout(
      () => {
        fillPreview(requestOf(plan, state))
          .then((next) => {
            if (cancelled) {
              return;
            }
            setPreview(next);
            if (drafted) {
              // New fields start as proposed, fields that stay keep her decision, removed ones go.
              setState((current) => {
                const merged = mergeDecisions(current.decisions, next.placeholders);
                return merged === current.decisions ? current : { ...current, decisions: merged };
              });
            }
          })
          .catch((raw: unknown) => {
            if (!cancelled) {
              setError(normaliseError(raw));
            }
          });
      },
      drafted ? DRAFT_PAUSE_MS : 0,
    );
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [plan, state, drafted]);

  const decide = (name: string, decision: Decision) =>
    setState((current) => ({ ...current, decisions: { ...current.decisions, [name]: decision } }));

  const chooseTemplate = (template: string) => {
    // Another template has other fields: fetch them, with the mapping proposed for its columns.
    setReport(null);
    fillPreview(requestOf(plan, { ...state, template, decisions: {} }))
      .then((next) => {
        first.current = true;
        setState((current) => ({ ...current, template, decisions: initialDecisions(next.placeholders) }));
        setPreview(next);
      })
      .catch((raw: unknown) => setError(normaliseError(raw)));
  };

  const generate = () => {
    setBusy(true);
    setError(null);
    fillGenerate(requestOf(plan, state))
      .then(setReport)
      .catch((raw: unknown) => setError(normaliseError(raw)))
      .finally(() => setBusy(false));
  };

  const noColumn = "";
  const draftTooLarge = drafted && draftBytes(state.draftText ?? "") > MAX_DRAFT_BYTES;
  const draftBlank = drafted && (state.draftText ?? "").trim() === "";
  const canGenerate =
    pending.length === 0 &&
    preview.letters > 0 &&
    !busy &&
    report === null &&
    !draftTooLarge &&
    !draftBlank;

  return (
    <div className="fill-card">
      <p className="fill-card__title">{t("fill.title")}</p>
      <p className="message__timing">
        {plan.keyValue !== null && plan.keyColumn !== null
          ? t("fill.sourceOne", { file: plan.dataFile, column: plan.keyColumn, value: plan.keyValue })
          : t("fill.sourceAll", { file: plan.dataFile })}
      </p>

      {drafted ? (
        <div className="fill-card__draft">
          <label htmlFor={draftId}>
            <strong>{t("fill.draftTitle")}</strong>
          </label>
          <p className="message__timing">{t("fill.draftNote")}</p>
          <textarea
            id={draftId}
            className="fill-card__draft-text"
            rows={10}
            value={state.draftText ?? ""}
            readOnly={report !== null}
            onChange={(event) => {
              const text = event.target.value;
              setState((current) => ({ ...current, draftText: text }));
            }}
          />
          {draftTooLarge ? (
            <p className="fill-card__error">{t("fill.draftTooLarge", { limit: MAX_DRAFT_BYTES })}</p>
          ) : null}
          {onDraftAgain === undefined || report !== null ? null : (
            <p className="message__actions">
              <button type="button" className="button button--compact" disabled={busy} onClick={onDraftAgain}>
                {t("fill.draftAgain")}
              </button>
            </p>
          )}
        </div>
      ) : (
        <label className="fill-card__row">
          <span>{t("fill.template")}</span>
          {plan.templates.length > 1 ? (
            <select value={state.template} onChange={(event) => chooseTemplate(event.target.value)}>
              {plan.templates.map((template) => (
                <option key={template} value={template}>
                  {template}
                </option>
              ))}
            </select>
          ) : (
            <strong>{state.template}</strong>
          )}
        </label>
      )}

      <fieldset className="fill-card__fieldset">
        <legend>{t("fill.mode")}</legend>
        {canGenerateOne(plan) ? (
          <label className="fill-card__row">
            <input
              type="radio"
              name={`mode-${plan.dataFile}-${plan.keyValue}`}
              checked={!state.all}
              onChange={() => setState((current) => ({ ...current, all: false }))}
            />
            <span>{t("fill.modeOne", { value: plan.keyValue ?? "" })}</span>
          </label>
        ) : null}
        <label className="fill-card__row">
          <input
            type="radio"
            name={`mode-${plan.dataFile}-${plan.keyValue}`}
            checked={state.all}
            onChange={() => setState((current) => ({ ...current, all: true }))}
          />
          <span>{t("fill.modeAll")}</span>
          <select
            value={state.keyColumn ?? noColumn}
            disabled={!state.all}
            onChange={(event) =>
              setState((current) => ({ ...current, keyColumn: event.target.value === noColumn ? null : event.target.value }))
            }
          >
            <option value={noColumn}>{t("fill.eachRow")}</option>
            {plan.columns.map((column) => (
              <option key={column} value={column}>
                {column}
              </option>
            ))}
          </select>
        </label>
      </fieldset>

      <fieldset className="fill-card__fieldset">
        <legend>{t("fill.fields")}</legend>
        {fields.length === 0 ? <p className="message__timing">{t("fill.noFields")}</p> : null}
        {fields.map((name) => {
          const decision = state.decisions[name];
          if (decision === undefined) {
            return null;
          }
          return (
            <div className="fill-card__row" key={name}>
              <code>{name}</code>
              <span aria-hidden="true">{"→"}</span>
              <select
                value={decision.column ?? noColumn}
                onChange={(event) =>
                  decide(name, { column: event.target.value === noColumn ? null : event.target.value, confirmed: true })
                }
              >
                <option value={noColumn}>{t("fill.leaveAsWritten")}</option>
                {plan.columns.map((column) => (
                  <option key={column} value={column}>
                    {column}
                  </option>
                ))}
              </select>
              {decision.confirmed ? null : (
                <button
                  type="button"
                  className="button button--compact"
                  onClick={() => decide(name, { ...decision, confirmed: true })}
                >
                  {t("fill.confirmProposal")}
                </button>
              )}
            </div>
          );
        })}
        {pending.length > 0 ? <p className="message__timing">{t("fill.pending", { fields: pending.join(", ") })}</p> : null}
      </fieldset>

      <details className="fill-card__preview" open>
        <summary className="disclosure">{t("fill.preview")}</summary>
        <pre className="fill-card__text">{preview.text}</pre>
      </details>

      {preview.unresolved.length > 0 ? (
        <p className="message__timing">{t("fill.unresolved", { fields: preview.unresolved.join(", ") })}</p>
      ) : null}
      <p className="message__timing">
        {t("fill.summary", { letters: preview.letters, name: preview.fileName, lines: preview.lines })}
      </p>

      {error === null ? null : <p className="fill-card__error">{errorMessage(t, error)}</p>}

      {report === null ? (
        <p className="message__actions">
          <button type="button" className="button button--primary" disabled={!canGenerate} onClick={generate}>
            {t("fill.generate", { letters: preview.letters })}
          </button>
        </p>
      ) : (
        <div className="fill-card__done">
          <p>{t("fill.done", { letters: report.written.length })}</p>
          <ul className="message__sources-list">
            {report.written.map((file) => (
              <li key={file.relativePath} className="message__source">
                {file.relativePath}{" "}
                <button
                  type="button"
                  className="button button--compact"
                  onClick={() => void revealWorkFile(file.relativePath)}
                >
                  {t("fill.reveal")}
                </button>
              </li>
            ))}
          </ul>
          {report.modelFile === undefined ? null : (
            <p className="message__timing">
              {t("fill.modelSaved", { file: report.modelFile })}{" "}
              <button
                type="button"
                className="button button--compact"
                onClick={() => void revealWorkFile(report.modelFile ?? "")}
              >
                {t("fill.reveal")}
              </button>
            </p>
          )}
          {report.unresolved.length > 0 ? (
            <p className="message__timing">{t("fill.unresolved", { fields: report.unresolved.join(", ") })}</p>
          ) : null}
        </div>
      )}
    </div>
  );
}
