import { useEffect, useRef, useState } from "react";
import { debounce, PICKER_DEBOUNCE_MS, PICKER_LIMIT } from "../lib/entityPicker";
import { searchEntities, type CombinedScope, type EntitySearch } from "../lib/ipc";

const NOTHING: EntitySearch = { entities: [], outsideSelection: [] };

/**
 * The names of the selection that begin with `prefix`, looked up shortly after the last keystroke.
 *
 * `prefix` is `null` while no `@name` is being typed, which asks for nothing and clears what was
 * found. An answer that arrives after a newer question was asked is dropped, and a failed lookup is
 * an empty list rather than an error: typing a question must never be blocked by the picker.
 */
export function useEntitySearch(
  prefix: string | null,
  scope: CombinedScope,
): { result: EntitySearch; searching: boolean } {
  const [result, setResult] = useState<EntitySearch>(NOTHING);
  const [searching, setSearching] = useState(false);
  /** Which lookup is the latest, so a slow answer to an old prefix cannot overwrite a newer one. */
  const latest = useRef(0);

  useEffect(() => {
    const ticket = ++latest.current;
    if (prefix === null || prefix.length === 0) {
      setResult(NOTHING);
      setSearching(false);
      return undefined;
    }
    setSearching(true);
    const lookup = debounce((typed: string) => {
      searchEntities(typed, scope, PICKER_LIMIT).then(
        (found) => {
          if (ticket === latest.current) {
            setResult(found);
            setSearching(false);
          }
        },
        () => {
          if (ticket === latest.current) {
            setResult(NOTHING);
            setSearching(false);
          }
        },
      );
    }, PICKER_DEBOUNCE_MS);
    lookup(prefix);
    return () => lookup.cancel();
  }, [prefix, scope]);

  return { result, searching };
}
