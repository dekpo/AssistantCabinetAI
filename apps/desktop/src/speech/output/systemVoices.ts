import {
  SpeechOutputError,
  type EngineVoice,
  type ReadAloudEngine,
  type SpeakOptions,
  type SpeakSegment,
} from "./engine";

/**
 * The operating system's voices through `speechSynthesis`, as a fallback engine. A system voice may
 * be an online one (WebView2 on Windows lists "Natural" voices with `localService === false`, which
 * send the text to a server), so a voice is used only when the system says it is local, and when none
 * is there the answer is a refusal, never a default or an online voice.
 */

/** The parts of a `SpeechSynthesisVoice` that are read. */
export interface SynthesisVoice {
  name: string;
  lang: string;
  voiceURI: string;
  localService: boolean;
}

/** The parts of a `SpeechSynthesisUtterance` that are written and listened to. */
export interface UtteranceLike {
  voice: SynthesisVoice | null;
  lang: string;
  rate: number;
  onend: (() => void) | null;
  onerror: ((event: { error?: string }) => void) | null;
}

export interface SynthesisLike {
  getVoices: () => SynthesisVoice[];
  speak: (utterance: UtteranceLike) => void;
  cancel: () => void;
  addEventListener: (type: "voiceschanged", listener: () => void) => void;
  removeEventListener: (type: "voiceschanged", listener: () => void) => void;
}

function normalised(tag: string): string {
  return tag.replace("_", "-").toLowerCase();
}

function primaryLanguage(tag: string): string {
  return normalised(tag).split("-")[0] ?? "";
}

/** The id of a voice as the settings and the voice list name it. */
export function systemVoiceId(voice: SynthesisVoice): string {
  return voice.voiceURI || voice.name;
}

/**
 * The local voice to read a language with: the exact language first (`fr-FR`), then the same
 * primary language (`fr-CA`); within a group the preferred voice if it is there, else the first.
 * A voice that is not marked local is never chosen; `null` when there is none.
 */
export function pickLocalVoice(
  voices: readonly SynthesisVoice[],
  locale: string,
  preferred: string | null,
): SynthesisVoice | null {
  const local = voices.filter((voice) => voice.localService === true);
  const wanted = normalised(locale);
  const groups = [
    local.filter((voice) => normalised(voice.lang) === wanted),
    local.filter((voice) => primaryLanguage(voice.lang) === primaryLanguage(locale)),
  ];
  for (const group of groups) {
    if (group.length === 0) {
      continue;
    }
    const wantedId = preferred?.toLowerCase();
    const chosen =
      wantedId === undefined
        ? undefined
        : group.find(
            (voice) =>
              systemVoiceId(voice).toLowerCase() === wantedId ||
              voice.name.toLowerCase() === wantedId,
          );
    return chosen ?? group[0] ?? null;
  }
  return null;
}

/** What the system offers for a language: the local voices that would be used, and how many online
 * ones for the same language are ignored. */
export interface VoiceSummary {
  names: string[];
  ignored: number;
}

export function summariseVoices(voices: readonly SynthesisVoice[], locale: string): VoiceSummary {
  const sameLanguage = voices.filter(
    (voice) => primaryLanguage(voice.lang) === primaryLanguage(locale),
  );
  return {
    names: sameLanguage.filter((voice) => voice.localService === true).map((voice) => voice.name),
    ignored: sameLanguage.filter((voice) => voice.localService !== true).length,
  };
}

export interface Timers {
  set: (callback: () => void, ms: number) => unknown;
  clear: (handle: unknown) => void;
}

const BROWSER_TIMERS: Timers = {
  set: (callback, ms) => setTimeout(callback, ms),
  clear: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
};

/** How long to wait for the system to announce its voices (they load after the first call). */
export const VOICES_TIMEOUT_MS = 1500;

/**
 * The voices, waiting for `voiceschanged` when the list is still empty, and giving up after
 * `timeoutMs` with whatever is there.
 */
export function loadVoices(
  synth: SynthesisLike,
  timeoutMs: number = VOICES_TIMEOUT_MS,
  timers: Timers = BROWSER_TIMERS,
): Promise<SynthesisVoice[]> {
  const now = synth.getVoices();
  if (now.length > 0) {
    return Promise.resolve(now);
  }
  return new Promise((resolve) => {
    const done = () => {
      timers.clear(handle);
      synth.removeEventListener("voiceschanged", done);
      resolve(synth.getVoices());
    };
    const handle = timers.set(done, timeoutMs);
    synth.addEventListener("voiceschanged", done);
  });
}

export interface SystemVoiceDeps {
  synth: SynthesisLike;
  makeUtterance: (text: string) => UtteranceLike & { text?: string };
  timers?: Timers;
  voicesTimeoutMs?: number;
}

/** The engine over the system voices: segments one by one, a pause timer between them, one voice at a
 * time (a new `speak` ends the one in progress), `stop` at any moment. */
export function createSystemVoiceEngine(deps: SystemVoiceDeps): ReadAloudEngine {
  const { synth, makeUtterance } = deps;
  const timers = deps.timers ?? BROWSER_TIMERS;
  let run: { cancelled: boolean; wake: (() => void) | null } | null = null;

  const stop = () => {
    const current = run;
    run = null;
    if (current !== null) {
      current.cancelled = true;
      synth.cancel();
      current.wake?.();
    }
  };

  const say = (
    segment: SpeakSegment,
    options: SpeakOptions,
    mine: NonNullable<typeof run>,
    voice: SynthesisVoice,
  ) =>
    new Promise<void>((resolve, reject) => {
      const utterance = makeUtterance(segment.text);
      utterance.voice = voice;
      utterance.lang = voice.lang || options.locale;
      utterance.rate = options.rate;
      utterance.onend = () => resolve();
      utterance.onerror = (event) => {
        // A cancel ends the utterance with an error event that is not a failure.
        if (mine.cancelled || event.error === "canceled" || event.error === "interrupted") {
          resolve();
        } else {
          reject(new SpeechOutputError("tts_failed"));
        }
      };
      mine.wake = () => resolve();
      synth.speak(utterance);
    });

  const rest = (ms: number, mine: NonNullable<typeof run>) =>
    new Promise<void>((resolve) => {
      if (ms <= 0 || mine.cancelled) {
        resolve();
        return;
      }
      const handle = timers.set(() => {
        mine.wake = null;
        resolve();
      }, ms);
      mine.wake = () => {
        timers.clear(handle);
        resolve();
      };
    });

  return {
    id: "system",
    voices: async (locale) => {
      const voices = await loadVoices(synth, deps.voicesTimeoutMs, timers);
      return voices
        .filter((voice) => voice.localService === true)
        .filter((voice) => primaryLanguage(voice.lang) === primaryLanguage(locale))
        .map(
          (voice): EngineVoice => ({ id: systemVoiceId(voice), name: voice.name, lang: voice.lang }),
        );
    },
    speak: async (segments, options) => {
      stop();
      const mine = { cancelled: false, wake: null as (() => void) | null };
      run = mine;
      const voice = pickLocalVoice(
        await loadVoices(synth, deps.voicesTimeoutMs, timers),
        options.locale,
        options.voiceId,
      );
      if (mine.cancelled) {
        return;
      }
      if (voice === null) {
        run = null;
        throw new SpeechOutputError("tts_no_local_voice");
      }
      try {
        options.onProgress?.({ done: 0, total: segments.length });
        for (const [index, segment] of segments.entries()) {
          await say(segment, options, mine, voice);
          if (mine.cancelled) {
            return;
          }
          options.onProgress?.({ done: index + 1, total: segments.length });
          if (index < segments.length - 1) {
            await rest(segment.pauseAfterMs, mine);
            if (mine.cancelled) {
              return;
            }
          }
        }
      } finally {
        if (run === mine) {
          run = null;
        }
      }
    },
    stop,
  };
}

/** The browser's synthesis and utterance, or `null` where the window has none. Read only when the
 * engine is chosen, never at import. */
export function browserSystemVoiceEngine(): ReadAloudEngine | null {
  if (typeof window === "undefined" || typeof window.speechSynthesis === "undefined") {
    return null;
  }
  const synth = window.speechSynthesis as unknown as SynthesisLike;
  return createSystemVoiceEngine({
    synth,
    makeUtterance: (text) => new SpeechSynthesisUtterance(text) as unknown as UtteranceLike,
  });
}

/** The system's own list of voices, or an empty one where there is no synthesis. */
export function browserSynthesis(): SynthesisLike | null {
  if (typeof window === "undefined" || typeof window.speechSynthesis === "undefined") {
    return null;
  }
  return window.speechSynthesis as unknown as SynthesisLike;
}
