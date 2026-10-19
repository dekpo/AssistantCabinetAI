import type { LicenceRating, SpeechStatus } from "./ipc";

/**
 * The data of the credits view of the voice (phase 13): which components are credited and which
 * voices, as catalogue keys. The sentences themselves live in the two catalogues (`speech.credits.*`),
 * because what a person reads is in their language; the Parakeet notice is the attribution text
 * the licence asks for. A voice row is made from what Rust reports (`speech_status.voices`, one row
 * per voice of the manifest), so a voice added to the manifest appears here with its rating, and a
 * test fails until its sentence exists.
 */

export interface ComponentCredit {
  id: string;
  /** The catalogue key of the credit sentence. */
  textKey: string;
}

export interface VoiceCredit {
  id: string;
  textKey: string;
  /** Never left out: a voice whose licence has to be read says so wherever it is listed. */
  rating: LicenceRating;
}

/** The components, in the order they are shown. Not read from Rust: they do not depend on what is installed. */
export const COMPONENT_CREDITS: ComponentCredit[] = [
  { id: "parakeet", textKey: "speech.credits.component.parakeet" },
  { id: "sherpa", textKey: "speech.credits.component.sherpa" },
  { id: "espeak", textKey: "speech.credits.component.espeak" },
  { id: "voices", textKey: "speech.credits.component.voices" },
];

export function voiceCreditKey(id: string): string {
  return `speech.credits.voice.${id}`;
}

/** The voices to credit: all those of the manifest, installed or not, in Rust's order. Empty before Rust answered. */
export function voiceCredits(status: SpeechStatus | null): VoiceCredit[] {
  return (status?.voices ?? []).map((voice) => ({
    id: voice.id,
    textKey: voiceCreditKey(voice.id),
    rating: voice.rating,
  }));
}
