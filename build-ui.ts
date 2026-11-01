// Builds ui-dist\ (gitignored) with no bundler, as LrC-AVG's hud/build-ui.ts does: Node's own type
// stripping turns ui\*.ts into JavaScript; html, css and fonts are copied beside it.
import { cpSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { stripTypeScriptTypes } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const src = join(here, "ui");
const out = join(here, "ui-dist");

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
cpSync(join(src, "fonts"), join(out, "fonts"), { recursive: true });
for (const file of readdirSync(src)) {
  if (file.endsWith(".html") || file.endsWith(".css")) cpSync(join(src, file), join(out, file));
  if (!file.endsWith(".ts") || file.endsWith(".test.ts")) continue;
  const js = stripTypeScriptTypes(readFileSync(join(src, file), "utf8"), { mode: "strip" })
    .replace(/from "\.\/([\w-]+)\.ts"/g, 'from "./$1.js"');
  writeFileSync(join(out, file.replace(/\.ts$/, ".js")), js);
}
console.log(`ui-dist written: ${out}`);
