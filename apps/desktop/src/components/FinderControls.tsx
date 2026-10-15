import { useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { facetChips, hasFilters, type FacetGroup } from "../lib/finderFilters";
import { foundNotAnalysed, selectedBySearch } from "../lib/finderField";
import type { FileRecord } from "../lib/ipc";
import { counted } from "../lib/plural";
import type { FinderSearch } from "../state/useFinderSearch";
import { BusinessTypesDialog } from "./BusinessTypesDialog";

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
  const [editingTypes, setEditingTypes] = useState(false);
  const active = search.found !== null;
  const chips = search.facets === null ? [] : facetChips(t, search.facets, search.filters, search.userTypeLabels);
  const groups: FacetGroup[] = ["type", "year", "format", "state"];
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
          {chips.length === 0 && !hasFilters(search.filters) ? null : (
            <div className="finder__filters" aria-label={t("finder.filters")}>
              {groups.map((group) => {
                const mine = chips.filter((chip) => chip.group === group);
                if (mine.length === 0) {
                  return null;
                }
                return (
                  <p key={group} className="finder__group">
                    <span className="finder__group-name">
                      {t(`finder.filter${group.charAt(0).toUpperCase()}${group.slice(1)}`)}
                    </span>
                    {mine.map((chip) => (
                      <button
                        key={chip.value}
                        type="button"
                        className={`finder__chip${chip.active ? " finder__chip--active" : ""}`}
                        aria-pressed={chip.active}
                        disabled={disabled}
                        onClick={() => search.toggle(chip.group, chip.value)}
                      >
                        {chip.label} ({chip.count})
                      </button>
                    ))}
                  </p>
                );
              })}
            </div>
          )}
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
      <button
        type="button"
        className="button button--quiet button--compact"
        onClick={() => setEditingTypes(true)}
      >
        {t("businessTypes.open")}
      </button>
      {editingTypes ? <BusinessTypesDialog onClose={() => setEditingTypes(false)} /> : null}
    </div>
  );
}
