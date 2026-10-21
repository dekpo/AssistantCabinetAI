import { useMemo, useState, type CSSProperties } from "react";
import { SETTINGS_LIST_ROWS } from "../config/layout";
import { useTranslation } from "../i18n/I18nProvider";
import { normaliseError, type AppError } from "../lib/errors";
import { countsByFamily, sortEntries, type ExplorerEntry, type Family } from "../lib/explorerModel";
import { folderState } from "../lib/explorerStates";
import { ROW_HEIGHT } from "../lib/explorerWindow";
import {
  cancelAnalysis,
  revealDataFile,
  revealWorkFile,
  type ExplorerDensity,
} from "../lib/ipc";
import { useExplorerFolders } from "../state/useExplorerFolders";
import { useFamilyInventories } from "../state/useFamilyInventories";
import type { IndexingState } from "../state/useIndexing";
import { useNow } from "../state/useNow";
import { useProfileStatus } from "../state/useProfileStatus";
import { ErrorBanner } from "./ErrorBanner";
import { ExplorerClearDialog } from "./ExplorerClearDialog";
import { ExplorerFamilySection } from "./ExplorerFamilySection";
import { ExplorerFamilySetup } from "./ExplorerSetup";

/**
 * The two folders of the Settings panel, one whole row each, with a title: the sidebar's family
 * sections (`ExplorerFamilySection`), read from the same two inventories and driven by the same folder commands and the same passes,
 * in their non-selectable mode - no checkbox, no search, no filter, no fold. Choosing a folder, showing
 * it, changing it, Analyser with its gauge and Stop, and clearing the analysis all work here exactly as
 * in the sidebar; what is ticked for the conversation is the sidebar's business alone.
 */
export function FolderSettings({
  workFolder,
  dataFolder,
  suggestedWorkFolder,
  suggestedDataFolder,
  onWorkFolderChosen,
  onDataFolderChosen,
  indexing,
  dataIndexing,
  density,
  onCleared,
  onOpenReport,
}: {
  workFolder: string | null;
  dataFolder: string | null;
  suggestedWorkFolder: string | null;
  suggestedDataFolder: string | null;
  /** The same two guarded changes of `App` the sidebar's box calls. */
  onWorkFolderChosen: (path: string) => void;
  onDataFolderChosen: (path: string) => void;
  /** The documents' pass and the workbooks' pass: the very ones the sidebar starts. */
  indexing: IndexingState;
  dataIndexing: IndexingState;
  density: ExplorerDensity;
  /** An analysis was cleared: the sidebar's box reads its folders again. */
  onCleared: (family: Family) => void;
  /** « Voir le rapport »: closes the panel and opens the centre on its overview. */
  onOpenReport: () => void;
}) {
  const { t } = useTranslation();
  const [error, setError] = useState<AppError | null>(null);
  const [confirmingClear, setConfirmingClear] = useState<Family | null>(null);
  const folders = useExplorerFolders({
    indexing,
    dataIndexing,
    onWorkFolderChosen,
    onDataFolderChosen,
    onError: setError,
    onCleared,
  });
  const now = useNow(indexing.running || dataIndexing.running);
  const { documentsInventory, dataInventory, merged } = useFamilyInventories({
    workFolder,
    dataFolder,
    documentsPasses: indexing.finishedPasses,
    dataPasses: dataIndexing.finishedPasses,
    notesToken: 0,
    documentsClears: folders.clears.documents,
    tablesClears: folders.clears.tables,
  });
  const { entries } = merged;
  const counts = useMemo(() => countsByFamily(entries), [entries]);
  const listed = useMemo(
    () => ({
      documents: sortEntries(
        entries.filter((entry) => entry.family === "documents"),
        "path",
      ),
      tables: sortEntries(
        entries.filter((entry) => entry.family === "tables"),
        "path",
      ),
    }),
    [entries],
  );
  const documentsState = folderState({
    folder: workFolder,
    loaded: documentsInventory !== null,
    folderFiles: documentsInventory?.files.length ?? 0,
    familyFiles: counts.documents.total,
  });
  const tablesState = folderState({
    folder: dataFolder,
    loaded: dataInventory !== null,
    folderFiles: dataInventory?.files.length ?? 0,
    familyFiles: counts.tables.total,
  });
  const documentsProfile = useProfileStatus("documents", workFolder, indexing.finishedPasses);
  const dataProfile = useProfileStatus("data", dataFolder, dataIndexing.finishedPasses);

  const reveal = async (entry: ExplorerEntry) => {
    setError(null);
    try {
      await (entry.root === "documents" ? revealWorkFile(entry.path) : revealDataFile(entry.path));
    } catch (raw: unknown) {
      setError(normaliseError(raw));
    }
  };

  return (
    <>
      <div className="settings-wide settings-folder">
        <span className="field__label">{t("settings.documentsFolderLabel")}</span>
        <section
          className="explorer"
          aria-label={t("settings.documentsFolderLabel")}
          style={{ "--explorer-row": `${ROW_HEIGHT[density]}px` } as CSSProperties}
        >
          {workFolder === null ? (
            <ExplorerFamilySetup
              family="documents"
              suggested={suggestedWorkFolder}
              busy={folders.busy}
              onCreate={() => void folders.createSuggested("documents")}
              onChoose={() => void folders.choose("documents")}
            />
          ) : (
            <ExplorerFamilySection
              family="documents"
              folder={workFolder}
              isEmpty={documentsState === "empty"}
              folderActions={{
                reveal: () => void folders.reveal("documents"),
                change: () => void folders.choose("documents"),
                clear: () => setConfirmingClear("documents"),
              }}
              counts={counts.documents}
              entries={listed.documents}
              allEntries={entries}
              open
              rows={SETTINGS_LIST_ROWS}
              rowHeight={ROW_HEIGHT[density]}
              indexing={indexing}
              now={now}
              onStop={() => void cancelAnalysis("documents")}
              onRevealFile={(entry) => void reveal(entry)}
              needsAnalyse={documentsProfile === "needsAnalyse"}
              onOpenReport={onOpenReport}
              filtering={false}
              detailed
            />
          )}
        </section>
      </div>
      <div className="settings-wide settings-folder">
        <span className="field__label">{t("settings.tablesFolderLabel")}</span>
        <section
          className="explorer"
          aria-label={t("settings.tablesFolderLabel")}
          style={{ "--explorer-row": `${ROW_HEIGHT[density]}px` } as CSSProperties}
        >
          {dataFolder === null ? (
            <ExplorerFamilySetup
              family="tables"
              suggested={suggestedDataFolder}
              busy={folders.busy}
              onCreate={() => void folders.createSuggested("tables")}
              onChoose={() => void folders.choose("tables")}
            />
          ) : (
            <ExplorerFamilySection
              family="tables"
              folder={dataFolder}
              isEmpty={tablesState === "empty"}
              folderActions={{
                reveal: () => void folders.reveal("tables"),
                change: () => void folders.choose("tables"),
                clear: () => setConfirmingClear("tables"),
              }}
              counts={counts.tables}
              entries={listed.tables}
              allEntries={entries}
              open
              rows={SETTINGS_LIST_ROWS}
              rowHeight={ROW_HEIGHT[density]}
              indexing={dataIndexing}
              now={now}
              onStop={() => void cancelAnalysis("data")}
              onRevealFile={(entry) => void reveal(entry)}
              needsAnalyse={dataProfile === "needsAnalyse"}
              onOpenReport={onOpenReport}
              filtering={false}
              detailed
            />
          )}
        </section>
      </div>

      {error === null ? null : <ErrorBanner error={error} />}
      {indexing.error === null ? null : <ErrorBanner error={indexing.error} />}
      {dataIndexing.error === null ? null : <ErrorBanner error={dataIndexing.error} />}

      {confirmingClear === null ? null : (
        <ExplorerClearDialog
          family={confirmingClear}
          busy={folders.resetting}
          onConfirm={() => void folders.clear(confirmingClear).then(() => setConfirmingClear(null))}
          onCancel={() => setConfirmingClear(null)}
        />
      )}
    </>
  );
}
