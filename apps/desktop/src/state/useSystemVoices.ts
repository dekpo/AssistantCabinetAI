import { useEffect, useState } from "react";
import {
  browserSynthesis,
  loadVoices,
  localVoiceChoices,
  summariseVoices,
  type VoiceChoice,
  type VoiceSummary,
} from "../speech/output/systemVoices";

/** What the system offers for a language: the summary of the settings line and the voices to choose. */
export interface SystemVoiceList extends VoiceSummary {
  choices: VoiceChoice[];
}

/**
 * The system voices for a language, read when `enabled` (the settings block shows them only when the
 * system engine is chosen). Listing voices starts nothing and speaks nothing. `null` until the list
 * has been read; an empty summary where the window has no speech synthesis.
 */
export function useSystemVoices(enabled: boolean, locale: string): SystemVoiceList | null {
  const [summary, setSummary] = useState<SystemVoiceList | null>(null);

  useEffect(() => {
    if (!enabled) {
      setSummary(null);
      return undefined;
    }
    const synth = browserSynthesis();
    if (synth === null) {
      setSummary({ names: [], ignored: 0, choices: [] });
      return undefined;
    }
    let current = true;
    void loadVoices(synth).then((voices) => {
      if (current) {
        setSummary({
          ...summariseVoices(voices, locale),
          choices: localVoiceChoices(voices, locale),
        });
      }
    });
    return () => {
      current = false;
    };
  }, [enabled, locale]);

  return summary;
}
