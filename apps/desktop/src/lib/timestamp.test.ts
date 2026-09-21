import { describe, expect, it } from "vitest";
import { timestampParts } from "./timestamp";

/** Local time, so every fixture is built from local components rather than from an ISO string. */
function at(
  year: number,
  month: number,
  day: number,
  hour: number,
  minute: number,
  second: number,
): number {
  return new Date(year, month - 1, day, hour, minute, second).getTime();
}

describe("timestampParts", () => {
  it("pads every part to a fixed width", () => {
    expect(timestampParts(at(2026, 9, 21, 16, 51, 3))).toStrictEqual({
      year: "2026",
      month: "09",
      day: "21",
      hour: "16",
      minute: "51",
      second: "03",
    });
  });

  it("keeps midnight at 00 rather than 12 or 24", () => {
    expect(timestampParts(at(2026, 1, 1, 0, 0, 0))).toStrictEqual({
      year: "2026",
      month: "01",
      day: "01",
      hour: "00",
      minute: "00",
      second: "00",
    });
  });

  it("leaves a two-digit day and month alone", () => {
    expect(timestampParts(at(2026, 12, 31, 23, 59, 59))).toStrictEqual({
      year: "2026",
      month: "12",
      day: "31",
      hour: "23",
      minute: "59",
      second: "59",
    });
  });

  it("writes the afternoon in 24-hour form", () => {
    expect(timestampParts(at(2026, 9, 21, 18, 53, 4)).hour).toBe("18");
  });
});
