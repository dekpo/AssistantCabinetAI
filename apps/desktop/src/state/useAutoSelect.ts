import { useCallback, useEffect, useRef, useState } from "react";
import { AUTO_OFF, autoStep, type AutoEvent, type AutoState } from "../lib/autoSelect";
import type { ScopesByRoot } from "../lib/explorerSelection";

/**
 * « Select. auto » in the box: the state machine of `lib/autoSelect.ts` held for the life of the box, and
 * the selection it asks for put in place through `apply`. Nothing is stored: the switch is off at every
 * launch. A question being answered locks it (`locked`), as it locks the selection.
 */
export function useAutoSelect({
  locked,
  apply,
}: {
  locked: boolean;
  /** Puts a selection in place: the two scopes the chat is sent with. */
  apply: (scopes: ScopesByRoot) => void;
}): { state: AutoState; send: (event: AutoEvent) => void } {
  const [state, setState] = useState<AutoState>(AUTO_OFF);
  const current = useRef(state);
  const applier = useRef(apply);
  applier.current = apply;

  const send = useCallback((event: AutoEvent) => {
    const step = autoStep(current.current, event);
    current.current = step.state;
    setState(step.state);
    if (step.apply !== null) {
      applier.current(step.apply);
    }
  }, []);

  const wasLocked = useRef(false);
  useEffect(() => {
    if (locked === wasLocked.current) {
      return;
    }
    wasLocked.current = locked;
    send({ type: locked ? "questionStarted" : "questionEnded" });
  }, [locked, send]);

  return { state, send };
}
