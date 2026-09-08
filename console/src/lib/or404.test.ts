import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * `or404` rethrows `Refused(401)` after `#253` rewrote a held session.
 * A page that neither wraps `withRefusal` nor checks `unavailable` lets
 * that throw leave the server component — Next redacts it to `#441`.
 *
 * Layouts that return `<Unavailable>` without `{children}` do not stop
 * the page from running. The wrap has to be on the page (or the read
 * has to be `orTransient` already, like `bookRecord`).
 */
function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, name.name);
    if (name.isDirectory()) walk(p, out);
    else if (name.name === "page.tsx" || name.name === "layout.tsx") out.push(p);
  }
  return out;
}

describe("or404 callers", () => {
  it("do not let a held-session 401 leave the server component, which production redacts to #441", () => {
    const root = join(process.cwd(), "src/app");
    const bare: string[] = [];
    for (const file of walk(root)) {
      const src = readFileSync(file, "utf8");
      if (!src.includes("or404(") && !src.includes("or404 (")) continue;
      const wrapped =
        src.includes("withRefusal") ||
        src.includes("unavailable") ||
        src.includes("<Unavailable");
      if (!wrapped) bare.push(file.slice(root.length + 1));
    }
    expect(bare).toEqual([]);
  });
});
