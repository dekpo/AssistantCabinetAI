/*
 * The machine codes a pass can report that name a missing capability, and the sentence each deserves.
 * What the last pass did is drawn by the overview (`lib/overviewModel.ts`).
 */

/** The machine codes a pass can report, and the sentence each one deserves. A code we do not
    recognise is dropped rather than shown raw: Rust names capabilities, this file names them in
    her language (`docs/LANGUAGE-AND-LOCALE.md`). */
export const CAPABILITY_KEYS: Record<string, string> = {
  ocrEngine: "workFolder.ocrEngineUnavailable",
  pageRasterizer: "workFolder.pageRasterizerUnavailable",
};
