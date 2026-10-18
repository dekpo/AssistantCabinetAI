/**
 * One microphone, one voice at a time. The chat dictation, the notes pill and the read-aloud
 * playback all go through this small object, owned once by the application: starting a recording
 * stops a playback in progress, a playback cannot start while something is recording, and a second
 * recorder is refused (the interface then disables it with a catalogue sentence) instead of
 * reaching the backend, which would answer `speech_busy`. Pure (no React, no Tauri).
 */

export type VoiceOwner = "dictation" | "notes" | "readAloud";

type Kind = "recording" | "playback";

const KIND: Record<VoiceOwner, Kind> = {
  dictation: "recording",
  notes: "recording",
  readAloud: "playback",
};

export type ClaimResult =
  | { granted: true }
  | {
      granted: false;
      /** The catalogue key of the sentence that says why. */
      reasonKey: "speech.session.microphoneBusy" | "speech.session.recording";
    };

export interface VoiceSession {
  /**
   * Asks for the microphone (or the speakers). `stop` is called if a recorder later takes the place
   * of a playback; it must end the playback and release the claim.
   */
  claim: (owner: VoiceOwner, stop: () => void) => ClaimResult;
  /** Gives the claim back. Ignored when `owner` is not the holder, so a late release of an
   * interrupted playback cannot free somebody else's claim. */
  release: (owner: VoiceOwner) => void;
  holder: () => VoiceOwner | null;
  subscribe: (listener: () => void) => () => void;
}

export function createVoiceSession(): VoiceSession {
  let held: { owner: VoiceOwner; stop: () => void } | null = null;
  const listeners = new Set<() => void>();
  const changed = () => listeners.forEach((listener) => listener());

  return {
    claim: (owner, stop) => {
      if (held !== null && held.owner === owner) {
        held = { owner, stop };
        return { granted: true };
      }
      if (held !== null) {
        const wantsToRecord = KIND[owner] === "recording";
        if (KIND[held.owner] === "recording") {
          return {
            granted: false,
            reasonKey: wantsToRecord
              ? "speech.session.microphoneBusy"
              : "speech.session.recording",
          };
        }
        // A playback gives way to a recording; a playback never replaces another playback.
        if (!wantsToRecord) {
          return { granted: false, reasonKey: "speech.session.microphoneBusy" };
        }
        const interrupted = held;
        held = null;
        interrupted.stop();
      }
      held = { owner, stop };
      changed();
      return { granted: true };
    },
    release: (owner) => {
      if (held !== null && held.owner === owner) {
        held = null;
        changed();
      }
    },
    holder: () => held?.owner ?? null,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}
