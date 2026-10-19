import { useTranslation } from "../i18n/I18nProvider";
import type { ExplorerEntry } from "../lib/explorerModel";
import { StateGlyph } from "./ExplorerGlyphs";

/**
 * One file of the explorer: a checkbox, the state glyph, the stem with an ellipsis and the extension
 * as a badge that is never cut, the grey sub-folder when two files share a name, a tag on a file that
 * sits in the other family's folder, and the "Open" and "See" buttons that show on hover and focus.
 * The full path is the row's tooltip. The row's height is the list's (`--explorer-row`): rows have a
 * fixed height so the list can draw only the ones in view.
 */
export function ExplorerFileRow({
  entry,
  checked,
  checkDisabled,
  onToggle,
  hint,
  onOpen,
  onReveal,
}: {
  entry: ExplorerEntry;
  checked: boolean;
  /** A question is being answered, or the file cannot be ticked (not analysed, or not its folder). */
  checkDisabled: boolean;
  onToggle: () => void;
  /** The grey sub-folder shown beside a name two files share; empty for none. */
  hint: string;
  /** Present only where opening is allowed: an analysed file of its own folder. */
  onOpen?: () => void;
  /** Present only where showing the file in the file manager is allowed. */
  onReveal?: () => void;
}) {
  const { t } = useTranslation();
  const stateText = t(`explorer.state.${entry.state}`);
  const stateLabel = entry.native ? stateText : t("explorer.foreignState");
  const tag = entry.native
    ? null
    : entry.root === "documents"
      ? t("explorer.fromDocs")
      : t("explorer.fromData");

  return (
    <li
      className={
        entry.native ? "explorer-row" : "explorer-row explorer-row--foreign"
      }
      title={entry.path}
    >
      <input
        type="checkbox"
        className="explorer-row__check"
        aria-label={t("explorer.useFile", { name: entry.name })}
        checked={checked}
        disabled={checkDisabled}
        onChange={onToggle}
      />
      <StateGlyph state={entry.state} label={stateLabel} />
      <span className="explorer-row__name">
        <span className="explorer-row__stem">{entry.stem}</span>
        {entry.extension === "" ? null : (
          <span className="explorer-row__ext">{`.${entry.extension}`}</span>
        )}
        {hint === "" ? null : (
          <span className="explorer-row__hint" title={entry.folder}>
            {hint}
          </span>
        )}
        {tag === null ? null : (
          <span className="explorer-row__tag" title={t("explorer.foreignHint")}>
            {tag}
          </span>
        )}
      </span>
      <span className="explorer-row__actions">
        {onOpen === undefined ? null : (
          <button
            type="button"
            className="explorer-row__button"
            title={t("viewer.openHint")}
            onClick={onOpen}
          >
            {t("viewer.open")}
          </button>
        )}
        {onReveal === undefined ? null : (
          <button
            type="button"
            className="explorer-row__button"
            title={t("workFolder.revealFileHint")}
            onClick={onReveal}
          >
            {t("workFolder.reveal")}
          </button>
        )}
      </span>
    </li>
  );
}
