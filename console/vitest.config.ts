import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The render suite. It is the half of `//web:rendered_test`'s job that a grep
// could not do — see `console/scripts/fields_test.py` for the other half and
// for why both exist.
//
// ⛔ NEGATIVE-TEST EVERY CASE HERE. Remove the field from the fixture and watch
// the case go red before believing it. CONTRIBUTING.md records three suites in
// this repository that were green, covered the code, and tested nothing.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
      // ⚠ `server-only` THROWS ON IMPORT OUTSIDE A SERVER COMPONENT, which is
      // its whole job — it is what stops `wire/client.ts` and the session from
      // being pulled into a browser bundle. Under jsdom there is no server
      // component graph to satisfy it, so it is stubbed here rather than
      // dropped from the modules it guards. Removing the import to make a test
      // pass would delete the guard to test the thing it guards.
      "server-only": fileURLToPath(
        new URL("./vitest.server-only-stub.ts", import.meta.url),
      ),
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
    setupFiles: ["./vitest.setup.ts"],
  },
});
