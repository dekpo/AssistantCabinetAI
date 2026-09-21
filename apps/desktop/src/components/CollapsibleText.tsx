import { useEffect, useRef, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { overflowsClamp } from "../lib/clamp";

/**
 * A long question shows its first lines and opens on demand. Plain text, never markdown, so
 * `white-space: pre-wrap` stays and the line breaks she typed are the ones she reads back.
 *
 * `<details>` would be the wrong element: it hides its content when closed, and a collapsed
 * question must stay readable. So a button carrying `aria-expanded`, and the clamp in the
 * stylesheet.
 */
export function CollapsibleText({ text }: { text: string }) {
  const { t } = useTranslation();
  const body = useRef<HTMLParagraphElement>(null);
  const [expanded, setExpanded] = useState(false);
  const [overflows, setOverflows] = useState(false);

  useEffect(() => {
    const element = body.current;
    // Measured only while clamped: an open question no longer overflows, and re-measuring it
    // would take away the toggle that closes it again.
    if (element === null || expanded) {
      return;
    }
    const measure = () => setOverflows(overflowsClamp(element.scrollHeight, element.clientHeight));
    measure();
    // The bubble width follows the window, and a wider bubble is a shorter question.
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [text, expanded]);

  return (
    <>
      <p ref={body} className={expanded ? "message__body" : "message__body message__body--clamped"}>
        {text}
      </p>
      {overflows ? (
        <button
          type="button"
          className="disclosure message__collapse"
          aria-expanded={expanded}
          onClick={() => setExpanded((open) => !open)}
        >
          {t(expanded ? "chat.showLess" : "chat.showMore")}
        </button>
      ) : null}
    </>
  );
}
