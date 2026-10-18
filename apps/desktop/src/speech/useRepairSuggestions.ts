import { useEffect, useMemo, useRef, useState } from "react";
import type { CombinedScope } from "../lib/ipc";
import { speechRepairNames } from "./ipc";
import {
  acceptChip,
  chipsFromSet,
  followEdit,
  withoutChip,
  type RepairChip,
  type RepairSource,
} from "./repairChips";

/**
 * What the name suggestions need from the screen: the chips, how many names are held back, and the
 * three things the user can do. Handed to a text box as one optional property.
 */
export interface RepairBridge {
  chips: readonly RepairChip[];
  /** Suggestions left out because names are hidden and not revealed; no name is known to the window. */
  hiddenCount: number;
  /** Replaces the words of a chip by one of its names, if the text still holds them there. */
  accept: (chipId: string, entityId: number) => void;
  ignore: (chipId: string) => void;
  /** Asks to see the names, for the session (the same switch the suggestions under a conversation use). */
  reveal: () => void;
}

interface Inputs {
  /** The last dictation, or null. Any text box can use the hook: it knows nothing of them. */
  source: RepairSource | null;
  draft: string;
  setDraft: (draft: string) => void;
  scope: CombinedScope;
  revealNames: boolean;
  onRevealNamesChange: (reveal: boolean) => void;
  /** False while the voice is off: the hook then asks nothing and returns `undefined`. */
  enabled: boolean;
}

/**
 * Name suggestions for the words just dictated. One cheap read of the knowledge base per dictation
 * (plus one more when the user reveals hidden names); no model, no write. Any failure is no chip.
 * The chips of several dictations (the segments of a long note) add up. They follow the edits of the
 * draft and vanish when their words are no longer there.
 */
export function useRepairSuggestions(inputs: Inputs): RepairBridge | undefined {
  const { source, draft, setDraft, scope, revealNames, onRevealNamesChange, enabled } = inputs;
  const [chips, setChips] = useState<RepairChip[]>([]);
  /* How many suggestions each dictation left out because names are hidden. */
  const [hidden, setHidden] = useState<Record<number, number>>({});
  const latest = useRef({ draft });
  latest.current = { draft };
  /* Bumped when the selection changes or the hook goes away: older answers are dropped. */
  const epoch = useRef(0);
  const serial = useRef(0);
  const known = useRef(new WeakMap<object, number>());
  /* The dictations that were asked about, and those whose names are still hidden. */
  const asked = useRef(new WeakSet<object>());
  const waiting = useRef(new Map<number, RepairSource>());
  const shownDraft = useRef(draft);

  /* The chips follow the edits made around them. */
  useEffect(() => {
    const before = shownDraft.current;
    shownDraft.current = draft;
    setChips((current) => (current.length === 0 ? current : followEdit(current, before, draft)));
  }, [draft]);

  /* Another selection holds other names: what was found for the last one means nothing. */
  useEffect(() => {
    epoch.current += 1;
    asked.current = new WeakSet();
    waiting.current = new Map();
    setChips([]);
    setHidden({});
  }, [scope]);

  useEffect(
    () => () => {
      epoch.current += 1;
    },
    [],
  );

  const ask = (words: RepairSource, reveal: boolean) => {
    let number = known.current.get(words.id);
    if (number === undefined) {
      number = serial.current += 1;
      known.current.set(words.id, number);
    }
    const mine = epoch.current;
    const place = (found: RepairChip[], hiddenCount: number) => {
      if (mine !== epoch.current) {
        return;
      }
      if (hiddenCount > 0) {
        waiting.current.set(number, words);
      } else {
        waiting.current.delete(number);
      }
      /* The box may have changed while the answer came: place the chips where the text is now. */
      const placed = followEdit(found, words.draft, latest.current.draft);
      setChips((current) => [
        ...current.filter((chip) => !chip.id.startsWith(`${number}-`)),
        ...placed,
      ]);
      setHidden((current) => ({ ...current, [number]: hiddenCount }));
    };
    speechRepairNames(words.words, scope, words.caseReliable, reveal).then(
      (set) => place(chipsFromSet(set, words, number), set.hiddenCount),
      () => place([], 0),
    );
  };
  const askRef = useRef(ask);
  askRef.current = ask;

  /* A new dictation, or a new selection for the one already there. */
  useEffect(() => {
    if (!enabled || source === null || asked.current.has(source.id)) {
      return;
    }
    asked.current.add(source.id);
    askRef.current(source, revealNames);
    // The dictation and the selection decide; `source` itself is rebuilt on every render.
  }, [source?.id, scope, enabled]);

  /* The user asked to see the names: the dictations that held some back are read again. */
  useEffect(() => {
    if (enabled && revealNames) {
      waiting.current.forEach((words) => askRef.current(words, true));
    }
  }, [revealNames, enabled]);

  const hiddenCount = Object.values(hidden).reduce((total, count) => total + count, 0);

  return useMemo(
    () =>
      enabled
        ? {
            chips,
            hiddenCount,
            accept: (chipId: string, entityId: number) => {
              const accepted = acceptChip(chips, latest.current.draft, chipId, entityId);
              setChips(accepted.chips);
              if (accepted.replacement !== null) {
                setDraft(accepted.replacement.text);
              }
            },
            ignore: (chipId: string) => setChips((current) => withoutChip(current, chipId)),
            reveal: () => onRevealNamesChange(true),
          }
        : undefined,
    [enabled, chips, hiddenCount, setDraft, onRevealNamesChange],
  );
}
