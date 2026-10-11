import { beforeEach, describe, expect, it, vi } from "vitest";

/* The bridge is replaced by a recorder: these tests read which command the webview asks Rust for and
   with which arguments. The names are the contract with `commands.rs`; Tauri turns a camelCase
   argument into the snake_case parameter of the command. */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import {
  acceptMatch,
  addAlias,
  createEntity,
  deleteEntity,
  dismissMatch,
  editEntity,
  entitySheet,
  knowledgeHealth,
  knowledgeOverview,
  knowledgeTypesAndRoles,
  listEntities,
  mergeEntities,
  possibleMatches,
  removeAlias,
  removeAttribute,
  resetKnowledge,
  restoreEntity,
  setAttribute,
  splitEntity,
  undoLastOperation,
  unmergeEntity,
} from "./ipc";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockResolvedValue({});
});

function lastCall(): [string, Record<string, unknown> | undefined] {
  const calls = vi.mocked(invoke).mock.calls;
  const call = calls[calls.length - 1];
  return [call?.[0] as string, call?.[1] as Record<string, unknown> | undefined];
}

describe("the management bridge", () => {
  it("reads with the commands Rust registers", async () => {
    await knowledgeOverview();
    expect(lastCall()[0]).toBe("knowledge_overview");

    await entitySheet(7);
    expect(lastCall()).toStrictEqual(["knowledge_entity_sheet", { entityId: 7 }]);

    await possibleMatches();
    expect(lastCall()).toStrictEqual(["knowledge_possible_matches", { page: 0 }]);
    await possibleMatches(2);
    expect(lastCall()).toStrictEqual(["knowledge_possible_matches", { page: 2 }]);

    await knowledgeHealth();
    expect(lastCall()[0]).toBe("knowledge_health");
    await knowledgeTypesAndRoles();
    expect(lastCall()[0]).toBe("knowledge_types_and_roles");
  });

  it("sends the list query as one object named query", async () => {
    const query = {
      text: "dup",
      typeId: "person",
      origin: null,
      view: "live" as const,
      problemsOnly: false,
      page: 1,
    };

    await listEntities(query);

    expect(lastCall()).toStrictEqual(["knowledge_list_entities", { query }]);
  });

  it("sends each write with the argument names the command declares", async () => {
    await createEntity({ name: "Anne Roux", typeId: "person" });
    expect(lastCall()).toStrictEqual([
      "knowledge_create_entity",
      { input: { name: "Anne Roux", typeId: "person" } },
    ]);

    await editEntity(3, { name: "Anne Roux-Petit" });
    expect(lastCall()).toStrictEqual([
      "knowledge_edit_entity",
      { entityId: 3, edit: { name: "Anne Roux-Petit" } },
    ]);

    await addAlias(3, "A. Roux");
    expect(lastCall()).toStrictEqual(["knowledge_add_alias", { entityId: 3, text: "A. Roux" }]);

    await removeAlias(9);
    expect(lastCall()).toStrictEqual(["knowledge_remove_alias", { aliasId: 9 }]);

    await setAttribute(3, "spoken_form", "Anne");
    expect(lastCall()).toStrictEqual([
      "knowledge_set_attribute",
      { entityId: 3, key: "spoken_form", value: "Anne" },
    ]);
    await removeAttribute(3, "spoken_form", "Anne");
    expect(lastCall()).toStrictEqual([
      "knowledge_remove_attribute",
      { entityId: 3, key: "spoken_form", value: "Anne" },
    ]);
  });

  it("says which side of a merge survives", async () => {
    await mergeEntities(1, 2);
    expect(lastCall()).toStrictEqual(["knowledge_merge", { survivor: 1, absorbed: 2 }]);

    await acceptMatch(2, 1);
    expect(lastCall()).toStrictEqual(["knowledge_accept_match", { survivor: 2, absorbed: 1 }]);

    await unmergeEntity(2);
    expect(lastCall()).toStrictEqual(["knowledge_unmerge", { entityId: 2 }]);

    await dismissMatch(1, 2);
    expect(lastCall()).toStrictEqual(["knowledge_dismiss_match", { one: 1, other: 2 }]);
  });

  it("sends a split as one request", async () => {
    const request = { entityId: 4, mentionIds: [11, 12], sourceIds: [], name: "Jean Dupont 2" };

    await splitEntity(request);

    expect(lastCall()).toStrictEqual(["knowledge_split", { request }]);
  });

  it("sends deletion, restoration, undo and the two strengths of reset", async () => {
    await deleteEntity(5);
    expect(lastCall()).toStrictEqual(["knowledge_delete_entity", { entityId: 5 }]);
    await restoreEntity(5);
    expect(lastCall()).toStrictEqual(["knowledge_restore_entity", { entityId: 5 }]);
    await undoLastOperation();
    expect(lastCall()[0]).toBe("knowledge_undo");

    await resetKnowledge(false);
    expect(lastCall()).toStrictEqual(["knowledge_reset", { includeManual: false }]);
    await resetKnowledge(true);
    expect(lastCall()).toStrictEqual(["knowledge_reset", { includeManual: true }]);
  });

  it("never sends a name or a passage through anything but a command argument", async () => {
    await addAlias(1, "Jean Dupont");

    // Nothing is stored on the webview side: the call is the whole of what leaves it.
    expect(vi.mocked(invoke)).toHaveBeenCalledTimes(1);
    expect(JSON.stringify(vi.mocked(invoke).mock.calls)).not.toContain("http");
  });
});
