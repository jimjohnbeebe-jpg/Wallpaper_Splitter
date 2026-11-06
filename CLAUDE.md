# Wallpaper Splitter

A Windows 11 app that splits one image into per-monitor wallpapers. They line up across monitors of any size, resolution and pixel density, and the app can set them as the wallpaper. Repo: https://github.com/jimjohnbeebe-jpg/Wallpaper_Splitter (public, MIT). Sibling of LrC-AVG (`D:\Developer\LrC_Autonomous_Gateway`), whose design system and review workflow it reuses.

- Product: `PRODUCT.md`.
- Design tokens and rules: `DESIGN.md`. Use the tokens, never raw hex in components.
- Plan of record: `C:\Users\jimbe\.claude\plans\iterative-cuddling-codd.md`. It now holds the distribution plan; the v1 build plan is archived in the Obsidian vault at `Projects/Wallpaper_Splitter/PLAN.md`.

## Stack

- **Shell and backend:** Tauri 2.12 (Rust, `src-tauri/`). The module map is at the top of `src-tauri/src/main.rs`. `model.rs` and `ui/ipc.ts` are the contract: change both together.
- **UI:** vanilla TypeScript, HTML and CSS in `ui/`, with no framework and no bundler. `build-ui.ts` strips the types into `ui-dist/`. The Tauri API comes from `window.__TAURI__` (`withGlobalTauri`), and `ui/ipc.ts` is the only place that touches it.
- **Layout maths lives only in `src-tauri/src/geometry.rs`.** The UI asks `compute_layout` and never re-derives crops.
- **Dependencies are pinned exactly** (`=x.y.z` in Cargo.toml, exact versions in package.json). **No AGPL crates.** That rules out the imazen "zen\*" family (zenjpeg, zenavif, zencodec): check the licence of anything new on crates.io before adding it.
- **TypeScript:** `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `verbatimModuleSyntax`, `erasableSyntaxOnly` (no `enum`, no `namespace`). Relative imports end in `.ts`.

## Module size

- Target: 300 lines per file and 50 lines per function.
- `tests/module-size.test.ts` fails any file in `ui/` or `src-tauri/src` over 400 lines. Split before adding.

## Never

- Commit Jim's photos or personal scripts. They sit in the repo folder and `.gitignore` keeps them out. Tests use small images they generate themselves.
- Leave Jim's desktop changed after a test. Anything that sets a wallpaper or switches HDR reads the old state first and puts it back.

## Checks before every push

```
npm run typecheck
npm test
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## Pull request + review workflow (from LrC-AVG `.claude/rules/04-workflow.md`)

1. Work on a branch (`feat/<topic>`, `fix/<topic>`). Never commit to `main`; branch protection requires PRs and the `ci` check.
2. `git push -u origin <branch>`, then `gh pr create --fill`.
3. **Wait for Greptile's review.** It runs once, when the PR is opened (`greptile.json`: strictness 3, logic comments only, Markdown ignored). Poll `gh pr view <n> --json body`, `gh pr view <n> --comments` and `gh api repos/jimjohnbeebe-jpg/Wallpaper_Splitter/pulls/<n>/comments` for a summary or comment whose author login contains `greptile`. Never `gh pr edit --body-file` after Greptile has written into the body. If no review arrives, stop and tell Jim.
4. **Triage every finding** in one table posted as a PR comment:

   | file:line | Greptile finding | decision | reason | commit (if fixed) |
   |---|---|---|---|---|

   The decision is exactly one of **fix**, **reject** or **defer**. When Greptile finds nothing, post the table with a single "no findings" row. All fixes from one review go in one push.
5. **Ask Jim before requesting a re-review** (`@greptileai` comment, about $0.50 each). Only ask when a fix changed logic the first review did not see.
6. Merge when the triage is posted, no fix rows are open and `ci` is green: `gh pr merge <n> --squash --delete-branch`, then `git switch main && git pull`.

## Releases

- Version is in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`, which must agree.
- Tagging `v<version>` runs `.github/workflows/release.yml`. It builds the portable exe (`tauri build --no-bundle`) and attaches `WallpaperSplitter-<version>.exe` and `SHA256SUMS.txt` to a **draft** release.
- Without a local git push (a work computer, or a cloud session, which may push only its own branch): Actions > release > Run workflow on `main` with **release** ticked. The run creates the tag `v<version>` on the commit it built, then the same draft release. It refuses to run from another branch or when the tag already exists.
- When the repository variables `MSIX_IDENTITY_NAME`, `MSIX_PUBLISHER` and `MSIX_PUBLISHER_DISPLAY_NAME` are set (from Partner Center > Product identity), the release also builds `WallpaperSplitter-<version>.msix` for the Microsoft Store with `packaging/msix/stage.ps1`. The package is unsigned, because the Store signs it.
- In the MSIX build, HKCU writes are virtualised, so `wallpaper::packaged()` turns the JPEG import fix off and the UI hides it.
- Jim publishes the release himself. The exe is unsigned.

## Steps for Jim

Automate first. Anything Jim does is a short numbered list with exact PowerShell commands (his shell is PowerShell 7) and what he should see. Troubleshooting goes under "If something goes wrong".
