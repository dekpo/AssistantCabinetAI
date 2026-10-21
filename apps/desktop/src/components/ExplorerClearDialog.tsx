import { useTranslation } from "../i18n/I18nProvider";
import type { Family } from "../lib/explorerModel";
import { ConfirmDialog } from "./ConfirmDialog";

/**
 * The question asked before the analysis of a family is cleared. The question names the family and
 * says what is lost; the files themselves are never touched. Shared by the sidebar's box and the
 * Settings panel, so both ask in the same words.
 */
export function ExplorerClearDialog({
  family,
  busy,
  onConfirm,
  onCancel,
}: {
  family: Family;
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const documents = family === "documents";
  return (
    <ConfirmDialog
      title={t(documents ? "workFolder.resetTitle" : "dataFolder.resetTitle")}
      body={t(documents ? "workFolder.resetBody" : "dataFolder.resetBody")}
      confirmLabel={t("explorer.menuClear")}
      destructive
      busy={busy}
      onConfirm={onConfirm}
      onCancel={onCancel}
    />
  );
}
