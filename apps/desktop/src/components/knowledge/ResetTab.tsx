import { useState } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { resetKnowledge } from "../../lib/ipc";
import type { ManageApi, PendingConfirm } from "../../lib/knowledgeManage";

/**
 * "Reset": the whole knowledge base, in two strengths. It is not one of the two folder Resets: those
 * empty what a folder's analysis produced, this one empties the names the base learned from it.
 * Both strengths ask first, and neither touches a file, an indexed passage or a spreadsheet.
 */
export function ResetTab({
  api,
  onConfirm,
}: {
  api: ManageApi;
  onConfirm: (pending: PendingConfirm) => void;
}) {
  const { t } = useTranslation();
  const [done, setDone] = useState(false);

  const ask = (includeManual: boolean) =>
    onConfirm({
      action: includeManual ? "resetAll" : "reset",
      values: {},
      apply: async () => {
        const outcome = await api.run(() => resetKnowledge(includeManual));
        setDone(outcome !== null);
      },
    });

  return (
    <div className="knowledge__review">
      <p className="field__description">{t("manage.reset.intro")}</p>

      <section className="sheet__section">
        <h3 className="sheet__heading">{t("manage.reset.keepTitle")}</h3>
        <p className="field__description">{t("manage.reset.keepBody")}</p>
        <div className="sheet__actions">
          <button
            type="button"
            className="button button--compact"
            disabled={api.busy}
            onClick={() => ask(false)}
          >
            {t("manage.reset.keepButton")}
          </button>
        </div>
      </section>

      <section className="sheet__section">
        <h3 className="sheet__heading">{t("manage.reset.allTitle")}</h3>
        <p className="field__description">{t("manage.reset.allBody")}</p>
        <div className="sheet__actions">
          <button
            type="button"
            className="button button--compact button--destructive"
            disabled={api.busy}
            onClick={() => ask(true)}
          >
            {t("manage.reset.allButton")}
          </button>
        </div>
      </section>

      <p className="field__description">{t("manage.reset.after")}</p>
      {done ? <p className="sheet__meta">{t("manage.reset.done")}</p> : null}
    </div>
  );
}
