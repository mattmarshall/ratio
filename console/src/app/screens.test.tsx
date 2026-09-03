import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import accountsFixture from "../../fixtures/accounts.json";
import householdAccountsFixture from "../../fixtures/householdAccounts.json";
import breakFixture from "../../fixtures/break.json";
import breaksFixture from "../../fixtures/breaks.json";
import changeLogFixture from "../../fixtures/changeLogEntries.json";
import explainFixture from "../../fixtures/explain.json";
import bookFixture from "../../fixtures/book.json";
import booksFixture from "../../fixtures/books.json";
import fundFixture from "../../fixtures/fund.json";
import fundsFixture from "../../fixtures/funds.json";
import lotsFixture from "../../fixtures/lots.json";
import navStrikesFixture from "../../fixtures/navStrikes.json";
import reconcileFixture from "../../fixtures/reconcile.json";
import positionsFixture from "../../fixtures/positions.json";
import replayFixture from "../../fixtures/replay.json";
import rulesFixture from "../../fixtures/rules.json";
import templatesFixture from "../../fixtures/templates.json";
import viewFixture from "../../fixtures/view.json";
import viewsFixture from "../../fixtures/views.json";
// ⚠ The fixtures are captured JSON, so TypeScript widens their enums to
// `string`. `//console:fixtures_test` checks their SHAPE against console.proto
// on every build, which is the check a cast here would otherwise be pretending
// to be.
import type { Rule } from "@/wire/types";

// ⛔ THE SUCCESSOR TO `//web:rendered_test`, AND ITS REASON IS UNCHANGED.
//
// That test greped the built HTML for class and field names because "a field
// can be declared, transcoded, served, typechecked and mirrored while NO
// COMPONENT READS IT. That has already happened once." Its own header called
// the grep "deliberately crude — it cannot tell a well-rendered field from a
// badly-rendered one."
//
// This is the half a grep could not do: the screens are RENDERED, against
// fixtures whose shape `//console:fixtures_test` checks against console.proto,
// and every field that mattered enough to be named there is asserted here.
// `//console:fields_test` keeps the crude half, because the two catch different
// things — a source grep sees a component nothing renders, and a render sees a
// component that renders nothing.
//
// ⛔ NEGATIVE-TEST EVERY CASE BELOW. Take the field out of the fixture and watch
// it go red before believing it. CONTRIBUTING.md records three suites here that
// were green, covered the code, and tested nothing.

vi.mock("@/lib/caller", () => ({
  caller: async () => ({ idToken: null }),
  principal: async () => ({ sub: "u-1", email: "e.marsh@example.com" }),
}));

vi.mock("next/headers", () => ({
  cookies: async () => ({ get: () => undefined, set: () => {} }),
  headers: async () => new Headers(),
}));

// The router hooks a client component reaches for. There is no app router
// mounted under jsdom, and the screens under test navigate rather than toggle —
// which is the point of the migration, so it is worth the four lines.
vi.mock("next/navigation", async () => {
  const actual =
    await vi.importActual<typeof import("next/navigation")>("next/navigation");
  return {
    ...actual,
    useRouter: () => ({ replace: () => {}, push: () => {}, refresh: () => {} }),
    usePathname: () =>
      "/books/harbourline-global-value/views/abor/breaks",
    useSelectedLayoutSegment: () => "breaks",
    // ⚠ FOUR SEGMENTS NOW, NOT TWO. `FundRail` reads [0] for the fund and
    // both `ScreenTabs` and `ViewSwitch` read [1] and [2] for the view and the
    // screen. A mock that stayed two long would have let every one of them
    // render the wrong link while these tests stayed green.
    useSelectedLayoutSegments: () => [
      "harbourline-global-value",
      "views",
      "abor",
      "breaks",
    ],
  };
});

const wire = {
  listBooks: async () => booksFixture,
  listFunds: async () => fundsFixture,
  // ⭐ KIND IS A PROPERTY OF THE BOOK, NOT OF THE SUITE. A mock that always
  // returned the household fixture made every fund view wear personal chrome
  // the moment GetBook started being read beside GetView.
  getBook: async (_c: unknown, book: string) => {
    const id = String(book).replace(/^books\//, "");
    const found = booksFixture.books.find((b) => b.name === `books/${id}`);
    return found ?? bookFixture;
  },
  createBook: async () => bookFixture,
  getFund: async () => fundFixture,
  getView: async () => viewFixture,
  listViews: async () => viewsFixture,
  reconcileViews: async () => reconcileFixture,
  getBreak: async () => breakFixture,
  listBreaks: async () => breaksFixture,
  listAccounts: async () => accountsFixture,
  listPositions: async () => positionsFixture,
  listLots: async () => lotsFixture,
  listNavStrikes: async () => navStrikesFixture,
  getNavStrike: async () => navStrikesFixture.navStrikes[0],
  replayNavStrike: async () => replayFixture,
  explainNavStrike: async () => explainFixture,
  listRules: async () => rulesFixture,
  listTemplates: async () => templatesFixture,
  listChangeLogEntries: async () => changeLogFixture,
};

vi.mock("@/wire/client", async () => {
  const actual = await vi.importActual<typeof import("@/wire/client")>(
    "@/wire/client",
  );
  // ⚠ LATE-BOUND, NOT SPREAD. `{ ...wire }` copies the function references the
  // moment the first test imports the wire, so a test that swaps one out —
  // the refusal tests below — would mutate an object nobody reads. Each key
  // delegates on CALL, so the current `wire.foo` is always the one invoked.
  const late = Object.fromEntries(
    Object.keys(wire).map((k) => [
      k,
      (...args: unknown[]) =>
        (wire as unknown as Record<string, (...a: unknown[]) => unknown>)[k]!(
          ...args,
        ),
    ]),
  );
  return { ...actual, ...late };
});

const FUND = "harbourline-global-value";
const VIEW = "abor";
const params = <T,>(v: T) => Promise.resolve(v);

/** Render an async server component by awaiting the element it returns.
 *  `ReactNode`, because a page wrapped in `withRefusal` is typed to return
 *  whichever of the page and the refusal it resolves to. */
async function renderAsync(el: Promise<React.ReactNode>) {
  render((await el) as React.ReactElement);
}

describe("a first-class book", () => {
  it("lists an independent book without a fund parent", async () => {
    const Books = (await import("./books/page")).default;
    await renderAsync(Books());
    expect(screen.getByText("Household")).toBeDefined();
    expect(screen.getAllByText(/independent/).length).toBeGreaterThan(0);
    expect(screen.getByRole("link", { name: "New book" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Funds" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Projects" })).toBeDefined();
    expect(screen.getByLabelText("Home workspace")).toBeDefined();
  });

  it("lists project books on /projects and opens them as books", async () => {
    const Projects = (await import("./projects/page")).default;
    await renderAsync(Projects());
    expect(screen.getByText("Bridge")).toBeDefined();
    expect(
      screen.getByRole("link", { name: /Bridge/ }).getAttribute("href"),
    ).toBe("/books/bridge");
    expect(screen.queryByText("Household")).toBeNull();
    expect(screen.queryByText("Harbourline Global Value")).toBeNull();
  });

  it("offers the three book templates CreateBook already knows", async () => {
    const NewBook = (await import("./books/new/page")).default;
    render(<NewBook />);
    expect(screen.getByText("Personal finance")).toBeDefined();
    expect(screen.getByText(/Cash and bank/)).toBeDefined();
    expect(screen.getByText("Investment / Fund")).toBeDefined();
    expect(screen.getByText(/Does not file a fund/)).toBeDefined();
    expect(screen.getByText("Project")).toBeDefined();
    expect(screen.getByText(/work in progress/)).toBeDefined();
    expect(
      (screen.getByRole("radio", { name: /Personal finance/ }) as HTMLInputElement)
        .checked,
    ).toBe(true);
  });

  it("reaches the book collection from the fund list", async () => {
    const Funds = (await import("./funds/page")).default;
    await renderAsync(Funds());
    expect(screen.getByRole("link", { name: "All books" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Projects" })).toBeDefined();
    // A seeded fund is an investment book. The list is funds-only; the job
    // lives on the book URL so a personal book never has to look like one.
    expect(
      screen.getByRole("link", { name: /Harbourline Global Value/ }).getAttribute("href"),
    ).toBe("/books/harbourline-global-value/views/abor/breaks");
  });

  it("opens a book as its own page", async () => {
    const Book = (await import("./books/[book]/page")).default;
    await renderAsync(Book({ params: params({ book: "household" }) }));
    expect(screen.getByRole("heading", { name: "Household" })).toBeDefined();
    expect(screen.getByText("Personal")).toBeDefined();
    expect(screen.getByText("independent")).toBeDefined();
    expect(screen.getByRole("link", { name: "Balance sheet" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Period P&L" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Trial balance" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Configuration" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Transfer between accounts" })).toBeDefined();
    expect(screen.getByText(/Net worth/)).toBeDefined();
    // ⛔ THE LABEL IS NOT THE PRODUCT. A personal hub that still offered
    // Exceptions / Positions / NAV would be fund-ops screens with a household
    // name on them — issue #65.
    expect(screen.queryByRole("link", { name: "Exceptions" })).toBeNull();
    expect(screen.queryByRole("link", { name: "Positions" })).toBeNull();
    expect(screen.queryByRole("link", { name: "NAV" })).toBeNull();
  });

  it("sends every job to /books/{book}/… and never through /funds", async () => {
    const Book = (await import("./books/[book]/page")).default;
    await renderAsync(Book({ params: params({ book: "household" }) }));
    expect(screen.getByRole("link", { name: "Balance sheet" }).getAttribute("href")).toBe(
      "/books/household/views/book/sheet",
    );
    expect(screen.getByRole("link", { name: "Period P&L" }).getAttribute("href")).toBe(
      "/books/household/views/book/pnl",
    );
    const trial = screen.getByRole("link", { name: "Trial balance" });
    expect(trial.getAttribute("href")).toBe(
      "/books/household/views/book/accounts",
    );
    const config = screen.getByRole("link", { name: "Configuration" });
    expect(config.getAttribute("href")).toBe("/books/household/config");
    expect(
      screen.getByRole("link", { name: "Transfer between accounts" }).getAttribute("href"),
    ).toBe("/books/household/transfer");
    for (const a of document.querySelectorAll("a[href]")) {
      expect(a.getAttribute("href")).not.toMatch(/\/funds\//);
    }
  });

  it("gives a book of record a page of its own", async () => {
    // ⭐ #53. ListViews returns this name; the URL used to 404 and the
    // palette compensated by landing on breaks. The page is the citation —
    // figures about the view, not a child screen — and a render without the
    // layout still has to show them.
    const View = (await import("./books/[book]/views/[view]/page")).default;
    await renderAsync(View({ params: params({ book: FUND, view: VIEW }) }));
    expect(screen.getByText("ABOR")).toBeDefined();
    expect(screen.getByText("134,439,187.51")).toBeDefined();
    expect(screen.getByText("2,000.00")).toBeDefined();
    expect(screen.getByText("Unplaceable")).toBeDefined();
    expect(screen.getAllByText("trade date").length).toBeGreaterThan(0);
    expect(screen.getByText("26 Feb 2026")).toBeDefined();
    expect(screen.getByRole("link", { name: "Exceptions" }).getAttribute("href")).toBe(
      `/books/${FUND}/views/${VIEW}/breaks`,
    );
    expect(screen.getByRole("link", { name: "Book" }).getAttribute("href")).toBe(
      `/books/${FUND}`,
    );
    for (const a of document.querySelectorAll("a[href]")) {
      expect(a.getAttribute("href")).not.toMatch(/\/funds\//);
    }
  });

  it("names a settlement view's convention and the day it has recognised through", async () => {
    const real = wire.getView;
    wire.getView = (async () => viewsFixture.views[1]) as typeof wire.getView;
    try {
      const View = (await import("./books/[book]/views/[view]/page")).default;
      await renderAsync(View({ params: params({ book: FUND, view: "ibor" }) }));
      expect(screen.getByText("IBOR")).toBeDefined();
      expect(screen.getAllByText("settled T+2").length).toBeGreaterThan(0);
      expect(screen.getByText("us-settlement")).toBeDefined();
      expect(screen.getByText("24 Feb 2026")).toBeDefined();
    } finally {
      wire.getView = real;
    }
  });

  it("says a recorded-basis view has no cut rather than inventing a day", async () => {
    // ⛔ ABSENT IS THE HONEST ANSWER. A recorded-basis view does not recognise
    // by date; printing the epoch (or yesterday) would be a cut nobody elected.
    const real = wire.getView;
    wire.getView = (async () => ({
      ...viewFixture,
      displayName: "book",
      basis: "RECORDED",
      settlementOpenDays: "0",
      calendar: "",
      declared: false,
      recognisedThrough: null,
    })) as unknown as typeof wire.getView;
    try {
      const View = (await import("./books/[book]/views/[view]/page")).default;
      await renderAsync(View({ params: params({ book: "household", view: "book" }) }));
      expect(screen.getAllByText("journal order").length).toBeGreaterThan(0);
      expect(screen.getByText("Recognised through").nextElementSibling?.textContent).toBe(
        "—",
      );
      expect(screen.getByText(/the journal's own order/)).toBeDefined();
    } finally {
      wire.getView = real;
    }
  });

  it("opens a personal book of record as net worth, not NAV", async () => {
    // ⚠ A DISTINCT VIEW ID FROM THE RECORDED-BASIS CASE ABOVE. `viewOf` is
    // React-cached per (book, view) for the process, so reusing `book`
    // would serve that test's override and this would never see the
    // default fixture.
    const View = (await import("./books/[book]/views/[view]/page")).default;
    await renderAsync(View({ params: params({ book: "household", view: "hearth" }) }));
    expect(screen.getByText("Net worth")).toBeDefined();
    expect(screen.queryByText("Net asset value")).toBeNull();
    expect(screen.queryByText("Open difference")).toBeNull();
    expect(screen.queryByText("Open breaks")).toBeNull();
    expect(screen.getByRole("link", { name: "Balance sheet" }).getAttribute("href")).toBe(
      "/books/household/views/hearth/sheet",
    );
    expect(screen.getByRole("link", { name: "Period P&L" }).getAttribute("href")).toBe(
      "/books/household/views/hearth/pnl",
    );
    expect(screen.queryByRole("link", { name: "Exceptions" })).toBeNull();
    expect(screen.queryByRole("link", { name: "NAV" })).toBeNull();
  });
});

describe("a household statement", () => {
  async function withHouseholdAccounts<T>(fn: () => Promise<T>): Promise<T> {
    const real = wire.listAccounts;
    wire.listAccounts = (async () =>
      householdAccountsFixture) as typeof wire.listAccounts;
    try {
      return await fn();
    } finally {
      wire.listAccounts = real;
    }
  }

  it("renders chart_for(Personal) on a citable balance sheet", async () => {
    await withHouseholdAccounts(async () => {
      const Sheet = (await import("./books/[book]/views/[view]/sheet/page")).default;
      await renderAsync(
        Sheet({
          params: params({ book: "household", view: "book" }),
          searchParams: params({}),
        }),
      );
      expect(screen.getByLabelText("Balance sheet")).toBeDefined();
      for (const name of [
        "Cash and bank",
        "Investments",
        "Credit cards and loans",
        "Opening equity",
      ]) {
        expect(screen.getByText(name)).toBeDefined();
      }
      expect(screen.queryByText("Cash and equivalents")).toBeNull();
      expect(screen.queryByText("Investments at fair value")).toBeNull();
      expect(screen.getByRole("link", { name: "Period P&L" })).toBeDefined();
      expect(screen.getByRole("link", { name: "Transfer" })).toBeDefined();
    });
  });

  it("renders a period P&L and says it is not since inception", async () => {
    await withHouseholdAccounts(async () => {
      const PnL = (await import("./books/[book]/views/[view]/pnl/page")).default;
      await renderAsync(
        PnL({
          params: params({ book: "household", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(screen.getByLabelText("Period profit and loss")).toBeDefined();
      expect(screen.getByText("Living expenses")).toBeDefined();
      expect(screen.getByText("Taxes")).toBeDefined();
      expect(screen.getAllByText("Income").length).toBeGreaterThan(0);
      expect(screen.getByText(/not since inception/)).toBeDefined();
      expect(screen.queryByText("Cash and bank")).toBeNull();
      expect(screen.getByRole("link", { name: "Balance sheet" })).toBeDefined();
    });
  });

  it("asks ListAccounts for a period window on the P&L", async () => {
    const calls: unknown[][] = [];
    const real = wire.listAccounts;
    wire.listAccounts = (async (...args: unknown[]) => {
      calls.push(args);
      return householdAccountsFixture;
    }) as typeof wire.listAccounts;
    try {
      const PnL = (await import("./books/[book]/views/[view]/pnl/page")).default;
      await renderAsync(
        PnL({
          params: params({ book: "household", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(calls[0]?.slice(1)).toEqual(["household", "book", "pnl", "2026-03"]);
    } finally {
      wire.listAccounts = real;
    }
  });
});

describe("the trial balance", () => {
  it("shows the untranslated per-currency split beneath a translated total", async () => {
    const Accounts = (await import("./books/[book]/views/[view]/accounts/page")).default;
    await renderAsync(
      Accounts({ params: params({ book: FUND, view: VIEW }), searchParams: params({}) }),
    );
    // ⛔ The figure above is a conversion; a reader checking it needs the facts
    // it was converted from. Two currencies are two independent conservation
    // laws — a flat total hides a currency mismatch, and this repository has
    // shipped a NAV that did exactly that.
    expect(screen.getByText("USD")).toBeDefined();
    expect(screen.getByText("EUR")).toBeDefined();
    expect(document.querySelectorAll(".ccyrow").length).toBe(2);
  });
});

describe("positions", () => {
  it("shows the open-lot count on the list", async () => {
    const Positions = (await import("./books/[book]/views/[view]/positions/page")).default;
    await renderAsync(Positions({ params: params({ book: FUND, view: VIEW }) }));
    expect(screen.getByText(/40 open lots/)).toBeDefined();
  });

  it("renders the lot book, and says when a lot has no trade date", async () => {
    const { LotBook } = await import("@/components/LotBook");
    await renderAsync(LotBook({ fund: FUND, view: VIEW, position: "ACME" }));
    expect(document.querySelectorAll(".lotrow").length).toBe(2);
    // A lot the engine cannot classify says so rather than showing a guess.
    expect(screen.getByText("no trade date")).toBeDefined();
  });
});

describe("views", () => {
  it("names the basis of every view, and marks one nobody declared", async () => {
    const { ViewSwitch } = await import("@/components/ViewSwitch");
    render(
      <ViewSwitch
        fund={FUND}
        views={viewsFixture.views as never}
        defaultView="abor"
      />,
    );
    expect(screen.getByText("ABOR")).toBeDefined();
    expect(screen.getByText("IBOR")).toBeDefined();
    // ⛔ THE DISTINCTION A DEFAULT DESTROYS. Both fixture views are declared,
    // so nothing is marked — and an undeclared one must be. Flip `declared` to
    // false in views.json and this goes red; that is the negative test.
    expect(screen.queryByText("default")).toBeNull();
  });

  it("shows two NAVs, their difference, and the entries that account for it", async () => {
    const Reconcile = (
      await import("./books/[book]/views/[view]/reconcile/page")
    ).default;
    await renderAsync(
      Reconcile({
        params: params({ book: FUND, view: VIEW }),
        searchParams: params({ against: "ibor" }),
      }),
    );

    // ⭐ THE ARITHMETIC IS RENDERED, NOT ASSERTED. A screen that shows a
    // difference it cannot itemize is one to distrust, and
    // `Ratio.Views.two_views_differ_by_exactly_what_is_in_flight` says the two
    // lists account for it exactly.
    expect(screen.getByText("134,439,187.51")).toBeDefined();
    expect(screen.getByText("134,102,187.51")).toBeDefined();

    // ⭐ TWICE ON THE SCREEN, AND THAT IS THE ASSERTION RATHER THAN AN
    // ACCIDENT. The headline difference and the subtotal of the entries listed
    // under it are the same figure — a trade-date view recognises no later than
    // a T+2 one, so nothing is in flight the other way and the one list
    // accounts for the whole gap. If the screen ever showed a difference its
    // rows did not add to, this drops to one.
    expect(screen.getAllByText("337,000.00").length).toBe(2);

    // And the lists really do add to it: 300,000.00 + 40,000.00 − 3,000.00.
    //
    // ⚠ BOTH LISTS, THOUGH ONE IS EMPTY HERE. A trade-date view recognises no
    // later than a T+2 one, so nothing is in flight the other way round for
    // THIS pair — but a recorded view against a trade-date one puts entries in
    // both, and a sum that read only one list would be right by accident.
    const sum = (rows: { netAssetValueEffect: string }[]) =>
      rows.reduce((n, r) => n + BigInt(r.netAssetValueEffect), 0n);
    const here = sum(reconcileFixture.recognisedHere);
    const there = sum(reconcileFixture.recognisedThere);
    expect((here + there).toString()).toBe(reconcileFixture.difference);

    // ⛔ AND THE ENTRIES NEITHER VIEW CAN PLACE ARE ON THE SCREEN. Hiding them
    // would make the difference look fully explained when it is not.
    expect(screen.getByText("Neither view can place these")).toBeDefined();
    expect(screen.getByText("Opening balance, migrated book")).toBeDefined();
  });
});

describe("the fund overview", () => {
  it("reads the realized gain, and flips it credit-normal exactly once", async () => {
    const Overview = (await import("./funds/[fund]/page")).default;
    await renderAsync(Overview({ params: params({ fund: FUND }) }));
    // ⛔ THE ROW NAMES ITS VIEW NOW, AND THAT IS THE ASSERTION. A realized
    // gain depends on which open lots had been recognised when the sale
    // arrived, so the same election gives up different lots in different
    // views. A row that printed the figure without saying which view is the
    // row already in HANDOFF.md's failure table.
    expect(screen.getByText(/Realized gain, in abor/)).toBeDefined();
    // ⛔ The raw figure is "-1500000" — the gain leg is `relieved − proceeds`,
    // so a PROFITABLE disposal carries a minus sign. A screen that printed it
    // unflipped would show every profitable fund as a loss.
    expect(screen.getByText("15,000.00")).toBeDefined();
    for (const t of ["Short-term", "Long-term", "Unclassified", "Basis relieved"]) {
      expect(screen.getByText(t)).toBeDefined();
    }
  });

  it("says whether the lot method was elected or merely defaulted", async () => {
    const Overview = (await import("./funds/[fund]/page")).default;
    await renderAsync(Overview({ params: params({ fund: FUND }) }));
    expect(screen.getByText("Lot method")).toBeDefined();
    // ⛔ BOTH CLAIMS THE ROW CAN MAKE. The fixture declares a method, so it must
    // say so; a book that declares none is relieved oldest-first by CUSTOM, and
    // printing "a term of the administration agreement" over that is asserting
    // something nobody agreed to.
    expect(
      screen.getByText(/a term of the administration agreement/),
    ).toBeDefined();
  });

  it("names the tax lots and the fold that does not read them", async () => {
    const Overview = (await import("./funds/[fund]/page")).default;
    await renderAsync(Overview({ params: params({ fund: FUND }) }));
    expect(screen.getByText("Tax lots")).toBeDefined();
  });
});

describe("a NAV strike", () => {
  it("puts the qualification before the figure, not behind a click", async () => {
    const Strikes = (await import("./books/[book]/views/[view]/strikes/page")).default;
    await renderAsync(Strikes({ params: params({ book: FUND, view: VIEW }) }));
    expect(
      screen.getByText(/a corporate action announced after this prefix/),
    ).toBeDefined();
  });

  it("reports history intact and reproduced as two separate claims", async () => {
    const Replay = (await import("./books/[book]/views/[view]/strikes/[strike]/replay/page"))
      .default;
    await renderAsync(
      Replay({ params: params({ book: FUND, view: VIEW, strike: "2026-02-26" }) }),
    );
    // ⛔ TWO CLAIMS, NOT ONE. Intact says the prefix still hashes to what the
    // strike recorded; reproduced says folding it again gives the same figure.
    // A book can report an intact history and a different number — that is what
    // a changed configuration does — and reading them as one claim is how a
    // restatement goes unnoticed.
    expect(screen.getByText("History intact")).toBeDefined();
    expect(screen.getByText("Reproduced")).toBeDefined();
  });
});

describe("how a NAV was computed", () => {
  const Plan = async () =>
    (await import("./books/[book]/views/[view]/strikes/[strike]/plan/page")).default;
  const show = async (qs: { analyze?: string; rejected?: string }) => {
    const P = await Plan();
    await renderAsync(
      P({
        params: params({ book: FUND, view: VIEW, strike: "2026-02-26" }),
        searchParams: params(qs),
      }),
    );
  };

  it("draws both curves, because quoting only the flat one is the overclaim", async () => {
    // ⛔ `ratio bench` "reports two curves and both must be quoted". The fold
    // grows with the journal and the maintained read does not, and a plan screen
    // carrying only the second would be that overclaim drawn as a diagram.
    await show({});
    expect(screen.getByText("The strike as recorded")).toBeDefined();
    expect(screen.getByText("The same figure off the maintained totals")).toBeDefined();
  });

  it("keeps the three costs on the page when the plans not taken are hidden", async () => {
    // ⛔ COLLAPSING BUYS A SMALLER PICTURE, NOT A FRIENDLIER NUMBER. This is the
    // whole reason hiding rejected steps by default is safe.
    await show({});
    expect(screen.queryByText("Scan Open Lots")).toBeNull();
    // ⚠ ASSERTED AGAINST THE STRIP ITSELF, not against the page. Every one of
    // these figures also appears on a box or an edge, so `getByText` would pass
    // on a page whose comparison had been deleted entirely.
    const strip = screen.getByText("Folding every open tax lot").closest("dl")!;
    const row = (label: string) =>
      [...strip.children].find((e) => e.textContent === label)?.nextElementSibling
        ?.textContent;
    // 252,843 lots and 21,085 rewrite-reads against 15 — `Ratio.Closure.the_cliff`.
    expect(row("This plan")).toBe("15");
    expect(row("Applying the open actions by rewriting the lots")).toBe("21,085");
    expect(row("Folding every open tax lot")).toBe("252,843");
  });

  it("shows the plans not taken when asked for them", async () => {
    await show({ rejected: "true" });
    expect(screen.getAllByText("Scan Open Lots").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Apply Open Actions by Rewrite").length).toBeGreaterThan(0);
  });

  it("keeps the unread lots on the page even with the plans not taken hidden", async () => {
    // ⛔ `UNREAD` IS NOT `REJECTED`. Twenty million tax lots costing nothing is
    // `Ratio.Closure.factored_nav_never_reads_the_lots` — the only claim on this
    // screen that is a theorem — and collapsing it with the plans not taken
    // would delete it from the default view.
    await show({});
    expect(screen.getAllByText("Open Tax Lots").length).toBeGreaterThan(0);
    expect(screen.getByText(/never read/)).toBeDefined();
  });

  it("says a step was not measured rather than showing it as zero", async () => {
    // ⛔ THE NEGATIVE CASE THAT MATTERS. A rendered `0` that nothing measured
    // reads as "instant"; `fx_rate` was two orders of magnitude wrong for months
    // because its only consumer discarded it, and this is that facing the other
    // way. The captured fixture is deliberately the UNANALYZED plan.
    await show({});
    expect(screen.getAllByText("not measured").length).toBeGreaterThan(0);
    expect(screen.getByText(/Nothing here has been measured/)).toBeDefined();
  });

  it("reports a proved zero as a zero, beside the steps that carry no figure", async () => {
    // The other half of the same distinction: the unread lots cost `0` reads and
    // that zero is a theorem, while a step the model does not cost is `—`.
    await show({});
    // ⚠ Each operator name appears twice — once in the drawn box and once in
    // the written-out plan beside it — so the row is picked out rather than
    // assumed unique.
    const step = (operator: string) =>
      screen
        .getAllByText(operator)
        .map((e) => e.closest("li"))
        .find(Boolean)!.textContent!;
    // The unread lots: a zero that `factored_nav_never_reads_the_lots` proves.
    expect(step("Open Tax Lots")).toContain("estimated 0 reads");
    // A step the model does not cost: blank, and never the same character.
    expect(step("Capital Activity")).toContain("estimated — reads");
    expect(step("Capital Activity")).not.toContain("estimated 0 reads");
  });

  it("carries the rate's provenance beside the durations it produced", async () => {
    // ⛔ Reads are proved; seconds are a property of a machine on a day. The
    // shipped rate was 250 until somebody measured it and found 4,436.
    await show({});
    expect(screen.getByText(/a FLOOR, not a typical rate/)).toBeDefined();
  });

  it("names the securities and the lots per security separately", async () => {
    // ⛔ 500 × 40,000 and 10,000 × 2,000 are both twenty million open lots and
    // are not the same fund. One price is read per SECURITY, so a single lot
    // count hides a twentyfold difference in the term that grows with the chart.
    await show({});
    expect(screen.getByText("Securities")).toBeDefined();
    expect(screen.getByText("Open lots per security")).toBeDefined();
  });

  it("cites something for every step it draws", async () => {
    await show({ rejected: "true" });
    expect(screen.getAllByText(/Ratio\.Closure\.factored_nav_never_reads_the_lots/).length)
      .toBeGreaterThan(0);
    expect(screen.getAllByText(/Ratio\.Plan\.aggregate_cost_is_the_securities/).length)
      .toBeGreaterThan(0);
  });
});

describe("a break", () => {
  it("shows the two figures and what produced ours", async () => {
    const Detail = (await import("./books/[book]/views/[view]/breaks/[break]/page")).default;
    await renderAsync(
      Detail({ params: params({ book: FUND, view: VIEW, break: "cash-usd-2026-02-26" }) }),
    );
    expect(screen.getByText("Ratio")).toBeDefined();
    expect(screen.getByText("Reported")).toBeDefined();
    // 2,000.00 is deploy/seed-demo-book.sh's number.
    expect(screen.getByText("2,000.00")).toBeDefined();
  });

  it("names the bounds the severity was graded against", async () => {
    // A grade whose terms a reader has to go and look up is a grade a reader
    // takes on trust — which is the one thing this product is not for.
    const Detail = (await import("./books/[book]/views/[view]/breaks/[break]/page"))
      .default;
    await renderAsync(
      Detail({
        params: params({ book: FUND, view: VIEW, break: "cash-usd-2026-02-26" }),
      }),
    );
    expect(screen.getByText(/graded at/)).toBeDefined();
    expect(screen.getByText(/1,000\.00 blocks/)).toBeDefined();
    expect(screen.getByText(/declared/)).toBeDefined();
  });

  it("shows an accepted explanation with the name on it", async () => {
    const Detail = (await import("./books/[book]/views/[view]/breaks/[break]/page"))
      .default;
    await renderAsync(
      Detail({
        params: params({ book: FUND, view: VIEW, break: "cash-usd-2026-02-26" }),
      }),
    );
    expect(screen.getByText("Why this is acceptable")).toBeDefined();
    expect(screen.getByText(/unsettled dividend/)).toBeDefined();
    expect(screen.getByText(/accepted by/)).toBeDefined();
    // ⛔ AND NO WAY TO ACCEPT ONE. The fence is that the screen displays and
    // does not decide — the same assertion the rules screen makes about
    // approval.
    expect(screen.queryByRole("button", { name: /accept/i })).toBeNull();
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("reads a stale explanation as neither explained nor open", async () => {
    // ⛔ THREE READINGS, NOT TWO. A break somebody explained and a break whose
    // explanation a later figure overtook are both "not open", and showing
    // them alike is how the second gets closed without anybody looking at
    // what moved.
    const Queue = (await import("./books/[book]/views/[view]/breaks/page")).default;
    await renderAsync(
      Queue({
        params: params({ book: FUND, view: VIEW }),
        searchParams: params({}),
      }),
    );
    expect(screen.getByText("stale")).toBeDefined();
  });

});

describe("rules", () => {
  it("keeps the active rules and the unapproved drafts as two lists", async () => {
    const Rules = (await import("./books/[book]/rules/page")).default;
    await renderAsync(Rules({ params: params({ book: FUND }) }));
    // ⛔ The gap between the two lists is exactly what a person's approval
    // bought, and merging them erases it.
    expect(screen.getByText("Active")).toBeDefined();
    expect(screen.getByText("Awaiting a person")).toBeDefined();
    expect(screen.getByText("perf_fee")).toBeDefined();
    // ⛔ AND THERE IS NO WAY ROUND THE FENCE. `approve_rule` is absent from the
    // model's tools on purpose; a button here would make that worth nothing.
    expect(screen.queryByRole("button", { name: /approve/i })).toBeNull();
  });
});

describe("the write screens", () => {
  // ⚠ SYNTHESIZED, AND ONLY THIS. Everything else in this suite runs off a
  // CAPTURED fixture, because a fixture somebody typed is a claim about what the
  // server sends. `rules.json` is a real capture and the fund it came from
  // declares one DIVIDEND rule and no trade rule — a case worth rendering, and
  // the first test below. A component test needs the other case too, so the rule
  // is built here, where it is plainly a test input rather than a claim about a
  // response.
  const TRADE_RULE = {
    name: "funds/harbourline-global-value/rules/equity_purchase",
    ruleId: "equity_purchase",
    kind: "TRADE" as const,
    description: "Equity purchase, settled in cash",
    form: "debit investments, credit cash, at the consideration",
    accounts: ["Investments at fair value", "Cash and equivalents"],
  };

  const XFER_CASH_INV = {
    name: "funds/household/rules/xfer_cash_investments",
    ruleId: "xfer_cash_investments",
    kind: "TRADE" as const,
    description: "Move cash to investments",
    form: "debit Investments, credit Cash and bank",
    accounts: ["Investments", "Cash and bank"],
  };

  /** ⚠ `view: ""` and no holdings is the cold-landing case — see the page. */
  async function ticket(rules = [TRADE_RULE], positions = [], view = "") {
    const { TradeTicket } = await import("./books/[book]/trade/TradeTicket");
    render(
      <TradeTicket
        fund={FUND}
        rules={rules}
        positions={positions}
        view={view}
      />,
    );
  }

  /** Arrived from a view's positions screen, so the holdings came with it. */
  async function ticketInView() {
    await ticket([TRADE_RULE], positionsFixture.positions as never, "abor");
  }

  /** The same ticket with every field on screen at once. */
  async function asForm(rules = [TRADE_RULE]) {
    await ticket(rules);
    fireEvent.click(screen.getByRole("button", { name: "Form" }));
  }

  /** Fill a whole trade ticket. Form view, because guided shows one at a time. */
  function fill() {
    for (const [name, value] of [
      ["Instrument", "ACME"],
      ["Units", "1000"],
      ["Price", "341.75"],
      ["Trade date", "2026-02-26"],
      ["Books under", "equity_purchase"],
    ] as const) {
      fireEvent.change(screen.getByLabelText(name), { target: { value } });
    }
  }

  // ── the pattern ────────────────────────────────────────────────────────
  //
  // ⭐ ALL FOUR WRITES, NOT JUST THE NEW ONE. The four were four forms doing the
  // same thing four ways; a test that only held the newest one to the pattern
  // would let the other three drift straight back.

  it("offers both ways through, on every screen that writes", async () => {
    const { RecordForm } = await import("./books/[book]/record/RecordForm");
    const { MarkForm } = await import("./books/[book]/mark/MarkForm");
    const { IngestForm } = await import("./books/[book]/ingest/IngestForm");
    const { TradeTicket } = await import("./books/[book]/trade/TradeTicket");
    const { TransferForm } = await import("./books/[book]/transfer/TransferForm");

    for (const [what, el] of [
      ["record", <RecordForm key="r" fund={FUND} rules={rulesFixture.rules as Rule[]} />],
      ["mark", <MarkForm key="m" fund={FUND} />],
      [
        "ingest",
        <IngestForm key="i" fund={FUND} templates={templatesFixture.templates} />,
      ],
      [
        "trade",
        <TradeTicket
          key="t"
          fund={FUND}
          rules={[TRADE_RULE]}
          positions={[]}
          view=""
        />,
      ],
      [
        "transfer",
        <TransferForm key="x" fund="household" rules={[XFER_CASH_INV]} />,
      ],
    ] as const) {
      const { unmount } = render(el);
      // The tree, the two views, and a way forward — the same three on each.
      expect(document.querySelector(".steps"), what).not.toBeNull();
      // ⛔ THE SAME CLASS AS THE BOOK-OF-RECORD SWITCH, AND A RENAME BROKE IT
      // ONCE. This control was `.views`; when that became `.viewswitch` the
      // toggle kept the dead name and rendered as two words of plain text — the
      // screen still worked and stopped looking like a control. Both switches
      // look identical on purpose, so both name one class.
      const guided = screen.getByRole("button", { name: "Guided" });
      expect(guided.closest(".viewswitch"), what).not.toBeNull();
      expect(screen.getByRole("button", { name: "Form" }), what).toBeDefined();
      expect(screen.getByRole("button", { name: "Next" }), what).toBeDefined();
      unmount();
    }
  });

  it("shows one question at a time, and will not skip one unanswered", async () => {
    await ticket();
    // Step one is the side, and it answers itself — so Next is open.
    expect(
      screen.getByRole("heading", { name: /purchase or a disposal/i }),
    ).toBeDefined();
    expect(screen.queryByLabelText("Price")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByLabelText("Instrument")).toBeDefined();
    // ⛔ AND NO FURTHER. Nothing has been chosen, so the step has no answer and
    // the way on is shut — a stepper that lets somebody past a question it then
    // complains about at the end is a form with extra clicks.
    expect(
      screen.getByRole("button", { name: "Next" }).hasAttribute("disabled"),
    ).toBe(true);

    fireEvent.change(screen.getByLabelText("Instrument"), {
      target: { value: "ACME" },
    });
    expect(
      screen.getByRole("button", { name: "Next" }).hasAttribute("disabled"),
    ).toBe(false);
  });

  it("keeps the tree honest about what has been answered", async () => {
    await ticket();
    const tree = () =>
      [...document.querySelectorAll(".steps .stepbtn")].map((b) => ({
        label: b.querySelector(".sk")?.textContent,
        value: b.querySelector(".sv")?.textContent,
        // ⚠ A tick against a digit, so the state reads on a printout too.
        done: b.classList.contains("done"),
        mark: b.querySelector(".sn")?.textContent,
      }));

    // Unanswered steps read as an em dash, not as a blank that looks answered.
    expect(tree().map((s) => s.value)).toEqual([
      "Buy", "—", "—", "—", "—", "—", "—",
    ]);
    // ⛔ AND REVIEW IS OUT OF REACH UNTIL THE TICKET IS WHOLE.
    expect(
      screen.getByRole("button", { name: /Review/ }).hasAttribute("disabled"),
    ).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Form" }));
    fill();
    fireEvent.click(screen.getByRole("button", { name: "Guided" }));

    expect(tree().map((s) => s.value)).toEqual([
      "Buy",
      "ACME",
      "1000",
      "341.75",
      "26 Feb 2026",
      "equity_purchase",
      "—",
    ]);
    // ⚠ Steps 1..5, not 0..5: the side has focus, and a step with focus is
    // marked current rather than done. Both are true of it and current is the
    // one worth showing.
    expect(tree().slice(1, 6).every((s) => s.done)).toBe(true);
    expect(tree().slice(1, 6).map((s) => s.mark)).toEqual(["✓", "✓", "✓", "✓", "✓"]);
    // The step with focus and the ones still to come keep their number.
    expect(tree()[0]?.mark).toBe("1");
    expect(tree()[6]?.mark).toBe("7");
    expect(
      screen.getByRole("button", { name: /Review/ }).hasAttribute("disabled"),
    ).toBe(false);
  });

  it("keeps every answer when the view is switched mid-ticket", async () => {
    // ⛔ TWO RENDERINGS OF ONE STATE, NEVER TWO FORMS. Two components each
    // holding half the answers is how a compact mode comes to silently drop the
    // field the other one had.
    await asForm();
    fill();
    fireEvent.click(screen.getByRole("button", { name: "Guided" }));
    fireEvent.click(screen.getByRole("button", { name: "Form" }));
    expect(screen.getByLabelText<HTMLInputElement>("Price").value).toBe("341.75");
    expect(screen.getByLabelText<HTMLSelectElement>("Books under").value).toBe(
      "equity_purchase",
    );
  });

  // ── the trade ticket ───────────────────────────────────────────────────

  it("offers what the fund holds once a view says which book to read", async () => {
    await ticketInView();
    fireEvent.click(screen.getByRole("button", { name: "Form" }));
    const picker = screen.getByLabelText<HTMLSelectElement>("Instrument");
    expect([...picker.options].map((o) => o.textContent)).toContain(
      "Acme Corporation · 1,000 held",
    );
    fireEvent.change(picker, { target: { value: "ACME" } });
    // ⛔ AND THE VIEW IS NAMED BESIDE THE FIGURES. Quantity and carrying value
    // depend on which entries a view recognises; printing either without saying
    // which book is the defect the view split exists to prevent.
    expect(screen.getByText(/1,000 units across 40 open lots/)).toBeDefined();
    expect(screen.getByText("abor")).toBeDefined();
  });

  it("does not offer the unattributed row as something to trade", async () => {
    // ⛔ AN EMPTY `instrument` IS A REAL ROW, NOT A MISSING ONE — value in the
    // account attributed to no instrument, which the rows sum to and the trial
    // balance agrees with. It is a holding of nothing, so it cannot be sold, and
    // an empty option in the picker is a ticket nobody could read.
    const unattributed = {
      ...positionsFixture.positions[0],
      name: `funds/${FUND}/views/abor/positions/-`,
      instrument: "",
      instrumentLabel: "",
    };
    await ticket(
      [TRADE_RULE],
      [...positionsFixture.positions, unattributed] as never,
      "abor",
    );
    fireEvent.click(screen.getByRole("button", { name: "Form" }));
    const picker = screen.getByLabelText<HTMLSelectElement>("Instrument");
    expect([...picker.options].map((o) => o.value)).toEqual([
      "",
      "ACME",
      "not-held",
    ]);
  });

  it("shows no holding figures at all when no view was named", async () => {
    // ⭐ THE HONEST DEGRADATION. Landing on `/trade` with no `?view=` — a
    // bookmark, a typed URL — means nothing said which book to read units and
    // values from, so the instrument is a plain field and no figure is offered.
    await ticket();
    fireEvent.click(screen.getByRole("button", { name: "Form" }));
    expect(screen.getByLabelText("Instrument").tagName).toBe("INPUT");
    expect(screen.queryByText(/units across/)).toBeNull();
  });

  it("still lets a trade be placed in something the fund does not hold", async () => {
    await ticketInView();
    fireEvent.click(screen.getByRole("button", { name: "Form" }));
    fireEvent.change(screen.getByLabelText("Instrument"), {
      target: { value: "not-held" },
    });
    // The picker is an affordance; the text field is the contract.
    fireEvent.change(screen.getByLabelText("Its identifier"), {
      target: { value: "NEWCO" },
    });
    const form = document.querySelector("form")!;
    expect(Object.fromEntries(new FormData(form).entries())).toMatchObject({
      instrument: "NEWCO",
    });
  });

  it("says so when the configuration in force declares no trade rule", async () => {
    // ⛔ AND OFFERS NOTHING INSTEAD. A rule is authored and approved at a
    // terminal; a screen that let one be chosen from somewhere else, or invented
    // a default, would be a way round the fence `ratio approve` is.
    const Trade = (await import("./books/[book]/trade/page")).default;
    await renderAsync(
      Trade({ params: params({ book: FUND }), searchParams: params({}) }),
    );
    expect(screen.getByText(/declares no trade rule/)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Form" }));
    // Both ways of writing are shut, not just the second: with no rule to book
    // under there is nothing to preview either. And the picker says why rather
    // than offering an empty list somebody has to interpret.
    for (const name of ["Preview", "Post"]) {
      expect(
        screen.getByRole("button", { name }).hasAttribute("disabled"),
      ).toBe(true);
    }
    expect(
      screen.getByLabelText("Books under").hasAttribute("disabled"),
    ).toBe(true);
    expect(screen.getByText("No trade rule in force")).toBeDefined();
  });

  it("derives the consideration from units and price, exactly", async () => {
    await asForm();
    fireEvent.change(screen.getByLabelText("Units"), { target: { value: "1000" } });
    fireEvent.change(screen.getByLabelText("Price"), { target: { value: "341.75" } });
    // ⭐ 1000 × 341.75. The operator typed neither this figure nor its minor
    // units, which is the point of the screen: a form that asks for the
    // consideration asks somebody to do this multiplication somewhere else and
    // retype the answer into the one field on this console that writes.
    expect(screen.getByText("341,750.00")).toBeDefined();
  });

  it("refuses a consideration that will not divide, rather than rounding it", async () => {
    await asForm();
    fireEvent.change(screen.getByLabelText("Units"), { target: { value: "1.5" } });
    fireEvent.change(screen.getByLabelText("Price"), { target: { value: "0.01" } });
    // ⛔ THE DATA PLANE'S OWN WORDS. `ratio_ingest::posting_for` refuses this
    // because which way to round is a term of an administration agreement. A
    // ticket that rounded what a file cannot would book one trade as two
    // different figures depending on the door it came through.
    expect(screen.getByText(/not a whole number of minor units/)).toBeDefined();
  });

  it("sends the instrument, the units and the day, so a lot opens", async () => {
    // ⭐ THE FIELDS THIS SCREEN EXISTED WITHOUT. `ApplyEventRequest` carried a
    // rule, an id and an amount, and `Projection::walk` skips any posting
    // lacking BOTH an instrument and a quantity — so every trade recorded here
    // opened no tax lot and relieved none, while the entry balanced, the trial
    // balance tied and the NAV moved by the right amount. Nothing objected.
    await asForm();
    fill();
    const form = document.querySelector("form")!;
    expect(Object.fromEntries(new FormData(form).entries())).toMatchObject({
      instrument: "ACME",
      units: "1000",
      tradeDate: "2026-02-26",
    });
    // And the screen says what that produces rather than what it cannot do.
    expect(screen.getByText("Carried:")).toBeDefined();
    expect(screen.getByText(/A tax lot opens against/)).toBeDefined();
  });

  it("refuses a fractional quantity rather than dropping it", async () => {
    // ⛔ THE DATA PLANE DROPS IT AND THIS MUST NOT. `admit_facts` carries a
    // non-whole quantity as no quantity, which is defensible for a file nobody
    // read and indefensible where a person typed the number — it is exactly how
    // the lot-less entry got made before.
    await asForm();
    fill();
    fireEvent.change(screen.getByLabelText("Units"), {
      target: { value: "10.5" },
    });
    expect(screen.getByText(/whole units/)).toBeDefined();
    expect(
      screen.getByRole("button", { name: "Preview" }).hasAttribute("disabled"),
    ).toBe(true);
  });

  it("says what a disposal does, which is not what a purchase does", async () => {
    await asForm();
    fireEvent.click(screen.getByRole("button", { name: "Sell" }));
    // ⛔ THE HALF THAT COSTS MORE. A purchase that opens no lot is a lot
    // missing; a disposal that relieves none reports a realized gain against no
    // basis — the figure with no counterparty. Both now happen properly, and
    // the screen distinguishes them because the operator's obligations differ.
    expect(screen.getByText(/Lots of .* are relieved/)).toBeDefined();
    expect(screen.queryByText(/A tax lot opens against/)).toBeNull();
  });

  it("sends every field the action reads", async () => {
    // ⛔ THE DEFECT THIS CAUGHT. Three inputs were controlled, on screen, bound
    // to state, rendering their values and feeding the derived consideration —
    // and carried no `name`, so `place()` received an empty units, price and
    // trade date and refused the ticket. Everything visible was right. Same
    // shape as `//console:route_manifest_test`'s "an RPC nobody reads", one
    // level down: a control nothing submits.
    await asForm();
    fill();

    const form = document.querySelector("form")!;
    expect(Object.fromEntries(new FormData(form).entries())).toMatchObject({
      fund: FUND,
      side: "buy",
      instrument: "ACME",
      units: "1000",
      price: "341.75",
      tradeDate: "2026-02-26",
      ruleId: "equity_purchase",
      // ⚠ Left blank by the operator, so the ticket's own suggestion travels —
      // which is the only reason the field is skippable.
      reference: "BUY-ACME-2026-02-26",
    });

    // ⛔ AND NOT ONE OF THEM IS A VISIBLE CONTROL. React resets a form after a
    // `<form action>` submit; a controlled `<select>` does not come back from
    // that, because its `value` prop has not changed and React writes nothing.
    // Both selects fell back to "Choose…" the moment a preview returned, while
    // the state, the ticket sentence and the rule's own form all still said
    // otherwise. Keeping the controls out of the form is the fix.
    expect(form.querySelector("select")).toBeNull();
    expect(form.querySelectorAll("input:not([type=hidden])").length).toBe(0);
  });

  it("will not post a ticket that has not been previewed", async () => {
    // ⭐ EVERY WRITE SCREEN HERE SAID "preview, then post" AND NOT ONE ENFORCED
    // IT. Post stays shut until a preview has come back for exactly these
    // inputs, so the figures on screen are the figures the button writes.
    await asForm();
    const post = screen.getByRole("button", { name: "Post" });
    expect(post.hasAttribute("disabled")).toBe(true);
    fill();
    expect(post.hasAttribute("disabled")).toBe(true);
  });

  // ── recording an event ─────────────────────────────────────────────────

  it("asks for an amount in the units the contract actually parses", async () => {
    // ⛔ THE DEFECT THIS FIXES. The field was labelled "minor units" and the
    // action validated `^-?\d+$`, so it REFUSED "250000.00" — the decimal the
    // contract documents — and read "25000000" as twenty-five million pounds.
    // `ratio_common::parse_minor("42")` is 4,200 minor units. The entry balances
    // at either size and the trial balance ties on it.
    const { RecordForm } = await import("./books/[book]/record/RecordForm");
    render(<RecordForm fund={FUND} rules={rulesFixture.rules as Rule[]} />);
    fireEvent.click(screen.getByRole("button", { name: "Form" }));
    fireEvent.change(screen.getByLabelText("Amount"), {
      target: { value: "250000.00" },
    });
    // Read back as the server will read it, before anything is sent.
    expect(screen.getByText("250,000.00")).toBeDefined();
    expect(screen.queryByText(/minor units/)).toBeNull();
  });

  it("refuses a third decimal place rather than dropping it", async () => {
    const { RecordForm } = await import("./books/[book]/record/RecordForm");
    render(<RecordForm fund={FUND} rules={rulesFixture.rules as Rule[]} />);
    fireEvent.click(screen.getByRole("button", { name: "Form" }));
    fireEvent.change(screen.getByLabelText("Amount"), {
      target: { value: "1.005" },
    });
    expect(screen.getByText(/more than two decimal places/)).toBeDefined();
  });

  it("posts a household transfer without an instrument or a quantity", async () => {
    // ⭐ CASH → INVESTMENTS IS A TRANSFER, NOT A SALE. The ticket that
    // asked for units would open a lot; this one must not even have the
    // fields.
    const { TransferForm } = await import("./books/[book]/transfer/TransferForm");
    render(<TransferForm fund="household" rules={[XFER_CASH_INV]} />);
    fireEvent.click(screen.getByRole("button", { name: "Form" }));
    expect(screen.getByText(/this is not a trade/)).toBeDefined();
    fireEvent.change(screen.getByLabelText("From"), {
      target: { value: "Cash and bank" },
    });
    fireEvent.change(screen.getByLabelText("To"), {
      target: { value: "Investments" },
    });
    fireEvent.change(screen.getByLabelText("Amount"), {
      target: { value: "250.00" },
    });
    fireEvent.change(screen.getByLabelText("Date"), {
      target: { value: "2026-03-15" },
    });
    expect(screen.getByText("xfer_cash_investments")).toBeDefined();
    expect(screen.queryByLabelText("Instrument")).toBeNull();
    expect(screen.queryByLabelText("Units")).toBeNull();
    const form = document.querySelector("form")!;
    const sent = Object.fromEntries(new FormData(form).entries());
    expect(sent).toMatchObject({
      fund: "household",
      ruleId: "xfer_cash_investments",
      amount: "250.00",
      date: "2026-03-15",
    });
    expect(sent).not.toHaveProperty("instrument");
    expect(sent).not.toHaveProperty("quantity");
  });
});

describe("sign-in", () => {
  it("offers a prompt rather than a wall of refusals", async () => {
    const SignIn = (await import("./signin/page")).default;
    await renderAsync(SignIn({ searchParams: params({}) }));
    expect(document.querySelector(".signin-btn")).not.toBeNull();
  });

  it("carries a deep link through, so a cited break is still where you land", async () => {
    // ⭐ THE POINT OF A CITABLE URL SURVIVES THE SIGN-IN GATE OR IT IS NOT ONE.
    // Somebody was sent a link to one break. Dropping them on the default queue
    // afterwards and making them find it again is the console this replaced,
    // with extra steps.
    const SignIn = (await import("./signin/page")).default;
    const deep = "/funds/harbourline-global-value/breaks/cash-usd-2026-02-26";
    await renderAsync(SignIn({ searchParams: params({ returnTo: deep }) }));
    const href = document.querySelector(".signin-btn")?.getAttribute("href");
    expect(href).toBe(`/sign-in?returnTo=${encodeURIComponent(deep)}`);
  });

  it("names the signed-in principal and offers the way out", async () => {
    const { Who } = await import("@/components/Who");
    await renderAsync(Who());
    expect(screen.getByText("e.marsh@example.com")).toBeDefined();
    expect(screen.getByRole("button", { name: "Sign out" })).toBeDefined();
  });
});

describe("a refusal", () => {
  // ⛔ THE SENTENCE, NOT A DIGEST. A refusal thrown out of a server component
  // reaches production as `Minified React error #441` and an opaque number —
  // Next redacts server errors — so the API's one explanatory sentence was
  // exactly what got hidden, on every view screen of the dual-basis demo fund.
  // These assert the sentence ARRIVES, which only holds while the pages treat
  // a `Refused` as a value rather than throwing it.
  const SENTENCE =
    'view "ibor" recognises entries by date, and the maintained projection behind this screen folds the whole journal with no cut';

  it("renders the sentence the API wrote on a list screen", async () => {
    const real = wire.listBreaks;
    wire.listBreaks = (async () => {
      const { Refused } = await import("@/wire/client");
      throw new Refused(400, SENTENCE);
    }) as typeof wire.listBreaks;
    try {
      const Breaks = (await import("./books/[book]/views/[view]/breaks/page")).default;
      await renderAsync(
        Breaks({ params: params({ book: FUND, view: "ibor" }), searchParams: params({}) }),
      );
      expect(screen.getByRole("status").textContent).toContain(
        "recognises entries by date",
      );
    } finally {
      wire.listBreaks = real;
    }
  });

  it("renders the sentence on the layout, which gates every view screen", async () => {
    const real = wire.getView;
    wire.getView = (async () => {
      const { Refused } = await import("@/wire/client");
      throw new Refused(400, SENTENCE);
    }) as typeof wire.getView;
    try {
      const Layout = (await import("./books/[book]/views/[view]/layout")).default;
      await renderAsync(
        // ⚠ A VIEW ID NO OTHER TEST HAS ASKED `viewOf` FOR. React `cache`
        // memoizes per (fund, view) for the process under vitest, and a
        // shared key would serve a sibling test's answer instead of this
        // throw — a green refusal test that never saw a refusal.
        Layout({ children: null, params: params({ book: FUND, view: "emir" }) }),
      );
      expect(screen.getByRole("status").textContent).toContain(
        "recognises entries by date",
      );
    } finally {
      wire.getView = real;
    }
  });
});
