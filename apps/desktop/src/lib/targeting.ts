import type { Translator } from "../i18n/translate";
import type {
  TargetingCandidate,
  TargetingEntity,
  TargetingReport,
  TargetingRequest,
} from "./ipc";
import { counted } from "./plural";

/**
 * What the interface makes of the knowledge base's report for one answer (`TargetingReport`).
 * Rust sends ids, counts and machine codes; every sentence is written here, in the user's language
 * (`docs/LANGUAGE-AND-LOCALE.md`), so a component only draws what these functions return.
 *
 * Nothing here talks to Rust, and nothing here is stored: the report belongs to one answer on the
 * screen and is gone with it.
 */

/** An automatic candidate is a capitalised phrase the base has not confirmed: it never narrows a
 * search, so it is never shown as a name or offered as a choice (defect KBD-11). */
export const CANDIDATE_MAX_CONFIDENCE = 0.4;

/** Names quoted in the targeting line before the rest are left out. */
const MAX_NAMES_IN_NOTE = 3;

/** Choices shown in one "which one?" row. */
const MAX_OPTIONS_SHOWN = 6;

/** What the user asked for on an answer, kept so that regenerating it asks the same thing. */
export interface TargetingChoice {
  /** `off` is the widen action: the old search on the whole selection. */
  mode?: "off";
  /** Entities the user picked, by a chip or by a button under an answer. */
  entityIds?: number[];
}

/** Whether a search was really narrowed. Read from the paths, never from `strategy`, which also
 * says `entity_files` for a narrowing that was only measured. */
export function reductionApplied(report: TargetingReport): boolean {
  return report.candidatePaths.length > 0 || report.workbookPaths.length > 0;
}

function isConfirmed(entity: TargetingEntity): boolean {
  return entity.confidence > CANDIDATE_MAX_CONFIDENCE;
}

/** The names the question settled on, each once, candidates left out. An identifier (an AVS number,
 * an invoice number) is not a name: its label is internal, and a personal one is a hash. */
export function confirmedNames(report: TargetingReport): string[] {
  const names: string[] = [];
  for (const entity of report.entities) {
    if (entity.typeId !== "identifier" && isConfirmed(entity) && !names.includes(entity.display)) {
      names.push(entity.display);
    }
  }
  return names;
}

/** The previous answer's confirmed entities, to be carried into a short follow-up. Ids only: the
 * client holds them, Rust keeps nothing about the conversation. */
export function carryOverIds(report: TargetingReport | undefined): number[] {
  if (report === undefined) {
    return [];
  }
  return [
    ...new Set(report.entities.filter(isConfirmed).map((entity) => entity.entityId)),
  ];
}

/** A question this short is read as the follow-up of the previous one: it is searched together with
 * it (`conversation::retrieval_query`) and, here, it carries the previous answer's names. The same
 * number as `conversation::FOLLOW_UP_MAX_WORDS` in Rust, which a test reads from the source. */
export const FOLLOW_UP_MAX_WORDS = 8;

/** Whether the question is short enough to be a follow-up: whitespace-separated words, as Rust
 * counts them. */
export function isShortFollowUp(question: string): boolean {
  return question.split(/\s+/u).filter((word) => word.length > 0).length <= FOLLOW_UP_MAX_WORDS;
}

/**
 * What to send with a question. A widen has no carried names: it is the old search. A choice of
 * names applies as chips. Otherwise the previous answer's names ride along **for a short question
 * only**, and Rust honours them only when that question names nobody itself.
 */
export function targetingRequestFor(
  choice: TargetingChoice | undefined,
  previous: TargetingReport | undefined,
  question: string,
): TargetingRequest | undefined {
  if (choice?.mode === "off") {
    return { mode: "off" };
  }
  const entityIds = choice?.entityIds ?? [];
  const carryOver = isShortFollowUp(question) ? carryOverIds(previous) : [];
  if (entityIds.length === 0 && carryOver.length === 0) {
    return undefined;
  }
  return {
    ...(entityIds.length > 0 ? { entityIds } : {}),
    ...(carryOver.length > 0 ? { carryOver } : {}),
  };
}

// ---------------------------------------------------------------------------------------------
// The line under an answer
// ---------------------------------------------------------------------------------------------

export interface TargetingNoteLine {
  kind: "applied" | "chip" | "widened" | "workbook";
  text: string;
  /** The action this line offers: both re-ask the question on the whole selection. */
  action: { label: string } | null;
}

export interface TargetingNoteModel {
  lines: TargetingNoteLine[];
  /** The files the narrowing kept, documents then workbooks, for "see the files". */
  files: string[];
}

function scopeText(t: Translator, report: TargetingReport): string {
  const parts: string[] = [];
  if (report.candidatePaths.length > 0) {
    parts.push(
      t("chat.targeting.portion", {
        kept: counted(
          report.candidatePaths.length,
          t("chat.targeting.documentOne"),
          t("chat.targeting.documentMany"),
        ),
        total: report.selectedDocuments,
      }),
    );
  }
  // One workbook left is told by its own line ("workbook chosen"), not counted here.
  if (report.workbookPaths.length > 0 && report.chosenWorkbook === null) {
    parts.push(
      t("chat.targeting.portion", {
        kept: counted(
          report.workbookPaths.length,
          t("chat.targeting.workbookOne"),
          t("chat.targeting.workbookMany"),
        ),
        total: report.selectedWorkbooks,
      }),
    );
  }
  return parts.join(t("chat.targeting.and"));
}

function namesSuffix(t: Translator, names: string[]): string {
  if (names.length === 0) {
    return "";
  }
  return t("chat.targeting.namesSuffix", { names: names.slice(0, MAX_NAMES_IN_NOTE).join(", ") });
}

/**
 * The line (or two) the answer carries when the knowledge base narrowed its search, or `null`.
 * Nothing is said for a narrowing that was only measured or skipped: the old search ran and there
 * is nothing to explain.
 */
export function targetingNote(
  t: Translator,
  report: TargetingReport | undefined,
): TargetingNoteModel | null {
  if (report === undefined || !reductionApplied(report)) {
    return null;
  }
  const names = confirmedNames(report);
  const widen = { label: t("chat.targeting.widen") };
  const lines: TargetingNoteLine[] = [];
  const documentsNarrowed = report.candidatePaths.length > 0;
  const tableNarrowed = report.workbookPaths.length > 0 && report.chosenWorkbook === null;

  if (report.widened === "auto" && documentsNarrowed) {
    // The narrowed search found too little, so the whole selection was searched: said, with no
    // action to take, since the answer already rests on everything she selected.
    lines.push({ kind: "widened", text: t("chat.targeting.widenedAuto"), action: null });
  } else if (documentsNarrowed || tableNarrowed) {
    const chip = report.requestedBy === "chip";
    lines.push({
      kind: chip ? "chip" : "applied",
      text:
        t(chip ? "chat.targeting.appliedByChip" : "chat.targeting.applied", {
          scope: scopeText(t, report),
        }) + namesSuffix(t, names),
      action: widen,
    });
  }

  if (report.chosenWorkbook !== null) {
    lines.push({
      kind: "workbook",
      text:
        t("chat.targeting.workbookChosen", { file: report.chosenWorkbook.file }) +
        (names.length === 0
          ? ""
          : t(
              names.length > 1 ? "chat.targeting.presentMany" : "chat.targeting.presentOne",
              { names: names.slice(0, MAX_NAMES_IN_NOTE).join(", ") },
            )),
      action: { label: t("chat.targeting.chooseAnotherWorkbook") },
    });
  }

  return { lines, files: [...report.candidatePaths, ...report.workbookPaths] };
}

/** The note as plain sentences, for "Copy the conversation": what is copied is what was seen. */
export function targetingNoteText(t: Translator, report: TargetingReport | undefined): string[] {
  return targetingNote(t, report)?.lines.map((line) => line.text) ?? [];
}

// ---------------------------------------------------------------------------------------------
// "Which one?" and "did you mean?"
// ---------------------------------------------------------------------------------------------

export interface EntityOption {
  entityId: number;
  /** The button's text: the name and how many selected files hold it. */
  label: string;
}

export interface EntityOffer {
  kind: "ambiguous" | "possible";
  text: string;
  options: EntityOption[];
}

/** A candidate counts files of the selection, or it is not in the selection (invariant I7). */
function inSelection(candidate: TargetingCandidate): boolean {
  return candidate.documents + candidate.workbooks > 0;
}

function optionsOf(
  t: Translator,
  candidates: TargetingCandidate[],
  taken: Set<number>,
  alreadyChosen: ReadonlySet<number>,
): EntityOption[] {
  const options: EntityOption[] = [];
  for (const candidate of candidates) {
    if (!inSelection(candidate) || alreadyChosen.has(candidate.entityId)) {
      continue;
    }
    if (taken.has(candidate.entityId)) {
      continue;
    }
    taken.add(candidate.entityId);
    options.push({
      entityId: candidate.entityId,
      label: t("chat.targeting.option", {
        name: candidate.display,
        files: counted(
          candidate.documents + candidate.workbooks,
          t("chat.scopeFileOne"),
          t("chat.scopeFileMany"),
        ),
      }),
    });
    if (options.length === MAX_OPTIONS_SHOWN) {
      break;
    }
  }
  return options;
}

/** The kind of thing the candidates are, when they all are the same kind. */
function commonKind(candidates: TargetingCandidate[]): string {
  const kinds = new Set(candidates.map((candidate) => candidate.typeId));
  const only = kinds.size === 1 ? [...kinds][0] : undefined;
  return only === "person" || only === "organization" || only === "location" ? only : "other";
}

/**
 * The questions the answer asks back, below it. The question was answered anyway, on the whole
 * selection; these buttons re-ask it with the name the user picks, which narrows the search at
 * once (a chip is an explicit request).
 *
 * Only names of the selection are offered: a candidate that names no selected file is dropped even
 * if Rust sent it. A group is dropped whole once the user has picked one of its names on this answer
 * (Rust still reports the ambiguity of the words, the user has since settled it).
 */
export function targetingOffers(
  t: Translator,
  report: TargetingReport | undefined,
  alreadyChosen: readonly number[] = [],
): EntityOffer[] {
  if (report === undefined) {
    return [];
  }
  const chosen = new Set(alreadyChosen);
  const taken = new Set<number>();
  const offers: EntityOffer[] = [];

  for (const ambiguity of report.ambiguities) {
    // One of these names was picked on this answer: the question has been answered.
    if (ambiguity.candidates.some((candidate) => chosen.has(candidate.entityId))) {
      continue;
    }
    const options = optionsOf(t, ambiguity.candidates, taken, chosen);
    // One name left is no longer a question.
    if (options.length < 2) {
      continue;
    }
    const span = ambiguity.span.trim();
    const kind = commonKind(ambiguity.candidates);
    offers.push({
      kind: "ambiguous",
      text:
        span.length === 0
          ? t("chat.targeting.ambiguousUnnamed")
          : t(`chat.targeting.ambiguous.${kind}`, { name: span }),
      options,
    });
  }
  for (const possible of report.possibles) {
    if (possible.candidates.some((candidate) => chosen.has(candidate.entityId))) {
      continue;
    }
    const options = optionsOf(t, possible.candidates, taken, chosen);
    if (options.length === 0) {
      continue;
    }
    offers.push({ kind: "possible", text: t("chat.targeting.didYouMean"), options });
  }
  return offers;
}
