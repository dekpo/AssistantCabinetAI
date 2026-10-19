import { voicesOfResolvedLanguage } from "./availability";
import type { LicenceRating, ModelState, SpeechStatus, VoiceGender } from "./ipc";

/**
 * The reading voices of the language speech works in, with what is on disk for each and its licence
 * rating: the rows of the "voices on this computer" part of the settings, and the options of the
 * voice list. A pure function of what Rust answered, so it is tested without a window. The rating
 * is never left out: a voice whose licence still has to be read says so wherever it is listed.
 */

export interface VoiceRow {
  id: string;
  gender: VoiceGender;
  state: ModelState;
  rating: LicenceRating | null;
  /** Size of the download, in whole megabytes (1 MB = 1,000,000 bytes), or 0 when unknown. */
  megabytes: number;
}

const BYTES_PER_MEGABYTE = 1_000_000;

export function voiceRows(status: SpeechStatus | null): VoiceRow[] {
  return voicesOfResolvedLanguage(status).map((voice) => {
    const model = status?.voices.find((candidate) => candidate.id === voice.id);
    return {
      id: voice.id,
      gender: voice.gender,
      state: model?.state ?? "missing",
      rating: model?.rating ?? null,
      megabytes: model === undefined ? 0 : Math.round(model.bytes / BYTES_PER_MEGABYTE),
    };
  });
}
