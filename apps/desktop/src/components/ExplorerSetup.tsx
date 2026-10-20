import { useTranslation } from "../i18n/I18nProvider";
import type { Family } from "../lib/explorerModel";
import { abbreviateFolderPath } from "../lib/folderPath";
import { FamilyFolderGlyph } from "./ExplorerGlyphs";

/**
 * « Pour commencer »: the three steps, and the one button that makes both suggested folders. Drawn
 * only while no folder at all is chosen. It changes nothing by itself: the button asks the box.
 */
export function ExplorerFirstStart({
  busy,
  canCreate,
  onCreateBoth,
}: {
  busy: boolean;
  /** At least one suggestion is known: without any, the two per-family buttons are all there is. */
  canCreate: boolean;
  onCreateBoth: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="explorer-start">
      <h3 className="explorer-start__title">{t("explorer.startTitle")}</h3>
      <ol className="explorer-start__steps">
        <li>{t("explorer.startStepChoose")}</li>
        <li>{t("explorer.startStepDrop")}</li>
        <li>{t("explorer.startStepAnalyse")}</li>
      </ol>
      {canCreate ? (
        <button
          type="button"
          className="button button--compact button--primary"
          disabled={busy}
          onClick={onCreateBoth}
        >
          {t("explorer.createBoth")}
        </button>
      ) : null}
    </div>
  );
}

/**
 * A family whose folder is not chosen: the family's header with the formats it reads, the suggested
 * folder (its short form on screen, the whole path and the reason it sits outside the user's own
 * Documents on hover), and the two ways to choose, side by side.
 */
export function ExplorerFamilySetup({
  family,
  suggested,
  busy,
  onCreate,
  onChoose,
}: {
  family: Family;
  /** The folder the software proposes, or null when none is known. */
  suggested: string | null;
  busy: boolean;
  onCreate: () => void;
  onChoose: () => void;
}) {
  const { t } = useTranslation();
  const documents = family === "documents";
  const typeLine = t("explorer.typeLine", {
    family: t(documents ? "explorer.familyDocuments" : "explorer.familyTables"),
    types: t(documents ? "explorer.typesDocuments" : "explorer.typesTables"),
  });
  return (
    <section className={`explorer-family explorer-family--${family} explorer-setup`}>
      <div className="explorer-family__head">
        <FamilyFolderGlyph family={family} />
        <span className="explorer-setup__types" title={typeLine}>
          {typeLine}
        </span>
      </div>
      {suggested === null ? null : (
        <p
          className="explorer-setup__path"
          title={`${suggested}\n${t(documents ? "explorer.suggestionWhyDocuments" : "explorer.suggestionWhyTables")}`}
        >
          {abbreviateFolderPath(suggested)}
        </p>
      )}
      <div className="explorer-setup__actions">
        {suggested === null ? null : (
          <button
            type="button"
            className="button button--compact button--primary"
            disabled={busy}
            onClick={onCreate}
          >
            {t("explorer.createAndUse")}
          </button>
        )}
        <button type="button" className="button button--compact" disabled={busy} onClick={onChoose}>
          {t("explorer.chooseFolder")}
        </button>
      </div>
    </section>
  );
}
