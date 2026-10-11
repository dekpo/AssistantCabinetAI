import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent, ReactNode } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import {
  insertAt,
  insertMention,
  mentionAt,
  offeredChoices,
  pickerKey,
  type Chip,
} from "../lib/entityPicker";
import { canDiscard, type GenerationPhase } from "../lib/generation";
import type { CombinedScope, EntityChoice, OutsideSelection } from "../lib/ipc";
import { useEntitySearch } from "../state/useEntitySearch";
import { EntityChips } from "./EntityChips";
import { EntityPicker } from "./EntityPicker";

/** The id the box names as the list it controls. */
const PICKER_LIST_ID = "entity-picker";

/**
 * The draft is held by the chat panel rather than here, because stopping a question offers its
 * text back: the text she was about to have answered belongs to the conversation, not to the box.
 *
 * Typing `@` and the first letters of a name opens a list of the names of the selection; choosing
 * one writes the name into the question and adds a chip above the box. The chips narrow the next
 * search to the files that hold them. The question text is plain text either way.
 */
export function Composer({
  draft,
  onDraftChange,
  phase,
  onSend,
  onStop,
  hint,
  modelAlias,
  aliases,
  onModelAliasChange,
  needsFolder = false,
  chips,
  onAddChip,
  onRemoveChip,
  scope,
  hasSelection,
  onAddToSelection,
}: {
  /** No documents folder is chosen yet. This is a practice assistant, not a chatbot: nothing is
   * asked until there is a folder, even an empty one (`docs/SELECTION-AND-MEMORY.md`). */
  needsFolder?: boolean;
  draft: string;
  onDraftChange: (draft: string) => void;
  phase: GenerationPhase;
  onSend: (question: string) => void;
  onStop: () => void;
  hint: ReactNode;
  modelAlias: string;
  aliases: string[];
  onModelAliasChange: (alias: string) => void;
  /** The names chosen for the next question. */
  chips: readonly Chip[];
  onAddChip: (chip: Chip) => void;
  onRemoveChip: (entityId: number) => void;
  /** The files the picker looks in: the same selection the question is sent with. */
  scope: CombinedScope;
  /** Whether anything is selected. The picker needs something to pick among. */
  hasSelection: boolean;
  /** Adds the files of a name that is only in unselected files to the selection. Her own click. */
  onAddToSelection: (outside: OutsideSelection) => void;
}) {
  const { t } = useTranslation();
  const input = useRef<HTMLTextAreaElement>(null);
  const [caret, setCaret] = useState(0);
  /* Where the mention she closed with Escape starts, so it does not reopen as she types on. */
  const [dismissedAt, setDismissedAt] = useState<number | null>(null);
  const [highlighted, setHighlighted] = useState(0);

  const pickerAllowed = hasSelection && !needsFolder;
  const mention = pickerAllowed ? mentionAt(draft, caret) : null;
  const open = mention !== null && mention.start !== dismissedAt;
  const { result, searching } = useEntitySearch(open ? mention.prefix : null, scope);
  const choices = offeredChoices(result.entities, chips);

  const noMention = mention === null;
  useEffect(() => {
    if (noMention) {
      setDismissedAt(null);
    }
  }, [noMention]);
  useEffect(() => setHighlighted(0), [choices.length, mention?.prefix]);

  const submit = () => {
    if (needsFolder || draft.trim().length === 0 || phase !== "idle") {
      return;
    }
    onSend(draft);
    onDraftChange("");
  };

  const putCaret = (position: number) => {
    // After the render that carries the new text: until then the box still holds the old one.
    requestAnimationFrame(() => {
      input.current?.focus();
      input.current?.setSelectionRange(position, position);
      setCaret(position);
    });
  };

  const choose = (choice: EntityChoice) => {
    if (mention === null) {
      return;
    }
    const at = input.current?.selectionStart ?? draft.length;
    const inserted = insertMention(draft, mention, at, choice.display);
    onDraftChange(inserted.text);
    onAddChip({ entityId: choice.entityId, display: choice.display });
    setDismissedAt(null);
    putCaret(inserted.caret);
  };

  const startMention = () => {
    const inserted = insertAt(draft, input.current?.selectionStart ?? draft.length);
    onDraftChange(inserted.text);
    setDismissedAt(null);
    putCaret(inserted.caret);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (open) {
      const action = pickerKey(event.key, highlighted, choices.length);
      if (action.kind === "move") {
        event.preventDefault();
        setHighlighted(action.index);
        return;
      }
      if (action.kind === "choose") {
        event.preventDefault();
        const picked = choices[action.index];
        if (picked !== undefined) {
          choose(picked);
        }
        return;
      }
      if (action.kind === "close") {
        event.preventDefault();
        setDismissedAt(mention.start);
        return;
      }
    }
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      submit();
    }
  };

  /* One button in one place, so the eye does not have to move and the keyboard does not lose it:
     it offers the stop only while the answer has not started, and goes back to sending - disabled,
     since the box is empty again - the moment the first words arrive. Stopping an answer that is
     already being written belongs under that answer, not here: it is a different act, because it
     keeps the text. */
  const stopping = canDiscard(phase);
  const showList = open && mention !== null;

  return (
    <div className="composer">
      <EntityChips chips={chips} onRemove={onRemoveChip} />
      <textarea
        ref={input}
        className="composer__input"
        value={draft}
        rows={3}
        disabled={needsFolder}
        placeholder={t(needsFolder ? "chat.needsFolder" : "chat.placeholder")}
        aria-label={t(needsFolder ? "chat.needsFolder" : "chat.placeholder")}
        aria-autocomplete={pickerAllowed ? "list" : undefined}
        aria-expanded={pickerAllowed ? showList && choices.length > 0 : undefined}
        aria-controls={showList && choices.length > 0 ? PICKER_LIST_ID : undefined}
        aria-activedescendant={
          showList && choices.length > 0 ? `${PICKER_LIST_ID}-${highlighted}` : undefined
        }
        onChange={(event) => {
          onDraftChange(event.target.value);
          setCaret(event.target.selectionStart);
        }}
        onSelect={(event) => setCaret(event.currentTarget.selectionStart)}
        onKeyDown={onKeyDown}
      />
      {showList ? (
        <EntityPicker
          id={PICKER_LIST_ID}
          prefix={mention.prefix}
          choices={choices}
          outside={result.outsideSelection}
          highlighted={highlighted}
          searching={searching}
          onChoose={choose}
          onHighlight={setHighlighted}
          onAddToSelection={onAddToSelection}
        />
      ) : null}
      <div className="composer__footer">
        <div className="composer__footnotes">{hint}</div>
        <div className="composer__actions">
          {/* Opens the same list as typing `@`, for whoever does not know the key. It needs a
              selection, and the hover says so when there is none. */}
          <button
            type="button"
            className="button composer__mention"
            aria-label={t("chat.picker.button")}
            title={t(hasSelection ? "chat.picker.button" : "chat.picker.needsSelection")}
            disabled={!pickerAllowed || phase !== "idle"}
            onClick={startMention}
          >
            @
          </button>
          {aliases.length === 0 ? null : (
            <select
              className="field__control composer__model-select"
              aria-label={t("chat.modelLabel")}
              value={modelAlias}
              onChange={(event) => onModelAliasChange(event.target.value)}
            >
              {aliases.map((alias) => (
                <option key={alias} value={alias}>
                  {alias}
                </option>
              ))}
            </select>
          )}
          <button
            type="button"
            className="button button--primary composer__send"
            onClick={stopping ? onStop : submit}
            disabled={
              !stopping && (needsFolder || phase !== "idle" || draft.trim().length === 0)
            }
          >
            {t(stopping ? "actions.stop" : "actions.send")}
          </button>
        </div>
      </div>
    </div>
  );
}
