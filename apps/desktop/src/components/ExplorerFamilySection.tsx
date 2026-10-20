import { useEffect, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { endLine, gaugeLabel, runLine } from "../lib/analysisLines";
import { progressView, type PassFamily } from "../lib/analysisProgress";
import {
  duplicateNames,
  folderHint,
  isTickable,
  type ExplorerEntry,
  type ExplorerState,
  type Family,
  type FamilyCounts,
} from "../lib/explorerModel";
import { abbreviateFolderPath } from "../lib/folderPath";
import { counted } from "../lib/plural";
import type { IndexingState } from "../state/useIndexing";
import { ExplorerFamilyMenu } from "./ExplorerFamilyMenu";
import { FamilyFolderGlyph, FoldGlyph, StateGlyph } from "./ExplorerGlyphs";
import { ExplorerFileRow } from "./ExplorerFileRow";
import { ExplorerList } from "./ExplorerList";
import { AnalysisGauge } from "./AnalysisGauge";
import { AnalyseButton } from "./FolderCardParts";

/**
 * The thin line under the header of a running family, filled to the stage's percentage. Built behind
 * this switch so it can be compared with and without in the real application (owner decision
 * pending, `docs/DECISIONS.md`, "Progress of an Analyse"): set it to `false` to draw none.
 */
const PROGRESS_HAIRLINE = true;

/** The three counters of a header, in the order of the states on screen. */
const COUNTERS: readonly { state: ExplorerState; one: string; many: string }[] = [
  { state: "analysed", one: "workFolder.analysedOne", many: "workFolder.analysedMany" },
  { state: "toAnalyse", one: "explorer.toAnalyseOne", many: "explorer.toAnalyseMany" },
  { state: "unreadable", one: "workFolder.unreadableOne", many: "workFolder.unreadableMany" },
];

/**
 * One family of the explorer: a two-line header (the folder's icon, the family's name and its three
 * counters, the Analyser button, the « ⋯ » menu and the fold button), the line of the running
 * analysis (the Analyser button is then the gauge with its Stop, and the menu is gone), the line of
 * the pass that just ended with its « Voir le rapport » link (the report itself is in the centre), a
 * quiet note, and the bounded list. The folder's path is the header's tooltip. A folder that holds nothing says so in place of the list, with the two things to
 * do about it, and its header says which formats go in.
 *
 * A file of the other family sits in this list, tagged, and is not counted by Analyser: its own
 * family's pass cannot read it yet, and the button's tooltip says so.
 */
export function ExplorerFamilySection({
  family,
  folder,
  isEmpty,
  folderActions,
  counts,
  entries,
  allEntries,
  open,
  onToggleOpen,
  rows,
  rowHeight,
  indexing,
  now,
  onStop,
  locked,
  isTicked,
  onToggleTick,
  onOpenFile,
  onRevealFile,
  needsAnalyse,
  onOpenReport,
  filtering,
  reasonOf,
}: {
  family: Family;
  /** The folder this family is analysed from (a family with no folder is `ExplorerFamilySetup`). */
  folder: string;
  /** The folder holds no file: nothing to list and nothing for Analyser to do. */
  isEmpty: boolean;
  /** What the « ⋯ » menu and the empty folder's two buttons do. */
  folderActions: { reveal: () => void; change: () => void; clear: () => void };
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
  /** The clock, ticking while a pass runs (`useNow`): an input, so the times are tested with any value. */
  now: number;
  /** Stops this family's pass and no other. */
  onStop: () => void;
  /** A question is being answered: the selection must not change under it. */
  locked: boolean;
  isTicked: (entry: ExplorerEntry) => boolean;
  onToggleTick: (entry: ExplorerEntry) => void;
  onOpenFile: (entry: ExplorerEntry) => void;
  onRevealFile: (entry: ExplorerEntry) => void;
  /** The index predates the file profile: only Analyse fills it. */
  needsAnalyse: boolean;
  /** « Voir le rapport »: opens the centre on its overview. */
  onOpenReport: () => void;
  /** The view is narrowed: an empty list then means "nothing matches", not "nothing here". */
  filtering: boolean;
  /** Why a text search found an entry, or undefined when none is active or it said nothing. */
  reasonOf?: (entry: ExplorerEntry) => { text: string; title: string } | undefined;
}) {
  const { t, locale } = useTranslation();
  const familyName = t(family === "documents" ? "explorer.familyDocuments" : "explorer.familyTables");
  const typesLine = t(family === "documents" ? "explorer.typesDocuments" : "explorer.typesTables");
  const duplicates = duplicateNames(allEntries);
  const running = indexing.running;
  const passFamily: PassFamily = family === "documents" ? "documents" : "tables";
  /* Pressed once, the Stop waits for the pass to end between two files; it cannot be pressed twice. */
  const [stopping, setStopping] = useState(false);
  useEffect(() => {
    if (!running) {
      setStopping(false);
    }
  }, [running]);
  const trace = indexing.trace;
  const view = running && trace !== null ? progressView(trace, now, passFamily) : null;
  const line = running
    ? runLine(t, locale, passFamily, view, trace === null ? 0 : now - trace.startedAt)
    : indexing.summary !== null
      ? endLine(t, locale, indexing.summary, indexing.durationMs)
      : null;
  const foreignNote =
    counts.foreign === 0
      ? undefined
      : t(
          counts.foreign > 1 ? "explorer.foreignExcludedMany" : "explorer.foreignExcludedOne",
          { count: counts.foreign },
        );

  return (
    <section className={`explorer-family explorer-family--${family}`}>
      <div className="explorer-family__head" title={folder}>
        <FamilyFolderGlyph family={family} />
        <div className="explorer-family__text">
          <span className="explorer-family__title">
            <span className="explorer-family__name">{familyName}</span>
            <span className="explorer-family__location">{abbreviateFolderPath(folder)}</span>
          </span>
          {isEmpty ? (
            <span className="explorer-family__types">{typesLine}</span>
          ) : (
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
          )}
        </div>
        {running ? (
          <AnalysisGauge
            label={stopping ? t("explorer.gaugeStopping") : gaugeLabel(t, view)}
            percent={view?.percent ?? 0}
            title={line ?? undefined}
            stopLabel={t("explorer.gaugeStop", { family: familyName })}
            stopHint={t("explorer.gaugeStopHint")}
            stopping={stopping}
            onStop={() => {
              setStopping(true);
              onStop();
            }}
          />
        ) : (
          <AnalyseButton
            indexing={indexing}
            emphasised={counts.analysable > 0}
            pendingCount={counts.analysable}
            title={isEmpty ? t("explorer.analyseEmptyHint") : foreignNote}
            disabled={isEmpty}
          />
        )}
        {/* Nothing in the menu can be done to a folder while its analysis runs. */}
        {running ? null : (
          <ExplorerFamilyMenu
            label={t("explorer.menuLabel", { family: familyName })}
            items={[
              { id: "reveal", label: t("explorer.menuReveal"), onSelect: folderActions.reveal },
              { id: "change", label: t("explorer.menuChange"), onSelect: folderActions.change },
              { id: "clear", label: t("explorer.menuClear"), onSelect: folderActions.clear },
            ]}
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
      {running && PROGRESS_HAIRLINE ? (
        <div className="explorer-family__line" aria-hidden="true">
          <span className="explorer-family__line-fill" style={{ width: `${view?.percent ?? 0}%` }} />
        </div>
      ) : null}
      {line === null ? null : (
        <p
          className={running ? "explorer-family__run" : "explorer-family__run explorer-family__run--end"}
          title={line}
        >
          {line}
        </p>
      )}
      {!running && indexing.summary !== null ? (
        <p className="explorer-family__report">
          <button
            type="button"
            className="button button--quiet button--compact"
            title={t("overview.viewReportHint")}
            onClick={onOpenReport}
          >
            {t("overview.viewReport")}
          </button>
        </p>
      ) : null}
      {running && view?.stage === "meaning" ? (
        <p className="explorer-note">{t("explorer.runWordsWork")}</p>
      ) : null}
      {open ? (
        <>
          {needsAnalyse && !running ? (
            <p className="explorer-note" role="status">
              {t("finder.needsAnalyse")}
            </p>
          ) : null}
          {isEmpty ? (
            <div className="explorer-empty explorer-empty--folder">
              <span className="explorer-empty__text">{t("explorer.emptyFolder")}</span>
              <span className="explorer-empty__actions">
                <button
                  type="button"
                  className="button button--compact button--primary"
                  onClick={folderActions.reveal}
                >
                  {t("explorer.menuReveal")}
                </button>
                <button
                  type="button"
                  className="button button--compact"
                  onClick={folderActions.change}
                >
                  {t("explorer.changeShort")}
                </button>
              </span>
            </div>
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
                  reason={reasonOf?.(entry)}
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
