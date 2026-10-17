import { useTranslation } from "../i18n/I18nProvider";
import type { WorkspaceView } from "../lib/workspaceView";

/** The content of the centre zone: one case per kind of `WorkspaceView`. */
export function WorkspacePanel({ view }: { view: WorkspaceView }) {
  const { t } = useTranslation();

  switch (view.kind) {
    case "welcome":
      return (
        <section className="workspace">
          <h2 className="workspace__title">{t("workspace.title")}</h2>
          <p className="workspace__text">{t("workspace.welcome")}</p>
        </section>
      );
  }
}
