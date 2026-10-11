import { useTranslation } from "../i18n/I18nProvider";
import type { Chip } from "../lib/entityPicker";

/**
 * The names chosen for the next question, above the box. Each one narrows that question's search
 * to the files that hold it and goes away after sending, or when she removes it. The text of the
 * question is not touched: a chip is an id, not a sentence.
 */
export function EntityChips({
  chips,
  onRemove,
  onOpenCard,
}: {
  chips: readonly Chip[];
  onRemove: (entityId: number) => void;
  /** Opens the sheet of a name: what the files of the selection say about it. */
  onOpenCard: (chip: Chip) => void;
}) {
  const { t } = useTranslation();
  if (chips.length === 0) {
    return null;
  }
  return (
    <ul className="chips" aria-label={t("chat.chips.label")}>
      {chips.map((chip) => (
        <li className="chip" key={chip.entityId}>
          <span className="chip__name">{chip.display}</span>
          <button
            type="button"
            className="chip__card"
            aria-label={t("entityCard.open", { name: chip.display })}
            title={t("entityCard.open", { name: chip.display })}
            onClick={() => onOpenCard(chip)}
          >
            i
          </button>
          <button
            type="button"
            className="chip__remove"
            aria-label={t("chat.chips.remove", { name: chip.display })}
            title={t("chat.chips.remove", { name: chip.display })}
            onClick={() => onRemove(chip.entityId)}
          >
            ×
          </button>
        </li>
      ))}
    </ul>
  );
}
