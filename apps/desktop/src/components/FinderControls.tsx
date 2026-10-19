import { useTranslation } from "../i18n/I18nProvider";
import { addScopeFiles } from "../lib/analysisScope";
import { foundNotAnalysed, resultsToTick } from "../lib/finderField";
import type { AnalysisScope, FileRecord } from "../lib/ipc";
import { counted } from "../lib/plural";
import type { FinderSearch } from "../state/useFinderSearch";

/**
 * The search field of a folder card, with what the search did under it: how many files it found, how
 * many of them cannot be ticked (not analysed yet), the button that adds the analysed ones to the
 * selection, and the two controls that end or widen the view. A search never changes the selection by
 * itself: only the button does. The same markup in the Documents and the Data cards.
 */
export function FinderControls({
  search,
  analysed,
  scope,
  onScopeChange,
  placeholder,
  disabled,
}: {
  search: FinderSearch;
  /** The analysed files of this folder: the ones the results can be ticked among. */
  analysed: FileRecord[];
  scope: AnalysisScope;
  onScopeChange: (scope: AnalysisScope) => void;
  placeholder: string;
  /** A question is being answered: changing the selection under it would describe another answer. */
  disabled: boolean;
}) {
  const { t } = useTranslation();
  const active = search.found !== null;
  const found = search.found ?? [];
  const toTick = resultsToTick(found, analysed, scope);
  const notAnalysed = foundNotAnalysed(found, analysed);

  return (
    <div className="finder">
      <input
        type="search"
        className="finder__input"
        value={search.text}
        placeholder={placeholder}
        aria-label={placeholder}
        disabled={disabled}
        spellCheck={false}
        onChange={(event) => search.setText(event.target.value)}
      />
      {search.failed ? <p className="finder__status">{t("finder.failed")}</p> : null}
      {active ? (
        <>
          <p className="finder__status" role="status">
            {found.length === 0
              ? t("finder.none", { query: search.text.trim() })
              : t("finder.found", {
                  files: counted(found.length, t("finder.foundOne"), t("finder.foundMany")),
                  query: search.text.trim(),
                })}
            {notAnalysed > 0
              ? ` ${t("finder.notAnalysed", {
                  count: counted(
                    notAnalysed,
                    t("finder.notAnalysedOne"),
                    t("finder.notAnalysedMany"),
                  ),
                })}`
              : ""}
          </p>
          {toTick.length > 0 ? (
            <div className="finder__actions">
              <button
                type="button"
                className="button button--compact"
                disabled={disabled}
                onClick={() => onScopeChange(addScopeFiles(scope, toTick, analysed, Date.now()))}
              >
                {toTick.length === 1
                  ? t("finder.tickOne")
                  : t("finder.tickMany", { count: toTick.length })}
              </button>
            </div>
          ) : null}
          <div className="finder__actions">
            <button
              type="button"
              className="button button--quiet button--compact"
              onClick={() => search.setShowAll(!search.showAll)}
            >
              {search.showAll ? t("finder.showFound") : t("finder.showAll")}
            </button>
            <button
              type="button"
              className="button button--quiet button--compact"
              onClick={search.reset}
            >
              {t("finder.reset")}
            </button>
          </div>
        </>
      ) : null}
    </div>
  );
}
