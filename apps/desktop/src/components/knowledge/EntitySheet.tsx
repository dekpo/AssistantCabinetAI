import { useEffect, useRef, useState } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { normaliseError, type AppError } from "../../lib/errors";
import {
  addAlias,
  deleteEntity,
  editEntity,
  entitySheet,
  listEntities,
  mergeEntities,
  removeAlias,
  removeAttribute,
  restoreEntity,
  setAttribute,
  splitEntity,
  unmergeEntity,
  type AliasView,
  type AttributeView,
  type EntityDetail,
  type EntityRow,
} from "../../lib/ipc";
import {
  canMergeWith,
  canSplit,
  defaultSurvivor,
  entityTitle,
  isShortName,
  statusLabel,
  suggestedSplitName,
  toggleId,
  originLabel,
  type ManageApi,
  type PendingConfirm,
} from "../../lib/knowledgeManage";
import { ErrorBanner } from "../ErrorBanner";
import {
  AliasList,
  AttributeList,
  ColumnList,
  PossibleList,
  ProvenanceList,
  RelationList,
  SheetCounts,
} from "./EntitySheetParts";

/** The types a person may give an entity by hand. An identifier is found by its shape. */
const EDITABLE_TYPES = ["person", "organization", "location", "item", "term"];

type Mode = "view" | "edit" | "merge" | "split";

/**
 * One entity: its name, its names, and above all **where it comes from** - every file and place the
 * base found it in, with the method and the confidence. The actions are the management operations;
 * the ones that remove, redirect or move something wait for a confirmation. Everything is read
 * again from Rust after each write, so the sheet never shows what a write has changed.
 */
export function EntitySheet({
  entityId,
  api,
  onConfirm,
  onClose,
}: {
  entityId: number;
  api: ManageApi;
  onConfirm: (pending: PendingConfirm) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [detail, setDetail] = useState<EntityDetail | null>(null);
  const [loadError, setLoadError] = useState<AppError | null>(null);
  const [mode, setMode] = useState<Mode>("view");
  /* The mentions ticked for a split. They belong to the sheet, not to the panel: the tick boxes are
     in the provenance list and the button that uses them is in the panel. */
  const [picked, setPicked] = useState<number[]>([]);

  useEffect(() => {
    setMode("view");
    setPicked([]);
  }, [entityId]);
  useEffect(() => setPicked([]), [mode]);

  useEffect(() => {
    let current = true;
    entitySheet(entityId).then(
      (found) => {
        if (current) {
          setDetail(found);
          setLoadError(null);
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
  }, [entityId, api.revision]);

  if (loadError !== null) {
    return (
      <div className="sheet">
        <ErrorBanner error={loadError} />
      </div>
    );
  }
  if (detail === null || detail.entity.entityId !== entityId) {
    return (
      <div className="sheet">
        <p className="sheet__meta">{t("manage.loading")}</p>
      </div>
    );
  }

  const { entity } = detail;
  const live = entity.status === "candidate" || entity.status === "active";
  const survivor = entity.status === "merged" ? detail.survivor : null;

  return (
    <div className="sheet">
      <header className="sheet__header">
        <div>
          <h3 className="sheet__title">{entityTitle(entity)}</h3>
          <SheetCounts detail={detail} />
          <p className="sheet__tags">
            <span className="tag">{statusLabel(t, entity.status)}</span>
            <span className="tag">{originLabel(t, entity.origin)}</span>
          </p>
        </div>
        <button type="button" className="button button--quiet button--compact" onClick={onClose}>
          {t("manage.sheet.close")}
        </button>
      </header>

      {survivor !== null ? (
        <div className="sheet__note">
          <p>{t("manage.sheet.mergedInto", { name: survivor.name })}</p>
          <div className="sheet__actions">
            <button
              type="button"
              className="button button--compact"
              disabled={api.busy}
              onClick={() => api.openEntity(survivor.entityId)}
            >
              {t("manage.sheet.openSurvivor", { name: survivor.name })}
            </button>
            <button
              type="button"
              className="button button--compact"
              disabled={api.busy}
              onClick={() => void api.run(() => unmergeEntity(entity.entityId))}
            >
              {t("manage.sheet.unmerge")}
            </button>
          </div>
        </div>
      ) : null}

      {entity.status === "deleted" ? (
        <div className="sheet__note">
          <p>{t("manage.sheet.deleted")}</p>
          <div className="sheet__actions">
            <button
              type="button"
              className="button button--compact button--primary"
              disabled={api.busy}
              onClick={() => void api.run(() => restoreEntity(entity.entityId))}
            >
              {t("manage.sheet.restore")}
            </button>
          </div>
        </div>
      ) : null}

      {live ? (
        <div className="sheet__actions">
          <button
            type="button"
            className="button button--compact"
            disabled={api.busy}
            onClick={() => setMode(mode === "edit" ? "view" : "edit")}
          >
            {t("manage.sheet.edit")}
          </button>
          <button
            type="button"
            className="button button--compact"
            disabled={api.busy}
            onClick={() => setMode(mode === "merge" ? "view" : "merge")}
          >
            {t("manage.sheet.merge")}
          </button>
          <button
            type="button"
            className="button button--compact"
            disabled={api.busy}
            onClick={() => setMode(mode === "split" ? "view" : "split")}
          >
            {t("manage.sheet.split")}
          </button>
          <button
            type="button"
            className="button button--compact button--destructive"
            disabled={api.busy}
            onClick={() =>
              onConfirm({
                action: "delete",
                values: { name: entityTitle(entity) },
                apply: async () => {
                  await api.run(() => deleteEntity(entity.entityId));
                },
              })
            }
          >
            {t("manage.sheet.delete")}
          </button>
        </div>
      ) : null}

      {live && mode === "edit" ? (
        <EditForm
          entity={entity}
          api={api}
          onDone={() => setMode("view")}
        />
      ) : null}
      {live && mode === "merge" ? (
        <MergePanel entity={entity} api={api} onConfirm={onConfirm} />
      ) : null}
      {live && mode === "split" ? (
        <SplitPanel
          detail={detail}
          picked={picked}
          api={api}
          onConfirm={onConfirm}
          onSplit={() => setPicked([])}
        />
      ) : null}

      <section className="sheet__section">
        <h4 className="sheet__heading">{t("manage.sheet.names")}</h4>
        <AliasList
          aliases={detail.aliases}
          ownerId={entity.entityId}
          editable={live}
          onRemove={(alias: AliasView) =>
            onConfirm({
              action: "removeAlias",
              values: { name: alias.display },
              apply: async () => {
                await api.run(() => removeAlias(alias.aliasId));
              },
            })
          }
        />
        {live ? <AddName entityId={entity.entityId} api={api} /> : null}
        {detail.mergedIn.length === 0 ? null : (
          <>
            <h4 className="sheet__heading">{t("manage.sheet.mergedIn")}</h4>
            <ul className="sheet__aliases">
              {detail.mergedIn.map((other) => (
                <li key={other.entityId} className="sheet__alias">
                  <button
                    type="button"
                    className="button button--quiet button--compact"
                    onClick={() => api.openEntity(other.entityId)}
                  >
                    {other.name}
                  </button>
                </li>
              ))}
            </ul>
          </>
        )}
      </section>

      <ProvenanceList
        detail={detail}
        picking={live && mode === "split"}
        picked={picked}
        onToggle={(mentionId: number) => setPicked((known) => toggleId(known, mentionId))}
      />

      <RelationList relations={detail.relations} />

      <section className="sheet__section">
        <h4 className="sheet__heading">{t("manage.sheet.attributes")}</h4>
        <AttributeList
          attributes={detail.attributes}
          editable={live}
          onRemove={(attribute: AttributeView) =>
            onConfirm({
              action: "removeAttribute",
              values: { name: attribute.value },
              apply: async () => {
                await api.run(() =>
                  removeAttribute(entity.entityId, attribute.key, attribute.value),
                );
              },
            })
          }
        />
        {live ? <AddPronunciation entityId={entity.entityId} api={api} /> : null}
      </section>

      <PossibleList matches={detail.possibleMatches} onOpen={api.openEntity} />
      <ColumnList workbooks={detail.columns} />
    </div>
  );
}

// ------------------------------------------------------------------------------------ edit

function EditForm({
  entity,
  api,
  onDone,
}: {
  entity: EntityRow;
  api: ManageApi;
  onDone: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(entity.name);
  const [typeId, setTypeId] = useState(entity.typeId);
  const [disambiguator, setDisambiguator] = useState(entity.disambiguator);
  const isIdentifier = entity.typeId === "identifier";

  const save = async () => {
    const outcome = await api.run(() =>
      editEntity(entity.entityId, {
        name: isIdentifier ? null : name,
        typeId: isIdentifier ? null : typeId,
        disambiguator,
      }),
    );
    if (outcome !== null) {
      onDone();
    }
  };

  return (
    <form
      className="sheet__form"
      onSubmit={(event) => {
        event.preventDefault();
        void save();
      }}
    >
      {isIdentifier ? null : (
        <>
          <label className="field">
            <span className="field__label">{t("manage.sheet.editName")}</span>
            <input
              className="field__control"
              value={name}
              onChange={(event) => setName(event.target.value)}
            />
          </label>
          <label className="field">
            <span className="field__label">{t("manage.sheet.editType")}</span>
            <select
              className="field__control"
              value={typeId}
              onChange={(event) => setTypeId(event.target.value)}
            >
              {EDITABLE_TYPES.map((type) => (
                <option key={type} value={type}>
                  {t(`manage.type.${type}`)}
                </option>
              ))}
            </select>
          </label>
        </>
      )}
      <label className="field">
        <span className="field__label">{t("manage.sheet.editDisambiguator")}</span>
        <input
          className="field__control"
          value={disambiguator}
          onChange={(event) => setDisambiguator(event.target.value)}
        />
      </label>
      <p className="field__description">{t("manage.sheet.editNote")}</p>
      <div className="sheet__actions">
        <button type="submit" className="button button--compact button--primary" disabled={api.busy}>
          {t("manage.sheet.editSave")}
        </button>
        <button type="button" className="button button--compact" onClick={onDone}>
          {t("actions.cancel")}
        </button>
      </div>
    </form>
  );
}

// -------------------------------------------------------------------------------- add a name

function AddName({ entityId, api }: { entityId: number; api: ManageApi }) {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [shared, setShared] = useState(0);
  const input = useRef<HTMLInputElement>(null);

  const add = async () => {
    const outcome = await api.run(() => addAlias(entityId, text));
    if (outcome !== null) {
      setShared(outcome.sharedWith);
      setText("");
    }
  };

  return (
    <form
      className="sheet__form"
      onSubmit={(event) => {
        event.preventDefault();
        void add();
      }}
    >
      <label className="field">
        <span className="field__label">{t("manage.sheet.nameAddLabel")}</span>
        <span className="field__row">
          <input
            ref={input}
            className="field__control"
            value={text}
            onChange={(event) => {
              setText(event.target.value);
              setShared(0);
            }}
          />
          <button
            type="submit"
            className="button button--compact"
            disabled={api.busy || text.trim() === ""}
          >
            {t("manage.sheet.nameAddButton")}
          </button>
        </span>
      </label>
      <p className="field__description">{t("manage.sheet.nameHint")}</p>
      {text.trim() !== "" && isShortName(text) ? (
        <p className="field__description">{t("manage.sheet.nameShort")}</p>
      ) : null}
      {shared === 0 ? null : (
        <p className="field__description">
          {shared === 1
            ? t("manage.sheet.nameSharedOne")
            : t("manage.sheet.nameSharedMany", { count: shared })}
        </p>
      )}
    </form>
  );
}

function AddPronunciation({ entityId, api }: { entityId: number; api: ManageApi }) {
  const { t } = useTranslation();
  const [text, setText] = useState("");

  const save = async () => {
    const outcome = await api.run(() => setAttribute(entityId, "spoken_form", text));
    if (outcome !== null) {
      setText("");
    }
  };

  return (
    <form
      className="sheet__form"
      onSubmit={(event) => {
        event.preventDefault();
        void save();
      }}
    >
      <label className="field">
        <span className="field__label">{t("manage.sheet.attributeAdd")}</span>
        <span className="field__row">
          <input
            className="field__control"
            value={text}
            onChange={(event) => setText(event.target.value)}
          />
          <button
            type="submit"
            className="button button--compact"
            disabled={api.busy || text.trim() === ""}
          >
            {t("manage.sheet.attributeAddButton")}
          </button>
        </span>
      </label>
    </form>
  );
}

// ------------------------------------------------------------------------------------ merge

function MergePanel({
  entity,
  api,
  onConfirm,
}: {
  entity: EntityRow;
  api: ManageApi;
  onConfirm: (pending: PendingConfirm) => void;
}) {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [found, setFound] = useState<EntityRow[]>([]);
  const [other, setOther] = useState<EntityRow | null>(null);
  const [keep, setKeep] = useState<number | null>(null);

  useEffect(() => {
    let current = true;
    const timer = window.setTimeout(() => {
      listEntities({
        text,
        typeId: entity.typeId,
        origin: null,
        view: "live",
        problemsOnly: false,
        page: 0,
      }).then(
        (page) => {
          if (current) {
            setFound(page.rows.filter((row) => canMergeWith(entity, row)));
          }
        },
        () => {
          if (current) {
            setFound([]);
          }
        },
      );
    }, 150);
    return () => {
      current = false;
      window.clearTimeout(timer);
    };
  }, [text, entity, api.revision]);

  const choose = (row: EntityRow) => {
    setOther(row);
    setKeep(defaultSurvivor(entity, row).survivor);
  };

  const survivorId = keep ?? entity.entityId;
  const absorbed =
    other === null ? null : survivorId === entity.entityId ? other : entity;
  const survivor = other === null ? null : survivorId === entity.entityId ? entity : other;

  return (
    <div className="sheet__form">
      <label className="field">
        <span className="field__label">{t("manage.merge.search")}</span>
        <input
          className="field__control"
          value={text}
          onChange={(event) => setText(event.target.value)}
        />
      </label>
      <p className="field__description">{t("manage.merge.searchHint")}</p>
      {found.length === 0 ? (
        <p className="sheet__meta">{t("manage.merge.none")}</p>
      ) : (
        <ul className="sheet__choices">
          {found.map((row) => (
            <li key={row.entityId}>
              <button
                type="button"
                className={
                  other?.entityId === row.entityId
                    ? "sheet__choice sheet__choice--picked"
                    : "sheet__choice"
                }
                onClick={() => choose(row)}
              >
                <span>{entityTitle(row)}</span>
                <span className="sheet__meta">
                  {`${row.mentions} · ${row.sources}`}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
      {other === null || survivor === null || absorbed === null ? null : (
        <>
          <fieldset className="sheet__direction">
            <legend className="field__label">{t("manage.merge.direction")}</legend>
            {[entity, other].map((row) => (
              <label key={row.entityId} className="field__row">
                <input
                  type="radio"
                  name="merge-direction"
                  checked={survivorId === row.entityId}
                  onChange={() => setKeep(row.entityId)}
                />
                <span>{entityTitle(row)}</span>
              </label>
            ))}
          </fieldset>
          <div className="sheet__actions">
            <button
              type="button"
              className="button button--compact button--primary"
              disabled={api.busy}
              onClick={() =>
                onConfirm({
                  action: "merge",
                  values: {
                    survivor: entityTitle(survivor),
                    absorbed: entityTitle(absorbed),
                  },
                  apply: async () => {
                    const outcome = await api.run(() =>
                      mergeEntities(survivor.entityId, absorbed.entityId),
                    );
                    if (outcome !== null) {
                      api.openEntity(survivor.entityId);
                    }
                  },
                })
              }
            >
              {t("manage.merge.go")}
            </button>
          </div>
        </>
      )}
    </div>
  );
}

// ------------------------------------------------------------------------------------ split

function SplitPanel({
  detail,
  picked,
  api,
  onConfirm,
  onSplit,
}: {
  detail: EntityDetail;
  picked: readonly number[];
  api: ManageApi;
  onConfirm: (pending: PendingConfirm) => void;
  onSplit: () => void;
}) {
  const { t } = useTranslation();
  const { entity } = detail;
  const [name, setName] = useState(suggestedSplitName(entity.name));

  const ready = canSplit(picked.length, detail.mentionCount, name);

  return (
    <div className="sheet__form">
      <p className="field__description">{t("manage.split.hint")}</p>
      <label className="field">
        <span className="field__label">{t("manage.split.nameLabel")}</span>
        <input
          className="field__control"
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
      </label>
      <p className="field__description">{t("manage.split.nameNote")}</p>
      <p className="sheet__meta">
        {picked.length === 0
          ? t("manage.split.pickSome")
          : t("manage.split.picked", { count: picked.length })}
      </p>
      <div className="sheet__actions">
        <button
          type="button"
          className="button button--compact button--primary"
          disabled={api.busy || !ready}
          onClick={() =>
            onConfirm({
              action: "split",
              values: {
                count: picked.length,
                name: entityTitle(entity),
                newName: name.trim(),
              },
              apply: async () => {
                const outcome = await api.run(() =>
                  splitEntity({
                    entityId: entity.entityId,
                    mentionIds: [...picked],
                    sourceIds: [],
                    name,
                  }),
                );
                if (outcome !== null && outcome.entityId !== null) {
                  onSplit();
                  api.openEntity(outcome.entityId);
                }
              },
            })
          }
        >
          {t("manage.split.go")}
        </button>
      </div>
    </div>
  );
}
