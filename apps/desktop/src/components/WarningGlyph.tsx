/**
 * A triangle with an exclamation mark, in the software's own quiet warning colour. Decorative:
 * the tooltip on the element that carries it says what it means, in her language.
 */
export function WarningGlyph() {
  return (
    <svg className="warning-glyph" viewBox="0 0 20 20" aria-hidden="true">
      <path
        d="M10 3 L18 17 H2 Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinejoin="round"
      />
      <path
        d="M10 8.25 V12.25"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
      <circle cx="10" cy="14.6" r="0.9" fill="currentColor" />
    </svg>
  );
}
