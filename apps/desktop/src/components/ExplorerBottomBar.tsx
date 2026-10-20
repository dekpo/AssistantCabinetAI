import { useTranslation } from "../i18n/I18nProvider";
import type { SelectionNote } from "../lib/explorerSelection";

/**
 * The bottom line of the explorer: how many files each side uses, the two small links that change
 * the selection by an explicit act ("Cocher les N" adds what the view shows, "Décocher" empties it),
 * and, only when there is something to say, a second line with the short warnings. Light background
 * and a top border like the header; no large button.
 */
export function ExplorerBottomBar({
  documentsUsed,
  tablesUsed,
  toTick,
  onTick,
  canClear,
  onClear,
  locked,
  notes,
  unsupported,
}: {
  documentsUsed: number;
  tablesUsed: number;
  /** How many shown files "Cocher" would add; zero hides the link. */
  toTick: number;
  onTick: () => void;
  /** Something is used on at least one side. */
  canClear: boolean;
  onClear: () => void;
  /** A question is being answered: the selection must not change under it. */
  locked: boolean;
  notes: readonly SelectionNote[];
  /** Files no pipeline reads, said once, never listed. */
  unsupported: number;
}) {
  const { t } = useTranslation();
  /* « Aucun fichier pour l'IA » is short enough to sit on the first line beside « aucun » (two lines
     in all, so the box never grows a third and no scroll bar appears); the whole-family warnings
     keep their own line. */
  const noteKey: Record<Exclude<SelectionNote, "none">, string> = {
    wholeDocuments: "explorer.noteWholeDocuments",
    wholeTables: "explorer.noteWholeTables",
  };

  return (
    <div className="explorer-bottom">
      <div className="explorer-bottom__line">
        <strong className="explorer-bottom__label">{t("explorer.used")}</strong>
        <span className="explorer-bottom__count">
          {documentsUsed + tablesUsed === 0
            ? t("explorer.usedNone")
            : t("explorer.usedCount", { documents: documentsUsed, tables: tablesUsed })}
        </span>
        {notes.includes("none") ? (
          <span className="explorer-bottom__hint">{t("explorer.noteNone")}</span>
        ) : null}
        {toTick > 0 ? (
          <button type="button" className="explorer-link" disabled={locked} onClick={onTick}>
            {toTick === 1 ? t("explorer.tickOne") : t("explorer.tickMany", { count: toTick })}
          </button>
        ) : null}
        {canClear ? (
          <button
            type="button"
            className="explorer-link explorer-link--quiet"
            disabled={locked}
            onClick={onClear}
          >
            {t("explorer.untick")}
          </button>
        ) : null}
      </div>
      {notes.flatMap((note) =>
        note === "none"
          ? []
          : [
              <p key={note} className="explorer-bottom__note">
                {t(noteKey[note])}
              </p>,
            ],
      )}
      {unsupported > 0 ? (
        <p className="explorer-bottom__note">
          {t(unsupported > 1 ? "explorer.unsupportedMany" : "explorer.unsupportedOne", {
            count: unsupported,
          })}
        </p>
      ) : null}
    </div>
  );
}
