import type { NextConfig } from "next";

// ⛔ THESE HEADERS USED TO COME FROM THE RUST SERVER AND NO LONGER DO.
//
// `security_headers()` in `crates/ratio/src/watch.rs` set every one of these on
// the embedded console. Vercel serves these pages now, so that function never
// runs for them — and forgetting one is silent, because a missing security
// header looks exactly like a present one until somebody goes looking.
//
// The Content-Security-Policy is NOT here: it carries a per-request nonce and
// is set in `src/middleware.ts`. See the ⛔ there for why the old policy's
// 'unsafe-inline' reasoning does not survive the move to server components.
const SECURITY_HEADERS = [
  // Clickjacking. Both, because the old server sent both.
  { key: "X-Frame-Options", value: "DENY" },
  { key: "X-Content-Type-Options", value: "nosniff" },
  // ⚠ A fund id and a break id are in the path. `no-referrer` keeps them out of
  // the Referer header on any outbound navigation.
  { key: "Referrer-Policy", value: "no-referrer" },
  // ⛔ HSTS was sent ONLY on the canonical host (watch.rs), because on the
  // shared *.execute-api.amazonaws.com hostname it would poison HSTS for every
  // other AWS API served from it. The console has its own hostname, so that
  // caveat does not apply and it is sent unconditionally.
  {
    key: "Strict-Transport-Security",
    value: "max-age=63072000; includeSubDomains",
  },
];

const nextConfig: NextConfig = {
  reactStrictMode: true,

  // ⚠ The console renders a customer's NAV. A page of it must not sit in a
  // shared cache, and `Cache-Control: no-store` is what the API already says
  // about every response it sends (watch.rs). Route segments additionally set
  // `export const dynamic = "force-dynamic"` — this is the belt to that braces,
  // because a page prerendered at build time serving a stale NAV is the worst
  // failure available to this product.
  // Nested book screens reuse the fund pages. `/books`, `/books/new` and
  // `/books/[book]` have their own files, so afterFiles rewrites do not steal
  // them. A personal book never has to live under `/funds/{fund}/…`.
  async rewrites() {
    return [
      {
        source: "/books/:book/views/:path*",
        destination: "/funds/:book/views/:path*",
      },
      {
        source: "/books/:book/:path*",
        destination: "/funds/:book/:path*",
      },
    ];
  },

  async headers() {
    return [
      {
        source: "/:path*",
        headers: [
          ...SECURITY_HEADERS,
          { key: "Cache-Control", value: "no-store" },
        ],
      },
    ];
  },
};

export default nextConfig;
