import { defineConfig, configDefaults } from "vitest/config";

// Agent worktrees under .claude/ hold their own copies of the tests.
export default defineConfig({ test: { exclude: [...configDefaults.exclude, ".claude/**"] } });
