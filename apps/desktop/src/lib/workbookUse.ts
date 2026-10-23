import type { Translator } from "../i18n/translate";
import { displayPathOfKey } from "./indexKey";
import type { WorkbookUse } from "./ipc";

/**
 * The line under an answer for each workbook it used, or did not use (TABULAR plan 4.3): what was
 * done with the file, and for a refusal what to do about it. Rust sends machine codes
 * (`workbook_use.rs`), the wording lives here and in the two catalogues (`workbookUse.*`).
 *
 * Principle: a file the user named never disappears without a visible line saying what was done with
 * it. A line is a *caution*, read aloud with the other cautions, when it tells the reader a file they
 * named, ticked or were given a card for did not serve as expected: every refusal except the
 * aggregate "N ticked spreadsheets were not used", and a card that was shrunk. A card sent, a figure
 * computed and a plan prepared are informative only.
 */
export interface WorkbookUseLine {
  text: string;
  caution: boolean;
}

/** More lines than this are folded behind one title on the screen (the cautions stay visible). */
export const FOLD_AFTER = 3;

export function workbookUseLines(t: Translator, uses: readonly WorkbookUse[]): WorkbookUseLine[] {
  return uses.map((use) => lineOf(t, use));
}

function lineOf(t: Translator, use: WorkbookUse): WorkbookUseLine {
  // A key shows as its plain path; a name the user typed, or an aggregate, is shown as it is.
  const path = displayPathOfKey("data", use.file);
  switch (use.how) {
    case "computed":
      return { text: t("workbookUse.computed", { path }), caution: false };
    case "plan_prepared":
      return { text: t("workbookUse.planPrepared", { path }), caution: false };
    case "card_sent": {
      const base = t(`workbookUse.cardSent.${use.reason ?? "named"}`, { path });
      return use.truncated
        ? { text: `${base}${t("workbookUse.cardTruncated")}`, caution: true }
        : { text: base, caution: false };
    }
    case "not_used": {
      if (use.reason === "not_relevant") {
        const count = use.count ?? 0;
        return {
          text: t(count > 1 ? "workbookUse.notUsed.not_relevant_many" : "workbookUse.notUsed.not_relevant_one", {
            count: String(count),
          }),
          caution: false,
        };
      }
      return {
        text: t(`workbookUse.notUsed.${use.reason ?? "not_sent"}`, {
          path,
          candidates: use.candidates.map((key) => displayPathOfKey("data", key)).join(", "),
        }),
        caution: true,
      };
    }
  }
}

/**
 * What the conversation view shows at once and what it folds behind one title: up to `FOLD_AFTER`
 * lines are all shown; more than that, the cautions stay visible and the informative lines fold. The
 * copy and the read-aloud do not fold: they take the lines in the order they were written.
 */
export function foldedLines(lines: readonly WorkbookUseLine[]): {
  shown: WorkbookUseLine[];
  folded: WorkbookUseLine[];
} {
  if (lines.length <= FOLD_AFTER) {
    return { shown: [...lines], folded: [] };
  }
  return {
    shown: lines.filter((line) => line.caution),
    folded: lines.filter((line) => !line.caution),
  };
}
