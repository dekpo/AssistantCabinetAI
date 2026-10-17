import { useTranslation } from "../../i18n/I18nProvider";
import { ConfirmDialog } from "../ConfirmDialog";

/** The one modal of the viewer: the open file was moved, renamed, deleted or modified outside the
 * software, so it was closed and must be analysed again before it can be shown. */
export function OutsideChangeNotice({ onAcknowledge }: { onAcknowledge: () => void }) {
  const { t } = useTranslation();

  return (
    <ConfirmDialog
      title={t("viewer.outsideChangeTitle")}
      body={t("viewer.outsideChangeBody")}
      confirmLabel={t("viewer.outsideChangeConfirm")}
      singleAction
      onConfirm={onAcknowledge}
      onCancel={onAcknowledge}
    />
  );
}
