import { memo } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

/**
 * An answer read as formatting rather than as punctuation, so `**Patiente**` and `-` bullets stop
 * reaching the screen as literal characters.
 *
 * `react-markdown` builds React elements instead of setting HTML, and no `rehype-raw` is
 * installed, so nothing the model wrote can become markup. That matters here specifically: the
 * answer is written from extracted document text, and a poisoned document must not be able to put
 * anything but text on screen.
 *
 * Formatting is all this carries. A link and an image are stripped to their text: anything that
 * names a file, a path or an action travels on the structured channel beside the stream, where
 * Rust re-validates it (`docs/CHAT-UX-ASSESSMENT.md`, item 1b). A `file:///` in an answer is a
 * path chosen by prose, and clicking it would navigate the window away from the application.
 */
const FORMATTING_ONLY: Components = {
  a: ({ children }) => <>{children}</>,
  img: ({ alt }) => <>{alt}</>,
};

/**
 * Memoised per answer: the array of entries is replaced on every delta, so without this the
 * whole conversation is re-parsed many times a second while one answer is being written.
 */
export const Markdown = memo(function Markdown({ text }: { text: string }) {
  return (
    <ReactMarkdown remarkPlugins={[remarkGfm]} components={FORMATTING_ONLY}>
      {text}
    </ReactMarkdown>
  );
});
