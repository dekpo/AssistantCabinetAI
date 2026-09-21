/** Small keyboard glyphs for the composer hint. Decorative; the catalogue carries the words. */
export function KeyGlyph({ name }: { name: "enter" | "shift" }) {
  if (name === "shift") {
    return (
      <svg className="key-glyph" viewBox="0 0 20 20" aria-hidden="true">
        <rect x="1.5" y="1.5" width="17" height="17" rx="3" fill="none" stroke="currentColor" strokeWidth="1.4" />
        <path
          d="M10 5.5 L14 11 H12 V14.5 H8 V11 H6 Z"
          fill="currentColor"
        />
      </svg>
    );
  }
  return (
    <svg className="key-glyph" viewBox="0 0 24 20" aria-hidden="true">
      <rect x="1.5" y="1.5" width="21" height="17" rx="3" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <path
        d="M8 6 H15 V10 H17.5 L13 14.5 L8.5 10 H11 V8 H8 Z"
        fill="currentColor"
      />
    </svg>
  );
}
