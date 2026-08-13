#!/usr/bin/env node
// Refuse to build a console that cannot sign anybody in.
//
// ⛔ THIS EXISTS BECAUSE A DEPLOYMENT SHIPPED GREEN AND WAS BROKEN. Every check
// this repository runs passed — Bazel, the five source-text checks, the render
// suite, `next build` — and the console went live with `RATIO_API_ORIGIN`
// pointing at its own URL. Nothing noticed, because nothing here had ever
// looked at the environment; the first thing that did was a person clicking
// Sign in and getting a 500. The configuration was the one part of the system
// with no test at all.
//
// ⭐ THE POINT IS *WHERE* IT FAILS, NOT THAT IT FAILS. A build that goes red on
// Vercel is a deploy that does not replace a working one, read by whoever
// pushed, while they are still looking. The same fact discovered at sign-in is
// an incident.
//
// ⚠ IT MUST BE SILENT WHEN THERE IS NOTHING TO CHECK. `pnpm check` runs
// `pnpm build`, so this runs in CI too — where no `RATIO_*` variable is set and
// none should be. Unset means "not a deployment", which is also `next dev`
// against a local `ratio watch`. That skip is load-bearing, not an escape
// hatch: without it every console push would need network and a live API.

const API = process.env.RATIO_API_ORIGIN?.replace(/\/+$/, "");
const DECLARED = process.env.RATIO_CONSOLE_ORIGIN?.replace(/\/+$/, "");
const KEYS = process.env.RATIO_SESSION_KEYS;
// Set by Vercel on every build. Distinguishes "deploying" from "a laptop with
// one variable exported", which changes what counts as a missing value.
const DEPLOYING = Boolean(process.env.VERCEL);

let failed = false;

const fail = (...m) => {
  failed = true;
  console.error("preflight ⛔", ...m);
};
const warn = (...m) => console.warn("preflight ⚠", ...m);
const ok = (...m) => console.log("preflight ✓", ...m);

if (!API) {
  console.log(
    "preflight — RATIO_API_ORIGIN is not set, so there is no deployment to check.",
  );
  process.exit(0);
}

// ---------------------------------------------------------------------------
// The API, and the OIDC configuration it publishes
// ---------------------------------------------------------------------------

const url = `${API}/authconfig.json`;
let published;

try {
  // ⚠ A DEADLINE, BECAUSE A BUILD THAT HANGS IS WORSE THAN ONE THAT FAILS. Ten
  // seconds is far past a healthy answer and far short of a Vercel build limit.
  const r = await fetch(url, {
    cache: "no-store",
    signal: AbortSignal.timeout(10_000),
  });

  if (r.status >= 500) {
    // ⛔ TRANSIENT IS NOT MISCONFIGURED, AND CONFLATING THEM WOULD BE WORSE THAN
    // NOT CHECKING. A 5xx means the API is rolling or briefly unwell — the
    // value is probably right and will work in a minute. Failing the build here
    // would couple every console deploy to the API's momentary health and teach
    // everybody to re-run a red build without reading it, which is how a real
    // failure gets clicked past.
    warn(`${url} returned ${r.status}. The API looks unwell rather than`,
      "misconfigured, so this is not fatal — but check it before trusting the deploy.");
  } else if (!r.ok) {
    // A 404 is the shape of every wrong value this variable can hold: the
    // console's own origin answers 404 here, and so does the API with a path
    // appended. Both are misconfigurations and neither fixes itself.
    // ⚠ NO EXAMPLE HOSTNAME IN THIS MESSAGE. `scripts/no_secrets_test.py`
    // refuses a deployment's own hostnames anywhere in the source, and a
    // plausible-looking placeholder is the shape it is looking for — the
    // instruction is the shape of the value, not a specimen of it. The real
    // one is the stack's `DemoUrl` output; `deploy/README.md` says so.
    fail(
      `${url} returned ${r.status}.`,
      "\n   RATIO_API_ORIGIN must be the API's origin — scheme and host only,",
      "\n   no path and no trailing slash. It is the stack's DemoUrl output.",
      `\n   It is currently ${API}`,
    );
  } else {
    const cfg = await r.json();
    if (!cfg || typeof cfg !== "object" || !("clientId" in cfg)) {
      fail(`${url} answered 200 but is not an auth config:`, JSON.stringify(cfg));
    } else {
      published = cfg.consoleOrigin?.replace(/\/+$/, "");
      ok(`${url} answers`, cfg.clientId ? "with an identity provider" : "with none configured");
    }
  }
} catch (e) {
  // A DNS failure, a refused connection, a timeout. Same reasoning as the 5xx:
  // this says nothing about whether the value is correct.
  warn(`${url} could not be reached (${e instanceof Error ? e.message : e}).`,
    "Not fatal — but nothing here has confirmed the API origin is right.");
}

// ---------------------------------------------------------------------------
// The origin the sign-in will hand to Cognito
// ---------------------------------------------------------------------------

// ⚠ A DISAGREEMENT IS NOT FATAL, BECAUSE THE CONSOLE RESOLVES IT CORRECTLY.
// `app/api/auth/redirect.ts` prefers the published value — the one Cognito was
// configured from — so a stale variable cannot break the sign-in any more. What
// it can do is mislead the next person who edits it expecting it to matter.
if (published && DECLARED && published !== DECLARED) {
  warn(
    `RATIO_CONSOLE_ORIGIN is ${DECLARED} but the API publishes ${published}.`,
    "\n   The published value is what Cognito registered and what the console will use.",
    "\n   Unset RATIO_CONSOLE_ORIGIN, or correct it to match.",
  );
}

// ⛔ NEITHER SOURCE HAS AN ORIGIN, ON A REAL DEPLOYMENT. There is no string to
// send as the OAuth `redirect_uri`, so every sign-in ends at the config error
// page. On a laptop this is ordinary; here it is a deployment that cannot work.
//
// ⚠ `=== ""` AND NOT `!published`, WHICH IS THE DIFFERENCE BETWEEN "PUBLISHED
// NOTHING" AND "DID NOT PUBLISH". An API older than this field leaves it
// `undefined`, and failing the build for that would make a console deploy
// impossible until the API deploy that adds it lands — an ordering trap on the
// very change that introduces the field. Empty is a deliberate answer from a
// server that has the field and has nothing to put in it.
if (DEPLOYING && published === "" && !DECLARED) {
  fail(
    "no console origin: the API publishes none and RATIO_CONSOLE_ORIGIN is not set.",
    "\n   Set the CONSOLE_ORIGIN repository variable so deploy.yml passes it to",
    "\n   CloudFormation — that registers the Cognito callback and publishes it here.",
  );
}

// ---------------------------------------------------------------------------
// The session key
// ---------------------------------------------------------------------------

// ⛔ NOTHING BELOW PRINTS THE VALUE, ONLY ITS SHAPE. This is the repository's
// one runtime secret. A build log is not a place a key goes, and a check that
// leaks what it is checking is worse than no check.
if (!KEYS) {
  if (DEPLOYING) {
    fail(
      "RATIO_SESSION_KEYS is not set. The sign-in would fail at the callback,",
      "\n   after the person has already authenticated — the worst place to find out.",
      "\n   Generate one with: openssl rand -base64 32",
    );
  }
} else {
  const entries = KEYS.split(",").map((k) => k.trim()).filter(Boolean);
  const bad = entries
    .map((k, i) => [i, Buffer.from(k, "base64").length])
    .filter(([, len]) => len !== 32);
  if (!entries.length) {
    fail("RATIO_SESSION_KEYS is set but empty.");
  } else if (bad.length) {
    fail(
      `RATIO_SESSION_KEYS entry ${bad.map(([i]) => i).join(", ")} decodes to`,
      `${bad.map(([, l]) => l).join(", ")} bytes; each must be 32.`,
      "\n   Generate one with: openssl rand -base64 32",
    );
  } else {
    // ⚠ The count, not the keys. More than one is a rotation in progress, which
    // is the design — the first seals, every one opens.
    ok(`${entries.length} session key(s), all 32 bytes`);
  }
}

if (failed) {
  console.error(
    "\npreflight failed — refusing to build a console that cannot sign anybody in.",
  );
  process.exit(1);
}
