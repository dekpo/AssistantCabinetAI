import { useEffect, useState } from "react";
import { browserSynthesis, loadVoices, summariseVoices, type VoiceSummary } from "../speech/output/systemVoices";

/**
 * The system voices for a language, read when `enabled` (the settings block shows them only when the
 * system engine is chosen). Listing voices starts nothing and speaks nothing. `null` until the list
 * has been read; an empty summary where the window has no speech synthesis.
 */
export function useSystemVoices(enabled: boolean, locale: string): VoiceSummary | null {
  const [summary, setSummary] = useState<VoiceSummary | null>(null);

  useEffect(() => {
    if (!enabled) {
      setSummary(null);
      return undefined;
    }
    const synth = browserSynthesis();
    if (synth === null) {
      setSummary({ names: [], ignored: 0 });
      return undefined;
    }
    let current = true;
    void loadVoices(synth).then((voices) => {
      if (current) {
        setSummary(summariseVoices(voices, locale));
      }
    });
    return () => {
      current = false;
    };
  }, [enabled, locale]);

  return summary;
}
