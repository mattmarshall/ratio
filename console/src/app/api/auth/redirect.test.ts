import { afterEach, describe, expect, it, vi } from "vitest";
import { safeReturnTo, sameOrigin } from "./redirect";

// The sign-in routes' redirect targets.
//
// ⛔ THIS FILE EXISTS BECAUSE A LIVE DEPLOYMENT COULD NOT SHOW ITS OWN ERROR
// PAGE. `RATIO_CONSOLE_ORIGIN` held `https://ratio-console.vercel.app`, a host
// that does not resolve, and every redirect out of `/api/auth/login` and
// `/api/auth/callback` was built by prefixing that value — including the ones
// whose only job was to report that the configuration was wrong. The reader got
// a dead hostname instead of the sentence naming the variable to fix.
//
// So the property under test is narrow and worth stating plainly: a redirect to
// a page this same process serves carries no host at all.

describe("sameOrigin", () => {
  it("emits a Location with no scheme and no host", () => {
    const r = sameOrigin("/signin?error=config");
    expect(r.status).toBe(307);
    expect(r.headers.get("location")).toBe("/signin?error=config");
  });

  // ⛔ THE REGRESSION, NAMED. If `sameOrigin` ever grows a base URL — an env
  // var, `req.url`, a forwarded header — this goes red. That is the point: the
  // absolute form is what failed, and it failed in the one situation the
  // redirect was there to handle.
  it("does not reintroduce an absolute URL", () => {
    const loc = sameOrigin("/signin?error=1").headers.get("location") ?? "";
    expect(loc.startsWith("/")).toBe(true);
    expect(loc).not.toMatch(/^https?:/);
    expect(loc).not.toContain("vercel.app");
  });

  it("carries 303 when asked, so a POST sign-out is followed with GET", () => {
    expect(sameOrigin("/signin", 303).status).toBe(303);
  });

  it("sends no body", async () => {
    expect(await sameOrigin("/signin").text()).toBe("");
  });
});

describe("safeReturnTo", () => {
  it("keeps a rooted path, which is the whole point of the deep link", () => {
    expect(safeReturnTo("/breaks/cash-usd-2026-02-26")).toBe(
      "/breaks/cash-usd-2026-02-26",
    );
    expect(safeReturnTo("/breaks?fund=harbourline-global-value")).toBe(
      "/breaks?fund=harbourline-global-value",
    );
  });

  it("falls back to / when there is nothing to return to", () => {
    expect(safeReturnTo(null)).toBe("/");
    expect(safeReturnTo(undefined)).toBe("/");
    expect(safeReturnTo("")).toBe("/");
  });

  // ⛔ AN OPEN REDIRECT ON A SIGN-IN ROUTE IS A TOKEN-EXFILTRATION BUG, and a
  // relative `Location` makes this guard load-bearing rather than
  // belt-and-braces: the browser resolves `//evil.example` against the scheme
  // and lands on another origin. `startsWith("/")` alone accepts both of these.
  it("refuses a protocol-relative URL dressed as a path", () => {
    expect(safeReturnTo("//evil.example/steal")).toBe("/");
    expect(safeReturnTo("/\\evil.example/steal")).toBe("/");
  });

  it("refuses anything that is not rooted", () => {
    expect(safeReturnTo("https://evil.example")).toBe("/");
    expect(safeReturnTo("javascript:alert(1)")).toBe("/");
    expect(safeReturnTo("breaks")).toBe("/");
  });

  // The composition is what ships: a tampered `returnTo` cannot make the
  // callback's success redirect leave this origin.
  it("cannot be composed into a cross-origin Location", () => {
    for (const hostile of ["//evil.example", "/\\evil.example", "https://evil.example"]) {
      const loc = sameOrigin(safeReturnTo(hostile)).headers.get("location");
      expect(loc).toBe("/");
    }
  });
});

// The OAuth `redirect_uri`, which is the one value here that a host is allowed
// to appear in — and the one place a disagreement costs every sign-in.
//
// ⚠ EACH CASE RE-IMPORTS THE MODULE. `authConfig()` memoizes, deliberately, so
// two cases sharing a module instance would test the first one's fetch twice.
// `vi.resetModules()` is what makes each of these independent.
async function originWith(opts: {
  published?: string;
  declared?: string;
  apiUp?: boolean;
}): Promise<string> {
  vi.resetModules();
  if (opts.declared === undefined) vi.stubEnv("RATIO_CONSOLE_ORIGIN", "");
  else vi.stubEnv("RATIO_CONSOLE_ORIGIN", opts.declared);
  vi.stubEnv("RATIO_API_ORIGIN", "https://api.example");
  vi.stubGlobal("fetch", async () => {
    if (opts.apiUp === false) throw new Error("connection refused");
    return new Response(
      JSON.stringify({
        issuer: "https://issuer.example",
        clientId: "cid",
        domain: "idp.example",
        scopes: ["openid", "email"],
        redirectPath: "/api/auth/callback",
        consoleOrigin: opts.published ?? "",
      }),
      { status: 200, headers: { "content-type": "application/json" } },
    );
  });
  const mod = await import("./redirect");
  return mod.consoleOrigin();
}

describe("consoleOrigin", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  });

  // ⛔ THE WHOLE CLAIM OF THE CHANGE, IN ONE CASE. A stale Vercel variable used
  // to decide the `redirect_uri`, and a wrong one meant Cognito refused every
  // sign-in. The value the API publishes is the string Cognito was configured
  // from, so it is the one that works — and it must win.
  it("prefers what the API publishes over a stale environment variable", async () => {
    expect(
      await originWith({
        published: "https://ratio-ims.vercel.app",
        declared: "https://ratio-console.vercel.app",
      }),
    ).toBe("https://ratio-ims.vercel.app");
  });

  it("says so in the log, because a variable that does nothing is a trap", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    await originWith({
      published: "https://ratio-ims.vercel.app",
      declared: "https://ratio-console.vercel.app",
    });
    expect(warn.mock.calls[0]?.[0]).toContain("ratio-console.vercel.app");
    expect(warn.mock.calls[0]?.[0]).toContain("ratio-ims.vercel.app");
    warn.mockRestore();
  });

  // ⚠ EMPTY IS THE LOCAL ANSWER, NOT A MISSING FIELD. A `ratio watch` on
  // loopback has no console to point at and publishes "", which is exactly when
  // the environment variable should decide — this is what keeps `next dev`
  // working against a laptop with no Cognito.
  it("falls back to the environment when the API publishes nothing", async () => {
    expect(
      await originWith({ published: "", declared: "http://localhost:3000" }),
    ).toBe("http://localhost:3000");
  });

  it("falls back when the API cannot be reached at all", async () => {
    expect(
      await originWith({ apiUp: false, declared: "http://localhost:3000" }),
    ).toBe("http://localhost:3000");
  });

  it("throws when neither has an answer, which the routes show as error=config", async () => {
    await expect(originWith({ published: "" })).rejects.toThrow(
      /no console origin/,
    );
  });

  it("tolerates a trailing slash on either, since Cognito would not", async () => {
    expect(await originWith({ published: "https://ratio-ims.vercel.app/" })).toBe(
      "https://ratio-ims.vercel.app",
    );
    expect(
      await originWith({ published: "", declared: "http://localhost:3000/" }),
    ).toBe("http://localhost:3000");
  });
});
