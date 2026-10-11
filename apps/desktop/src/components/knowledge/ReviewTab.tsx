import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { normaliseError, type AppError } from "../../lib/errors";
import {
  acceptMatch,
  dismissMatch,
  knowledgeHealth,
  possibleMatches,
  type EntityBriefView,
  type HealthReport,
  type PossiblePage,
  type PossiblePair,
  type SourceBrief,
} from "../../lib/ipc";
import {
  confidenceText,
  reasonLabel,
  typeLabel,
  type ManageApi,
  type PendingConfirm,
} from "../../lib/knowledgeManage";
import { counted } from "../../lib/plural";
import { ErrorBanner } from "../ErrorBanner";

/**
 * "To check": the pairs that might be one entity, and the counts that say where the library may need
 * a look. Nothing is merged or corrected here without a click, and the pairs she merges go through
 * the same confirmation as any merge.
 */
export function ReviewTab({
  api,
  onConfirm,
  onShowProblems,
}: {
  api: ManageApi;
  onConfirm: (pending: PendingConfirm) => void;
  /** Go to the Entities tab with the "worth a look" filter on. */
  onShowProblems: () => void;
}) {
  const { t } = useTranslation();
  const [pairs, setPairs] = useState<PossiblePage | null>(null);
  const [health, setHealth] = useState<HealthReport | null>(null);
  const [pairsError, setPairsError] = useState<AppError | null>(null);
  const [healthError, setHealthError] = useState<AppError | null>(null);

  useEffect(() => {
    let current = true;
    possibleMatches(0).then(
      (found) => {
        if (current) {
          setPairs(found);
          setPairsError(null);
        }
      },
      (raw: unknown) => {
        if (current) {
          setPairsError(normaliseError(raw));
        }
      },
    );
    knowledgeHealth().then(
      (found) => {
        if (current) {
          setHealth(found);
          setHealthError(null);
        }
      },
      (raw: unknown) => {
        if (current) {
          setHealthError(normaliseError(raw));
        }
      },
    );
    return () => {
      current = false;
    };
  }, [api.revision]);

  return (
    <div className="knowledge__review">
      <section className="sheet__section">
        <h3 className="sheet__heading">{t("manage.review.pairs")}</h3>
        <p className="field__description">{t("manage.review.pairsIntro")}</p>
        {pairsError === null ? null : <ErrorBanner error={pairsError} />}
        {pairs !== null && pairs.pairs.length === 0 ? (
          <p className="sheet__meta">{t("manage.review.pairsNone")}</p>
        ) : null}
        <ul className="knowledge__pairs">
          {(pairs?.pairs ?? []).map((pair) => (
            <PairCard
              key={`${pair.one.entityId}-${pair.other.entityId}`}
              pair={pair}
              api={api}
              onConfirm={onConfirm}
            />
          ))}
        </ul>
        {pairs !== null && pairs.total > pairs.pairs.length ? (
          <p className="sheet__meta">
            {t("manage.review.more", { shown: pairs.pairs.length, total: pairs.total })}
          </p>
        ) : null}
      </section>

      <section className="sheet__section">
        <h3 className="sheet__heading">{t("manage.review.health")}</h3>
        <p className="field__description">{t("manage.review.healthIntro")}</p>
        {healthError === null ? null : <ErrorBanner error={healthError} />}
        {health === null ? null : (
          <HealthRows health={health} api={api} onShowProblems={onShowProblems} />
        )}
      </section>
    </div>
  );
}

function PairCard({
  pair,
  api,
  onConfirm,
}: {
  pair: PossiblePair;
  api: ManageApi;
  onConfirm: (pending: PendingConfirm) => void;
}) {
  const { t } = useTranslation();

  const mergeInto = (survivor: EntityBriefView, absorbed: EntityBriefView) =>
    onConfirm({
      action: "merge",
      values: { survivor: survivor.name, absorbed: absorbed.name },
      apply: async () => {
        const outcome = await api.run(() => acceptMatch(survivor.entityId, absorbed.entityId));
        if (outcome !== null) {
          api.openEntity(survivor.entityId);
        }
      },
    });

  return (
    <li className="knowledge__pair">
      <p className="sheet__where">
        {pair.one.name}
        {" · "}
        {pair.other.name}
      </p>
      <p className="sheet__meta">
        {`${typeLabel(t, pair.one.typeId)} · ${reasonLabel(t, pair.reason)} · ${confidenceText(
          pair.score,
        )}`}
      </p>
      <p className="sheet__meta">
        {[
          t("manage.review.pairCount", {
            name: pair.one.name,
            count: counted(
              pair.oneMentions,
              t("manage.countMentionOne"),
              t("manage.countMentionMany"),
            ),
          }),
          t("manage.review.pairCount", {
            name: pair.other.name,
            count: counted(
              pair.otherMentions,
              t("manage.countMentionOne"),
              t("manage.countMentionMany"),
            ),
          }),
        ].join(" · ")}
      </p>
      <div className="sheet__actions">
        <button
          type="button"
          className="button button--compact"
          disabled={api.busy}
          onClick={() => mergeInto(pair.one, pair.other)}
        >
          {t("manage.review.mergeInto", { name: pair.one.name })}
        </button>
        <button
          type="button"
          className="button button--compact"
          disabled={api.busy}
          onClick={() => mergeInto(pair.other, pair.one)}
        >
          {t("manage.review.mergeInto", { name: pair.other.name })}
        </button>
        <button
          type="button"
          className="button button--compact"
          disabled={api.busy}
          onClick={() => void api.run(() => dismissMatch(pair.one.entityId, pair.other.entityId))}
        >
          {t("manage.review.distinct")}
        </button>
        <button
          type="button"
          className="button button--quiet button--compact"
          onClick={() => api.openEntity(pair.one.entityId)}
        >
          {t("manage.review.open")}
        </button>
      </div>
    </li>
  );
}

/** One counted line of the health report, with a list that opens on demand. */
function HealthRow({
  label,
  total,
  children,
}: {
  label: string;
  total: number;
  children?: ReactNode;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  return (
    <li className="knowledge__health">
      <div className="knowledge__bar">
        <span>{t("manage.review.countLine", { label, total })}</span>
        {total > 0 && children !== undefined ? (
          <button
            type="button"
            className="button button--quiet button--compact"
            aria-expanded={open}
            onClick={() => setOpen((value) => !value)}
          >
            {open ? t("manage.review.hide") : t("manage.review.show")}
          </button>
        ) : null}
      </div>
      {open ? children : null}
    </li>
  );
}

function EntityLinks({
  items,
  total,
  api,
}: {
  items: EntityBriefView[];
  total: number;
  api: ManageApi;
}) {
  const { t } = useTranslation();
  return (
    <>
      <ul className="sheet__aliases">
        {items.map((item) => (
          <li key={item.entityId} className="sheet__alias">
            <button
              type="button"
              className="button button--quiet button--compact"
              onClick={() => api.openEntity(item.entityId)}
            >
              {item.name}
            </button>
            <span className="sheet__meta">{typeLabel(t, item.typeId)}</span>
          </li>
        ))}
      </ul>
      {total > items.length ? (
        <p className="sheet__meta">
          {t("manage.review.more", { shown: items.length, total })}
        </p>
      ) : null}
    </>
  );
}

function SourceList({ items, total }: { items: SourceBrief[]; total: number }) {
  const { t } = useTranslation();
  return (
    <>
      <ul className="sheet__aliases">
        {items.map((item) => (
          <li key={item.sourceId} className="sheet__alias">
            <span className="sheet__path">{item.relativePath}</span>
          </li>
        ))}
      </ul>
      {total > items.length ? (
        <p className="sheet__meta">
          {t("manage.review.more", { shown: items.length, total })}
        </p>
      ) : null}
    </>
  );
}

function HealthRows({
  health,
  api,
  onShowProblems,
}: {
  health: HealthReport;
  api: ManageApi;
  onShowProblems: () => void;
}) {
  const { t } = useTranslation();
  return (
    <ul className="knowledge__healths">
      <HealthRow label={t("manage.review.seenOnce")} total={health.seenOnce.total}>
        <EntityLinks items={health.seenOnce.items} total={health.seenOnce.total} api={api} />
      </HealthRow>
      <li className="knowledge__health">
        <div className="knowledge__bar">
          <span>
            {t("manage.review.countLine", {
              label: t("manage.review.candidates"),
              total: health.candidates,
            })}
          </span>
          {health.candidates > 0 ? (
            <button
              type="button"
              className="button button--quiet button--compact"
              onClick={onShowProblems}
            >
              {t("manage.review.showEntities")}
            </button>
          ) : null}
        </div>
      </li>
      <HealthRow
        label={t("manage.review.documentsWithoutEntity")}
        total={health.documentsWithoutEntity.total}
      >
        <SourceList
          items={health.documentsWithoutEntity.items}
          total={health.documentsWithoutEntity.total}
        />
      </HealthRow>
      <HealthRow
        label={t("manage.review.workbooksWithoutColumn")}
        total={health.workbooksWithoutUsableColumn.total}
      >
        <SourceList
          items={health.workbooksWithoutUsableColumn.items}
          total={health.workbooksWithoutUsableColumn.total}
        />
      </HealthRow>
      <HealthRow label={t("manage.review.duplicates")} total={health.probableDuplicates} />
      <HealthRow
        label={t("manage.review.inDocumentsNotInTables")}
        total={health.inDocumentsNotInTables.total}
      >
        <EntityLinks
          items={health.inDocumentsNotInTables.items}
          total={health.inDocumentsNotInTables.total}
          api={api}
        />
      </HealthRow>
    </ul>
  );
}
