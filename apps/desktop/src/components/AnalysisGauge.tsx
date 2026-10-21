import { StopGlyph } from "./StopGlyph";

/**
 * The Analyser button while its pass runs: a spinner and « Lecture 45 % » over a fill that follows
 * the percentage, then a square stop button beside it. It looks like a button that cannot be
 * pressed, but it is not a disabled one: the text stays at full contrast, and `role="status"` lets a
 * screen reader hear the percentage as it moves. The Stop is a button of its own, always pressable
 * until it has been pressed once.
 */
export function AnalysisGauge({
  label,
  percent,
  title,
  stopLabel,
  stopHint,
  onStop,
  stopping,
}: {
  label: string;
  /** 0 to 100: the width of the fill. */
  percent: number;
  /** The gauge's tooltip: the full line under the header, for a narrow zone. */
  title?: string;
  stopLabel: string;
  stopHint: string;
  onStop: () => void;
  /** The Stop was pressed and the pass has not ended yet: the button waits. */
  stopping: boolean;
}) {
  return (
    <div className="gauge">
      <span className="gauge__body" role="status" title={title}>
        <span className="gauge__fill" style={{ width: `${percent}%` }} aria-hidden="true" />
        <span className="spinner gauge__spinner" aria-hidden="true" />
        <span className="gauge__label">{label}</span>
      </span>
      <button
        type="button"
        className="gauge__stop"
        aria-label={stopLabel}
        title={stopHint}
        disabled={stopping}
        onClick={onStop}
      >
        <StopGlyph />
      </button>
    </div>
  );
}
