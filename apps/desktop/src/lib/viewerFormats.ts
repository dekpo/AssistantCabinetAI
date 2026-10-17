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

/**
 * What the second mode of a file is. "markdown": the text drawn as formatting by the application's
 * own Markdown component. "readable": the text the assistant reads (mail headers and body decoded,
 * one line per subtitle cue), produced by Rust, shown as plain text.
 */
export type PreviewKind = "markdown" | "readable";

/** Extensions that have a second mode besides the raw one, and which. */
const PREVIEW_KINDS: Readonly<Record<string, PreviewKind>> = {
  md: "markdown",
  eml: "readable",
  srt: "readable",
  vtt: "readable",
};

function extensionKey(extension: string): string {
  return extension.trim().replace(/^\./, "").toLowerCase();
}

/** The kind of second mode a file has, or `null` when it has only the raw one. */
export function previewKind(extension: string): PreviewKind | null {
  const key = extensionKey(extension);
  return (Object.hasOwn(PREVIEW_KINDS, key) ? PREVIEW_KINDS[key] : undefined) ?? null;
}

/**
 * The modes a file offers, the default (raw) first. The switch is shown only when there are two or
 * more, so every format without a second mode gets no control at all.
 */
export function previewModes(extension: string): PreviewMode[] {
  return previewKind(extension) === null ? ["raw"] : ["raw", "preview"];
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
