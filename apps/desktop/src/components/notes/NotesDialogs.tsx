import { useTranslation } from "../../i18n/I18nProvider";
import { pluralise } from "../../lib/plural";
import { ConfirmDialog } from "../ConfirmDialog";

/**
 * "A note would be lost": Cancel stays where it is, Discard goes on without the notes, and the
 * confirming answer takes the user to the exact files that would be written. It never saves on its
 * own: writing a file is the click on the plan.
 */
export function UnsavedNoteDialog({
  count,
  onReview,
  onDiscard,
  onCancel,
}: {
  count: number;
  onReview: () => void;
  onDiscard: () => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();

  return (
    <ConfirmDialog
      title={t("notes.unsaved.title")}
      body={pluralise(count, t("notes.unsaved.bodyOne"), t("notes.unsaved.bodyMany", { count }))}
      confirmLabel={t("notes.unsaved.review")}
      secondary={{ label: t("notes.unsaved.discard"), onSelect: onDiscard, destructive: true }}
      onConfirm={onReview}
      onCancel={onCancel}
    />
  );
}

/** Other documents are also waiting: the pass reads the whole folder, so the user is asked first. */
export function AnalysisChoiceDialog({
  onAnalyse,
  onSkip,
}: {
  onAnalyse: () => void;
  onSkip: () => void;
}) {
  const { t } = useTranslation();

  return (
    <ConfirmDialog
      title={t("notes.analysis.title")}
      body={t("notes.analysis.body")}
      confirmLabel={t("notes.analysis.confirm")}
      onConfirm={onAnalyse}
      onCancel={onSkip}
    />
  );
}
