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
  recording = false,
  onReview,
  onDiscard,
  onCancel,
}: {
  count: number;
  /** A dictation is running: the answer also stops it, and the dialog says so. */
  recording?: boolean;
  onReview: () => void;
  onDiscard: () => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();

  return (
    <ConfirmDialog
      title={t("notes.unsaved.title")}
      body={
        pluralise(count, t("notes.unsaved.bodyOne"), t("notes.unsaved.bodyMany", { count })) +
        (recording ? `
${t("notes.unsaved.recording")}` : "")
      }
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

/**
 * "A dictation is running": Cancel keeps recording, the confirming answer stops it and drops the
 * words not yet written. Nothing already in a note is touched by this answer.
 */
export function RecordingDialog({
  onStop,
  onCancel,
}: {
  onStop: () => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();

  return (
    <ConfirmDialog
      title={t("notes.recording.title")}
      body={t("notes.recording.body")}
      confirmLabel={t("notes.recording.stop")}
      destructive
      onConfirm={onStop}
      onCancel={onCancel}
    />
  );
}
