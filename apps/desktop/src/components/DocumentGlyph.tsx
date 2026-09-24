/**
 * A sheet with a folded corner and two lines of text: the card is about text documents, as
 * distinct from the tabular folder that will sit beside it. Decorative - the heading next to it
 * carries the words, in her language.
 */
export function DocumentGlyph() {
  return (
    <svg className="card__glyph" viewBox="0 0 20 20" aria-hidden="true">
      <path
        d="M5 2.5 H12 L15.5 6 V17.5 H5 Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinejoin="round"
      />
      <path
        d="M11.75 2.75 V6.25 H15.25"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinejoin="round"
      />
      <path
        d="M7.5 10.5 H13 M7.5 13.5 H13"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
      />
    </svg>
  );
}
