import { useTranslation } from "../i18n/I18nProvider";
import type { IndexingState } from "../state/useIndexing";

/*
 * The two small controls that outlived the old folder cards: the button that starts a pass (the
 * family sections draw it) and the link to the knowledge base (the overview draws it).
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
  title,
  disabled = false,
}: {
  indexing: IndexingState;
  emphasised: boolean;
  /** How many files the pass would still change. Shown only while it is worth acting on. */
  pendingCount: number;
  /** A tooltip, when the count needs a word of explanation. */
  title?: string;
  /** The pass has nothing to do (an empty folder): the button stays, greyed, and `title` says why. */
  disabled?: boolean;
}) {
  const { t } = useTranslation();

  return (
    <button
      type="button"
      className={emphasised ? "button button--compact button--primary" : "button button--compact"}
      title={title}
      disabled={indexing.running || disabled}
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

/**
 * Under the analysis summary, once the knowledge base took part in the pass: one click to the place
 * where what it found can be seen, with where each name comes from. Opens the management dialog
 * (`KnowledgeDialog`); changes nothing by itself.
 */
export function KnowledgeLink({ onOpen }: { onOpen: () => void }) {
  const { t } = useTranslation();
  return (
    <button type="button" className="button button--quiet button--compact" onClick={onOpen}>
      {t("analysis.knowledge.manageLink")}
    </button>
  );
}
