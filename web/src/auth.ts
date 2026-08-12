// Sign-in for the console: OIDC authorization-code + PKCE against the Cognito
// Hosted UI, whose public client configuration the server serves at
// `/authconfig.json`. The access token is sent as a bearer on every `/v1` call;
// the id token names the signed-in person for the header chip.
//
// ⛔ NO CLIENT SECRET. A browser cannot keep one, which is why PKCE exists: the
// `code_verifier`, held only in this tab's `sessionStorage`, is what proves the
// token request came from the same client that started the login. The verifier
// never leaves the browser; only its SHA-256 (the challenge) is sent to start.
//
// ⚠ Browser + Cognito verify this end to end; a typecheck cannot. The flow is
// deliberately small and standard so what needs a real IdP to confirm is as
// little as possible.

/** The public OIDC client configuration, from `/authconfig.json`. */
export interface AuthConfig {
  issuer: string;
  clientId: string;
  /** The Cognito hosted-UI domain, no scheme. */
  domain: string;
  scopes: string[];
  redirectPath: string;
}

/** Who is signed in, decoded from the id token. */
export interface Principal {
  sub: string;
  email: string;
}

const ACCESS_KEY = "ratio.access_token";
const ID_KEY = "ratio.id_token";
const VERIFIER_KEY = "ratio.pkce_verifier";

let configPromise: Promise<AuthConfig> | null = null;

/** The deployment's OIDC config, fetched once. Empty on a local run, which is
 *  how the console knows there is no identity provider and stays open. */
function config(): Promise<AuthConfig> {
  if (!configPromise) {
    configPromise = fetch("../authconfig.json").then((r) => r.json() as Promise<AuthConfig>);
  }
  return configPromise;
}

/** Whether the deployment has an identity provider configured at all. */
export async function authConfigured(): Promise<boolean> {
  const c = await config();
  return Boolean(c.domain && c.clientId);
}

/** The token to send as the API bearer, or null when not signed in.
 *
 * ⛔ THE ID TOKEN, NOT THE ACCESS TOKEN. The gateway's JWT authorizer accepts
 * either — an id token's `aud` is the client id, which is what the authorizer
 * validates — but only the id token carries the `email` (and `cognito:groups`)
 * claims the server authorizes on at `book_path`. A Cognito access token has
 * `sub` and `client_id` and NO email, so scoping a caller to their funds by
 * email would match nothing and every signed-in person would see an empty rail.
 * A federated (Google) caller is the same story: their email arrives only in the
 * id token. The access token is still kept (it is what an OAuth API bearer
 * normally is) in case a future route wants its scopes. */
export function bearerToken(): string | null {
  return sessionStorage.getItem(ID_KEY);
}

/** The signed-in person, decoded from the id token, or null. */
export function principal(): Principal | null {
  const id = sessionStorage.getItem(ID_KEY);
  if (!id) return null;
  try {
    const payload = id.split(".")[1];
    if (!payload) return null;
    const claims = JSON.parse(
      atob(payload.replace(/-/g, "+").replace(/_/g, "/")),
    ) as { sub?: string; email?: string };
    return { sub: claims.sub ?? "", email: claims.email ?? "" };
  } catch {
    return null;
  }
}

export function signOut(): void {
  sessionStorage.removeItem(ACCESS_KEY);
  sessionStorage.removeItem(ID_KEY);
  location.assign("/app");
}

function base64url(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

async function sha256(input: string): Promise<Uint8Array> {
  const d = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(input));
  return new Uint8Array(d);
}

/** Start the login: redirect this tab to the Cognito Hosted UI. */
export async function beginSignIn(): Promise<void> {
  const c = await config();
  if (!c.domain || !c.clientId) throw new Error("no identity provider is configured");
  const verifier = base64url(crypto.getRandomValues(new Uint8Array(32)));
  sessionStorage.setItem(VERIFIER_KEY, verifier);
  const challenge = base64url(await sha256(verifier));
  const url = new URL(`https://${c.domain}/oauth2/authorize`);
  url.search = new URLSearchParams({
    response_type: "code",
    client_id: c.clientId,
    redirect_uri: location.origin + c.redirectPath,
    scope: c.scopes.join(" "),
    code_challenge: challenge,
    code_challenge_method: "S256",
  }).toString();
  location.assign(url.toString());
}

/** If this load is an OAuth callback (`?code=`), exchange the code for tokens.
 *  Returns true if a sign-in just completed, so the caller can re-render. */
export async function completeSignIn(): Promise<boolean> {
  const code = new URLSearchParams(location.search).get("code");
  if (!code) return false;
  const c = await config();
  const verifier = sessionStorage.getItem(VERIFIER_KEY) ?? "";
  const r = await fetch(`https://${c.domain}/oauth2/token`, {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      grant_type: "authorization_code",
      client_id: c.clientId,
      code,
      redirect_uri: location.origin + c.redirectPath,
      code_verifier: verifier,
    }).toString(),
  });
  if (!r.ok) throw new Error("sign-in did not complete");
  const t = (await r.json()) as { access_token: string; id_token: string };
  sessionStorage.setItem(ACCESS_KEY, t.access_token);
  sessionStorage.setItem(ID_KEY, t.id_token);
  sessionStorage.removeItem(VERIFIER_KEY);
  // Drop `?code=` so a refresh does not try to re-exchange a spent code.
  history.replaceState({}, "", c.redirectPath);
  return true;
}
