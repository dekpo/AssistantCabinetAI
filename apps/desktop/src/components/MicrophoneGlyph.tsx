/**
 * A microphone: a capsule on a stand. Decorative - the button that holds it carries the words, in
 * the user's language.
 */
export function MicrophoneGlyph() {
  return (
    <svg className="glyph" viewBox="0 0 20 20" aria-hidden="true">
      <rect
        x="7.5"
        y="2.5"
        width="5"
        height="9"
        rx="2.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
      />
      <path
        d="M4.5 9.5 A5.5 5.5 0 0 0 15.5 9.5 M10 15 V17.5 M7 17.5 H13"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
