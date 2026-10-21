import { useId, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import type { EntityKind } from "../lib/explorerFacts";
import {
  chosenEntityIds,
  suggestEntities,
  type EntityOption,
  type FilterKind,
  type TypeChoice,
  type YearChoice,
} from "../lib/explorerFilters";
import { businessTypeLabel, roleChoiceLabel } from "../lib/explorerLabels";
import { parseEntityChoice } from "../lib/explorerModel";

/**
 * The suggestions of a field of names: at most five, with their file counts, and the number of those not
 * listed. Like the folder field, the buttons swallow the mouse-down so that the list does not close
 * before the click lands.
 */
function EntitySuggestionList({
  options,
  typed,
  chosen,
  onPick,
}: {
  options: readonly EntityOption[];
  typed: string;
  chosen: ReadonlySet<number>;
  onPick: (option: EntityOption) => void;
}) {
  const { t } = useTranslation();
  const { shown, more } = suggestEntities(options, typed, chosen);
  return (
    <div className="explorer-suggest" role="listbox" aria-label={t("explorer.nameSuggestions")}>
      {shown.map((option) => (
        <button
          key={option.entityId}
          type="button"
          role="option"
          aria-selected={false}
          className="explorer-suggest__item"
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => onPick(option)}
        >
          <span className="explorer-suggest__name">{option.name}</span>
          <small className="explorer-suggest__count">{option.count}</small>
        </button>
      ))}
      {more > 0 ? (
        <p className="explorer-suggest__more">
          {more === 1 ? t("explorer.nameMoreOne") : t("explorer.nameMoreMany", { count: more })}
        </p>
      ) : null}
      {shown.length === 0 && more === 0 ? (
        <p className="explorer-suggest__more">{t("explorer.nameNone")}</p>
      ) : null}
    </div>
  );
}

/**
 * One field of names (people, or organisations): a typed field that offers its suggestions only while it
 * has the focus, the names chosen as removable chips, and the second step, a role, for a chosen name that
 * has several. A chosen name the base no longer knows says so and can be removed.
 */
function EntityField({
  kind,
  title,
  options,
  chosen,
  disabled,
  onToggle,
  onRole,
}: {
  kind: EntityKind;
  title: string;
  options: readonly EntityOption[];
  chosen: ReadonlySet<string>;
  disabled: boolean;
  onToggle: (kind: FilterKind, value: string) => void;
  onRole: (kind: EntityKind, entityId: number, role: string | null) => void;
}) {
  const { t } = useTranslation();
  const fieldId = useId();
  const [typed, setTyped] = useState("");
  const [focused, setFocused] = useState(false);
  const known = options.filter((option) => option.name !== null);
  return (
    <div className="explorer-filters__block">
      <label className="explorer-filters__title" htmlFor={fieldId}>
        {title}
      </label>
      <input
        id={fieldId}
        type="search"
        className="explorer-filters__field"
        value={typed}
        placeholder={t("explorer.namePlaceholder")}
        disabled={disabled || known.length === 0}
        spellCheck={false}
        autoComplete="off"
        onChange={(event) => setTyped(event.target.value)}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
      />
      {focused ? (
        <EntitySuggestionList
          options={options}
          typed={typed}
          chosen={chosenEntityIds(chosen)}
          onPick={(option) => {
            onToggle(kind, String(option.entityId));
            setTyped("");
          }}
        />
      ) : null}
      {[...chosen].map((value) => {
        const { entityId, role } = parseEntityChoice(value);
        const option = options.find((candidate) => candidate.entityId === entityId);
        const roles = (option?.roles ?? []).filter((each) => roleChoiceLabel(t, each.role) !== null);
        return (
          <div key={value} className="explorer-chips explorer-chips--entity">
            <button
              type="button"
              className="explorer-chip"
              aria-pressed="true"
              title={t("explorer.removeName")}
              onClick={() => onToggle(kind, value)}
            >
              {option?.name ?? t("explorer.nameMissing")}
              <small className="explorer-chip__count">{option?.count ?? 0}</small>
            </button>
            {roles.length >= 2 || role !== null ? (
              <>
                <button
                  type="button"
                  className="explorer-chip explorer-chip--role"
                  aria-pressed={role === null}
                  onClick={() => onRole(kind, entityId, null)}
                >
                  {t("explorer.roleAny")}
                </button>
                {roles.map((each) => (
                  <button
                    key={each.role}
                    type="button"
                    className="explorer-chip explorer-chip--role"
                    aria-pressed={role === each.role}
                    onClick={() => onRole(kind, entityId, each.role)}
                  >
                    {roleChoiceLabel(t, each.role)}
                    <small className="explorer-chip__count">{each.count}</small>
                  </button>
                ))}
              </>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}

/**
 * The filters of the knowledge base in the « Filtres » panel: business type and year as chips with their
 * counts, people and organisations as typed fields. Every count is worked out by the explorer over what
 * the other filters leave. Nothing here knows the selection.
 */
export function ExplorerKnowledgeFilters({
  loaded,
  types,
  chosenTypes,
  years,
  chosenYears,
  people,
  chosenPeople,
  organisations,
  chosenOrganisations,
  disabled,
  onToggle,
  onRole,
}: {
  /** The facts have been read. */
  loaded: boolean;
  types: readonly TypeChoice[];
  chosenTypes: ReadonlySet<string>;
  years: readonly YearChoice[];
  chosenYears: ReadonlySet<string>;
  people: readonly EntityOption[];
  chosenPeople: ReadonlySet<string>;
  organisations: readonly EntityOption[];
  chosenOrganisations: ReadonlySet<string>;
  disabled: boolean;
  onToggle: (kind: FilterKind, value: string) => void;
  onRole: (kind: EntityKind, entityId: number, role: string | null) => void;
}) {
  const { t } = useTranslation();
  const nothing =
    types.length === 0 && years.length === 0 && people.length === 0 && organisations.length === 0;
  if (!loaded) {
    return (
      <p className="explorer-filters__none" role="status">
        {t("explorer.kbReading")}
      </p>
    );
  }
  if (nothing) {
    return <p className="explorer-filters__none">{t("explorer.kbNone")}</p>;
  }
  return (
    <>
      {types.length === 0 ? null : (
        <div className="explorer-filters__block">
          <h3 className="explorer-filters__title">{t("explorer.filterType")}</h3>
          <div className="explorer-chips">
            {types.map(({ code, count }) => {
              const lit = chosenTypes.has(code);
              return (
                <button
                  key={code}
                  type="button"
                  className="explorer-chip"
                  aria-pressed={lit}
                  disabled={disabled}
                  data-empty={count === 0 && !lit ? "true" : undefined}
                  onClick={() => onToggle("type", code)}
                >
                  {businessTypeLabel(t, code)}
                  <small className="explorer-chip__count">{count}</small>
                </button>
              );
            })}
          </div>
        </div>
      )}
      {years.length === 0 ? null : (
        <div className="explorer-filters__block">
          <h3 className="explorer-filters__title">{t("explorer.filterYear")}</h3>
          <div className="explorer-chips">
            {years.map(({ year, count }) => {
              const lit = chosenYears.has(year);
              return (
                <button
                  key={year}
                  type="button"
                  className="explorer-chip explorer-chip--mono"
                  aria-pressed={lit}
                  disabled={disabled}
                  data-empty={count === 0 && !lit ? "true" : undefined}
                  onClick={() => onToggle("year", year)}
                >
                  {year}
                  <small className="explorer-chip__count">{count}</small>
                </button>
              );
            })}
          </div>
        </div>
      )}
      {people.length === 0 && chosenPeople.size === 0 ? null : (
        <EntityField
          kind="person"
          title={t("explorer.filterPeople")}
          options={people}
          chosen={chosenPeople}
          disabled={disabled}
          onToggle={onToggle}
          onRole={onRole}
        />
      )}
      {organisations.length === 0 && chosenOrganisations.size === 0 ? null : (
        <EntityField
          kind="organisation"
          title={t("explorer.filterOrganisations")}
          options={organisations}
          chosen={chosenOrganisations}
          disabled={disabled}
          onToggle={onToggle}
          onRole={onRole}
        />
      )}
    </>
  );
}
