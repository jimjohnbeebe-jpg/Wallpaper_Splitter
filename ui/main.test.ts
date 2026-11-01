import { expect, test } from "vitest";
import { navTarget } from "./main.ts";

test("arrow keys wrap around the nav", () => {
  expect(navTarget(0, 3, "ArrowDown")).toBe(1);
  expect(navTarget(2, 3, "ArrowDown")).toBe(0);
  expect(navTarget(0, 3, "ArrowUp")).toBe(2);
  expect(navTarget(1, 3, "End")).toBe(2);
  expect(navTarget(1, 3, "Enter")).toBeNull();
});
