# Speech phase 6: name suggestions after a dictation (lot report)

Date: 18 October 2026. Branch: `feat/speech-name-suggestions`, from `feat/speech-integration` (phase 5 merged, PR #51; working tree clean when the work started; the changes below are uncommitted).

## What was done

After a dictation in the chat, the words that probably meant a name of the selected files are shown under the question box as "heard -> name" with **Replace** and **Ignore**; when several names fit, the alternatives are buttons. Nothing is replaced until she presses a name, and nothing is written to the knowledge base.

- **Rust.** `speech::repair::propose_for` wraps `knowledge::repair::propose` (selection only, read only). It converts the byte spans to UTF-16 offsets (how the window slices a string) and applies the hidden-names guard **in Rust**: while the setting hides names and `revealNames` is not set, the answer is `{ proposals: [], hiddenCount }`, so no name, not even the words heard, reaches the window. Any failure gives an empty answer (`or_nothing`). The command `speech_repair_names(transcript, scope, caseReliable, revealNames)` sits in `commands.rs` next to `knowledge_entity_card` and answers empty before opening the index when the knowledge base is off.
- **Engine descriptor.** `case_reliable` is a property of the engine: the model manifest row has `casedOutput` (true for Parakeet), `ModelSpec.cased_output`, `SttResult.case_reliable` (`caseReliable` in the webview). The dictation carries it (and the engine id) in its result; the command takes it from the window as the prompt specifies, it only decides which words are looked at.
- **Webview.** `repairChips.ts` (pure: place the answer in the draft, `followEdit`, `acceptChip`), `useRepairSuggestions.ts` (the hook), `RepairChips.tsx`. Chips follow the edits of the draft (text typed before them moves them, a letter attached to the words or an edit inside them drops the chip) and are dropped when their words are not alone at their place. Accept re-checks that the draft still holds `heard` there. The chips of several dictations add up. The hidden line "N suggestions are hidden" has a **Show names** button (`onRevealNamesChange(true)`), and the hook asks again for the hidden ones. Neither the hook nor the component knows the Composer: phase 12 mounts them in the notes editor.
- With the voice off nothing is drawn: the Composer gets no `repair` property and its markup is byte for byte the old one (test).

## Files created

| File | Content |
|---|---|
| `src-tauri/src/speech/repair.rs` | `RepairSet`, `RepairItem`, `RepairChoice`, `propose_for`, `or_nothing`; 4 unit tests |
| `src-tauri/tests/speech_repair.rs` | 9 tests over the real Analyse of invented files (the existing `repair-fr.json`, no fixture added) |
| `src/speech/repairChips.ts`, `useRepairSuggestions.ts`, `src/components/RepairChips.tsx` | pure logic, hook, component |
| `src/speech/repairChips.test.ts`, `repairSources.test.ts`, `src/components/repairRender.test.ts` | 21 + 15 + 10 tests |

## Existing files edited (every edit)

| File | Edit |
|---|---|
| `src-tauri/src/commands.rs` | `use crate::speech::repair`; the command `speech_repair_names` (before the knowledge management section). **No helper widened, none duplicated**: it sits in the file that owns `open_index`, `resolve_selection`, `AppState::read` and `AppState::vocabulary`, so they stay private |
| `src-tauri/src/lib.rs` | handler `commands::speech_repair_names` |
| `src-tauri/src/speech/mod.rs` | `pub mod repair;` and its doc line |
| `src-tauri/src/speech/models.rs` | `ModelSpec.cased_output` (`casedOutput`, default false) |
| `src-tauri/resources/speech/models-manifest.json` | `"casedOutput": true` on the Parakeet row |
| `src-tauri/src/speech/stt.rs`, `worker_stt.rs` | `SttResult.case_reliable`, set from the manifest row |
| `src-tauri/src/speech/fake.rs`, `stt_tests.rs`, `timing_log.rs`, `test_support.rs` | the new field in four test constructions |
| `src/speech/ipc.ts` | `SpeechTranscript.caseReliable`, `speechRepairNames` and its types |
| `src/speech/insertion.ts` | `Insertion` gains `start` and `words` |
| `src/speech/dictation.ts` | `DictationResult` gains `engineId`, `caseReliable` |
| `src/speech/useDictation.ts` | the bridge gains `heard` (the words of the last dictation, kept after the box is edited) |
| `src/components/Composer.tsx` | optional prop `repair`, `RepairChips` drawn under the dictation line |
| `src/components/ChatPanel.tsx` | one `useRepairSuggestions` call, `repair` handed to the Composer |
| `src/locales/en-US.json`, `fr-FR.json` | 10 keys under `speech.repair` |
| `src/styles.css` | `.repair*` |
| `src/speech/dictation.test.ts`, `insertion.test.ts`, `ipc.test.ts`, `src/components/dictationRender.test.ts` | the new fields and cases |
| `docs/SPEECH.md` | section 7.2 "Built as follows", file list, the descriptor sentence, the invariant row |

## Decisions of this phase (the owner may overrule any of them)

1. **Guard 2 in Rust, not in the window.** The prompt said "collapse to one line in the window"; collapsing there would still have sent the names to the webview. Rust now withholds them. Cost: pressing "Show names" asks the base again (one cheap read).
2. **The hidden count never leaks existence.** It counts only proposals inside the selection (test `repair_never_leaves_the_selection`).
3. **Accepting does not add the name to the question's name chips** (no narrowing of the search): "nothing new".
4. **The caret is not moved after Replace** (focus is on the button); the box keeps its text.
5. `speech_repair_names` returns `Ok(empty)` rather than an error on any failure, so there is no new error code and no catalogue sentence.

## Tests

One targeted pre-check before the first edit: `cargo test --lib speech` (130 passed, 2 ignored). Final pass, once, after the last edit: `cargo test --lib --tests -j 2` exit 0 (lib 1039 passed, 6 ignored, was 1035; `speech_repair` 9 passed; every other target green); `cargo clippy --all-targets`: no warning in the files I wrote (warnings elsewhere are old); `pnpm run build` ok; `pnpm run test`: 85 files, 2058 tests passed (was 1995; the Rust language guard and the literal guard included). `rustfmt --check` clean on `repair.rs` and `tests/speech_repair.rs` (new; the latter checked on a copy without `mod common;` because rustfmt follows it into old files), `commands.rs` 0 diff blocks before and after. After the formatting fix of one `use` line in `commands.rs` I ran `cargo check` only.

Named tests: `the_contract_file_holds_through_the_function_the_command_wraps` (must-propose, must-not-change, no-case cases, the span in UTF-16), `repair_never_leaves_the_selection`, `with_the_knowledge_base_off_there_is_nothing_to_repair`, `the_command_answers_before_it_opens_the_index_when_the_base_is_off` (source scan), `hidden_names_reach_the_window_as_a_count_only`, `dictating_does_not_change_the_knowledge_base` (rows of every `kb_*` table), `a_failure_of_the_reading_is_an_empty_answer`, `the_entity_type_changes_nothing` (the same names retyped organisation / item with an unshipped subtype), `the_module_names_no_entity_type_and_no_medicine`; in the window the chip placement, edits around a chip, accept / ignore / stale span, hidden collapse in both languages, escaping of what was heard, the Composer unchanged without a bridge, and the scans (no send, no network, one Rust call, one `setDraft`).

**Not proven by a test**: the React hook itself (the repository has no DOM test environment; its logic is in pure functions that are tested), the real engine writing a name in a way the base catches, the click on the buttons, the "Show names" round trip in the window. The human test below is the first place they run. I did not play it.

## Limit to state

The suggestions depend on what Parakeet writes. If it writes the name exactly as the base holds it, there is correctly no chip. I could not read her index to pick a name that is sure to be misheard (the data is hers), so the human test starts by finding out.

## Human test (proposed, not played)

About 10 minutes, nothing is written to any folder, so no backup and no restore.

**You start with:** the application open, Documents Folder = `C:\Users\elise\AssistantCabinetAI-test\Docs` already analysed, all its files ticked, Knowledge mode not "Off", voice switched on, the chat empty, no dialog open.

1. Click the microphone, say slowly "J'ai croisé Du pont au cabinet ce matin", click the microphone again, wait. Expected: the sentence appears in the box. Then one of three things, all correct: (a) a line "Du pont -> <a name>" appears under the box with **Replace** and **Ignore** (the name is one of the files' names, for example Jean Dupont); (b) the engine wrote "Dupont" in one word and nothing appears (the word is already a name); (c) the engine wrote something else and nothing appears. Note which.
2. If (b) or (c), repeat with the name of one person whose file is in the folder, spoken in an unusual way (another stress, or spelled in two parts), until a line appears. If none appears after three tries, write down the three texts and stop at part 6.
3. When a line is there, type the word "bonjour " at the very start of the box. Expected: the line stays.
4. Click **Replace**. Expected: only those words are replaced by the full name, the rest of the sentence is untouched, the line disappears.
5. Dictate the same sentence again, then click **Ignore**. Expected: the line disappears, the text is unchanged.
6. Dictate once more, then edit the heard word in the box by hand (delete a letter). Expected: the line disappears by itself.
7. In Settings, tick "Masquer les noms dans les suggestions" if it is not ticked, close Settings, dictate the sentence of step 1 until a line would have appeared. Expected: instead of a name, one line "1 suggestion de nom est masquée." with **Afficher les noms**, and no name anywhere under the box. Click **Afficher les noms**. Expected: the line is replaced by the suggestion. (If the names were already shown, untick it, dictate again, and play this step.)
8. Click **Send** with a suggestion on screen. Expected: the suggestion disappears with the box.

**You leave with:** the application open, the chat as it is, the setting "Masquer les noms dans les suggestions" as you found it. If you changed it, put it back.

## For the next phases

- Phase 12 reuses `useRepairSuggestions` and `RepairChips`; see "Alignment with phase 6" in `10_speech-12_notes-dictation.md`.

## Prompt alignment

Checked `10_speech-12_notes-dictation.md`: appended "Alignment with phase 6" (hook and component names, the `RepairSource` to hand over per segment, `caseReliable`, the scan that forbids Composer words in the repair files). `STATUS.md` updated. No owner decision needed.
