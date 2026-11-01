import { expect, test } from "vitest";
import { gridLines } from "./preview.ts";

test("grid lines sit on desk millimetres, so they continue across monitors", () => {
  // A monitor from x = 695 mm, 30 mm wide, 2 px per mm: lines at 700, 710, 720 mm.
  expect(gridLines(695, 30, 2, 10)).toEqual([10, 30, 50]);
  // The next monitor starting at 727 mm picks up at 730 mm.
  expect(gridLines(727, 20, 2, 10)).toEqual([6, 26]);
  expect(gridLines(0, 250, 1, 100)).toEqual([0, 100, 200]);
});

test("the grid moves with the nudge", () => {
  expect(gridLines(695, 30, 2, 10, 4)).toEqual([14, 34, 54]);
  expect(gridLines(700, 30, 2, 10, -4)).toEqual([16, 36, 56]);
});
