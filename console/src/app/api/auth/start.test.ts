import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * The AuthKit initiate-login handler.
 *
 * ⛔ `getSignInUrl` SEALS THE PKCE VERIFIER WITH `cookies().set()`. A
 * `NextResponse.redirect` constructed afterwards is a different Response
 * than that cookie store, so the verifier never reaches the browser. WorkOS
 * then finishes, `/callback` cannot match `state`, and the operator is back
 * on `/signin`. AuthKit's README uses `redirect()` from `next/navigation`.
 */
describe("startAuthKit", () => {
  it("sends the operator to AuthKit via next/navigation redirect so the PKCE cookie is on that response", () => {
    const src = readFileSync(join(process.cwd(), "src/app/api/auth/start.ts"), "utf8");
    expect(src).toMatch(/from "next\/navigation"/);
    expect(src).toMatch(/redirect\(url\)/);
    expect(src).toMatch(/unstable_rethrow/);
    expect(src).not.toMatch(/NextResponse\.redirect\(url\)/);
  });
});

describe("AuthKit callback", () => {
  it("sends a failed exchange to /signin?error=1, not a 500 JSON body", () => {
    const src = readFileSync(
      join(process.cwd(), "src/app/callback/route.ts"),
      "utf8",
    );
    expect(src).toMatch(/handleAuth\(\{/);
    expect(src).toMatch(/\/signin\?error=1/);
  });

  it("sends a completed exchange through next/navigation redirect so the session cookie is on that response", () => {
    const src = readFileSync(
      join(process.cwd(), "src/app/callback/route.ts"),
      "utf8",
    );
    expect(src).toMatch(/continueWithSealedSession/);
    expect(src).toMatch(/from "next\/navigation"/);
    expect(src).toMatch(/unstable_rethrow/);
  });
});
