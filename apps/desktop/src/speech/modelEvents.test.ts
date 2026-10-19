import { describe, expect, it, vi } from "vitest";
import { modelsRevision, notifyModelsChanged, subscribeModelsChanged } from "./modelEvents";

describe("the models revision", () => {
  it("moves each time a model job ended and tells its subscribers", () => {
    const listener = vi.fn();
    const before = modelsRevision();
    const unsubscribe = subscribeModelsChanged(listener);

    notifyModelsChanged();
    notifyModelsChanged();

    expect(modelsRevision()).toBe(before + 2);
    expect(listener).toHaveBeenCalledTimes(2);

    unsubscribe();
    notifyModelsChanged();
    expect(listener).toHaveBeenCalledTimes(2);
  });
});
