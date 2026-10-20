import { useId, useState, type Ref } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import {
  STATE_CHOICES,
  suggestFolders,
  type FolderChoice,
  type FormatChoice,
  type FilterKind,
} from "../lib/explorerFilters";
import type { ExplorerState } from "../lib/explorerModel";

/** The name of a folder choice; the root is said only when the two roots hold a folder of this name. */
export function useFolderLabel(): (choice: Pick<FolderChoice, "folder" | "root" | "ambiguous">) => string {
  const { t } = useTranslation();
  return (choice) =>
    choice.ambiguous
      ? t("explorer.folderIn", {
          folder: choice.folder,
          root: t(choice.root === "documents" ? "explorer.rootDocuments" : "explorer.rootData"),
        })
      : choice.folder;
}

/**
 * The suggestions of the origin-folder field: at most five folders with their file counts, and the
 * number of those not listed. Pressing a suggestion must not take the focus off the field, or the
 * list would close before the click lands, so the buttons swallow the mouse-down.
 */
export function FolderSuggestionList({
  choices,
  typed,
  chosen,
  onPick,
}: {
  choices: readonly FolderChoice[];
  typed: string;
  chosen: ReadonlySet<string>;
  onPick: (choice: FolderChoice) => void;
}) {
  const { t } = useTranslation();
  const label = useFolderLabel();
  const { shown, more } = suggestFolders(choices, typed, chosen);
  return (
    <div className="explorer-suggest" role="listbox" aria-label={t("explorer.folderSuggestions")}>
      {shown.map((choice) => (
        <button
          key={choice.key}
          type="button"
          role="option"
          aria-selected={false}
          className="explorer-suggest__item"
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => onPick(choice)}
        >
          <span className="explorer-suggest__name">{label(choice)}</span>
          <small className="explorer-suggest__count">{choice.count}</small>
        </button>
      ))}
      {more > 0 ? (
        <p className="explorer-suggest__more">
          {more === 1 ? t("explorer.folderMoreOne") : t("explorer.folderMoreMany", { count: more })}
        </p>
      ) : null}
      {shown.length === 0 && more === 0 ? (
        <p className="explorer-suggest__more">{t("explorer.folderNone")}</p>
      ) : null}
    </div>
  );
}

/**
 * The « Filtres » panel, opened in the flow of the box (the box clips what overflows it, and nothing
 * may be drawn under it). Format and state are pills, always few; the origin folder is a typed field
 * that offers its suggestions only while it has the focus. There is no scroll area: the format pills
 * wrap. The segmented control of the families stays outside, in the box's head.
 *
 * A pill lit by a typed extension is lit from the search's own echo and is pressed to take the
 * extension out of the text; the others empty or fill the box's sets. A filter never touches the
 * selection: nothing here knows it.
 */
export function ExplorerFilterPanel({
  id,
  formats,
  chosenFormats,
  typedFormats,
  stateCounts,
  chosenStates,
  folders,
  chosenFolders,
  disabled,
  anyActive,
  onToggle,
  onRemoveTyped,
  onClear,
  onClose,
  panelRef,
}: {
  id: string;
  formats: readonly FormatChoice[];
  chosenFormats: ReadonlySet<string>;
  typedFormats: readonly string[];
  stateCounts: Readonly<Record<ExplorerState, number>>;
  chosenStates: ReadonlySet<ExplorerState>;
  folders: readonly FolderChoice[];
  chosenFolders: ReadonlySet<string>;
  /** Nothing to search or filter yet. */
  disabled: boolean;
  anyActive: boolean;
  onToggle: (kind: FilterKind, value: string) => void;
  onRemoveTyped: (extension: string) => void;
  onClear: () => void;
  onClose: () => void;
  panelRef?: Ref<HTMLDivElement>;
}) {
  const { t } = useTranslation();
  const fieldId = useId();
  const [typed, setTyped] = useState("");
  const [focused, setFocused] = useState(false);

  return (
    <div className="explorer-filters" id={id} ref={panelRef} role="group" aria-label={t("explorer.filtersTitle")}>
      <div className="explorer-filters__block">
        <h3 className="explorer-filters__title">{t("explorer.filterFormat")}</h3>
        <div className="explorer-chips">
          {formats.length === 0 ? (
            <p className="explorer-filters__none">{t("explorer.filterNoFormat")}</p>
          ) : null}
          {formats.map(({ extension, count }) => {
            const isTyped = typedFormats.includes(extension);
            const lit = isTyped || chosenFormats.has(extension);
            return (
              <button
                key={extension}
                type="button"
                className="explorer-chip explorer-chip--mono"
                aria-pressed={lit}
                disabled={disabled}
                data-empty={count === 0 && !lit ? "true" : undefined}
                title={isTyped ? t("explorer.typedFormatHint") : undefined}
                onClick={() => (isTyped ? onRemoveTyped(extension) : onToggle("format", extension))}
              >
                {`.${extension}`}
                <small className="explorer-chip__count">{count}</small>
              </button>
            );
          })}
        </div>
      </div>
      <div className="explorer-filters__block">
        <h3 className="explorer-filters__title">{t("explorer.filterState")}</h3>
        <div className="explorer-chips">
          {STATE_CHOICES.map((state) => {
            const lit = chosenStates.has(state);
            return (
              <button
                key={state}
                type="button"
                className="explorer-chip"
                aria-pressed={lit}
                disabled={disabled}
                data-empty={stateCounts[state] === 0 && !lit ? "true" : undefined}
                onClick={() => onToggle("state", state)}
              >
                {t(`explorer.state.${state}`)}
                <small className="explorer-chip__count">{stateCounts[state]}</small>
              </button>
            );
          })}
        </div>
      </div>
      <div className="explorer-filters__block">
        <label className="explorer-filters__title" htmlFor={fieldId}>
          {t("explorer.filterFolder")}
        </label>
        <input
          id={fieldId}
          type="search"
          className="explorer-filters__field"
          value={typed}
          placeholder={t("explorer.folderPlaceholder")}
          disabled={disabled || folders.length === 0}
          spellCheck={false}
          autoComplete="off"
          onChange={(event) => setTyped(event.target.value)}
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
        />
        {focused ? (
          <FolderSuggestionList
            choices={folders}
            typed={typed}
            chosen={chosenFolders}
            onPick={(choice) => {
              onToggle("folder", choice.key);
              setTyped("");
            }}
          />
        ) : null}
        {folders.length === 0 ? (
          <p className="explorer-filters__none">{t("explorer.folderNoSubfolder")}</p>
        ) : null}
      </div>
      <div className="explorer-filters__foot">
        <button
          type="button"
          className="button button--compact"
          disabled={!anyActive}
          onClick={onClear}
        >
          {t("explorer.clearFilters")}
        </button>
        <button type="button" className="button button--compact button--primary" onClick={onClose}>
          {t("explorer.filtersDone")}
        </button>
      </div>
    </div>
  );
}
