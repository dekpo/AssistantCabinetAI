/**
 * The mail merge as the interface sees it (HAP-1, lot E, steps E4 and E5;
 * `docs/test-reports/human-acceptance-pass-1/11-lot-e-plan.md`).
 *
 * A question that asks for a letter returns a plan, and a plan is only a proposal: what is filled
 * from which column, and how many letters, are decisions of the user, taken on the plan card. This
 * module holds the pure part - the types crossing the bridge and the rules that turn the card's state
 * into the request Rust acts on - so that the rules are tested without a component.
 */

export type PlaceholderStatus = "exact" | "proposed" | "unresolved";

export interface FillPlaceholder {
  name: string;
  /** The column the field would be filled from: the exact one, or only a proposal. */
  column: string | null;
  status: PlaceholderStatus;
}

export interface FillPreview {
  /** The text of the (first) letter. */
  text: string;
  /** How many letters one approval would write. */
  letters: number;
  /** How many table rows the previewed letter holds. */
  lines: number;
  /** Fields left as written: no column was confirmed for them. */
  unresolved: string[];
  /** The name the previewed letter would get. */
  fileName: string;
  /** The template's fields as proposed, for when another template is chosen. */
  placeholders: FillPlaceholder[];
}

export interface FillPlan {
  template: string;
  /** Every template that could be used, the chosen one first. */
  templates: string[];
  dataFile: string;
  sheet: string;
  columns: string[];
  /** What the question named; both null when it asked for every row. */
  keyColumn: string | null;
  keyValue: string | null;
  placeholders: FillPlaceholder[];
  preview: FillPreview;
}

export interface FillRequest {
  template: string;
  dataFile: string;
  keyColumn: string | null;
  keyValue: string | null;
  all: boolean;
  mapping: { placeholder: string; column: string | null }[];
}

export interface FillReport {
  written: { relativePath: string; key: string }[];
  unresolved: string[];
}

/** What the user decided for one field: the column, and whether that is confirmed. */
export interface Decision {
  column: string | null;
  confirmed: boolean;
}

/**
 * The starting decisions for a template's fields. An exact name is confirmed; a proposal is **not**
 * (it is shown selected but waits for her); a field with no column has nothing to confirm - it stays
 * as written in the letter.
 */
export function initialDecisions(placeholders: FillPlaceholder[]): Record<string, Decision> {
  const decisions: Record<string, Decision> = {};
  for (const placeholder of placeholders) {
    decisions[placeholder.name] = {
      column: placeholder.column,
      confirmed: placeholder.status !== "proposed",
    };
  }
  return decisions;
}

/** The fields whose proposal she has not confirmed yet. The letters cannot be written while any is. */
export function pendingConfirmations(decisions: Record<string, Decision>): string[] {
  return Object.entries(decisions)
    .filter(([, decision]) => !decision.confirmed)
    .map(([name]) => name);
}

export interface CardState {
  template: string;
  /** One letter per value of the key column (one per row with none), instead of the one named. */
  all: boolean;
  keyColumn: string | null;
  decisions: Record<string, Decision>;
}

/** The request for Rust. A proposal she has not confirmed is sent as "leave as written": it is never
 * filled on the strength of a default. */
export function requestOf(plan: Pick<FillPlan, "dataFile" | "keyValue">, state: CardState): FillRequest {
  return {
    template: state.template,
    dataFile: plan.dataFile,
    keyColumn: state.keyColumn,
    keyValue: state.all ? null : plan.keyValue,
    all: state.all,
    mapping: Object.entries(state.decisions).map(([placeholder, decision]) => ({
      placeholder,
      column: decision.confirmed ? decision.column : null,
    })),
  };
}

/** Whether "this order only" is possible: the question named one. */
export function canGenerateOne(plan: Pick<FillPlan, "keyValue">): boolean {
  return plan.keyValue !== null;
}
