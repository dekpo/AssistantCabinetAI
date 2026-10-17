import { useTranslation } from "../i18n/I18nProvider";
import type { WorkspaceView } from "../lib/workspaceView";
import { FileViewer } from "./viewer/FileViewer";

/** The content of the centre zone: one case per kind of `WorkspaceView`. */
export function WorkspacePanel({
  view,
  onClose,
  onFileMissing,
}: {
  view: WorkspaceView;
  onClose: () => void;
  /** A file that is no longer where it was; must be stable. */
  onFileMissing: () => void;
}) {
  const { t } = useTranslation();

  switch (view.kind) {
    case "welcome":
      return (
        <section className="workspace">
          <h2 className="workspace__title">{t("workspace.title")}</h2>
          <p className="workspace__text">{t("workspace.welcome")}</p>
        </section>
      );
    case "file":
      /* Keyed by the file, so another file starts from a clean viewer. */
      return (
        <FileViewer
          key={`${view.folder}:${view.record.relativePath}`}
          view={view}
          onClose={onClose}
          onMissing={onFileMissing}
        />
      );
  }
}
