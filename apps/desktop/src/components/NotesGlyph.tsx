/**
 * A sheet with three lines of writing: the Notes button of the title bar. Decorative - the button
 * carries the words, in the user's language.
 */
export function NotesGlyph() {
  return (
    <svg className="glyph" viewBox="0 0 20 20" aria-hidden="true">
      <rect
        x="3.5"
        y="2.5"
        width="13"
        height="15"
        rx="1.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinejoin="round"
      />
      <path
        d="M6.75 7 H13.25 M6.75 10 H13.25 M6.75 13 H10.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
      />
    </svg>
  );
}
