import "server-only";

import { createCipheriv, createDecipheriv, randomBytes } from "node:crypto";
import { cookies } from "next/headers";

// Who is signed in, and the token the API is called with.
//
// ⛔ THE ID TOKEN LIVES HERE AND NOWHERE THE BROWSER CAN REACH. The console it
// replaces kept it in `sessionStorage`, which any script on the page can read.
// This is an httpOnly cookie, sealed with AES-256-GCM, and the page never sees
// it — the token is attached to the upstream request by `wire/client.ts`, on
// the server, after this module has opened the envelope.
//
// ⛔ NO REFRESH TOKEN IN THE COOKIE. A Cognito refresh token is good for thirty
// days by default and a cookie is a bearer; an id token is good for about an
// hour. Sealing both would also overflow the 4096-byte cookie limit, and a
// federated Google id token — the largest — is what would find that first. So
// the session expires in an hour and the person signs in again, which is
// exactly what the console did before. Refresh is a deliberate later change,
// not something to get by accident.

const COOKIE = "ratio_session";
const VERIFIER_COOKIE = "ratio_pkce";

/** What the sealed cookie carries. */
export interface Session {
  /** The Cognito id token. ⛔ NOT the access token — see `wire/client.ts`. */
  idToken: string;
  /** The `sub` claim: opaque, stable, and what a write is attributed to. */
  sub: string;
  /** The verified email. The tenant boundary matches on this. */
  email: string;
  /** Unix seconds. */
  expiresAt: number;
}

/**
 * The keys this deployment will decrypt with, newest first.
 *
 * ⚠ A KEYRING, NOT A KEY, SO ROTATION DOES NOT SIGN EVERYONE OUT. The first
 * entry seals; every entry opens. Rotate by prepending, deploy, and drop the
 * old one a session-lifetime later.
 */
function keys(): Buffer[] {
  const raw = process.env.RATIO_SESSION_KEYS;
  if (!raw) throw new Error("RATIO_SESSION_KEYS is not set");
  const ks = raw
    .split(",")
    .map((k) => k.trim())
    .filter(Boolean)
    .map((k) => Buffer.from(k, "base64"));
  if (!ks.length) throw new Error("RATIO_SESSION_KEYS is empty");
  for (const k of ks) {
    if (k.length !== 32) {
      throw new Error("each RATIO_SESSION_KEYS entry must be 32 base64 bytes");
    }
  }
  return ks;
}

function seal(value: unknown): string {
  const key = keys()[0]!;
  const iv = randomBytes(12);
  const c = createCipheriv("aes-256-gcm", key, iv);
  const body = Buffer.concat([
    c.update(JSON.stringify(value), "utf8"),
    c.final(),
  ]);
  return [iv, c.getAuthTag(), body].map((b) => b.toString("base64url")).join(".");
}

function unseal<T>(sealed: string): T | null {
  const parts = sealed.split(".");
  if (parts.length !== 3) return null;
  const [iv, tag, body] = parts.map((p) => Buffer.from(p, "base64url"));
  if (!iv || !tag || !body) return null;
  // Every key on the ring, so a cookie sealed before a rotation still opens.
  for (const key of keys()) {
    try {
      const d = createDecipheriv("aes-256-gcm", key, iv);
      d.setAuthTag(tag);
      const out = Buffer.concat([d.update(body), d.final()]);
      return JSON.parse(out.toString("utf8")) as T;
    } catch {
      /* wrong key, or a forged cookie — try the next */
    }
  }
  return null;
}

/** The session on this request, or null when there is none or it has expired. */
export async function readSession(): Promise<Session | null> {
  const raw = (await cookies()).get(COOKIE)?.value;
  if (!raw) return null;
  const s = unseal<Session>(raw);
  if (!s) return null;
  if (s.expiresAt * 1000 <= Date.now()) return null;
  return s;
}

export async function writeSession(s: Session): Promise<void> {
  (await cookies()).set(COOKIE, seal(s), {
    httpOnly: true,
    secure: true,
    // ⛔ Lax, NEVER Strict. The OAuth callback arrives as a top-level
    // navigation from the Cognito Hosted UI; `Strict` withholds the cookie on
    // exactly that request, and sign-in then fails with a message that reads
    // like a PKCE bug rather than a cookie one.
    sameSite: "lax",
    path: "/",
    expires: new Date(s.expiresAt * 1000),
  });
}

export async function clearSession(): Promise<void> {
  const jar = await cookies();
  jar.delete(COOKIE);
  jar.delete(VERIFIER_COOKIE);
}

/** What one in-flight sign-in needs to remember. */
export interface Pending {
  /** The PKCE verifier. Its SHA-256 is what went to the IdP. */
  verifier: string;
  /** ⛔ CSRF. The callback must present the same value or it is not our flow. */
  state: string;
  /** Where to land afterwards.
   *
   * ⛔ A PATH, NEVER A URL, AND `/api/auth/callback` REFUSES ANYTHING ELSE.
   * A return target taken from the outside and redirected to unchecked is an
   * open redirect, and on a sign-in route it is a token-exfiltration bug. It is
   * sealed in this cookie rather than carried in `state` so it cannot be
   * rewritten in flight, and it is re-checked on the way out anyway. */
  returnTo: string;
}

/** Stash what the sign-in needs, for the length of one sign-in. */
export async function writePending(p: Pending): Promise<void> {
  (await cookies()).set(VERIFIER_COOKIE, seal(p), {
    httpOnly: true,
    secure: true,
    sameSite: "lax",
    path: "/",
    maxAge: 600,
  });
}

export async function readPending(): Promise<Pending | null> {
  const raw = (await cookies()).get(VERIFIER_COOKIE)?.value;
  if (!raw) return null;
  return unseal<Pending>(raw);
}

export async function clearPending(): Promise<void> {
  (await cookies()).delete(VERIFIER_COOKIE);
}

/** Who to show in the header chip. */
export interface Principal {
  sub: string;
  email: string;
}

export function principalOf(s: Session | null): Principal | null {
  return s ? { sub: s.sub, email: s.email } : null;
}
