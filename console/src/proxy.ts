import { authkit } from "@workos-inc/authkit-nextjs";
import { NextResponse, type NextRequest } from "next/server";
import { workosConfigured } from "@/lib/workos";

// The Content-Security-Policy, with a per-request nonce.
//
// ⛔ THE OLD POLICY'S EXCUSE DOES NOT SURVIVE THE MOVE TO SERVER COMPONENTS.
// `security_headers()` in `crates/ratio/src/watch.rs` allowed 'unsafe-inline'
// for script, and argued for it carefully: the inline blocks were compile-time
// constants and every dynamic node was built with `textContent` — never
// `innerHTML`, which `//crates/ratio:ratio_test` enforced — so there was no
// reflected-injection vector for a hash to close. That reasoning is gone. A
// server-rendered React page inlines its data into the document as part of the
// payload that hydrates it, so the inline script is no longer a constant. A
// nonce is what replaces the argument.
//
// ⛔ AND `connect-src` NO LONGER NEEDS THE COGNITO DOMAIN. The old console had
// to reach the IdP's token endpoint from the tab to complete PKCE. That
// exchange is server-side now, and the only thing the browser does with the IdP
// is a top-level navigation, which CSP does not gate. One fewer origin the page
// may talk to is the security win of moving the flow, and it is worth naming.
const POLICY = (nonce: string) =>
  [
    "default-src 'self'",
    // 'strict-dynamic' lets the nonced bootstrap load the chunks it needs
    // without naming each; the host-source fallbacks are for browsers that do
    // not implement it, which ignore the nonce and honour 'self'.
    `script-src 'self' 'nonce-${nonce}' 'strict-dynamic'`,
    // ⚠ Styles stay 'unsafe-inline'. React sets style attributes and Next
    // inlines the stylesheet on first paint; there is no nonce path for either
    // that does not mean shipping a second copy of the CSS. A style injection
    // is a defacement, not an exfiltration, and `default-src 'self'` still
    // forbids the network call that would make it one.
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data:",
    // The console talks to itself and to nothing else. Its API calls are made
    // by the server, not by this page.
    "connect-src 'self'",
    "form-action 'self'",
    "frame-ancestors 'none'",
    "base-uri 'none'",
    "object-src 'none'",
  ].join("; ");

// ⚠ `proxy`, not `middleware`. Next 16 renamed the convention and warns on the
// old filename; the export name follows the file.
export default async function proxy(req: NextRequest) {
  const nonce = Buffer.from(crypto.randomUUID()).toString("base64");

  const headers = new Headers(req.headers);
  headers.set("x-nonce", nonce);
  // ⛔ SO A DEEP LINK SURVIVES SIGNING IN. A server component cannot see its own
  // URL, and without this `caller()` can only send an unauthenticated visitor to
  // the default queue — which throws away the very thing this console exists to
  // provide. Somebody sent a link to ONE break; landing them somewhere else and
  // making them find it again is the old console's behaviour with extra steps.
  headers.set("x-pathname", req.nextUrl.pathname + req.nextUrl.search);

  // AuthKit's composable helper refreshes the session cookie. Route protection
  // stays in `caller()` / `withAuth()`, the same page-based pattern the docs
  // describe — this proxy already has CSP work that `authkitMiddleware()`
  // would replace. https://workos.com/docs/authkit/nextjs
  let authkitHeaders = new Headers();
  if (workosConfigured()) {
    const result = await authkit(req);
    authkitHeaders = result.headers;
  }

  const res = NextResponse.next({ request: { headers } });
  for (const [key, value] of authkitHeaders) {
    if (key.toLowerCase() === "set-cookie") {
      res.headers.append(key, value);
    } else {
      res.headers.set(key, value);
    }
  }
  res.headers.set("Content-Security-Policy", POLICY(nonce));
  return res;
}

export const config = {
  matcher: [
    // Everything except Next's own static output and the favicon — those are
    // immutable files with no document to protect.
    {
      source: "/((?!_next/static|_next/image|favicon.ico).*)",
      missing: [
        { type: "header", key: "next-router-prefetch" },
        { type: "header", key: "purpose", value: "prefetch" },
      ],
    },
  ],
};
