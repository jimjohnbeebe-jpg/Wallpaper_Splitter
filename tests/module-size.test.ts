// CLAUDE.md "Module size": no source file over 400 lines (target 300), counted as `wc -l` counts.
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "vitest";

const roots = ["ui", join("src-tauri", "src")];
const files = roots.flatMap((r) =>
  readdirSync(r, { recursive: true, encoding: "utf8" })
    .filter((f) => /\.(ts|rs|css|html)$/.test(f))
    .map((f) => join(r, f)),
);

test.each(files)("%s is at most 400 lines", (f) => {
  const lines = readFileSync(f, "utf8").split("\n").length - 1;
  expect(lines).toBeLessThanOrEqual(400);
});
