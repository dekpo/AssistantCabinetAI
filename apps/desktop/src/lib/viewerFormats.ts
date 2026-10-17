import type { ViewerFormat } from "./viewerEngine";

/**
 * Which renderer shows a file, decided from its extension and never from its content: the library
 * sniffs bytes and a wrong guess shows the wrong thing, so the engine is forced here. Three
 * answers: the library, the application's own plain text view (so the characters appear exactly as
 * stored: the library's text engine joins single line breaks), or "unsupported".
 */
export type ViewerRenderer =
  | { renderer: "library"; format: ViewerFormat }
  | { renderer: "text" }
  | { renderer: "unsupported" };

const LIBRARY_FORMATS: Readonly<Record<string, ViewerFormat>> = {
  pdf: "pdf",
  docx: "docx",
  xlsx: "xlsx",
  pptx: "pptx",
  odt: "odt",
  ods: "ods",
  odp: "odp",
  rtf: "rtf",
  csv: "csv",
  png: "image",
  jpg: "image",
  jpeg: "image",
  gif: "image",
  bmp: "image",
  webp: "image",
  svg: "image",
};

/** Shown as stored, line by line, without Markdown rendering. */
const TEXT_EXTENSIONS: ReadonlySet<string> = new Set(["txt", "md", "srt", "vtt", "eml"]);

/**
 * How a file can be looked at. "raw" is the characters as stored, the default and the only mode of
 * most formats; "preview" is a rendered reading of them. A typed value so a later tab can hold it.
 */
export type PreviewMode = "raw" | "preview";

/** Extensions whose text has a rendered reading besides the raw one. */
const PREVIEWABLE_EXTENSIONS: ReadonlySet<string> = new Set(["md"]);

/**
 * The modes a file offers, the default (raw) first. The switch is shown only when there are two or
 * more, so every format without a rendered reading gets no control at all.
 */
export function previewModes(extension: string): PreviewMode[] {
  const key = extension.trim().replace(/^\./, "").toLowerCase();
  return PREVIEWABLE_EXTENSIONS.has(key) ? ["raw", "preview"] : ["raw"];
}

/**
 * `extension` is the one `FileRecord` carries (lowercase, no dot); case and a leading dot are
 * tolerated. Everything else is unsupported, the legacy binary formats (`doc`, `xls`, `ppt`) and
 * `tiff` and `heic` included: they are never advertised and never converted.
 */
export function viewerRendererOf(extension: string): ViewerRenderer {
  const key = extension.trim().replace(/^\./, "").toLowerCase();
  const format = Object.hasOwn(LIBRARY_FORMATS, key) ? LIBRARY_FORMATS[key] : undefined;
  if (format !== undefined) {
    return { renderer: "library", format };
  }
  if (TEXT_EXTENSIONS.has(key)) {
    return { renderer: "text" };
  }
  return { renderer: "unsupported" };
}
