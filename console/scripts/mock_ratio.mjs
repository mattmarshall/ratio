// A stand-in `ratio watch`, serving the committed fixtures at the /v1 routes.
//
// ⛔ THE FIXTURES ARE THE ONLY DATA IT KNOWS. capture_fixtures.sh's header
// draws the line this file lives behind: a fixture the server sent is what the
// server sends, and `//console:fixtures_test` holds the shape of every file
// here to console.proto. This mock invents nothing on top — a list route
// serves the captured list, a singleton route serves the first element of the
// captured list — so a screen rendered against it is a screen rendered against
// what a real book once said.
//
// ⚠ TEMPLATES ARE THE ONE LIST THAT IS KIND-AWARE. CreateBook writes a
// different mapping per kind, so a real `/v1/funds/{id}/templates` never
// offers `custodian-positions` on a household book. The committed fixture
// holds every seeded template in one file (a recapture of harbourline would
// drop the Personal/Project rows). Subsetting by the kind `books.json`
// already records is what the live server does by reading that book's
// config — not a second menu.
//
// ⚠ IT EXISTS FOR phone_test.mjs, NOT FOR DEVELOPMENT. A local run against
// real figures is `ratio watch --book <dir>` (see ../README.md); this exists
// so a layout check can boot the console with no Rust toolchain in the job.
// `/authconfig.json` answers with empty strings for the same reason it does on
// a local `ratio watch`: no identity provider means no sign-in gate.
//
// Usage: node mock_ratio.mjs [port]   (default 4373)

import { createServer } from "node:http";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const FIXTURES = join(dirname(fileURLToPath(import.meta.url)), "..", "fixtures");
const fixture = (name) => readFileSync(join(FIXTURES, `${name}.json`), "utf8");

// One entry per google.api.http rule the screens read. `:first` marks a
// singleton served as the first element of its captured list — the same
// derivation capture_fixtures.sh uses to pick an id.
//
// ⚠ `[^/:]` IN THE SINGLETON SEGMENTS, NOT `[^/]`. A custom method is
// `name:verb` in one path segment, so a singleton pattern that admits `:`
// swallows `…/navStrikes/x:explain` before the explain route is consulted —
// and answers it with the wrong shape.
const ROUTES = [
  [/^\/v1\/books$/, "books"],
  [/^\/v1\/books\/[^/:]+$/, "book"],
  [/^\/v1\/funds$/, "funds"],
  [/^\/v1\/funds\/[^/:]+$/, "fund"],
  [/^\/v1\/funds\/[^/]+\/views$/, "views"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/:]+$/, "view"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+:reconcile$/, "reconcile"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+\/breaks$/, "breaks"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+\/breaks\/[^/:]+$/, "break"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+\/accounts$/, "accounts"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+\/accounts\/[^/:]+$/, "accounts:first"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+\/accounts\/[^/]+\/postings$/, "postings"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+\/accounts\/[^/]+\/postings\/[^/:]+$/, "postings:first"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+\/positions$/, "positions"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+\/positions\/[^/:]+$/, "positions:first"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+\/positions\/[^/]+\/lots$/, "lots"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+\/positions\/[^/]+\/lots\/[^/:]+$/, "lots:first"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+\/navStrikes$/, "navStrikes"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+\/navStrikes\/[^/:]+$/, "navStrikes:first"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+\/navStrikes\/[^/]+:replay$/, "replay"],
  [/^\/v1\/funds\/[^/]+\/views\/[^/]+\/navStrikes\/[^/]+:explain$/, "explain"],
  [/^\/v1\/funds\/[^/]+\/configVersions$/, "configVersions"],
  [/^\/v1\/funds\/[^/]+\/configVersions\/[^/:]+$/, "configVersions:first"],
  [/^\/v1\/funds\/[^/]+\/configVersions\/[^/]+:diff$/, "diff"],
  [/^\/v1\/funds\/[^/]+\/rules$/, "rules"],
  [/^\/v1\/funds\/[^/]+\/rules\/[^/:]+$/, "rules:first"],
  [/^\/v1\/funds\/[^/]+\/templates$/, "templates"],
  [/^\/v1\/funds\/[^/]+\/templates\/[^/:]+$/, "templates:first"],
  [/^\/v1\/funds\/[^/]+\/deliveries$/, "deliveries"],
  [/^\/v1\/funds\/[^/]+\/deliveries\/[^/:]+$/, "deliveries:first"],
  [/^\/v1\/funds\/[^/]+\/pendingFacts$/, "pendingFacts"],
  [/^\/v1\/funds\/[^/]+\/pendingFacts\/[^/:]+$/, "pendingFacts:first"],
  [/^\/v1\/funds\/[^/]+\/corporateActions$/, "corporateActions"],
  [/^\/v1\/funds\/[^/]+\/corporateActions\/[^/:]+$/, "corporateActions:first"],
  [/^\/v1\/funds\/[^/]+\/changeLogEntries$/, "changeLogEntries"],
  [/^\/v1\/funds\/[^/]+\/changeLogEntries\/[^/:]+$/, "changeLogEntries:first"],
  [/^\/v1\/funds\/[^/]+\/entries$/, "entries"],
  [/^\/v1\/funds\/[^/]+\/entries\/[^/:]+$/, "entry"],
];

// CreateBook seed ids. An id that is not in this map (the demo book's
// `vendor_eod_prices`) is treated as Investment — prices ride the same
// book as the trade loop.
const INGEST_TEMPLATE_KIND = {
  "bank-statement": "PERSONAL",
  "custodian-positions": "INVESTMENT",
  "prime_equity_trades": "INVESTMENT",
  "project-invoices": "PROJECT",
};

function bookKind(bookId) {
  const books = JSON.parse(fixture("books"));
  const row = books.books.find((b) => b.name.split("/").pop() === bookId);
  return row?.kind ?? "INVESTMENT";
}

function templatesDoc(path) {
  const bookId = path.split("/")[3];
  const kind = bookKind(bookId);
  const doc = JSON.parse(fixture("templates"));
  doc.templates = doc.templates.filter((t) => {
    const owner = INGEST_TEMPLATE_KIND[t.templateId] ?? "INVESTMENT";
    return owner === kind;
  });
  for (const t of doc.templates) {
    t.name = `funds/${bookId}/templates/${t.templateId}`;
  }
  return doc;
}

function body(name, path) {
  if (name === "templates") return JSON.stringify(templatesDoc(path));
  if (name === "templates:first") {
    return JSON.stringify(templatesDoc(path).templates[0]);
  }
  // ⭐ KIND SELECTS CHROME. GetBook used to serve `book.json` (a personal
  // household) for every id. Look the id up in the captured list.
  if (name === "book") {
    const id = path.split("/").pop();
    const books = JSON.parse(fixture("books")).books;
    const found = books.find((b) => b.name === `books/${id}`);
    if (found) return JSON.stringify(found);
    return fixture("book");
  }
  if (!name.endsWith(":first")) return fixture(name);
  const doc = JSON.parse(fixture(name.slice(0, -":first".length)));
  const key = Object.keys(doc).find((k) => Array.isArray(doc[k]));
  return JSON.stringify(doc[key][0]);
}

export function serve(port = 4373) {
  return new Promise((resolve) => {
    const server = createServer((req, res) => {
      const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
      if (path === "/authconfig.json") {
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({
          issuer: "", clientId: "", domain: "", scopes: [],
          redirectPath: "", consoleOrigin: "",
        }));
        return;
      }
      // ⭐ KIND IS PER BOOK. A singleton `book.json` is Household (Personal);
      // serving it for every id made a Project URL wear fund-ops chrome in
      // the phone pass. Look the id up in the list the hub already uses.
      if (/^\/v1\/books\/[^/:]+$/.test(path)) {
        const id = path.slice("/v1/books/".length);
        const list = JSON.parse(fixture("books"));
        const found = list.books.find((b) => b.name === `books/${id}`);
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify(found ?? JSON.parse(fixture("book"))));
        return;
      }
      const hit = ROUTES.find(([re]) => re.test(path));
      if (!hit) {
        res.writeHead(404, { "content-type": "application/json" });
        res.end(JSON.stringify({ error: `no fixture route for ${path}` }));
        return;
      }
      res.writeHead(200, { "content-type": "application/json" });
      res.end(body(hit[1], path));
    });
    server.listen(port, "127.0.0.1", () => resolve(server));
  });
}

// Runnable on its own, for pointing a `pnpm dev` at fixture data by hand.
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const port = Number(process.argv[2] || 4373);
  await serve(port);
  console.log(`mock ratio on 127.0.0.1:${port} — RATIO_API_ORIGIN=http://127.0.0.1:${port}`);
}
