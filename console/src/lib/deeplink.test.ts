import { describe, expect, it } from "vitest";

import {
  COLLECTION_TO_SEGMENT,
  candidatesForId,
  hrefForResourceName,
} from "./deeplink";

// ⛔ THE HALF A RENDER TEST CANNOT DO. `hrefForResourceName` is the one place in
// this console where a whole resource name becomes a URL, and seven of the
// fifteen collections are spelled differently on the two sides. A palette that
// sent `navStrikes` into the path would 404 on a resource that exists — and it
// would do it in the confident voice it uses for the eight that pass through
// unchanged.
//
// ⛔ NEGATIVE-TEST EVERY CASE BELOW. Take the entry out of
// COLLECTION_TO_SEGMENT, or add a heuristic to `candidatesForId`, and watch the
// case go red before believing it.

const FUND = "harbourline-global-value";
const VIEW = "abor";

describe("a pasted resource name", () => {
  // One per `pattern:` in console.proto, in the order they are declared there.
  // ⚠ WHEN THE CONTRACT GROWS A RESOURCE, IT GETS A LINE HERE. The palette is
  // the only reader that translates a whole name, so nothing else would notice.
  const patterns: ReadonlyArray<readonly [string, string]> = [
    [`books/${FUND}`, `/books/${FUND}`],
    [`funds/${FUND}`, `/funds/${FUND}`],
    [`funds/${FUND}/views/${VIEW}`, `/books/${FUND}/views/${VIEW}`],
    [
      `funds/${FUND}/views/${VIEW}/breaks/cash-usd-2026-02-26`,
      `/books/${FUND}/views/${VIEW}/breaks/cash-usd-2026-02-26`,
    ],
    [`funds/${FUND}/changeLogEntries/1`, `/books/${FUND}/changes/1`],
    [
      `funds/${FUND}/views/${VIEW}/navStrikes/2026-02-26`,
      `/books/${FUND}/views/${VIEW}/strikes/2026-02-26`,
    ],
    [`funds/${FUND}/corporateActions/acme-3for2`, `/books/${FUND}/actions/acme-3for2`],
    [
      `funds/${FUND}/configVersions/9f2c1ab7de40551c`,
      `/books/${FUND}/config/9f2c1ab7de40551c`,
    ],
    [
      `funds/${FUND}/views/${VIEW}/accounts/1010`,
      `/books/${FUND}/views/${VIEW}/accounts/1010`,
    ],
    [
      `funds/${FUND}/views/${VIEW}/accounts/1010/postings/p-1`,
      `/books/${FUND}/views/${VIEW}/accounts/1010/postings/p-1`,
    ],
    [`funds/${FUND}/rules/perf_fee`, `/books/${FUND}/rules/perf_fee`],
    [`funds/${FUND}/deliveries/7c1e0d`, `/books/${FUND}/data/deliveries/7c1e0d`],
    [`funds/${FUND}/pendingFacts/pf-1`, `/books/${FUND}/data/pending/pf-1`],
    [
      `funds/${FUND}/views/${VIEW}/positions/ACME`,
      `/books/${FUND}/views/${VIEW}/positions/ACME`,
    ],
    [
      `funds/${FUND}/views/${VIEW}/positions/ACME/lots/1`,
      `/books/${FUND}/views/${VIEW}/positions/ACME/lots/1`,
    ],
    [
      `funds/${FUND}/templates/custodian-positions`,
      `/books/${FUND}/data/templates/custodian-positions`,
    ],
  ];

  it.each(patterns)("resolves %s to exactly one url", (name, href) => {
    expect(hrefForResourceName(name)).toBe(href);
  });

  it("spells a nav strike navStrikes on the wire and strikes in the url", () => {
    // ⭐ The rename that has the most room to be wrong: `wire/client.ts` calls
    // `/views/{view}/navStrikes` and `routes.ts` serves `/views/[view]/strikes`.
    expect(COLLECTION_TO_SEGMENT["navStrikes"]).toBe("strikes");
    expect(
      hrefForResourceName(`funds/${FUND}/views/${VIEW}/navStrikes/2026-02-26`),
    ).toContain("/strikes/");
  });

  it("nests the three data-plane collections under data", () => {
    expect(hrefForResourceName(`funds/${FUND}/deliveries/7c1e0d`)).toContain("/data/");
    expect(hrefForResourceName(`funds/${FUND}/pendingFacts/pf-1`)).toContain("/data/");
    expect(hrefForResourceName(`funds/${FUND}/templates/t`)).toContain("/data/");
  });

  it("keeps a fund with no children on the filing page", () => {
    // ⭐ THE FILING LAYER. Jobs moved under /books; ListFunds and the fund
    // overview stay. A name that is only `funds/{id}` is that page, not a job.
    expect(hrefForResourceName(`funds/${FUND}`)).toBe(`/funds/${FUND}`);
  });

  it("lands a book of record on the view itself", () => {
    // #53: `views/[view]` is now a page. Redirecting past it hid the citation.
    expect(hrefForResourceName(`funds/${FUND}/views/${VIEW}`)).toBe(
      `/books/${FUND}/views/${VIEW}`,
    );
  });

  it("refuses a resource this console gives no url", () => {
    // ⛔ `funds/{fund}/entries/{entry}` IS IN THE CONTRACT AND HAS NO SCREEN. An
    // entry is cited by postings and rendered inside them. Inventing a URL here
    // would offer a 404 in the same voice as the fifteen that work.
    expect(hrefForResourceName(`funds/${FUND}/entries/5`)).toBeNull();
  });

  it("refuses a collection with nothing in it", () => {
    // `funds/f/breaks` names the LIST, not a break — an odd segment count.
    expect(hrefForResourceName(`funds/${FUND}/views/${VIEW}/breaks`)).toBeNull();
  });

  it("refuses a string that is not a resource name at all", () => {
    expect(hrefForResourceName("cash-usd-2026-02-26")).toBeNull();
    expect(hrefForResourceName("")).toBeNull();
    expect(hrefForResourceName("/etc/passwd")).toBeNull();
    expect(hrefForResourceName(`funds/${FUND}/whatever/x`)).toBeNull();
  });

  it("reads a name whether or not somebody kept the leading slash", () => {
    expect(hrefForResourceName(`  /funds/${FUND}/rules/perf_fee  `)).toBe(
      `/books/${FUND}/rules/perf_fee`,
    );
  });

  it("escapes an id rather than letting it add a path segment", () => {
    expect(hrefForResourceName(`funds/${FUND}/rules/a%2Fb`)).toBe(
      `/books/${FUND}/rules/a%252Fb`,
    );
  });
});

describe("a bare id", () => {
  it("offers every collection it could belong to and picks none of them", () => {
    // ⛔ THE ASSERTION IS THE PLURAL. `2026-02-26` is a valid NAV strike id and
    // the tail of a break id, and nothing here can look either up. Add a
    // heuristic that narrows this to one and this case goes red.
    const c = candidatesForId(FUND, VIEW, "2026-02-26");
    expect(c.length).toBeGreaterThan(1);
    expect(c.map((x) => x.key)).toContain("strikes");
    expect(c.map((x) => x.key)).toContain("breaks");
  });

  it("offers the same candidates whatever the id looks like", () => {
    // ⛔ THE POINT, STATED AS AN EQUALITY. A date, a ticker, a GL code and a
    // slug all get the same eleven routes, because the namespaces collide and
    // the console cannot check. Any shape-based ranking breaks this.
    const keys = (id: string) => candidatesForId(FUND, VIEW, id).map((c) => c.key);
    expect(keys("2026-02-26")).toEqual(keys("ACME"));
    expect(keys("1010")).toEqual(keys("perf_fee"));
  });

  it("never claims the resource exists", () => {
    // Every label is an instruction to open something AS a kind of thing. None
    // asserts that it is one — nothing here has checked, and a row reading
    // "Break cash-usd-2026-02-26" would be a fact about the book.
    for (const c of candidatesForId(FUND, VIEW, "cash-usd-2026-02-26")) {
      expect(c.label).toMatch(/^Open “cash-usd-2026-02-26” as an? /);
    }
  });

  it("sends every candidate to a route that exists", () => {
    // ⚠ The shapes `routes.ts` declares. A candidate whose href has no page is
    // the defect this whole tier is trying not to introduce.
    const shapes = [
      `/books/${FUND}/views/${VIEW}/breaks/`,
      `/books/${FUND}/views/${VIEW}/positions/`,
      `/books/${FUND}/views/${VIEW}/accounts/`,
      `/books/${FUND}/views/${VIEW}/strikes/`,
      `/books/${FUND}/rules/`,
      `/books/${FUND}/changes/`,
      `/books/${FUND}/config/`,
      `/books/${FUND}/actions/`,
      `/books/${FUND}/data/deliveries/`,
      `/books/${FUND}/data/pending/`,
      `/books/${FUND}/data/templates/`,
    ];
    expect(candidatesForId(FUND, VIEW, "x9").map((c) => c.href)).toEqual(
      shapes.map((s) => `${s}x9`),
    );
  });

  it("puts the literal views segment in every view-scoped candidate", () => {
    // ⛔ Never `/books/{f}/{view}/…`. Next resolves static segments before
    // dynamic ones, so a view a fund happens to name `config` would silently
    // shadow the configuration screen. `routes.ts` argues this where it declares
    // the view layer, and a view named `config` is the case that proves it.
    const scoped = new Set(["breaks", "positions", "accounts", "strikes"]);
    for (const c of candidatesForId(FUND, "config", "x9")) {
      if (scoped.has(c.key)) {
        expect(c.href).toBe(`/books/${FUND}/views/config/${c.key}/x9`);
      } else {
        // The control-plane screens are not view-scoped, so they carry no view
        // at all — a rule set is the same document whichever way you recognise
        // the entries it produced.
        expect(c.href).not.toContain("/views/");
      }
    }
  });

  it("offers no lot and no posting, because a bare id cannot address one", () => {
    // ⭐ Both take TWO ids — a lot under its position, a posting under its
    // account. A fact about the route tree, not an omission.
    const keys = candidatesForId(FUND, VIEW, "1").map((c) => c.key);
    expect(keys).not.toContain("lots");
    expect(keys).not.toContain("postings");
  });

  it("stays quiet on one character and on a resource name", () => {
    expect(candidatesForId(FUND, VIEW, "A")).toHaveLength(0);
    // A slash means it is a name, which `hrefForResourceName` answers exactly.
    expect(candidatesForId(FUND, VIEW, `funds/${FUND}`)).toHaveLength(0);
  });

  it("separates its keywords with commas", () => {
    // ⚠ kbar's Fuse config reads this field as `keywords.split(",")`. Spaces
    // would make each one a single long token that matches nothing.
    for (const c of candidatesForId(FUND, VIEW, "x9")) {
      expect(c.keywords).toContain(",");
    }
  });
});
