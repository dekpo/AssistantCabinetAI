/**
 * A loudspeaker with sound waves. Decorative - the button that holds it carries the words, in the
 * user's language. While a reading is going, the waves pulse (`glyph--speaking`).
 */
export function SpeakerGlyph({ speaking = false }: { speaking?: boolean }) {
  return (
    <svg
      className={speaking ? "glyph glyph--speaking" : "glyph"}
      viewBox="0 0 20 20"
      aria-hidden="true"
    >
      <path
        d="M3 7.5 H6 L10 4 V16 L6 12.5 H3 Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinejoin="round"
      />
      <path
        className="glyph__waves"
        d="M12.5 7.5 A3.5 3.5 0 0 1 12.5 12.5 M14.5 5.5 A6.5 6.5 0 0 1 14.5 14.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
      />
    </svg>
  );
}
