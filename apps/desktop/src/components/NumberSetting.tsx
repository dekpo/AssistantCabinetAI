import { useEffect, useState } from "react";

/**
 * One number of the settings dialog, committed when she leaves the field rather than on every
 * keystroke: typing "300" would otherwise save 3, then 30. `commit` applies what was typed and
 * returns the text to show afterwards (the value Rust will keep, clamped), so a number outside its
 * bounds is visibly pulled back instead of staying on screen as though it had been accepted.
 */
export function NumberSetting({
  label,
  description,
  value,
  min,
  max,
  step,
  unit,
  commit,
}: {
  label: string;
  description: string;
  /** The stored value, as the field shows it. */
  value: string;
  min: number;
  max: number;
  step: number;
  unit?: string;
  commit: (typed: string) => string;
}) {
  const [draft, setDraft] = useState(value);

  /* A reset or a save replaces the stored value underneath the draft; without this the old text
     would sit in the field looking like a setting that survived. */
  useEffect(() => {
    setDraft(value);
  }, [value]);

  return (
    <label className="field">
      <span className="field__label">{label}</span>
      <span className="field__description">{description}</span>
      <div className="field__row">
        <input
          type="number"
          className="field__control field__control--number"
          min={min}
          max={max}
          step={step}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={() => {
            if (draft.trim() !== value) {
              setDraft(commit(draft));
            }
          }}
        />
        {unit === undefined ? null : <span className="field__unit">{unit}</span>}
      </div>
    </label>
  );
}
