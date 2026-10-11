import type { Translator } from "../i18n/translate";
import type { KnowledgeAggregates, RetrievalDiagnostics } from "./ipc";

/**
 * The rows of the developer panel (`DiagnosticsPanel`), shown only when the setting is on. Rust
 * sends numbers and machine codes; the labels come from the catalogue like everything else, in
 * technical wording because the reader is a developer. Nothing here holds a question, a name or a
 * file name: the panel can be screenshotted without leaking any of them.
 */

export interface DiagnosticRow {
  label: string;
  value: string;
}

/** A machine code as a label, or the code itself when the catalogue has no word for it yet. */
export function codeLabel(t: Translator, prefix: string, code: string): string {
  const key = `${prefix}.${code}`;
  const label = t(key);
  return label === key ? code : label;
}

function milliseconds(value: number | null): string {
  return value === null ? "-" : `${Math.round(value)} ms`;
}

function share(value: number | null | undefined): string {
  return value === null || value === undefined ? "-" : `${Math.round(value * 100)} %`;
}

/** One question: its path, its counts, where its time went, and what the knowledge base did. */
export function answerDiagnosticRows(
  t: Translator,
  diagnostics: RetrievalDiagnostics,
): DiagnosticRow[] {
  const row = (key: string, value: string): DiagnosticRow => ({
    label: t(`diagnostics.${key}`),
    value,
  });
  const rows: DiagnosticRow[] = [
    row("path", codeLabel(t, "diagnostics.pathName", diagnostics.path)),
    row(
      "plan",
      diagnostics.plan === null ? "-" : codeLabel(t, "diagnostics.planName", diagnostics.plan),
    ),
    row(
      "selected",
      t("diagnostics.selectedValue", {
        documents: diagnostics.selectedDocuments,
        workbooks: diagnostics.selectedWorkbooks,
      }),
    ),
    row(
      "chunks",
      t("diagnostics.chunksValue", {
        selected: diagnostics.chunksSelected,
        considered: diagnostics.chunksConsidered,
      }),
    ),
    row("inputChars", String(diagnostics.estimatedInputChars)),
    row("inventoryMs", milliseconds(diagnostics.inventoryMs)),
    row("scopeMs", milliseconds(diagnostics.scopeMs)),
    row("routingMs", milliseconds(diagnostics.routingMs)),
    row("embeddingMs", milliseconds(diagnostics.embeddingMs)),
    row("searchMs", milliseconds(diagnostics.searchMs)),
    row("workbookLoadMs", milliseconds(diagnostics.workbookLoadMs)),
    row("firstTokenMs", milliseconds(diagnostics.generationFirstTokenMs)),
    row("generationMs", milliseconds(diagnostics.generationTotalMs)),
    row("totalMs", milliseconds(diagnostics.totalMs)),
  ];

  const base = diagnostics.targeting;
  if (base !== null) {
    rows.push(
      row("mode", codeLabel(t, "diagnostics.modeName", base.mode)),
      row("outcome", codeLabel(t, "diagnostics.outcomeName", base.outcome)),
      row("strategy", codeLabel(t, "diagnostics.strategyName", base.strategy)),
      row(
        "skippedReason",
        base.skippedReason === null
          ? "-"
          : codeLabel(t, "diagnostics.reasonName", base.skippedReason),
      ),
      row("lookupMs", milliseconds(base.knowledgeLookupMs)),
      row("queryEntities", String(base.queryEntities.length)),
      row(
        "candidates",
        t("diagnostics.selectedValue", {
          documents: base.candidateDocuments,
          workbooks: base.candidateWorkbooks,
        }),
      ),
      row("reduction", `${base.reductionPercent} %`),
      row("widened", t(base.widened ? "diagnostics.yes" : "diagnostics.no")),
      row("wouldApply", t(base.shadow.wouldApply ? "diagnostics.yes" : "diagnostics.no")),
      row(
        "cited",
        t("diagnostics.citedValue", {
          held: base.shadow.citedInCandidates,
          cited: base.shadow.cited,
        }),
      ),
      row(
        "misses",
        t("diagnostics.missesValue", {
          noMention: base.shadow.missNoMention,
          capped: base.shadow.missCapped,
          unresolved: base.shadow.missEntityNotResolved,
        }),
      ),
    );
  }
  return rows;
}

/** The recorded questions added up: what a release gate would read. */
export function aggregateRows(t: Translator, totals: KnowledgeAggregates): DiagnosticRow[] {
  const row = (key: string, value: string): DiagnosticRow => ({
    label: t(`diagnostics.${key}`),
    value,
  });
  const rows: DiagnosticRow[] = [
    row("questions", String(totals.questions)),
    row("withEntities", `${totals.withEntities} (${share(totals.withEntitiesShare)})`),
    row("wouldApplyCount", `${totals.wouldApply} (${share(totals.wouldApplyRate)})`),
    row("appliedCount", String(totals.applied)),
    row("widenedCount", String(totals.widened)),
    row(
      "medianReduction",
      totals.medianReductionPercent === null ? "-" : `${totals.medianReductionPercent} %`,
    ),
    row("recallOverall", share(totals.recallOverall)),
    row("recallWhenWouldApply", share(totals.recallWhenWouldApply)),
    row(
      "missesTotal",
      t("diagnostics.missesValue", {
        noMention: totals.missNoMention,
        capped: totals.missCapped,
        unresolved: totals.missEntityNotResolved,
      }),
    ),
  ];
  for (const [reason, count] of Object.entries(totals.skippedByReason).sort(
    ([a], [b]) => a.localeCompare(b),
  )) {
    rows.push({
      label: t("diagnostics.skippedFor", {
        reason: codeLabel(t, "diagnostics.reasonName", reason),
      }),
      value: String(count),
    });
  }
  return rows;
}
