import type { ExplorerState, Family } from "../lib/explorerModel";

/*
 * The small drawings of the file explorer. Decorative or labelled by the caller: a drawing here
 * carries no word of its own, the words come from the catalogues (`docs/LANGUAGE-AND-LOCALE.md`).
 * The shape of a state glyph, not only its colour, says the state: a filled disc, a hollow ring, a
 * triangle.
 */

/** A file's state: filled disc (analysed), hollow ring (to analyse, or changed since), triangle
 * (unreadable). `label` is what a screen reader says. */
export function StateGlyph({ state, label }: { state: ExplorerState; label: string }) {
  return (
    <svg
      className={`state-glyph state-glyph--${state}`}
      viewBox="0 0 12 12"
      role="img"
      aria-label={label}
    >
      {state === "analysed" ? (
        <circle cx="6" cy="6" r="5" fill="currentColor" />
      ) : state === "toAnalyse" ? (
        <circle cx="6" cy="6" r="4.2" fill="none" stroke="currentColor" strokeWidth="2" />
      ) : (
        <path d="M6 1 L11.2 10.5 H0.8 Z" fill="currentColor" />
      )}
    </svg>
  );
}

/** A small mark on a spreadsheet that is read with a caveat: a circle with an exclamation mark. */
export function CaveatGlyph() {
  return (
    <svg className="caveat-glyph" viewBox="0 0 12 12" aria-hidden="true">
      <circle cx="6" cy="6" r="5" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <path d="M6 3 V6.6" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      <circle cx="6" cy="8.6" r="0.9" fill="currentColor" />
    </svg>
  );
}

/** The folder of a family, in the family's colour: lines of text for documents, a grid for tables.
 * Two different looks, so the two sections are told apart without reading. */
export function FamilyFolderGlyph({ family }: { family: Family }) {
  return (
    <svg
      className={`family-glyph family-glyph--${family}`}
      viewBox="0 0 30 26"
      fill="none"
      aria-hidden="true"
    >
      <path
        className="family-glyph__body"
        d="M2 5a2 2 0 0 1 2-2h7l3 3h12a2 2 0 0 1 2 2v13a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2Z"
        strokeWidth="1.5"
      />
      {family === "documents" ? (
        <path
          className="family-glyph__mark"
          d="M8 12h14M8 16h10"
          strokeWidth="1.5"
          strokeLinecap="round"
        />
      ) : (
        <path
          className="family-glyph__mark"
          d="M8 10h14M8 14h14M8 18h14M13 10v8M18 10v8"
          strokeWidth="1.2"
        />
      )}
    </svg>
  );
}

/** The fold arrow of a family: points down while the list is open, right when it is folded. */
export function FoldGlyph({ open }: { open: boolean }) {
  return (
    <svg
      className={open ? "fold-glyph" : "fold-glyph fold-glyph--folded"}
      viewBox="0 0 20 20"
      aria-hidden="true"
    >
      <path
        d="M5 7.5 L10 12.5 L15 7.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/** The magnifying glass of the search field. */
export function SearchGlyph() {
  return (
    <svg className="search-glyph" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="7" cy="7" r="4.5" stroke="currentColor" strokeWidth="1.6" />
      <path d="M10.5 10.5 L14 14" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  );
}
