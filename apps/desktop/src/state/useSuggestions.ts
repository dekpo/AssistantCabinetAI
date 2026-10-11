import { useEffect, useRef, useState } from "react";
import { suggestQuestions, type CombinedScope, type SuggestionSet } from "../lib/ipc";
import { combinedHasSelection } from "../lib/analysisScope";

/** How long the selection must stay as it is before the questions are looked for. A tick box
 * pressed three times in a row asks once. */
export const SUGGESTIONS_DEBOUNCE_MS = 500;

/**
 * The questions worth asking about the current selection (`knowledge_suggest_questions`).
 *
 * Mounted only by the places that draw them (the empty conversation, the analysis summary), so
 * nothing is looked up while no one is looking. A read, started by looking at the screen: no model,
 * no write, and Rust answers a second look at the same screen from memory. An answer that arrives
 * after a newer lookup was started is dropped, and a failed lookup is no suggestion rather than an
 * error: the screen it decorates must never be blocked by it.
 *
 * `refresh` changes whenever the base may have changed (an Analyse finished): the selection alone
 * does not say so. `enabled` is false while an Analyse is writing.
 */
export function useSuggestions(
  scope: CombinedScope,
  revealNames: boolean,
  refresh: unknown,
  enabled: boolean,
): SuggestionSet | null {
  const [set, setSet] = useState<SuggestionSet | null>(null);
  const latest = useRef(0);
  const selected = combinedHasSelection(scope);

  useEffect(() => {
    const ticket = ++latest.current;
    if (!enabled || !selected) {
      setSet(null);
      return undefined;
    }
    const timer = setTimeout(() => {
      suggestQuestions(scope, revealNames).then(
        (found) => {
          if (ticket === latest.current) {
            setSet(found);
          }
        },
        () => {
          if (ticket === latest.current) {
            setSet(null);
          }
        },
      );
    }, SUGGESTIONS_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [scope, revealNames, refresh, enabled, selected]);

  return set;
}
