/**
 * A window with its middle column drawn in: the centre zone of the layout, which the button next
 * to it opens and closes. Decorative - the button carries the words, in the user's language.
 */
export function WorkspaceGlyph() {
  return (
    <svg className="glyph" viewBox="0 0 20 20" aria-hidden="true">
      <rect
        x="2.5"
        y="3.5"
        width="15"
        height="13"
        rx="1.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinejoin="round"
      />
      <path d="M7.5 3.75 V16.25 M12.5 3.75 V16.25" fill="none" stroke="currentColor" strokeWidth="1.4" />
    </svg>
  );
}
