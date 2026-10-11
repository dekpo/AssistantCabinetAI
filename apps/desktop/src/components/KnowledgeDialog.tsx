import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { normaliseError, type AppError } from "../lib/errors";
import {
  knowledgeOverview,
  undoLastOperation,
  type KnowledgeMode,
  type KnowledgeOverview,
} from "../lib/ipc";
import {
  confirmationFor,
  EMPTY_FILTERS,
  isBusy,
  pendingNote,
  summaryLine,
  undoState,
  type ListFilters,
  type ManageApi,
  type PendingConfirm,
} from "../lib/knowledgeManage";
import { ConfirmDialog } from "./ConfirmDialog";
import { ErrorBanner } from "./ErrorBanner";
import { EntitiesTab } from "./knowledge/EntitiesTab";
import { ResetTab } from "./knowledge/ResetTab";
import { ReviewTab } from "./knowledge/ReviewTab";
import { TypesTab } from "./knowledge/TypesTab";

type Tab = "entities" | "review" | "types" | "reset";
const TABS: Tab[] = ["entities", "review", "types", "reset"];

/**
 * The place where the knowledge base can be seen, corrected and extended: an administrative dialog
 * opened from the settings or from the analysis summary, never from the conversation. It reads and
 * writes names, aliases and links - never a file of either folder - and it starts no work: a change
 * only says how many documents the next Analyse will read again, from the text already stored.
 *
 * Every write goes through `run`: one place that shows the busy state, reports a failure (a busy
 * database gets a retry, since an Analyse writing can make one wait), and reads everything again on
 * success. Anything that removes, redirects or moves something waits for the confirmation dialog.
 */
export function KnowledgeDialog({
  knowledgeMode,
  onClose,
}: {
  knowledgeMode: KnowledgeMode;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>("entities");
  const [overview, setOverview] = useState<KnowledgeOverview | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [retry, setRetry] = useState<(() => void) | null>(null);
  const [revision, setRevision] = useState(0);
  const [busy, setBusy] = useState(false);
  const [filters, setFilters] = useState<ListFilters>(EMPTY_FILTERS);
  const [page, setPage] = useState(0);
  const [selected, setSelected] = useState<number | null>(null);
  const [confirming, setConfirming] = useState<PendingConfirm | null>(null);

  const readOverview = useCallback(() => {
    knowledgeOverview().then(
      (found) => setOverview(found),
      (raw: unknown) => setError(normaliseError(raw)),
    );
  }, []);

  useEffect(readOverview, [readOverview, revision]);

  const run = useCallback<ManageApi["run"]>(
    async (operation) => {
      setBusy(true);
      setError(null);
      setRetry(null);
      try {
        const outcome = await operation();
        setRevision((value) => value + 1);
        return outcome;
      } catch (raw: unknown) {
        const failure = normaliseError(raw);
        setError(failure);
        if (isBusy(failure.code)) {
          setRetry(() => () => void run(operation));
        }
        return null;
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const openEntity = useCallback((entityId: number) => {
    setTab("entities");
    setSelected(entityId);
  }, []);

  const api: ManageApi = useMemo(
    () => ({ run, revision, busy, openEntity }),
    [run, revision, busy, openEntity],
  );

  const undo = () => void run(undoLastOperation);

  const answerConfirmation = async () => {
    const pending = confirming;
    setConfirming(null);
    if (pending !== null) {
      await pending.apply();
    }
  };

  const undoing = undoState(t, overview?.lastOperation ?? null);
  const pending = overview === null ? null : pendingNote(t, overview.documentsDue);

  return (
    <div className="dialog dialog--wide" role="dialog" aria-label={t("manage.title")}>
      <header className="dialog__header">
        <h2 className="dialog__title">{t("manage.title")}</h2>
        <button type="button" className="button button--quiet" onClick={onClose}>
          {t("actions.close")}
        </button>
      </header>

      <div className="dialog__body knowledge">
        <p className="field__description">{t("manage.intro")}</p>

        {knowledgeMode === "off" ? (
          <p className="knowledge__note">{t("manage.offNote")}</p>
        ) : null}
        {overview !== null && !overview.consistent ? (
          <p className="knowledge__note">{t("manage.inconsistent")}</p>
        ) : null}

        <div className="knowledge__status">
          <p className="sheet__meta">
            {overview === null ? t("manage.loading") : summaryLine(t, overview)}
          </p>
          {pending === null ? null : <p className="sheet__meta">{pending}</p>}
          <button
            type="button"
            className="button button--compact"
            disabled={busy || !undoing.enabled}
            onClick={undo}
          >
            {undoing.label}
          </button>
        </div>

        {error === null ? null : (
          <ErrorBanner
            error={error}
            onRetry={retry ?? undefined}
            onDismiss={() => {
              setError(null);
              setRetry(null);
            }}
          />
        )}

        <div className="knowledge__tabs" role="tablist">
          {TABS.map((name) => (
            <button
              key={name}
              type="button"
              role="tab"
              aria-selected={tab === name}
              className={tab === name ? "knowledge__tab knowledge__tab--on" : "knowledge__tab"}
              onClick={() => setTab(name)}
            >
              {t(`manage.tab.${name}`)}
            </button>
          ))}
        </div>

        <div role="tabpanel">
          {tab === "entities" ? (
            <EntitiesTab
              api={api}
              filters={filters}
              onFilters={setFilters}
              page={page}
              onPage={setPage}
              selected={selected}
              onSelect={setSelected}
              onConfirm={setConfirming}
            />
          ) : null}
          {tab === "review" ? (
            <ReviewTab
              api={api}
              onConfirm={setConfirming}
              onShowProblems={() => {
                setFilters({ ...EMPTY_FILTERS, problemsOnly: true });
                setPage(0);
                setTab("entities");
              }}
            />
          ) : null}
          {tab === "types" ? <TypesTab api={api} /> : null}
          {tab === "reset" ? <ResetTab api={api} onConfirm={setConfirming} /> : null}
        </div>
      </div>

      {confirming === null ? null : (
        <ConfirmDialog
          title={t(`${confirmationFor(confirming.action).prefix}.title`, confirming.values)}
          body={t(`${confirmationFor(confirming.action).prefix}.body`, confirming.values)}
          confirmLabel={t(`${confirmationFor(confirming.action).prefix}.confirm`, confirming.values)}
          destructive={confirmationFor(confirming.action).destructive}
          busy={busy}
          onConfirm={() => void answerConfirmation()}
          onCancel={() => setConfirming(null)}
        />
      )}
    </div>
  );
}
