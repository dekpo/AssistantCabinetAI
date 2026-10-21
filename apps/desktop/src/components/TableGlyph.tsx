/**
 * A grid of cells: a table (CSV, XLSX), as distinct from a text document. Decorative - the title
 * next to it carries the words, in her language.
 */
export function TableGlyph() {
  return (
    <svg className="file-glyph" viewBox="0 0 20 20" aria-hidden="true">
      <path
        d="M3 4 H17 V16 H3 Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinejoin="round"
      />
      <path
        d="M3 8.5 H17 M3 12.5 H17 M9 4 V16"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
      />
    </svg>
  );
}
