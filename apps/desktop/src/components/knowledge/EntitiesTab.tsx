import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { normaliseError, type AppError } from "../../lib/errors";
import {
  createEntity,
  listEntities,
  type EntityPage,
  type EntityRow,
} from "../../lib/ipc";
import {
  clampPage,
  entityTitle,
  listQueryFor,
  originLabel,
  pageCount,
  statusLabel,
  typeLabel,
  withFilter,
  type ListFilters,
  type ManageApi,
  type PendingConfirm,
} from "../../lib/knowledgeManage";
import { counted } from "../../lib/plural";
import { ErrorBanner } from "../ErrorBanner";
import { EntitySheet } from "./EntitySheet";

const FILTER_TYPES = ["person", "organization", "location", "identifier", "item", "term"];
const NEW_TYPES = ["person", "organization", "location", "item", "term"];

/** How long a pause in typing counts as "she stopped": one cheap read per pause, never per key. */
const SEARCH_DELAY_MS = 200;

/**
 * The Entities tab: the list on the left (search, filters, pages of 50), the sheet of the chosen
 * entity on the right. The filters and the page are held by the dialog, so switching tabs and back
 * leaves her where she was.
 */
export function EntitiesTab({
  api,
  filters,
  onFilters,
  page,
  onPage,
  selected,
  onSelect,
  onConfirm,
}: {
  api: ManageApi;
  filters: ListFilters;
  onFilters: (filters: ListFilters) => void;
  page: number;
  onPage: (page: number) => void;
  selected: number | null;
  onSelect: (entityId: number | null) => void;
  onConfirm: (pending: PendingConfirm) => void;
}) {
  const { t } = useTranslation();
  const [result, setResult] = useState<EntityPage | null>(null);
  const [loadError, setLoadError] = useState<AppError | null>(null);
  const [textDraft, setTextDraft] = useState(filters.text);
  const [adding, setAdding] = useState(false);

  /* The text is applied after a pause, and a change of text goes back to the first page. */
  useEffect(() => {
    if (textDraft === filters.text) {
      return;
    }
    const timer = window.setTimeout(() => {
      onFilters(withFilter(filters, { text: textDraft }));
      onPage(0);
    }, SEARCH_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [textDraft, filters, onFilters, onPage]);

  useEffect(() => {
    let current = true;
    listEntities(listQueryFor(filters, page)).then(
      (found) => {
        if (!current) {
          return;
        }
        setResult(found);
        setLoadError(null);
        const clamped = clampPage(page, found.total, found.pageSize);
        if (clamped !== page) {
          onPage(clamped);
        }
      },
      (raw: unknown) => {
        if (current) {
          setLoadError(normaliseError(raw));
        }
      },
    );
    return () => {
      current = false;
    };
  }, [filters, page, api.revision, onPage]);

  const change = (patch: Partial<ListFilters>) => {
    onFilters(withFilter(filters, patch));
    onPage(0);
  };

  const pages = result === null ? 1 : pageCount(result.total, result.pageSize);
  const noFilter =
    filters.text === "" && filters.typeId === "" && filters.origin === "" && !filters.problemsOnly;

  return (
    <div className="knowledge__panes">
      <section className="knowledge__list" aria-label={t("manage.tab.entities")}>
        <label className="field">
          <span className="field__label">{t("manage.entities.searchLabel")}</span>
          <input
            type="search"
            className="field__control"
            value={textDraft}
            placeholder={t("manage.entities.searchPlaceholder")}
            onChange={(event) => setTextDraft(event.target.value)}
          />
        </label>

        <div className="knowledge__filters">
          <label className="field">
            <span className="field__label">{t("manage.entities.typeLabel")}</span>
            <select
              className="field__control"
              value={filters.typeId}
              onChange={(event) => change({ typeId: event.target.value })}
            >
              <option value="">{t("manage.entities.typeAll")}</option>
              {FILTER_TYPES.map((type) => (
                <option key={type} value={type}>
                  {t(`manage.type.${type}`)}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span className="field__label">{t("manage.entities.originLabel")}</span>
            <select
              className="field__control"
              value={filters.origin}
              onChange={(event) =>
                change({ origin: event.target.value as ListFilters["origin"] })
              }
            >
              <option value="">{t("manage.entities.originAll")}</option>
              <option value="automatic">{t("manage.entities.originAutomatic")}</option>
              <option value="manual">{t("manage.entities.originManual")}</option>
            </select>
          </label>
          <label className="field">
            <span className="field__label">{t("manage.entities.viewLabel")}</span>
            <select
              className="field__control"
              value={filters.view}
              onChange={(event) => change({ view: event.target.value as ListFilters["view"] })}
            >
              <option value="live">{t("manage.entities.viewLive")}</option>
              <option value="deleted">{t("manage.entities.viewDeleted")}</option>
              <option value="merged">{t("manage.entities.viewMerged")}</option>
            </select>
          </label>
        </div>

        <label className="field__row">
          <input
            type="checkbox"
            checked={filters.problemsOnly}
            onChange={(event) => change({ problemsOnly: event.target.checked })}
          />
          <span>{t("manage.entities.problemsOnly")}</span>
        </label>

        <div className="knowledge__bar">
          <span className="sheet__meta">
            {result === null
              ? t("manage.loading")
              : counted(
                  result.total,
                  t("manage.entities.resultOne"),
                  t("manage.entities.resultMany"),
                )}
          </span>
          <button
            type="button"
            className="button button--compact"
            disabled={api.busy}
            onClick={() => setAdding((open) => !open)}
          >
            {t("manage.entities.newButton")}
          </button>
        </div>

        {adding ? (
          <NewEntityForm
            api={api}
            onDone={(entityId) => {
              setAdding(false);
              if (entityId !== null) {
                onSelect(entityId);
              }
            }}
          />
        ) : null}

        {loadError === null ? null : <ErrorBanner error={loadError} />}

        {result !== null && result.rows.length === 0 ? (
          <p className="sheet__meta">
            {noFilter && filters.view === "live"
              ? t("manage.entities.emptyBase")
              : t("manage.entities.empty")}
          </p>
        ) : (
          <ul className="knowledge__rows">
            {(result?.rows ?? []).map((row) => (
              <EntityRowItem
                key={row.entityId}
                row={row}
                selected={row.entityId === selected}
                onSelect={() => onSelect(row.entityId)}
              />
            ))}
          </ul>
        )}

        <div className="knowledge__bar">
          <button
            type="button"
            className="button button--compact"
            disabled={page <= 0}
            onClick={() => onPage(page - 1)}
          >
            {t("manage.entities.previous")}
          </button>
          <span className="sheet__meta">
            {t("manage.entities.pageOf", { page: page + 1, pages })}
          </span>
          <button
            type="button"
            className="button button--compact"
            disabled={page + 1 >= pages}
            onClick={() => onPage(page + 1)}
          >
            {t("manage.entities.next")}
          </button>
        </div>
      </section>

      <section className="knowledge__detail">
        {selected === null ? (
          <p className="sheet__meta">{t("manage.sheet.pick")}</p>
        ) : (
          <EntitySheet
            entityId={selected}
            api={api}
            onConfirm={onConfirm}
            onClose={() => onSelect(null)}
          />
        )}
      </section>
    </div>
  );
}

function EntityRowItem({
  row,
  selected,
  onSelect,
}: {
  row: EntityRow;
  selected: boolean;
  onSelect: () => void;
}) {
  const { t } = useTranslation();
  return (
    <li>
      <button
        type="button"
        className={selected ? "knowledge__row knowledge__row--selected" : "knowledge__row"}
        aria-pressed={selected}
        onClick={onSelect}
      >
        <span className="knowledge__row-name">{entityTitle(row)}</span>
        <span className="sheet__meta">
          {[
            typeLabel(t, row.typeId),
            counted(row.mentions, t("manage.countMentionOne"), t("manage.countMentionMany")),
            counted(row.sources, t("manage.entities.fileOne"), t("manage.entities.fileMany")),
          ].join(" · ")}
        </span>
        <span className="sheet__tags">
          {row.status === "candidate" || row.status === "merged" || row.status === "deleted" ? (
            <span className="tag">{statusLabel(t, row.status)}</span>
          ) : null}
          {row.origin === "manual" ? (
            <span className="tag">{originLabel(t, row.origin)}</span>
          ) : null}
          {row.openMatches > 0 ? (
            <span className="tag">
              {counted(
                row.openMatches,
                t("manage.entities.matchOne"),
                t("manage.entities.matchMany"),
              )}
            </span>
          ) : null}
        </span>
      </button>
    </li>
  );
}

function NewEntityForm({
  api,
  onDone,
}: {
  api: ManageApi;
  onDone: (entityId: number | null) => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [typeId, setTypeId] = useState("person");

  const create = async () => {
    const outcome = await api.run(() => createEntity({ name, typeId }));
    if (outcome !== null) {
      onDone(outcome.entityId);
    }
  };

  return (
    <form
      className="sheet__form"
      onSubmit={(event) => {
        event.preventDefault();
        void create();
      }}
    >
      <label className="field">
        <span className="field__label">{t("manage.entities.newName")}</span>
        <input
          className="field__control"
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
      </label>
      <label className="field">
        <span className="field__label">{t("manage.entities.newType")}</span>
        <select
          className="field__control"
          value={typeId}
          onChange={(event) => setTypeId(event.target.value)}
        >
          {NEW_TYPES.map((type) => (
            <option key={type} value={type}>
              {t(`manage.type.${type}`)}
            </option>
          ))}
        </select>
      </label>
      <p className="field__description">{t("manage.entities.newHint")}</p>
      <div className="sheet__actions">
        <button
          type="submit"
          className="button button--compact button--primary"
          disabled={api.busy || name.trim() === ""}
        >
          {t("manage.entities.newCreate")}
        </button>
        <button type="button" className="button button--compact" onClick={() => onDone(null)}>
          {t("actions.cancel")}
        </button>
      </div>
    </form>
  );
}
