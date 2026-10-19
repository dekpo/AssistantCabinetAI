import { SpeechOutputError, type EngineVoice, type ReadAloudEngine, type SpeakOptions, type SpeakSegment } from "./engine";
import type { SpeechStatus } from "../ipc";
import type { BufferPlayer } from "./playback";

/**
 * The read-aloud engine over the voice worker of Rust. The worker speaks one segment at a time and
 * hands the audio back as a WAV; this engine plays the segments in order through the Web Audio
 * player and, while one plays, asks for the next one, so a pause between two sentences is the pause
 * of the text and not the time the voice takes. Only one segment is prepared ahead: the voice works
 * at about a tenth of real time, so a longer queue would only hold audio in memory.
 *
 * The voice and the speed are not chosen here: Rust reads them from the settings (the window sends
 * the text and the language and nothing else), and the player runs at the natural rate because a
 * changed playback rate would also change the pitch. Nothing here names a network, a blob, a media
 * element or a store; the audio lives in memory for as long as it takes to play it.
 */

export interface PiperDeps {
  /** Speaks one segment: the WAV of `text`. Rejects with a machine code (`code`). */
  synthesize: (text: string, locale: string) => Promise<Uint8Array>;
  /** Stops the synthesis in progress. Fire and forget. */
  cancel: () => Promise<unknown>;
  player: BufferPlayer;
  /** The voices that would read a language, for a list of voices. */
  voices: (locale: string) => Promise<EngineVoice[]>;
  timers?: Timers;
}

export interface Timers {
  set: (callback: () => void, ms: number) => unknown;
  clear: (handle: unknown) => void;
}

const BROWSER_TIMERS: Timers = {
  set: (callback, ms) => setTimeout(callback, ms),
  clear: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
};

/** The code Rust answers when a newer request or a stop ended a synthesis: not a failure. */
const CANCELLED = "speech_cancelled";

function codeOf(error: unknown): string {
  if (typeof error === "object" && error !== null && "code" in error) {
    const { code } = error as { code: unknown };
    if (typeof code === "string" && code.length > 0) {
      return code;
    }
  }
  return "tts_failed";
}

/** The voices Rust reports installed for a language, in the order of the language table. */
export function installedVoices(status: SpeechStatus, locale: string): EngineVoice[] {
  const row = status.languages.find((language) => language.id.toLowerCase() === locale.toLowerCase());
  return (row?.voices ?? [])
    .filter((voice) =>
      status.voices.some((model) => model.id === voice.id && model.state === "present"),
    )
    .map((voice) => ({ id: voice.id, name: voice.id, lang: row?.id ?? locale }));
}

export function createPiperEngine(deps: PiperDeps): ReadAloudEngine {
  const timers = deps.timers ?? BROWSER_TIMERS;
  interface Run {
    cancelled: boolean;
    wake: (() => void) | null;
    /** Settles, with null, the moment the reading is stopped, so a wait on the voice never outlives it. */
    stopped: Promise<null>;
    markStopped: () => void;
  }
  let run: Run | null = null;

  const stop = () => {
    const current = run;
    run = null;
    if (current !== null) {
      current.cancelled = true;
      current.markStopped();
      deps.player.stop();
      void deps.cancel().catch(() => undefined);
      current.wake?.();
    }
  };

  const rest = (ms: number, mine: Run) =>
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
    id: "piper",
    voices: deps.voices,
    speak: async (segments: SpeakSegment[], options: SpeakOptions) => {
      stop();
      let markStopped: () => void = () => undefined;
      const stopped = new Promise<null>((resolve) => {
        markStopped = () => resolve(null);
      });
      const mine: Run = { cancelled: false, wake: null, stopped, markStopped };
      run = mine;

      /* A segment is requested at once and its outcome is kept as a value, so that a failure of the
         one prepared ahead surfaces when its turn comes and never as an unhandled rejection. */
      const prepare = (index: number) =>
        deps.synthesize(segments[index]?.text ?? "", options.locale).then(
          (wav) => ({ wav }) as const,
          (error: unknown) => ({ error }) as const,
        );

      try {
        options.onProgress?.({ done: 0, total: segments.length });
        let upcoming = segments.length > 0 ? prepare(0) : null;
        for (const [index, segment] of segments.entries()) {
          const outcome = await Promise.race([upcoming, mine.stopped]);
          if (mine.cancelled || outcome === null) {
            return;
          }
          if ("error" in outcome) {
            const code = codeOf(outcome.error);
            if (code === CANCELLED) {
              return;
            }
            throw new SpeechOutputError(code);
          }
          upcoming = index + 1 < segments.length ? prepare(index + 1) : null;
          try {
            await deps.player.play(outcome.wav);
          } catch {
            if (mine.cancelled) {
              return;
            }
            throw new SpeechOutputError("tts_failed");
          }
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
      } catch (error) {
        // A failure ends the reading: nothing prepared ahead may go on in the background.
        if (!mine.cancelled) {
          mine.cancelled = true;
          void deps.cancel().catch(() => undefined);
        }
        throw error;
      } finally {
        if (run === mine) {
          run = null;
        }
      }
    },
    stop,
  };
}
