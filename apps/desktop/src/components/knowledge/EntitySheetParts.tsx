import { counted } from "../../lib/plural";
import { useTranslation } from "../../i18n/I18nProvider";
import type {
  AliasView,
  AttributeView,
  ColumnView,
  EntityDetail,
  MentionView,
  PossibleView,
  RelationView,
  WorkbookColumns,
} from "../../lib/ipc";
import {
  aliasKindLabel,
  columnReasonLabel,
  confidenceText,
  locatorText,
  methodLabel,
  predicateLabel,
  reasonLabel,
  roleLabel,
  subtypeLabel,
  typeLabel,
} from "../../lib/knowledgeManage";

/*
 * The sections of one entity's sheet. Each draws data Rust already computed (the provenance of every
 * association, the names, the links, the details, the pairs, the way the spreadsheet columns were
 * read); none decides anything.
 */

/** One mention: where it was found, the passage around it, and what the base relied on. */
function MentionRow({
  mention,
  ownerId,
  picking,
  picked,
  onToggle,
}: {
  mention: MentionView;
  ownerId: number;
  picking: boolean;
  picked: boolean;
  onToggle: () => void;
}) {
  const { t } = useTranslation();
  const role = roleLabel(t, mention.role);
  return (
    <li className="sheet__mention">
      {picking ? (
        <input
          type="checkbox"
          className="sheet__pick"
          checked={picked}
          onChange={onToggle}
          aria-label={t("manage.mention.pick")}
        />
      ) : null}
      <div className="sheet__mention-body">
        <p className="sheet__where">
          {locatorText(t, mention.locator)}
          {mention.occurrences > 1
            ? ` · ${t("manage.mention.times", { count: mention.occurrences })}`
            : ""}
        </p>
        {mention.excerpt === null ? null : (
          <blockquote className="sheet__excerpt">
            {mention.excerpt.cutBefore ? "… " : ""}
            {mention.excerpt.text}
            {mention.excerpt.cutAfter ? " …" : ""}
          </blockquote>
        )}
        <p className="sheet__meta">
          {t("manage.mention.foundBy", {
            method: methodLabel(t, mention.method),
            confidence: confidenceText(mention.confidence),
          })}
        </p>
        {mention.alias === null ? null : (
          <p className="sheet__meta">{t("manage.mention.matched", { alias: mention.alias })}</p>
        )}
        {role === null ? null : (
          <p className="sheet__meta">{t("manage.mention.role", { role })}</p>
        )}
        {mention.origin === "manual" ? (
          <p className="sheet__meta">{t("manage.mention.manual")}</p>
        ) : null}
        {mention.splitFrom === null ? null : (
          <p className="sheet__meta">{t("manage.mention.separated")}</p>
        )}
        {mention.entityId === ownerId ? null : (
          <p className="sheet__meta">{t("manage.mention.viaMerged")}</p>
        )}
      </div>
    </li>
  );
}

export function ProvenanceList({
  detail,
  picking,
  picked,
  onToggle,
}: {
  detail: EntityDetail;
  picking: boolean;
  picked: readonly number[];
  onToggle: (mentionId: number) => void;
}) {
  const { t } = useTranslation();
  const shown = detail.sources.reduce((total, source) => total + source.mentions.length, 0);
  return (
    <section className="sheet__section">
      <h4 className="sheet__heading">{t("manage.sheet.provenance")}</h4>
      <p className="field__description">{t("manage.sheet.provenanceIntro")}</p>
      {detail.sources.length === 0 ? (
        <p className="sheet__meta">{t("manage.sheet.provenanceNone")}</p>
      ) : (
        detail.sources.map((source) => (
          <div key={source.sourceId} className="sheet__source">
            <p className="sheet__source-title">
              <span className="tag">
                {t(
                  source.domain === "data"
                    ? "manage.sheet.sourceTable"
                    : "manage.sheet.sourceDocument",
                )}
              </span>{" "}
              <span className="sheet__path">{source.relativePath}</span>
            </p>
            {source.truncated ? (
              <p className="sheet__meta" role="note">
                {t("manage.sheet.sourceTruncated")}
              </p>
            ) : null}
            <ul className="sheet__mentions">
              {source.mentions.map((mention) => (
                <MentionRow
                  key={mention.mentionId}
                  mention={mention}
                  ownerId={detail.entity.entityId}
                  picking={picking}
                  picked={picked.includes(mention.mentionId)}
                  onToggle={() => onToggle(mention.mentionId)}
                />
              ))}
            </ul>
          </div>
        ))
      )}
      {shown < detail.mentionCount ? (
        <p className="sheet__meta">
          {t("manage.sheet.provenanceMore", { shown, total: detail.mentionCount })}
        </p>
      ) : null}
    </section>
  );
}

export function AliasList({
  aliases,
  ownerId,
  editable,
  onRemove,
}: {
  aliases: AliasView[];
  ownerId: number;
  editable: boolean;
  onRemove: (alias: AliasView) => void;
}) {
  const { t } = useTranslation();
  return (
    <ul className="sheet__aliases">
      {aliases.map((alias) => (
        <li key={alias.aliasId} className="sheet__alias">
          <span className="sheet__alias-name">{alias.display}</span>
          <span className="tag">{aliasKindLabel(t, alias.kind)}</span>
          {alias.entityId === ownerId ? null : (
            <span className="tag">{t("manage.mention.viaMerged")}</span>
          )}
          {alias.origin === "manual" && editable ? (
            <button
              type="button"
              className="button button--quiet button--compact"
              onClick={() => onRemove(alias)}
            >
              {t("manage.sheet.removeName")}
            </button>
          ) : alias.origin === "automatic" ? (
            <span className="sheet__meta" title={t("manage.sheet.automaticName")}>
              {t("manage.origin.automatic")}
            </span>
          ) : null}
        </li>
      ))}
    </ul>
  );
}

export function AttributeList({
  attributes,
  editable,
  onRemove,
}: {
  attributes: AttributeView[];
  editable: boolean;
  onRemove: (attribute: AttributeView) => void;
}) {
  const { t } = useTranslation();
  if (attributes.length === 0) {
    return null;
  }
  return (
    <ul className="sheet__aliases">
      {attributes.map((attribute) => {
        const key = `manage.attribute.${attribute.key}`;
        const label = t(key);
        return (
          <li key={`${attribute.key}:${attribute.value}`} className="sheet__alias">
            <span className="sheet__meta">{label === key ? "" : label}</span>
            <span className="sheet__alias-name">{attribute.value}</span>
            {attribute.origin === "manual" && editable ? (
              <button
                type="button"
                className="button button--quiet button--compact"
                onClick={() => onRemove(attribute)}
              >
                {t("manage.sheet.attributeRemove")}
              </button>
            ) : null}
          </li>
        );
      })}
    </ul>
  );
}

export function RelationList({ relations }: { relations: RelationView[] }) {
  const { t } = useTranslation();
  if (relations.length === 0) {
    return null;
  }
  return (
    <section className="sheet__section">
      <h4 className="sheet__heading">{t("manage.sheet.relations")}</h4>
      <ul className="sheet__aliases">
        {relations.map((relation) => (
          <li key={relation.relationId} className="sheet__alias">
            <span className="sheet__alias-name">{relation.subject.name}</span>
            <span className="sheet__meta">{predicateLabel(t, relation.predicate)}</span>
            <span className="sheet__alias-name">{relation.object.name}</span>
            {relation.relativePath === null ? null : (
              <span className="sheet__path">{relation.relativePath}</span>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

export function PossibleList({
  matches,
  onOpen,
}: {
  matches: PossibleView[];
  onOpen: (entityId: number) => void;
}) {
  const { t } = useTranslation();
  if (matches.length === 0) {
    return null;
  }
  return (
    <section className="sheet__section">
      <h4 className="sheet__heading">{t("manage.sheet.possible")}</h4>
      <ul className="sheet__aliases">
        {matches.map((match) => (
          <li key={match.other.entityId} className="sheet__alias">
            <span className="sheet__alias-name">{match.other.name}</span>
            <span className="tag">{t(`manage.matchState.${match.state}`)}</span>
            <span className="sheet__meta">{reasonLabel(t, match.reason)}</span>
            <button
              type="button"
              className="button button--quiet button--compact"
              onClick={() => onOpen(match.other.entityId)}
            >
              {t("manage.review.open")}
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

function ColumnRow({ column }: { column: ColumnView }) {
  const { t } = useTranslation();
  const type = typeLabel(t, column.semanticType);
  const subtype = subtypeLabel(t, column.subtype);
  const role = roleLabel(t, column.role);
  return (
    <li className="sheet__alias">
      <span className="sheet__alias-name">{`${column.sheet} / ${column.column}`}</span>
      <span className="tag">
        {column.read
          ? t("manage.sheet.columnRead", { type: subtype ?? type })
          : t("manage.sheet.columnNotRead")}
      </span>
      {role === null ? null : <span className="sheet__meta">{role}</span>}
      <span className="sheet__meta">
        {`${columnReasonLabel(t, column.reason)}, ${t("manage.sheet.columnConfidence", {
          confidence: confidenceText(column.confidence),
        })}`}
      </span>
    </li>
  );
}

export function ColumnList({ workbooks }: { workbooks: WorkbookColumns[] }) {
  const { t } = useTranslation();
  if (workbooks.length === 0) {
    return null;
  }
  return (
    <section className="sheet__section">
      <h4 className="sheet__heading">{t("manage.sheet.columns")}</h4>
      {workbooks.map((workbook) => (
        <div key={workbook.relativePath} className="sheet__source">
          <p className="sheet__source-title">
            <span className="tag">{t("manage.sheet.sourceTable")}</span>{" "}
            <span className="sheet__path">{workbook.relativePath}</span>
          </p>
          <ul className="sheet__aliases">
            {workbook.columns.map((column) => (
              <ColumnRow key={`${column.sheet}/${column.column}`} column={column} />
            ))}
          </ul>
        </div>
      ))}
    </section>
  );
}

/** The counts under the title: how often and in how many files the base saw this entity. */
export function SheetCounts({ detail }: { detail: EntityDetail }) {
  const { t } = useTranslation();
  const { entity } = detail;
  const subtype = subtypeLabel(t, entity.subtype);
  return (
    <p className="sheet__meta">
      {[
        typeLabel(t, entity.typeId),
        ...(subtype === null ? [] : [subtype]),
        counted(detail.mentionCount, t("manage.countMentionOne"), t("manage.countMentionMany")),
        counted(entity.sources, t("manage.entities.fileOne"), t("manage.entities.fileMany")),
      ].join(" · ")}
    </p>
  );
}
