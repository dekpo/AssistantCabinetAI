import type { RepairChoice, RepairProposal, RepairSet } from "./ipc";

/**
 * The name suggestions of a dictation as plain data and pure functions: no React, no Tauri, no box
 * of the chat. A chip is a span of the draft ("heard") and the names it may have meant; it is placed
 * once, follows the edits made around it, and is dropped the moment the draft no longer holds the
 * words heard at its place. Nothing here changes a text on its own: accepting is the user's click,
 * and the replacement is computed here from the draft she sees.
 */

/** The words of one dictation, as the question box (or, later, a note) holds them. */
export interface RepairSource {
  /** Identity of the dictation: a new object for each one. */
  id: object;
  /** The words as the engine wrote them: what the knowledge base reads. */
  words: string;
  /** Where `words` begin in `draft`. */
  start: number;
  /** The whole text right after the words went in. */
  draft: string;
  /** Whether the engine writes capitals where a name has them. */
  caseReliable: boolean;
}

export interface RepairChip {
  id: string;
  /** UTF-16 offsets in the current draft. */
  start: number;
  end: number;
  heard: string;
  action: RepairProposal["action"];
  candidates: RepairChoice[];
}

/** The chips of an answer, placed in the draft the words went into. */
export function chipsFromSet(set: RepairSet, source: RepairSource, serial: number): RepairChip[] {
  return set.proposals
    .filter(
      (proposal) =>
        proposal.candidates.length > 0 &&
        source.draft.slice(source.start + proposal.start, source.start + proposal.end) ===
          proposal.heard,
    )
    .map((proposal, index) => ({
      id: `${serial}-${index}`,
      start: source.start + proposal.start,
      end: source.start + proposal.end,
      heard: proposal.heard,
      action: proposal.action,
      candidates: proposal.candidates,
    }));
}

const WORD_CHARACTER = /[\p{L}\p{N}]/u;

/** The words heard are still at the chip's place and no letter or digit has been attached to them
 * (a name typed on after the words is a different word). */
function stillStandsAlone(text: string, chip: RepairChip): boolean {
  return (
    text.slice(chip.start, chip.end) === chip.heard &&
    !WORD_CHARACTER.test(text.charAt(chip.start - 1)) &&
    !WORD_CHARACTER.test(text.charAt(chip.end))
  );
}

/**
 * Follows an edit of the draft: a chip before the edit stays, a chip after it moves by the change
 * in length, a chip the edit touched is dropped, and so is any chip whose place no longer holds the
 * words heard, or whose words had a letter attached to them. The edit is the part between the longest common beginning and the longest common
 * end of the two texts.
 */
export function followEdit(chips: RepairChip[], before: string, after: string): RepairChip[] {
  if (before === after) {
    return chips;
  }
  const shortest = Math.min(before.length, after.length);
  let head = 0;
  while (head < shortest && before[head] === after[head]) {
    head += 1;
  }
  let tail = 0;
  while (
    tail < shortest - head &&
    before[before.length - 1 - tail] === after[after.length - 1 - tail]
  ) {
    tail += 1;
  }
  const delta = after.length - before.length;
  const kept: RepairChip[] = [];
  for (const chip of chips) {
    let moved: RepairChip | null = null;
    if (chip.end <= head) {
      moved = chip;
    } else if (chip.start >= before.length - tail) {
      moved = { ...chip, start: chip.start + delta, end: chip.end + delta };
    }
    if (moved !== null && stillStandsAlone(after, moved)) {
      kept.push(moved);
    }
  }
  return kept;
}

export interface Replacement {
  text: string;
  caret: number;
}

/** The draft with the words of a chip replaced by a name, or `null` if the draft no longer holds
 * those words at that place (the chip is then dropped, never applied somewhere else). */
export function replacementFor(
  draft: string,
  chip: RepairChip,
  choice: RepairChoice,
): Replacement | null {
  if (draft.slice(chip.start, chip.end) !== chip.heard) {
    return null;
  }
  return {
    text: draft.slice(0, chip.start) + choice.display + draft.slice(chip.end),
    caret: chip.start + choice.display.length,
  };
}

/** The chips without one of them (Ignore, or a chip that was used). */
export function withoutChip(chips: readonly RepairChip[], chipId: string): RepairChip[] {
  return chips.filter((chip) => chip.id !== chipId);
}

export interface Accepted {
  /** The chips that remain: the accepted one is always gone, applied or not. */
  chips: RepairChip[];
  /** The new draft, or null when the draft no longer held the words (nothing is changed then). */
  replacement: Replacement | null;
}

/** What pressing a name does: the replacement if the words are still there, and the chip is gone
 * either way. A chip or a name that does not exist changes nothing. */
export function acceptChip(
  chips: readonly RepairChip[],
  draft: string,
  chipId: string,
  entityId: number,
): Accepted {
  const chip = chips.find((candidate) => candidate.id === chipId);
  const choice = chip?.candidates.find((candidate) => candidate.entityId === entityId);
  if (chip === undefined || choice === undefined) {
    return { chips: [...chips], replacement: null };
  }
  return { chips: withoutChip(chips, chipId), replacement: replacementFor(draft, chip, choice) };
}
