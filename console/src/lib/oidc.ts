import "server-only";

import { createHash, randomBytes } from "node:crypto";

// Deprecated on the sign-in path. WorkOS AuthKit is the IdP
// (`@workos-inc/authkit-nextjs`, `/callback`). This module still fetches
// `/authconfig.json` so `redirect.ts` and preflight can see the API origin.
//
// Was: OIDC authorization-code + PKCE against the Cognito Hosted UI.
//
// ⛔ THE FLOW MOVED OFF THE BROWSER, AND THAT IS THE POINT OF THE MOVE. The
// console this replaces ran PKCE in the page: the verifier sat in
// `sessionStorage`, the token exchange was a `fetch` from the tab, and the
// Cognito domain had to be admitted to `connect-src` for it to work. Here the
// browser only ever performs one top-level navigation to the IdP and one back;
// the code, the verifier and the tokens never touch it.
//
// ⛔ NO CLIENT SECRET, STILL. The app client is public, which is why PKCE
// exists. Running it from a server does not make a secret available — it makes
// the verifier storable somewhere the page cannot read.

/** The public OIDC client configuration, exactly as `/authconfig.json` serves it. */
export interface AuthConfig {
  issuer: string;
  clientId: string;
  /** The Cognito hosted-UI domain, no scheme. */
  domain: string;
  scopes: string[];
  redirectPath: string;
  /**
   * Where the console is served from, as the deployment declared it.
   *
   * ⛔ THIS IS THE STRING COGNITO REGISTERED, WHICH IS WHY IT IS WORTH FETCHING.
   * `deploy/app.yaml` builds the app client's callback URL and this value out of
   * one `ConsoleOrigin` parameter, so a `redirect_uri` taken from here cannot
   * have drifted from the one the IdP will accept. Empty on a local `ratio
   * watch`; see `app/api/auth/redirect.ts` for the fallback.
   */
  consoleOrigin: string;
}

let cached: Promise<AuthConfig> | null = null;

/**
 * The deployment's OIDC configuration.
 *
 * ⭐ FETCHED FROM THE API RATHER THAN DUPLICATED AS ENVIRONMENT VARIABLES.
 * `deploy/app.yaml` declares the issuer, client id and hosted-UI domain once,
 * and the server publishes them at a public route the unauthenticated console
 * needs anyway. Copying them into Vercel would make two places that must agree
 * about which pool a token comes from, and nothing would notice when they
 * stopped agreeing.
 */
export async function authConfig(): Promise<AuthConfig> {
  if (cached) return cached;

  const origin = process.env.RATIO_API_ORIGIN;
  if (!origin) throw new Error("RATIO_API_ORIGIN is not set");
  const url = `${origin.replace(/\/+$/, "")}/authconfig.json`;

  const config = await fetch(url, { cache: "no-store" }).then((r) => {
    if (r.ok) return r.json() as Promise<AuthConfig>;
    // ⛔ NAME THE URL. The first version of this said only
    // "/authconfig.json returned 404", which is true of every wrong value
    // RATIO_API_ORIGIN can hold and tells you nothing about which one it is —
    // the console's own origin 404s here, and so does the API with a path
    // appended. An error about a misconfiguration that does not print the
    // misconfiguration costs somebody an afternoon, and did.
    throw new Error(
      `${url} returned ${r.status}. RATIO_API_ORIGIN must be the API's ` +
        `origin with no path — scheme and host only.`,
    );
  });

  // ⛔ ONLY A SUCCESS IS CACHED, AND THE FIRST VERSION CACHED THE PROMISE.
  // Assigning `cached` before awaiting means a REJECTED promise is memoized:
  // one blip while the API rolls, and that lambda instance answers with the
  // same stale failure for as long as it lives, with nothing to retry it. On a
  // serverless runtime that is a fault which outlives its cause and looks
  // permanent — a fixed deployment that keeps failing.
  cached = Promise.resolve(config);
  return config;
}

/**
 * Whether this deployment has an identity provider at all.
 *
 * ⭐ EMPTY MEANS LOCAL, AND THAT IS THE WHOLE LOCAL-DEVELOPMENT STORY. A
 * `ratio watch` on loopback sets none of the `RATIO_COGNITO_*` variables, so it
 * serves empty strings here; the console then skips sign-in entirely and the
 * server answers as `Subject::Local`, unrestricted. `next dev` against a local
 * book therefore needs no Cognito, no secrets and no network. This is the same
 * test the old `auth.ts` made, kept because it is what makes the thing runnable
 * on a laptop.
 */
export async function authConfigured(): Promise<boolean> {
  try {
    const c = await authConfig();
    return Boolean(c.domain && c.clientId);
  } catch {
    // An API that cannot be reached is not an identity provider that is absent.
    // Say "configured" so the caller surfaces the failure rather than silently
    // dropping the sign-in gate.
    return true;
  }
}

function base64url(b: Buffer): string {
  return b.toString("base64url");
}

export function newVerifier(): string {
  return base64url(randomBytes(32));
}

export function challengeFor(verifier: string): string {
  return base64url(createHash("sha256").update(verifier).digest());
}

/** Where to send the browser to start the sign-in. */
export async function authorizeUrl(
  redirectUri: string,
  challenge: string,
  state: string,
): Promise<string> {
  const c = await authConfig();
  if (!c.domain || !c.clientId) {
    throw new Error("no identity provider is configured");
  }
  const url = new URL(`https://${c.domain}/oauth2/authorize`);
  url.search = new URLSearchParams({
    response_type: "code",
    client_id: c.clientId,
    redirect_uri: redirectUri,
    scope: c.scopes.join(" "),
    code_challenge: challenge,
    code_challenge_method: "S256",
    state,
  }).toString();
  return url.toString();
}

export interface Tokens {
  idToken: string;
  expiresAt: number;
}

/** Exchange the authorization code. Server-side, so no CORS is involved. */
export async function exchange(
  code: string,
  verifier: string,
  redirectUri: string,
): Promise<Tokens> {
  const c = await authConfig();
  const r = await fetch(`https://${c.domain}/oauth2/token`, {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      grant_type: "authorization_code",
      client_id: c.clientId,
      code,
      redirect_uri: redirectUri,
      code_verifier: verifier,
    }).toString(),
    cache: "no-store",
  });
  if (!r.ok) throw new Error("sign-in did not complete");
  // ⛔ `id_token`, and the access token is deliberately dropped on the floor.
  // Only the id token carries the `email` claim the tenant boundary matches on;
  // keeping the access token beside it would be keeping the one a future reader
  // reaches for by mistake. See `wire/client.ts`.
  const t = (await r.json()) as { id_token: string; expires_in: number };
  return {
    idToken: t.id_token,
    expiresAt: Math.floor(Date.now() / 1000) + (t.expires_in || 3600),
  };
}

/**
 * The claims inside an id token.
 *
 * ⛔ DECODED, NOT VERIFIED, AND THAT IS CORRECT HERE — but only here. This
 * token was just handed to us by Cognito's own token endpoint over TLS, in
 * response to a code we started, so there is no untrusted party in between. The
 * signature check that matters happens on every API call, at the gateway's JWT
 * authorizer, which is the one place a forged token could arrive. Never reuse
 * this on a token that came from a client.
 */
export function claimsOf(idToken: string): { sub: string; email: string } {
  const payload = idToken.split(".")[1];
  if (!payload) return { sub: "", email: "" };
  const claims = JSON.parse(
    Buffer.from(payload, "base64url").toString("utf8"),
  ) as { sub?: string; email?: string };
  return { sub: claims.sub ?? "", email: claims.email ?? "" };
}
