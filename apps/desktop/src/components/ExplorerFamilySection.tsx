import { Fragment } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { analysisFraction, vectorPercent } from "../lib/analysis";
import {
  duplicateNames,
  folderHint,
  isTickable,
  type ExplorerEntry,
  type ExplorerState,
  type Family,
  type FamilyCounts,
} from "../lib/explorerModel";
import { counted } from "../lib/plural";
import type { IndexingState } from "../state/useIndexing";
import { FamilyFolderGlyph, FoldGlyph, StateGlyph } from "./ExplorerGlyphs";
import { ExplorerFileRow } from "./ExplorerFileRow";
import { ExplorerList } from "./ExplorerList";
import { AnalyseButton, AnalysisProgress, KnowledgeLink } from "./FolderCardParts";

/** The three counters of a header, in the order of the states on screen. */
const COUNTERS: readonly { state: ExplorerState; one: string; many: string }[] = [
  { state: "analysed", one: "workFolder.analysedOne", many: "workFolder.analysedMany" },
  { state: "toAnalyse", one: "explorer.toAnalyseOne", many: "explorer.toAnalyseMany" },
  { state: "unreadable", one: "workFolder.unreadableOne", many: "workFolder.unreadableMany" },
];

/**
 * One family of the explorer: a two-line header (the folder's icon, the family's name and its three
 * counters, the Analyser button and the fold button), the line of the running analysis, a few quiet
 * notes, what the last pass did, and the bounded list. The folder's path is the header's tooltip.
 *
 * A file of the other family sits in this list, tagged, and is not counted by Analyser: its own
 * family's pass cannot read it yet, and the button's tooltip says so.
 */
export function ExplorerFamilySection({
  family,
  folder,
  counts,
  entries,
  allEntries,
  open,
  onToggleOpen,
  rows,
  rowHeight,
  indexing,
  onStop,
  locked,
  isTicked,
  onToggleTick,
  onOpenFile,
  onRevealFile,
  needsAnalyse,
  meaningPercent,
  passLines,
  showKnowledgeLink,
  onOpenKnowledge,
  filtering,
}: {
  family: Family;
  /** The folder this family is analysed from, or null while none is chosen. */
  folder: string | null;
  counts: FamilyCounts;
  /** The entries to list: this family's, filtered and sorted. */
  entries: readonly ExplorerEntry[];
  /** Every entry of the box, to tell two files that share a name. */
  allEntries: readonly ExplorerEntry[];
  open: boolean;
  onToggleOpen: () => void;
  /** The most rows the list may show now (the shared height budget). */
  rows: number;
  rowHeight: number;
  indexing: IndexingState;
  /** Present where a pass can be stopped. */
  onStop?: () => void;
  /** A question is being answered: the selection must not change under it. */
  locked: boolean;
  isTicked: (entry: ExplorerEntry) => boolean;
  onToggleTick: (entry: ExplorerEntry) => void;
  onOpenFile: (entry: ExplorerEntry) => void;
  onRevealFile: (entry: ExplorerEntry) => void;
  /** The index predates the file profile: only Analyse fills it. */
  needsAnalyse: boolean;
  /** The share of the meaning search computed, while it is incomplete. */
  meaningPercent: number | null;
  /** What the last pass did, one short line each. */
  passLines: readonly string[];
  showKnowledgeLink: boolean;
  onOpenKnowledge?: () => void;
  /** The view is narrowed: an empty list then means "nothing matches", not "nothing here". */
  filtering: boolean;
}) {
  const { t } = useTranslation();
  const familyName = t(family === "documents" ? "explorer.familyDocuments" : "explorer.familyTables");
  const duplicates = duplicateNames(allEntries);
  const running = indexing.running;
  const foreignNote =
    counts.foreign === 0
      ? undefined
      : t(
          counts.foreign > 1 ? "explorer.foreignExcludedMany" : "explorer.foreignExcludedOne",
          { count: counts.foreign },
        );

  return (
    <section className={`explorer-family explorer-family--${family}`}>
      <div className="explorer-family__head" title={folder ?? undefined}>
        <FamilyFolderGlyph family={family} />
        <div className="explorer-family__text">
          <span className="explorer-family__name">{familyName}</span>
          <span className="explorer-family__counters">
            {COUNTERS.map(({ state, one, many }) => {
              const count = counts[state];
              return (
                <span key={state} className="explorer-family__counter">
                  <StateGlyph state={state} label={counted(count, t(one), t(many))} />
                  <span aria-hidden="true">{count}</span>
                </span>
              );
            })}
          </span>
        </div>
        {folder === null ? null : (
          <AnalyseButton
            indexing={indexing}
            emphasised={counts.analysable > 0}
            pendingCount={counts.analysable}
            title={foreignNote}
          />
        )}
        <button
          type="button"
          className="explorer-family__fold"
          aria-expanded={open}
          aria-label={t(open ? "explorer.foldClose" : "explorer.foldOpen", { family: familyName })}
          title={t(open ? "explorer.foldCloseHint" : "explorer.foldOpenHint")}
          onClick={onToggleOpen}
        >
          <FoldGlyph open={open} />
        </button>
      </div>
      {running ? (
        <div className="explorer-family__run">
          {indexing.progress === null ? null : (
            <AnalysisProgress
              fraction={analysisFraction(indexing.progress)}
              label={
                indexing.progress.readingNames === true
                  ? t(family === "documents" ? "workFolder.progressNamesLabel" : "dataFolder.progressNamesLabel")
                  : vectorPercent(indexing.progress) !== null
                    ? t("workFolder.progressMeaningLabel", {
                        percent: vectorPercent(indexing.progress) ?? 0,
                      })
                    : t(family === "documents" ? "workFolder.progressLabel" : "dataFolder.progressLabel")
              }
            />
          )}
          {onStop === undefined ? null : (
            <button type="button" className="button button--compact" onClick={onStop}>
              {t("workFolder.stop")}
            </button>
          )}
        </div>
      ) : null}
      {open ? (
        <>
          {needsAnalyse ? (
            <p className="explorer-note" role="status">
              {t("finder.needsAnalyse")}
            </p>
          ) : null}
          {meaningPercent === null ? null : (
            <p className="explorer-note" role="status">
              {t("workFolder.meaningShare", { percent: meaningPercent })}
            </p>
          )}
          {passLines.length === 0 ? null : (
            <details
              key={indexing.finishedPasses}
              className="explorer-pass"
              open={(indexing.summary?.failedFiles.length ?? 0) > 0}
            >
              <summary className="explorer-pass__summary">{t("explorer.passDetails")}</summary>
              <p className="explorer-pass__lines">
                {passLines.map((line, index) => (
                  <Fragment key={index}>
                    {index > 0 ? <br /> : null}
                    {line}
                  </Fragment>
                ))}
              </p>
              {showKnowledgeLink && onOpenKnowledge !== undefined ? (
                <KnowledgeLink onOpen={onOpenKnowledge} />
              ) : null}
            </details>
          )}
          {folder === null ? (
            <p className="explorer-empty">{t("explorer.noFolder")}</p>
          ) : entries.length === 0 ? (
            <p className="explorer-empty">
              {t(filtering ? "explorer.noMatch" : "explorer.emptyFamily")}
            </p>
          ) : (
            <ExplorerList
              entries={entries}
              rows={rows}
              rowHeight={rowHeight}
              label={t("explorer.listLabel", { family: familyName })}
              renderRow={(entry) => (
                <ExplorerFileRow
                  key={entry.key}
                  entry={entry}
                  checked={isTicked(entry)}
                  checkDisabled={locked || !isTickable(entry)}
                  onToggle={() => onToggleTick(entry)}
                  hint={duplicates.has(entry.name.toLowerCase()) ? folderHint(entry) : ""}
                  onOpen={isTickable(entry) ? () => onOpenFile(entry) : undefined}
                  onReveal={entry.native ? () => onRevealFile(entry) : undefined}
                />
              )}
            />
          )}
        </>
      ) : null}
    </section>
  );
}
