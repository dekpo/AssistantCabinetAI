import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // The suite covers pure logic and source guards, so it needs no browser environment.
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
