import { useTranslation } from "../i18n/I18nProvider";
import { codeLabel } from "../lib/diagnostics";
import { outsideFileCount } from "../lib/entityPicker";
import type { EntityChoice, OutsideSelection } from "../lib/ipc";
import { counted } from "../lib/plural";

/**
 * The list under the composer while a `@name` is being typed: the names of the current selection
 * that begin with the letters so far, each with the number of selected files that hold it, and one
 * clearly labelled line for a name that only exists in files she did not select.
 *
 * Drawn from `lib/entityPicker.ts` and `state/useEntitySearch.ts`; the keys are handled by the
 * composer, which owns the focus. A mouse press is taken on `mousedown` and its default prevented,
 * so choosing a name never takes the focus (and the caret) away from the question being typed.
 */
export function EntityPicker({
  id,
  prefix,
  choices,
  outside,
  highlighted,
  searching,
  onChoose,
  onHighlight,
  onAddToSelection,
}: {
  /** The id of the list, so the box can name it as the thing it controls. */
  id: string;
  prefix: string;
  choices: readonly EntityChoice[];
  outside: readonly OutsideSelection[];
  highlighted: number;
  searching: boolean;
  onChoose: (choice: EntityChoice) => void;
  onHighlight: (index: number) => void;
  onAddToSelection: (outside: OutsideSelection) => void;
}) {
  const { t } = useTranslation();
  const nothingYet = prefix.length === 0;
  const empty = choices.length === 0 && outside.length === 0;

  return (
    <div className="picker">
      {choices.length > 0 ? (
        <ul className="picker__list" role="listbox" id={id} aria-label={t("chat.picker.label")}>
          {choices.map((choice, index) => (
            <li
              key={choice.entityId}
              id={`${id}-${index}`}
              role="option"
              aria-selected={index === highlighted}
              className={
                index === highlighted ? "picker__item picker__item--active" : "picker__item"
              }
              onMouseEnter={() => onHighlight(index)}
              onMouseDown={(event) => {
                event.preventDefault();
                onChoose(choice);
              }}
            >
              <span className="picker__name">{choice.display}</span>
              <span className="picker__meta">
                {codeLabel(t, "chat.picker.type", choice.typeId)}
                {" · "}
                {counted(
                  choice.documents + choice.workbooks,
                  t("chat.scopeFileOne"),
                  t("chat.scopeFileMany"),
                )}
              </span>
            </li>
          ))}
        </ul>
      ) : null}
      {empty ? (
        <p className="picker__hint">
          {t(
            nothingYet
              ? "chat.picker.typeMore"
              : searching
                ? "chat.picker.searching"
                : "chat.picker.noMatch",
          )}
        </p>
      ) : null}
      {outside.map((each) => (
        <p className="picker__outside" key={each.entityId}>
          <span>
            {t("chat.picker.outside", {
              name: each.display,
              files: counted(
                outsideFileCount(each),
                t("chat.picker.outsideFileOne"),
                t("chat.picker.outsideFileMany"),
              ),
            })}
          </span>
          <button
            type="button"
            className="button button--compact"
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => onAddToSelection(each)}
          >
            {t("chat.picker.addToSelection")}
          </button>
        </p>
      ))}
    </div>
  );
}
