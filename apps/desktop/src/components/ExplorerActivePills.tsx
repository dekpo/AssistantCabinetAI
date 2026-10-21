import { useTranslation } from "../i18n/I18nProvider";
import { NO_FACTS, type KnowledgeFacts } from "../lib/explorerFacts";
import { folderOfKey, type ActivePill, type FolderChoice } from "../lib/explorerFilters";
import { businessTypeLabel, entityChoiceLabel } from "../lib/explorerLabels";
import type { ExplorerState } from "../lib/explorerModel";
import { useFolderLabel } from "./ExplorerFilterPanel";

/**
 * The filters in force, as removable pills under the segmented control, with a « Tout effacer » link
 * when there are two or more. Nothing here changes the selection. A pill that comes from the search
 * text (a typed extension) is removed by taking the extension out of the text.
 */
export function ExplorerActivePills({
  pills,
  folders,
  foldersKnown = false,
  facts = NO_FACTS,
  factsLoaded = false,
  onRemove,
  onClear,
}: {
  pills: readonly ActivePill[];
  /** The folder choices, to say a folder with the root when both roots hold one of that name. */
  folders: readonly FolderChoice[];
  /** The folders have been read: a folder choice that is not among them says it is not found. */
  foldersKnown?: boolean;
  /** The knowledge-base facts, for the names of the people and organisations. */
  facts?: KnowledgeFacts;
  /** The facts have been read: a name that is not among them says it is not found. */
  factsLoaded?: boolean;
  onRemove: (pill: ActivePill) => void;
  onClear: () => void;
}) {
  const { t } = useTranslation();
  const folderLabel = useFolderLabel();
  if (pills.length === 0) {
    return null;
  }
  const labelOf = (pill: ActivePill): string => {
    switch (pill.kind) {
      case "format":
        return `.${pill.value}`;
      case "state":
        return t(`explorer.state.${pill.value as ExplorerState}`);
      case "folder": {
        const choice = folders.find((candidate) => candidate.key === pill.value);
        if (choice !== undefined) {
          return folderLabel(choice);
        }
        return foldersKnown
          ? t("explorer.folderMissing", { folder: folderOfKey(pill.value) })
          : folderOfKey(pill.value);
      }
      case "person":
      case "organisation":
        return entityChoiceLabel(t, pill.value, facts, factsLoaded);
      case "type":
        return businessTypeLabel(t, pill.value);
      case "year":
        return pill.value;
      case "conversation":
        return t("explorer.pillConversation");
      case "recent":
        return t("explorer.pillRecent");
    }
  };
  return (
    <div className="explorer-pills" role="group" aria-label={t("explorer.activeFilters")}>
      {pills.map((pill) => {
        const label = labelOf(pill);
        return (
          <span key={`${pill.kind}:${pill.value}`} className="explorer-pill" title={label}>
            <span className="explorer-pill__label">{label}</span>
            <button
              type="button"
              className="explorer-pill__remove"
              aria-label={t("explorer.removeFilter", { filter: label })}
              onClick={() => onRemove(pill)}
            >
              <svg viewBox="0 0 10 10" width="8" height="8" aria-hidden="true">
                <path d="M2 2l6 6M8 2l-6 6" stroke="currentColor" strokeWidth="1.4" fill="none" />
              </svg>
            </button>
          </span>
        );
      })}
      {pills.length >= 2 ? (
        <button type="button" className="explorer-link explorer-link--quiet" onClick={onClear}>
          {t("explorer.clearAll")}
        </button>
      ) : null}
    </div>
  );
}
