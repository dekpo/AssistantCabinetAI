import regularFontUrl from "../assets/fonts/NotoSans-Regular.ttf?url";

/**
 * The one place that talks to the document viewer library (`shrimp-doc-viewer`). Nothing else may
 * import it, so the library can be replaced without touching the panels. The library is loaded on
 * first use, so a user who never opens a file never pays for its WebAssembly.
 *
 * The library's own `index.d.ts` is stale (it omits `format`, `plain`, `fonts`, `odfKind` and still
 * says the Office formats throw), so the options and the handle are typed here from its sources.
 *
 * Offline by construction: the font, the WebAssembly files and the workers are all assets of the
 * bundle (`vite.config.ts` keeps the library's own 7 MB default font out of it), and the content
 * security policy lets the window fetch nothing else.
 */

/** The engine names the library accepts (`dispatch` in its `index.js`). */
export type ViewerFormat =
  | "pdf"
  | "image"
  | "csv"
  | "text"
  | "rtf"
  | "xlsx"
  | "docx"
  | "pptx"
  | "odt"
  | "ods"
  | "odp";

export interface ViewerMountOptions {
  /** Forced: the library never guesses, because a wrong guess shows the wrong thing. */
  format: ViewerFormat;
  /** Text engine only: show the characters as stored instead of rendering Markdown. */
  plain?: boolean;
  zoom?: number;
  /** A CSS height that turns the viewer into a box that scrolls on its own. */
  height?: string;
  scrollRoot?: Element | null;
  /** XLSX and ODS: the sheet to show, 0-based. */
  sheetIndex?: number;
  onZoom?: (zoom: number) => void;
  /** Ask for the worker and fail when it cannot start, instead of the silent main-thread fallback. */
  useWorker?: boolean;
}

/** What `mount` resolves to; the members present depend on the format. */
export interface MountedViewer {
  destroy(): void;
  pageCount?: number;
  sheetNames?: string[];
  readonly zoom?: number;
  setZoom?(zoom: number): void;
  zoomIn?(): void;
  zoomOut?(): void;
  fitWidth?(): void;
  /** XLSX and ODS: switches the sheet without remounting. */
  setSheet?(index: number): void;
  /** PPTX: the slides, shown one at a time. */
  readonly slideCount?: number;
  readonly slideIndex?: number;
  next?(): void;
  prev?(): void;
}

type LibraryMount = (
  target: HTMLElement,
  source: ArrayBuffer | Uint8Array,
  options: ViewerMountOptions & { fontUrl: string },
) => Promise<MountedViewer>;

/**
 * Draws `bytes` into `host`. The library moves the buffer into a worker, which empties it, so a
 * caller that needs the bytes afterwards passes a copy.
 */
export async function mountDocument(
  host: HTMLElement,
  bytes: ArrayBuffer | Uint8Array,
  options: ViewerMountOptions,
): Promise<MountedViewer> {
  const library = await import("shrimp-doc-viewer");
  const mount = library.mount as unknown as LibraryMount;
  return mount(host, bytes, { ...options, fontUrl: regularFontUrl });
}
