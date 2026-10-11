import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { cardView } from "../lib/entityCard";
import { entityCard, type CombinedScope, type EntityCard } from "../lib/ipc";

function CardSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="entity-card__section">
      <h3 className="entity-card__heading">{title}</h3>
      {children}
    </section>
  );
}

function CardList({ lines }: { lines: readonly { key: string; text: string }[] }) {
  return (
    <ul className="entity-card__list">
      {lines.map((line) => (
        <li key={line.key}>{line.text}</li>
      ))}
    </ul>
  );
}

/** The sheet, drawn from what Rust sent (or from the fact that nothing could be read). */
export function EntityCardSheet({
  card,
  display,
  onClose,
}: {
  /** `undefined` while it is read, `null` for a name that no longer exists or a read that failed. */
  card: EntityCard | null | undefined;
  /** The name as the chip or the list showed it, for the title while the sheet is read. */
  display: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const close = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    close.current?.focus();
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const view = card === undefined || card === null ? null : cardView(t, card);
  const title = view === null ? t("entityCard.title", { name: display }) : view.title;

  return (
    <div className="confirm__backdrop" onMouseDown={onClose}>
      <div
        className="entity-card"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <h2 className="entity-card__title">{title}</h2>
        {card === undefined ? <p className="entity-card__note">{t("entityCard.loading")}</p> : null}
        {card === null ? <p className="entity-card__note">{t("entityCard.notFound")}</p> : null}
        {view === null ? null : (
          <>
            <p className="entity-card__kind">{view.kind}</p>
            {view.unconfirmed ? <p className="entity-card__note">{t("entityCard.unconfirmed")}</p> : null}
            <CardSection title={t("entityCard.aliasesTitle")}>
              {view.aliases.length === 0 ? (
                <p className="entity-card__note">{t("entityCard.aliasesNone")}</p>
              ) : (
                <p className="entity-card__line">{view.aliases.join(" · ")}</p>
              )}
            </CardSection>
            {view.roles.length === 0 ? null : (
              <CardSection title={t("entityCard.rolesTitle")}>
                <p className="entity-card__line">{view.roles.join(" · ")}</p>
              </CardSection>
            )}
            {view.documents.length === 0 ? null : (
              <CardSection title={t("entityCard.documentsTitle")}>
                <CardList lines={view.documents} />
              </CardSection>
            )}
            {view.tables.length === 0 ? null : (
              <CardSection title={t("entityCard.tablesTitle")}>
                <CardList lines={view.tables} />
              </CardSection>
            )}
            {view.nowhere ? <p className="entity-card__note">{t("entityCard.nowhere")}</p> : null}
            {view.outside === null ? null : <p className="entity-card__note">{view.outside}</p>}
            {view.relations.length === 0 ? null : (
              <CardSection title={t("entityCard.relationsTitle")}>
                <CardList lines={view.relations} />
              </CardSection>
            )}
            {view.details.length === 0 ? null : (
              <CardSection title={t("entityCard.detailsTitle")}>
                <CardList lines={view.details} />
              </CardSection>
            )}
          </>
        )}
        <div className="confirm__actions">
          <button type="button" ref={close} className="button button--compact" onClick={onClose}>
            {t("entityCard.close")}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * The sheet of one name, inside the current selection: the other ways it is written, the files that
 * hold it (documents with their pages, tables with the sheet and the column), a count for what lies
 * outside the selection, and the links the files state.
 *
 * Opened from a chip or from the name list, and only ever read: it changes neither the selection nor
 * the knowledge base. Drawn from `lib/entityCard.ts`; Rust sends names, paths and counts, never a
 * sentence (`knowledge::sheet`).
 */
export function EntityCardDialog({
  entityId,
  display,
  scope,
  onClose,
}: {
  entityId: number;
  /** The name as the chip or the list showed it, for the title while the sheet is read. */
  display: string;
  scope: CombinedScope;
  onClose: () => void;
}) {
  /* `undefined` while it is read, `null` for a name that no longer exists or a read that failed. */
  const [card, setCard] = useState<EntityCard | null | undefined>(undefined);

  useEffect(() => {
    let live = true;
    setCard(undefined);
    entityCard(entityId, scope).then(
      (found) => {
        if (live) {
          setCard(found);
        }
      },
      () => {
        if (live) {
          setCard(null);
        }
      },
    );
    return () => {
      live = false;
    };
  }, [entityId, scope]);

  return <EntityCardSheet card={card} display={display} onClose={onClose} />;
}
