import { useTranslation } from "../i18n/I18nProvider";
import { foundNotAnalysed, selectedBySearch } from "../lib/finderField";
import type { FileRecord } from "../lib/ipc";
import { counted } from "../lib/plural";
import type { FinderSearch } from "../state/useFinderSearch";

/**
 * The search field of a folder card, with what the search did under it: how many files it selected,
 * how many it found without being able to select them (not analysed yet), and the two controls that
 * end or widen the view. The same markup in the Documents and the Data cards.
 */
export function FinderControls({
  search,
  analysed,
  placeholder,
  disabled,
}: {
  search: FinderSearch;
  /** The analysed files of this folder: the ones a search can select. */
  analysed: FileRecord[];
  placeholder: string;
  /** A question is being answered: changing the selection under it would describe another answer. */
  disabled: boolean;
}) {
  const { t } = useTranslation();
  const active = search.found !== null;
  const found = search.found ?? [];
  const selected = selectedBySearch(found, analysed);
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
              : t("finder.selected", {
                  files: counted(selected, t("chat.scopeFileOne"), t("chat.scopeFileMany")),
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
