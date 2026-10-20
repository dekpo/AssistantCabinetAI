import { useId, useState, type FormEvent } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import type { KnowledgeFacts } from "../lib/explorerFacts";
import type { ExplorerView } from "../lib/explorerView";
import {
  BUILT_IN_VIEWS,
  isEmptyView,
  MAX_SAVED_VIEWS,
  MAX_VIEW_NAME,
  missingEntities,
  nameProblem,
  sameView,
  viewFromSaved,
} from "../lib/explorerViews";
import type { SavedViewSetting } from "../lib/ipc";

/**
 * The « Vues » side of the explorer: the views the application brings, the ones the user saved (open,
 * rename, delete) and the form that saves the current filters under a name. A view holds filters only;
 * opening one replaces the current filters and never touches the selection, the lists or the typed text.
 * A saved view that names someone the files no longer link to says so, instead of showing nothing.
 */
export function ExplorerViewsPanel({
  saved,
  current,
  facts,
  factsLoaded,
  onOpen,
  onSave,
  onRename,
  onDelete,
}: {
  saved: readonly SavedViewSetting[];
  /** The filters in force now: what « Enregistrer » would keep, and which view is the open one. */
  current: ExplorerView;
  facts: KnowledgeFacts;
  /** The facts have been read: a name that is not among them is not found any more. */
  factsLoaded: boolean;
  onOpen: (view: ExplorerView) => void;
  onSave: (name: string) => void;
  onRename: (id: string, name: string) => void;
  onDelete: (id: string) => void;
}) {
  const { t } = useTranslation();
  const saveId = useId();
  const [name, setName] = useState("");
  const [renaming, setRenaming] = useState<{ id: string; draft: string } | null>(null);
  const known = new Set(facts.entities.keys());
  const empty = isEmptyView(current);
  const full = saved.length >= MAX_SAVED_VIEWS;
  const problem = name.trim() === "" ? null : nameProblem(saved, name);
  const canSave = !empty && !full && name.trim() !== "" && problem === null;

  const submitSave = (event: FormEvent) => {
    event.preventDefault();
    if (canSave) {
      onSave(name);
      setName("");
    }
  };
  const submitRename = (event: FormEvent) => {
    event.preventDefault();
    if (renaming !== null && nameProblem(saved, renaming.draft, renaming.id) === null) {
      onRename(renaming.id, renaming.draft);
      setRenaming(null);
    }
  };

  return (
    <div className="explorer-views">
      <div className="explorer-filters__block">
        <h3 className="explorer-filters__title">{t("explorer.viewsBuiltIn")}</h3>
        <div className="explorer-chips">
          {BUILT_IN_VIEWS.map(({ id, view }) => (
            <button
              key={id}
              type="button"
              className="explorer-chip"
              aria-pressed={sameView(current, view)}
              onClick={() => onOpen(view)}
            >
              {t(`explorer.builtin.${id}`)}
            </button>
          ))}
        </div>
      </div>

      <div className="explorer-filters__block">
        <h3 className="explorer-filters__title">{t("explorer.viewsMine")}</h3>
        {saved.length === 0 ? <p className="explorer-filters__none">{t("explorer.viewsNone")}</p> : null}
        <ul className="explorer-views__list">
          {saved.map((view) => {
            const opened = viewFromSaved(view);
            const missing = factsLoaded ? missingEntities(opened, known).length : 0;
            const editing = renaming?.id === view.id;
            return (
              <li key={view.id} className="explorer-views__item">
                {editing ? (
                  <form className="explorer-views__row" onSubmit={submitRename}>
                    <input
                      type="text"
                      className="explorer-filters__field"
                      value={renaming.draft}
                      maxLength={MAX_VIEW_NAME}
                      aria-label={t("explorer.viewRenameLabel")}
                      autoFocus
                      spellCheck={false}
                      onChange={(event) => setRenaming({ id: view.id, draft: event.target.value })}
                      onKeyDown={(event) => {
                        if (event.key === "Escape") {
                          event.stopPropagation();
                          setRenaming(null);
                        }
                      }}
                    />
                    <button
                      type="submit"
                      className="button button--compact button--primary"
                      disabled={nameProblem(saved, renaming.draft, view.id) !== null}
                    >
                      {t("explorer.viewRenameOk")}
                    </button>
                    <button
                      type="button"
                      className="button button--compact"
                      onClick={() => setRenaming(null)}
                    >
                      {t("explorer.viewRenameCancel")}
                    </button>
                  </form>
                ) : (
                  <div className="explorer-views__row">
                    <button
                      type="button"
                      className="explorer-chip explorer-views__open"
                      aria-pressed={sameView(current, opened)}
                      title={view.name}
                      onClick={() => onOpen(opened)}
                    >
                      <span className="explorer-views__name">{view.name}</span>
                    </button>
                    <button
                      type="button"
                      className="explorer-link explorer-link--quiet"
                      onClick={() => setRenaming({ id: view.id, draft: view.name })}
                    >
                      {t("explorer.viewRename")}
                    </button>
                    <button
                      type="button"
                      className="explorer-link explorer-link--quiet"
                      onClick={() => onDelete(view.id)}
                    >
                      {t("explorer.viewDelete")}
                    </button>
                  </div>
                )}
                {missing > 0 ? (
                  <p className="explorer-views__missing" role="status">
                    {missing === 1
                      ? t("explorer.viewMissingOne")
                      : t("explorer.viewMissingMany", { count: missing })}
                  </p>
                ) : null}
              </li>
            );
          })}
        </ul>
      </div>

      <form className="explorer-filters__block" onSubmit={submitSave}>
        <label className="explorer-filters__title" htmlFor={saveId}>
          {t("explorer.viewSaveTitle")}
        </label>
        <div className="explorer-views__row">
          <input
            id={saveId}
            type="text"
            className="explorer-filters__field"
            value={name}
            maxLength={MAX_VIEW_NAME}
            placeholder={t("explorer.viewSavePlaceholder")}
            disabled={empty || full}
            spellCheck={false}
            autoComplete="off"
            onChange={(event) => setName(event.target.value)}
          />
          <button type="submit" className="button button--compact button--primary" disabled={!canSave}>
            {t("explorer.viewSave")}
          </button>
        </div>
        {empty ? <p className="explorer-filters__none">{t("explorer.viewSaveEmpty")}</p> : null}
        {full ? (
          <p className="explorer-filters__none">{t("explorer.viewSaveFull", { count: MAX_SAVED_VIEWS })}</p>
        ) : null}
        {problem === "taken" ? (
          <p className="explorer-filters__none" role="status">
            {t("explorer.viewSaveTaken")}
          </p>
        ) : null}
      </form>
    </div>
  );
}
