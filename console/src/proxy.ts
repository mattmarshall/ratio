import {
  applyResponseHeaders,
  authkit,
} from "@workos-inc/authkit-nextjs";
import { NextResponse, type NextRequest } from "next/server";
import { mergeAuthkitProxyHeaders } from "@/lib/authkitProxy";
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
  const pathname = req.nextUrl.pathname + req.nextUrl.search;

  let requestHeaders = new Headers(req.headers);
  let responseHeaders = new Headers();

  if (workosConfigured()) {
    const { headers: authkitHeaders } = await authkit(req);
    ({ requestHeaders, responseHeaders } = mergeAuthkitProxyHeaders(
      req,
      authkitHeaders,
      { "x-nonce": nonce, "x-pathname": pathname },
    ));
  } else {
    requestHeaders.set("x-nonce", nonce);
    requestHeaders.set("x-pathname", pathname);
  }

  const res = applyResponseHeaders(
    NextResponse.next({ request: { headers: requestHeaders } }),
    responseHeaders,
  );
  res.headers.set("Content-Security-Policy", POLICY(nonce));
  return res;
}

// ⛔ PREFETCH MUST STILL RUN AUTHKIT. Next's matcher `missing` list used to
// skip `next-router-prefetch` / `purpose: prefetch`. Client navigations and
// RSC fetches to `/books/[book]` arrive as prefetch; middleware never set
// `x-workos-middleware`; `withAuth()` threw; Next redacted to digest
// `2094318646`. AuthKit's catch-all (authkit-nextjs README) excludes only
// static assets — not prefetch. A route that can call `withAuth()` is a
// route this matcher must hit.
export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico).*)"],
};
