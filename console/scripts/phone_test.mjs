// The console fits a phone, held by a browser rather than claimed by a diff.
//
// ⛔ WHAT WENT WRONG, SO THIS EXISTS. The mobile layout grid briefly read
// `grid-template-columns:1fr` — which is minmax(auto,1fr), so the screen tabs'
// min-content width set the width of every page and every screen scrolled
// sideways by 122px on an iPhone. One character of CSS, invisible to every
// check in this directory, because every other check reads source text and
// this defect only exists once a layout engine has run. So a layout engine
// runs: the built console boots against the committed fixtures, a browser
// opens every screen at 375×812 (the smallest phone anybody names, an
// iPhone 13 mini), and two things are asserted per screen —
//
//   * nothing scrolls sideways that was not designed to: not the document,
//     and not any scroll container outside the named allowlist. The second
//     half is load-bearing — `.queue` is an overflow-y scroller, which makes
//     its computed overflow-x `auto`, so a too-wide tab row scrolls INSIDE it
//     and the document's own scrollWidth never moves. The allowlist is the
//     set of boxes that scroll on purpose: the trial balance, the plan
//     diagram, the step tree, the screen tabs, and a textarea.
//   * the screen's content actually rendered — the named landmark for detail
//     screens, `main` otherwise — and is visible. This holds the other
//     regression this file was born from: a `.detail{display:none}` media
//     query that would have blanked every detail page below 1180px, saved
//     only by a later rule of equal specificity.
//
// ⚠ AGAINST `next start`, NOT `next dev`. What ships is the production build;
// a dev-server-only pass would hold nothing about it. Run `pnpm build` first —
// this script refuses to start without one.
//
// ⚠ THE BROWSER IS THE ONE PIECE NOT IN node_modules. `playwright` is a
// devDependency (its browser-downloading postinstall is blocked by this
// package's `onlyBuiltDependencies` policy, like every other install hook);
// CI fetches the browser explicitly with `pnpm exec playwright install
// chromium`, and a developer with a Chromium somewhere else points
// CHROMIUM_PATH at it instead.
//
// Usage: node scripts/phone_test.mjs   (from console/, after `pnpm build`)

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { chromium } from "playwright";
import { serve } from "./mock_ratio.mjs";

const CONSOLE = join(dirname(fileURLToPath(import.meta.url)), "..");
const API_PORT = Number(process.env.MOCK_RATIO_PORT || 4373);
const APP_PORT = Number(process.env.PHONE_TEST_PORT || 4300);

// The fixtures' two funds: the blocked book carries the break, the pending
// fact and the explanation state; the struck book is the only one with a NAV
// strike to open, replay or explain — capture_fixtures.sh says why.
const F = "harbourline-global-value";
const S = "northstar-multi-strategy";

// Every screen the route manifest names, one representative URL each. The id
// segments are arbitrary — the mock serves each singleton as the first element
// of its captured list whatever it is asked for.
const SCREENS = [
  ["/books", "main"],
  ["/books/new", "main"],
  ["/projects", "main"],
  [`/books/${F}`, "main"],
  ["/funds", "main"],
  [`/funds/${F}`, "main"],
  [`/books/${F}/views/abor/breaks`, "main"],
  [`/books/${F}/views/abor/breaks/x`, '[aria-label="Break detail"]'],
  [`/books/${F}/views/abor/accounts`, "main"],
  [`/books/${F}/views/abor/accounts/x`, '[aria-label="Account detail"]'],
  [`/books/${F}/views/abor/accounts/x/postings/x`, '[aria-label="Posting detail"]'],
  [`/books/${F}/views/abor/positions`, "main"],
  [`/books/${F}/views/abor/positions/x`, '[aria-label="Position detail"]'],
  [`/books/${F}/views/abor/positions/x/lots/x`, '[aria-label="Lot detail"]'],
  [`/books/${F}/views/abor/reconcile?against=ibor`, "main"],
  [`/books/${S}/views/abor/strikes`, "main"],
  [`/books/${S}/views/abor/strikes/x`, '[aria-label="NAV strike"]'],
  [`/books/${S}/views/abor/strikes/x/plan`, '[aria-label="How this NAV was computed"]'],
  [`/books/${S}/views/abor/strikes/x/replay`, '[aria-label="Replay"]'],
  [`/books/${F}/trade`, "main"],
  [`/books/${F}/record`, "main"],
  [`/books/${F}/ingest`, "main"],
  [`/books/${F}/mark`, "main"],
  [`/books/${F}/config`, "main"],
  [`/books/${F}/config/x`, '[aria-label="Configuration version"]'],
  [`/books/${F}/config/x/diff`, '[aria-label="Configuration diff"]'],
  [`/books/${F}/rules`, "main"],
  [`/books/${F}/rules/x`, '[aria-label="Rule"]'],
  [`/books/${F}/data`, "main"],
  [`/books/${F}/data/deliveries/x`, '[aria-label="Delivery"]'],
  [`/books/${F}/data/pending/x`, '[aria-label="Pending fact"]'],
  [`/books/${F}/data/templates`, "main"],
  [`/books/${F}/data/templates/x`, '[aria-label="Template"]'],
  [`/books/${F}/changes`, "main"],
  [`/books/${F}/changes/x`, '[aria-label="Change log entry"]'],
  [`/books/${F}/actions`, "main"],
  [`/books/${F}/actions/x`, '[aria-label="Corporate action"]'],
];

function fail(msg) {
  console.error(`::error::${msg}`);
  process.exitCode = 1;
}

if (!existsSync(join(CONSOLE, ".next"))) {
  console.error("::error::no .next build — run `pnpm build` first; this checks what ships");
  process.exit(1);
}

const api = await serve(API_PORT);

const app = spawn("pnpm", ["exec", "next", "start", "-p", String(APP_PORT)], {
  cwd: CONSOLE,
  env: { ...process.env, RATIO_API_ORIGIN: `http://127.0.0.1:${API_PORT}` },
  stdio: ["ignore", "pipe", "inherit"],
});
app.stdout.resume();

// Readiness is the server answering, not a line on stdout — Next has reworded
// its banner between majors.
const origin = `http://127.0.0.1:${APP_PORT}`;
for (let tries = 0; ; tries++) {
  try {
    await fetch(`${origin}/funds`);
    break;
  } catch {
    if (tries > 60) {
      console.error("::error::next start never answered");
      process.exit(1);
    }
    await new Promise((r) => setTimeout(r, 500));
  }
}

const browser = await chromium.launch({
  executablePath: process.env.CHROMIUM_PATH || undefined,
});
const page = await browser.newContext({
  // An iPhone 13 mini. isMobile matters: it is what makes Chromium apply the
  // viewport meta the way a phone does.
  viewport: { width: 375, height: 812 },
  deviceScaleFactor: 3,
  isMobile: true,
  hasTouch: true,
}).then((ctx) => ctx.newPage());

let checked = 0;
for (const [path, landmark] of SCREENS) {
  const r = await page.goto(origin + path, { waitUntil: "networkidle" });
  if (!r.ok()) {
    fail(`${path}: HTTP ${r.status()}`);
    continue;
  }
  const { overflow, sideScrollers, visible } = await page.evaluate((sel) => {
    // The boxes that scroll sideways BY DESIGN — each one a deliberate "the
    // row scrolls, the page does not" in globals.css.
    const ALLOW = ".tb,.planscroll,.steps,.screens,textarea";
    const sideScrollers = [];
    for (const el of document.querySelectorAll("*")) {
      const ox = getComputedStyle(el).overflowX;
      if (
        (ox === "auto" || ox === "scroll") &&
        el.scrollWidth > el.clientWidth + 1 &&
        !el.matches(ALLOW)
      ) {
        sideScrollers.push(
          `${el.tagName.toLowerCase()}.${[...el.classList].join(".")} by ${
            el.scrollWidth - el.clientWidth
          }px`,
        );
      }
    }
    const el = document.querySelector(sel);
    return {
      overflow:
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
      sideScrollers,
      visible: !!el && el.getClientRects().length > 0,
    };
  }, landmark);
  if (overflow > 0) fail(`${path}: the page scrolls sideways by ${overflow}px at 375px`);
  for (const s of sideScrollers) {
    fail(`${path}: ${s} scrolls sideways and is not one of the boxes designed to`);
  }
  if (!visible) fail(`${path}: ${landmark} did not render, or is display:none`);
  checked++;
}

// Seeded demo permalinks still resolve: the fund job URL lands on the book.
{
  const legacy = `/funds/${F}/views/abor/breaks`;
  const r = await page.goto(origin + legacy, { waitUntil: "networkidle" });
  if (!r || !r.ok()) fail(`${legacy}: HTTP ${r?.status()}`);
  const landed = new URL(page.url()).pathname;
  if (landed !== `/books/${F}/views/abor/breaks`) {
    fail(`${legacy}: redirected to ${landed}, not the book URL`);
  }
}

await browser.close();
app.kill();
api.close();

if (process.exitCode) {
  console.error(`\n${SCREENS.length - checked ? "some screens failed to answer; " : ""}the console does not fit a phone`);
} else {
  console.log(`  ok  ${checked} screen(s) at 375px: no sideways scroll, every landmark visible`);
}
