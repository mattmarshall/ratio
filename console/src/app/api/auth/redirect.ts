import "server-only";

/**
 * The origin this console is served from.
 *
 * ⛔ AN ENVIRONMENT VARIABLE, NEVER THE `Host` HEADER. The value is used as an
 * OAuth `redirect_uri`, and Cognito matches it against a registered callback
 * exactly. Deriving it from the request would let a forged `Host` choose where
 * the code is delivered; declaring it means a mismatch is a refused sign-in,
 * which is a loud failure rather than a quiet one.
 *
 * ⚠ Cognito accepts no wildcards, so exactly three values are registered in
 * `deploy/app.yaml`: production, one stable preview alias, and
 * `http://localhost:3000`. A preview deployment on its own generated hostname
 * cannot sign in, by design — previews run against `console/fixtures/`.
 */
export function consoleOrigin(): string {
  const o = process.env.RATIO_CONSOLE_ORIGIN;
  if (!o) throw new Error("RATIO_CONSOLE_ORIGIN is not set");
  return o.replace(/\/+$/, "");
}

/**
 * A return target that cannot leave this origin.
 *
 * ⛔ AN OPEN REDIRECT ON A SIGN-IN ROUTE IS A TOKEN-EXFILTRATION BUG, so this
 * refuses everything except a single-slash-rooted path. `//evil.example` and
 * `/\evil.example` are both protocol-relative URLs that a naive
 * `startsWith("/")` accepts, and both are why the second character is checked.
 */
export function safeReturnTo(raw: string | null | undefined): string {
  if (!raw) return "/";
  if (!raw.startsWith("/")) return "/";
  if (raw.startsWith("//") || raw.startsWith("/\\")) return "/";
  return raw;
}
