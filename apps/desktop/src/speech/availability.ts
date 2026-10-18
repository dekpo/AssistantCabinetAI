import type { ModelState, SpeechFeature, SpeechLanguage, SpeechStatus } from "./ipc";

/**
 * What the settings block and the features that follow it say about "can this run". A pure
 * function of what Rust answered and of what the webview can capture, so the matrix is tested
 * without a window. Every sentence is a catalogue key; a Rust reason code maps to the sentence that
 * already exists for it (`errors.<code>`), and nothing here adds a duplicate.
 */

/** What the webview offers for capturing audio. */
export type MicrophoneSupport = "supported" | "unsupported";

export type Availability =
  /** Nothing was answered yet: no sentence, nothing changes on screen. */
  | { state: "unknown"; reasonKey: null }
  /** The feature is switched off, which is a choice and not a problem: no sentence. */
  | { state: "off"; reasonKey: null }
  /** Something is missing; `reasonKey` is the sentence that says what. */
  | { state: "blocked"; reasonKey: string }
  | { state: "ready"; reasonKey: null };

/** The code Rust answers when the switch, not a missing piece, is why a feature does not run. */
const SWITCHED_OFF = "speech_unavailable";

/** Features that listen, and so need the microphone as well as the engine. */
const NEEDS_MICROPHONE: ReadonlySet<SpeechFeature> = new Set<SpeechFeature>(["dictation"]);

export function featureAvailability(
  status: SpeechStatus | null,
  feature: SpeechFeature,
  microphone: MicrophoneSupport,
): Availability {
  if (status === null) {
    return { state: "unknown", reasonKey: null };
  }
  const answer = status.features[feature];
  if (!answer.available) {
    if (answer.reason === null || answer.reason === SWITCHED_OFF) {
      return { state: "off", reasonKey: null };
    }
    return { state: "blocked", reasonKey: `errors.${answer.reason}` };
  }
  if (NEEDS_MICROPHONE.has(feature) && microphone === "unsupported") {
    return { state: "blocked", reasonKey: "speech.microphone.unsupported" };
  }
  return { state: "ready", reasonKey: null };
}

/** The catalogue key of the model line ("ready", "not installed", "damaged"). */
export function modelStatusKey(model: ModelState): string {
  return `speech.model.${model}`;
}

/** Whether the pieces behind the Install button exist. The download and the engine arrive with the
 * speech-to-text worker; the flag moves to true in that phase and the button then works. */
export const INSTALL_AVAILABLE = false;

/** Whether the read-aloud engine and voices exist. They arrive with the read-aloud button, and the
 * controls stay shown but disabled until then. */
export const READ_ALOUD_CONTROLS_AVAILABLE = false;

export interface InstallButton {
  disabled: boolean;
  /** The sentence that says why, or null when the button is usable. */
  reasonKey: string | null;
}

/**
 * The Install button. The download and the worker that uses the model arrive with the speech-to-text
 * worker, so until then the button is shown and disabled with its reason; a model already present
 * needs no install, and says so through the model line instead.
 */
export function installButton(installAvailable: boolean): InstallButton {
  return installAvailable
    ? { disabled: false, reasonKey: null }
    : { disabled: true, reasonKey: "speech.install.notYet" };
}

/** The languages the override may name: the table's enabled rows, in the table's order. */
export function overrideLanguages(status: SpeechStatus | null): SpeechLanguage[] {
  return (status?.languages ?? []).filter((language) => language.support === "enabled");
}

/** The voices of the language speech resolved to; none when it is not in the table. */
export function voicesOfResolvedLanguage(status: SpeechStatus | null) {
  const resolved = status?.languages.find(
    (language) => language.id.toLowerCase() === status.locale.toLowerCase(),
  );
  return resolved?.voices ?? [];
}
