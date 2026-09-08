import { describe, expect, it, vi } from "vitest";
import type { NextRequest } from "next/server";

/**
 * Request-vs-response header partition (#112).
 *
 * ⛔ REQUEST HEADERS, NOT RESPONSE ONES. `withAuth()` reads
 * `x-workos-middleware` from the incoming request. Putting AuthKit's
 * session headers on the response instead reached production as
 * `Minified React error #441` on `/books` after sign-in.
 */

vi.mock("@workos-inc/authkit-nextjs", () => ({
  partitionAuthkitHeaders: (
    req: { headers: Headers },
    authkitHeaders: Headers,
  ) => {
    const requestHeaders = new Headers(req.headers);
    const responseHeaders = new Headers();
    const cookies =
      typeof authkitHeaders.getSetCookie === "function"
        ? authkitHeaders.getSetCookie()
        : [];
    for (const [key, value] of authkitHeaders.entries()) {
      if (key.toLowerCase() === "set-cookie") continue;
      requestHeaders.set(key, value);
    }
    for (const cookie of cookies) {
      responseHeaders.append("set-cookie", cookie);
    }
    return { requestHeaders, responseHeaders };
  },
}));

describe("mergeAuthkitProxyHeaders", () => {
  it("puts AuthKit session headers and extras on the request, cookies on the response", async () => {
    const { mergeAuthkitProxyHeaders } = await import("./authkitProxy");
    const req = {
      headers: new Headers({ cookie: "wos-session=sealed" }),
    } as NextRequest;
    const authkitHeaders = new Headers({
      "x-workos-middleware": "true",
      "x-workos-session": "sealed",
      "set-cookie": "wos-session=sealed; Path=/",
    });

    const { requestHeaders, responseHeaders } = mergeAuthkitProxyHeaders(
      req,
      authkitHeaders,
      { "x-nonce": "n", "x-pathname": "/books/harbourline" },
    );

    expect(requestHeaders.get("x-workos-middleware")).toBe("true");
    expect(requestHeaders.get("x-workos-session")).toBe("sealed");
    expect(requestHeaders.get("x-pathname")).toBe("/books/harbourline");
    expect(requestHeaders.get("x-nonce")).toBe("n");
    expect(requestHeaders.get("set-cookie")).toBeNull();
    expect(responseHeaders.get("set-cookie")).toContain("wos-session");
  });

  it("does not forward a PKCE verifier cookie on NextResponse.next()", async () => {
    const { mergeAuthkitProxyHeaders } = await import("./authkitProxy");
    const req = { headers: new Headers() } as NextRequest;
    const authkitHeaders = new Headers({ "x-workos-middleware": "true" });
    authkitHeaders.append("set-cookie", "wos-session=sealed; Path=/");
    authkitHeaders.append(
      "set-cookie",
      "wos-auth-verifier-abcd=sealed; Path=/",
    );

    const { responseHeaders } = mergeAuthkitProxyHeaders(req, authkitHeaders, {
      "x-nonce": "n",
      "x-pathname": "/books",
    });

    const cookies = responseHeaders.getSetCookie();
    expect(cookies.some((c) => c.startsWith("wos-session="))).toBe(true);
    expect(cookies.some((c) => c.includes("wos-auth-verifier"))).toBe(false);
  });
});
