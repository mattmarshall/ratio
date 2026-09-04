import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import accountsFixture from "../../fixtures/accounts.json";
import householdAccountsFixture from "../../fixtures/householdAccounts.json";
import operatingAccountsFixture from "../../fixtures/operatingAccounts.json";
import capitalAccountsFixture from "../../fixtures/capitalAccounts.json";
import capitalCommitmentsFixture from "../../fixtures/capitalCommitments.json";
import breakFixture from "../../fixtures/break.json";
import breaksFixture from "../../fixtures/breaks.json";
import changeLogFixture from "../../fixtures/changeLogEntries.json";
import entriesFixture from "../../fixtures/entries.json";
import entryFixture from "../../fixtures/entry.json";
import explainFixture from "../../fixtures/explain.json";
import factsFixture from "../../fixtures/facts.json";
import postingsFixture from "../../fixtures/postings.json";
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
import projectProgressFixture from "../../fixtures/projectProgress.json";
import operatingAgingFixture from "../../fixtures/operatingAging.json";
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
  getBook: async (_c: unknown, book?: string) => {
    const id = String(book ?? "").replace(/^books\//, "");
    const found = booksFixture.books.find(
      (b) => b.name === `books/${id}` || b.name.split("/").pop() === id,
    );
    return found ?? bookFixture;
  },
  createBook: async () => bookFixture,
  getFund: async () => fundFixture,
  getView: async () => viewFixture,
  listViews: async () => viewsFixture,
  reconcileViews: async () => reconcileFixture,
  projectProgress: async () => projectProgressFixture,
  operatingAging: async () => operatingAgingFixture,
  getBreak: async () => breakFixture,
  listBreaks: async () => breaksFixture,
  listAccounts: async () => accountsFixture,
  getPosting: async () => postingsFixture.postings[0],
  listEntries: async () => entriesFixture,
  getEntry: async () => entryFixture,
  listPositions: async () => positionsFixture,
  getPosition: async () => positionsFixture.positions[0],
  listLots: async () => lotsFixture,
  listFacts: async () => factsFixture,
  getFact: async () => factsFixture.facts[0],
  listNavStrikes: async () => navStrikesFixture,
  getNavStrike: async () => navStrikesFixture.navStrikes[0],
  listPeriodCloses: async () => ({ periodCloses: [], nextPageToken: "" }),
  getPeriodClose: async () => {
    throw new Error("no period close in the fixture");
  },
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

  it("offers the book templates CreateBook already knows", async () => {
    const NewBook = (await import("./books/new/page")).default;
    render(<NewBook />);
    expect(screen.getByText("Personal finance")).toBeDefined();
    expect(screen.getByText(/Cash and bank/)).toBeDefined();
    expect(screen.getByText("Investment / Fund")).toBeDefined();
    expect(screen.getByText(/Does not file a fund/)).toBeDefined();
    expect(screen.getByText("Project")).toBeDefined();
    expect(screen.getByText(/work in progress/)).toBeDefined();
    expect(screen.getByText("Operating business")).toBeDefined();
    expect(screen.getByText(/AR\/AP aging when due dates/)).toBeDefined();
    expect(screen.getByText(/Independent of a Fund/)).toBeDefined();
    expect(
      (screen.getByRole("radio", { name: /Personal finance/ }) as HTMLInputElement)
        .checked,
    ).toBe(true);
  });

  it("lists the Personal statement template and not the fund snapshot", async () => {
    const Templates = (await import("./books/[book]/data/templates/page")).default;
    await renderAsync(Templates({ params: params({ book: "household" }) }));
    expect(screen.getByText("bank-statement")).toBeDefined();
    expect(screen.getByText("loan-payment")).toBeDefined();
    expect(screen.getByText("statement")).toBeDefined();
    expect(screen.getByText("payment")).toBeDefined();
    expect(screen.getAllByText("posts").length).toBeGreaterThan(0);
    expect(screen.queryByText("custodian-positions")).toBeNull();
    expect(screen.queryByText("prime_equity_trades")).toBeNull();
    expect(screen.queryByText("capital-calls")).toBeNull();
    expect(
      screen.getByRole("link", { name: /bank-statement/ }).getAttribute("href"),
    ).toBe("/books/household/data/templates/bank-statement");
    expect(
      screen.getByRole("link", { name: /loan-payment/ }).getAttribute("href"),
    ).toBe("/books/household/data/templates/loan-payment");
  });

  it("lists the Project invoice template and not the fund snapshot", async () => {
    const Templates = (await import("./books/[book]/data/templates/page")).default;
    await renderAsync(Templates({ params: params({ book: "bridge" }) }));
    expect(screen.getByText("project-invoices")).toBeDefined();
    expect(screen.getByText("change-orders")).toBeDefined();
    expect(screen.getByText("purchase-orders")).toBeDefined();
    expect(screen.getByText("invoice")).toBeDefined();
    expect(screen.queryByText("custodian-positions")).toBeNull();
    expect(screen.queryByText("prime_equity_trades")).toBeNull();
    expect(screen.queryByText("capital-calls")).toBeNull();
    expect(screen.queryByText("bank-statement")).toBeNull();
    expect(screen.queryByText("loan-payment")).toBeNull();
  });

  it("lists the Operating invoice and bill templates and not the fund snapshot", async () => {
    const Templates = (await import("./books/[book]/data/templates/page")).default;
    await renderAsync(Templates({ params: params({ book: "studio" }) }));
    expect(screen.getByText("customer-invoices")).toBeDefined();
    expect(screen.getByText("vendor-bills")).toBeDefined();
    expect(screen.getByText("invoice")).toBeDefined();
    expect(screen.getByText("bill")).toBeDefined();
    expect(screen.queryByText("custodian-positions")).toBeNull();
    expect(screen.queryByText("project-invoices")).toBeNull();
    expect(screen.queryByText("bank-statement")).toBeNull();
    expect(
      screen.getByRole("link", { name: /customer-invoices/ }).getAttribute("href"),
    ).toBe("/books/studio/data/templates/customer-invoices");
  });

  it("keeps the custodian snapshot and the trade file on an Investment book", async () => {
    const Templates = (await import("./books/[book]/data/templates/page")).default;
    await renderAsync(
      Templates({ params: params({ book: "harbourline-global-value" }) }),
    );
    expect(screen.getByText("custodian-positions")).toBeDefined();
    expect(screen.getByText("position")).toBeDefined();
    expect(screen.getByText("records")).toBeDefined();
    expect(screen.getByText("prime_equity_trades")).toBeDefined();
    expect(screen.getByText("trade")).toBeDefined();
    expect(screen.getAllByText("posts").length).toBeGreaterThan(1);
    expect(screen.getByText("capital-calls")).toBeDefined();
    expect(
      screen.getByRole("link", { name: /prime_equity_trades/ }).getAttribute("href"),
    ).toBe("/books/harbourline-global-value/data/templates/prime_equity_trades");
    expect(screen.queryByText("bank-statement")).toBeNull();
    expect(screen.queryByText("loan-payment")).toBeNull();
    expect(screen.queryByText("project-invoices")).toBeNull();
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
    expect(screen.getByRole("link", { name: "Net-worth bridge" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Cash flow" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Period close" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Budget vs actual" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Loan schedule" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Trial balance" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Configuration" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Transfer between accounts" })).toBeDefined();
    expect(screen.getByText(/Net worth/)).toBeDefined();
    expect(screen.getByText("unset — [personal] budget on the configuration")).toBeDefined();
    // ⛔ THE LABEL IS NOT THE PRODUCT. A personal hub that still offered
    // Exceptions / Positions / NAV would be fund-ops screens with a household
    // name on them — issue #65.
    expect(screen.queryByRole("link", { name: "Exceptions" })).toBeNull();
    expect(screen.queryByRole("link", { name: "Positions" })).toBeNull();
    expect(screen.queryByRole("link", { name: "NAV" })).toBeNull();
    expect(screen.queryByRole("link", { name: "Capital activity" })).toBeNull();
    expect(screen.queryByRole("link", { name: "WIP" })).toBeNull();
  });

  it("a project book opens budget vs actual and WIP, not Exceptions or NAV", async () => {
    const real = wire.getBook;
    const bridge = booksFixture.books.find((b) => b.kind === "PROJECT")!;
    wire.getBook = (async () => ({ ...bridge, defaultView: "book" })) as typeof wire.getBook;
    try {
      const Book = (await import("./books/[book]/page")).default;
      await renderAsync(Book({ params: params({ book: "bridge" }) }));
      expect(screen.getByRole("heading", { name: "Bridge" })).toBeDefined();
      expect(screen.getByText("Project")).toBeDefined();
      const budget = screen.getByRole("link", { name: "Budget vs actual" });
      expect(budget.getAttribute("href")).toBe(
        "/books/bridge/views/book/budget",
      );
      expect(
        screen.getByRole("link", { name: "WIP" }).getAttribute("href"),
      ).toBe("/books/bridge/views/book/wip");
      expect(
        screen.getByRole("link", { name: "Billing" }).getAttribute("href"),
      ).toBe("/books/bridge/views/book/billing");
      expect(
        screen.getByRole("link", { name: "Period close" }).getAttribute("href"),
      ).toBe("/books/bridge/views/book/close");
      expect(screen.queryByRole("link", { name: "Exceptions" })).toBeNull();
      expect(screen.queryByRole("link", { name: "NAV" })).toBeNull();
      expect(screen.queryByRole("link", { name: "Positions" })).toBeNull();
      expect(screen.queryByRole("link", { name: "Cash flow" })).toBeNull();
      expect(screen.getByText(/unset/)).toBeDefined();
    } finally {
      wire.getBook = real;
    }
  });

  it("an operating book opens a sheet and income statement, not Exceptions, NAV, or Billing", async () => {
    const studio = booksFixture.books.find((b) => b.kind === "OPERATING")!;
    const real = wire.getBook;
    wire.getBook = (async () => ({ ...studio, defaultView: "book" })) as typeof wire.getBook;
    try {
      const Book = (await import("./books/[book]/page")).default;
      await renderAsync(Book({ params: params({ book: "studio" }) }));
      expect(screen.getByRole("heading", { name: "Studio" })).toBeDefined();
      expect(screen.getByText("Operating")).toBeDefined();
      expect(screen.getByText("independent")).toBeDefined();
      expect(
        screen.getByRole("link", { name: "Balance sheet" }).getAttribute("href"),
      ).toBe("/books/studio/views/book/sheet");
      expect(
        screen.getByRole("link", { name: "Income statement" }).getAttribute("href"),
      ).toBe("/books/studio/views/book/pnl");
      expect(
        screen.getByRole("link", { name: "Cash flow" }).getAttribute("href"),
      ).toBe("/books/studio/views/book/cashflow");
      expect(
        screen.getByRole("link", { name: "Period close" }).getAttribute("href"),
      ).toBe("/books/studio/views/book/close");
      expect(
        screen.getByRole("link", { name: "AR/AP aging" }).getAttribute("href"),
      ).toBe("/books/studio/views/book/aging");
      expect(screen.getByRole("link", { name: "Trial balance" })).toBeDefined();
      expect(screen.getByText(/aged open items by due date/)).toBeDefined();
      expect(screen.queryByRole("link", { name: "Exceptions" })).toBeNull();
      expect(screen.queryByRole("link", { name: "NAV" })).toBeNull();
      expect(screen.queryByRole("link", { name: "Positions" })).toBeNull();
      expect(screen.queryByRole("link", { name: "Billing" })).toBeNull();
      expect(screen.queryByRole("link", { name: "WIP" })).toBeNull();
      expect(screen.queryByRole("link", { name: "Net-worth bridge" })).toBeNull();
      expect(screen.queryByText(/Net worth/)).toBeNull();
    } finally {
      wire.getBook = real;
    }
  });

  it("an investment book hub leads with capital activity and still offers NAV", async () => {
    const harbour = booksFixture.books.find((b) => b.kind === "INVESTMENT")!;
    const realBook = wire.getBook;
    const realView = wire.getView;
    wire.getBook = (async () => harbour) as typeof wire.getBook;
    try {
      const Book = (await import("./books/[book]/page")).default;
      await renderAsync(
        Book({ params: params({ book: "harbourline-global-value" }) }),
      );
      const capital = screen.getByRole("link", { name: "Capital activity" });
      expect(capital.getAttribute("href")).toBe(
        "/books/harbourline-global-value/views/abor/capital",
      );
      expect(
        screen.getByRole("link", { name: "NAV roll-forward" }).getAttribute("href"),
      ).toBe("/books/harbourline-global-value/views/abor/nav");
      expect(
        screen.getByRole("link", { name: "Period close" }).getAttribute("href"),
      ).toBe("/books/harbourline-global-value/views/abor/close");
      expect(screen.getByRole("link", { name: /^NAV$/ })).toBeDefined();
      expect(screen.getByRole("link", { name: "Exceptions" })).toBeDefined();
      expect(screen.getByRole("link", { name: "Positions" })).toBeDefined();
      expect(screen.queryByRole("link", { name: "Cash flow" })).toBeNull();
    } finally {
      wire.getBook = realBook;
      wire.getView = realView;
    }
  });

  it("cites partner capital in and out and ending, and says it is not a return", async () => {
    const harbour = booksFixture.books.find((b) => b.kind === "INVESTMENT")!;
    const realBook = wire.getBook;
    const realAccounts = wire.listAccounts;
    wire.getBook = (async () => harbour) as typeof wire.getBook;
    wire.listAccounts = (async () => capitalAccountsFixture) as typeof wire.listAccounts;
    try {
      const Capital = (await import("./books/[book]/views/[view]/capital/page"))
        .default;
      await renderAsync(
        Capital({
          params: params({ book: "harbourline-global-value", view: "abor" }),
          searchParams: params({}),
        }),
      );
      expect(screen.getByLabelText("Capital activity")).toBeDefined();
      expect(screen.getByText("Partner capital — LP")).toBeDefined();
      expect(screen.getByText("Partner capital — GP")).toBeDefined();
      expect(screen.getAllByText("Distributions").length).toBeGreaterThan(0);
      expect(screen.getAllByText("75.00").length).toBeGreaterThan(0);
      expect(screen.getByText("115.00")).toBeDefined();
      expect(screen.getByText(/not a return, not attribution/)).toBeDefined();
      expect(screen.getByText(/not IRR/)).toBeDefined();
      expect(screen.getByLabelText("Capital account statement")).toBeDefined();
      expect(screen.getByText("Capital account — LP")).toBeDefined();
      expect(screen.getByText("Capital account — GP")).toBeDefined();
      expect(
        screen.getAllByText(/since inception has no prior prefix — not a measured zero beginning/)
          .length,
      ).toBeGreaterThan(0);
      expect(
        screen.getAllByText(
          /unset — no partner-cut of period income, not an equal share of book NAV/,
        ).length,
      ).toBeGreaterThan(0);
      expect(
        screen.getAllByText(
          /unset — no partner-cut of Unrealized gain — not a silent equal allocation/,
        ).length,
      ).toBeGreaterThan(0);
      expect(
        screen.getAllByText(
          /unset — no units issued on this partner, not a fake zero/,
        ).length,
      ).toBeGreaterThan(0);
      expect(screen.queryByText(/^Unrealized gain$/)).toBeNull();
      expect(
        screen.getByRole("link", { name: "Record an event" }).getAttribute("href"),
      ).toBe("/books/harbourline-global-value/record");
      expect(screen.getByLabelText("Undrawn commitment")).toBeDefined();
      expect(
        screen.getByText(/unset — no commitment has been posted, not a callable zero/),
      ).toBeDefined();
      expect(screen.getByText("unset")).toBeDefined();
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = realAccounts;
    }
  });

  it("cites remaining undrawn after a call, and leaves an unposted partner unset", async () => {
    const harbour = booksFixture.books.find((b) => b.kind === "INVESTMENT")!;
    const realBook = wire.getBook;
    const realAccounts = wire.listAccounts;
    wire.getBook = (async () => harbour) as typeof wire.getBook;
    wire.listAccounts = (async () => capitalCommitmentsFixture) as typeof wire.listAccounts;
    try {
      const Capital = (await import("./books/[book]/views/[view]/capital/page"))
        .default;
      await renderAsync(
        Capital({
          params: params({ book: "harbourline-global-value", view: "abor" }),
          searchParams: params({}),
        }),
      );
      expect(screen.getByLabelText("Undrawn commitment")).toBeDefined();
      expect(screen.getByText("Undrawn commitments — LP")).toBeDefined();
      expect(screen.getAllByText("60.00").length).toBeGreaterThan(1);
      expect(screen.getByText("Undrawn commitments — GP")).toBeDefined();
      expect(screen.getAllByText("unset").length).toBeGreaterThan(0);
      expect(screen.queryByText(/not a callable zero/)).toBeNull();
      expect(screen.getByText(/remaining commitment, partner grain/)).toBeDefined();
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = realAccounts;
    }
  });

  it("cites a period capital account from the NAV fold and refuses an equal split", async () => {
    const harbour = booksFixture.books.find((b) => b.kind === "INVESTMENT")!;
    const realBook = wire.getBook;
    const realAccounts = wire.listAccounts;
    const calls: unknown[][] = [];
    // Loan-shaped March: LP began at 100.00, contributed 40, distributed 10.
    // GP contributed 20. Book income 30.00 / unrealized 20.00 — equal-split
    // would print 15.00 and 10.00 on each partner. Those must not appear
    // as allocated plugs.
    const period = {
      accounts: [
        {
          name: "funds/harbourline-global-value/views/abor/accounts/2",
          displayName: "Cash and equivalents",
          dimension: "2",
          type: "ASSET",
          debit: "6000",
          credit: "1000",
          balance: "15000",
          abnormal: false,
          postingCount: "3",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/50",
          displayName: "Partner capital — LP",
          dimension: "50",
          type: "EQUITY",
          debit: "1000",
          credit: "4000",
          balance: "-13000",
          abnormal: false,
          postingCount: "2",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/51",
          displayName: "Partner capital — GP",
          dimension: "51",
          type: "EQUITY",
          debit: "0",
          credit: "2000",
          balance: "-2000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/30",
          displayName: "Dividend income",
          dimension: "30",
          type: "REVENUE",
          debit: "0",
          credit: "3000",
          balance: "-3000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/21",
          displayName: "Unrealized gain",
          dimension: "21",
          type: "EQUITY",
          debit: "0",
          credit: "2000",
          balance: "-2000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    };
    wire.getBook = (async () => harbour) as typeof wire.getBook;
    wire.listAccounts = (async (...args: unknown[]) => {
      calls.push(args);
      if (args[3] === "nav") return period;
      return capitalAccountsFixture;
    }) as typeof wire.listAccounts;
    try {
      const Capital = (await import("./books/[book]/views/[view]/capital/page"))
        .default;
      await renderAsync(
        Capital({
          params: params({ book: "harbourline-global-value", view: "abor" }),
          searchParams: params({ filter: "capital-2026-03" }),
        }),
      );
      expect(calls.some((a) => a[3] === "nav" && a[4] === "2026-03")).toBe(true);
      expect(screen.getByLabelText("Capital account statement")).toBeDefined();
      expect(screen.getByText("Capital account — LP")).toBeDefined();
      expect(screen.getAllByText("100.00").length).toBeGreaterThan(0);
      expect(screen.getByText("130.00")).toBeDefined();
      expect(screen.getByText("Capital account — GP")).toBeDefined();
      expect(screen.getAllByText("20.00").length).toBeGreaterThan(0);
      expect(
        screen.getAllByText(/the same Loan-shaped fold \/nav cites/).length,
      ).toBeGreaterThan(0);
      const income = screen.getAllByText("Allocated income");
      expect(income.length).toBe(2);
      for (const label of income) {
        const row = label.closest("[role=row]");
        expect(row?.textContent).toContain("—");
        expect(row?.textContent).not.toMatch(/15\.00/);
        expect(row?.textContent).not.toMatch(/0\.00/);
      }
      const unreal = screen.getAllByText("Unrealized");
      expect(unreal.length).toBe(2);
      for (const label of unreal) {
        const row = label.closest("[role=row]");
        expect(row?.textContent).toContain("—");
        expect(row?.textContent).not.toMatch(/10\.00/);
        expect(row?.textContent).not.toMatch(/0\.00/);
      }
      expect(screen.queryByText("15.00")).toBeNull();
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = realAccounts;
    }
  });

  it("fills allocated plugs from a named partner cut and still refuses 1/N", async () => {
    const harbour = {
      ...booksFixture.books.find((b) => b.kind === "INVESTMENT")!,
      partnerCut: [
        { partner: "LP", weight: "80" },
        { partner: "GP", weight: "20" },
      ],
    };
    const realBook = wire.getBook;
    const realAccounts = wire.listAccounts;
    const period = {
      accounts: [
        {
          name: "funds/harbourline-global-value/views/abor/accounts/2",
          displayName: "Cash and equivalents",
          dimension: "2",
          type: "ASSET",
          debit: "6000",
          credit: "1000",
          balance: "15000",
          abnormal: false,
          postingCount: "3",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/50",
          displayName: "Partner capital — LP",
          dimension: "50",
          type: "EQUITY",
          debit: "1000",
          credit: "4000",
          balance: "-13000",
          abnormal: false,
          postingCount: "2",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/51",
          displayName: "Partner capital — GP",
          dimension: "51",
          type: "EQUITY",
          debit: "0",
          credit: "2000",
          balance: "-2000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/30",
          displayName: "Dividend income",
          dimension: "30",
          type: "REVENUE",
          debit: "0",
          credit: "3000",
          balance: "-3000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/21",
          displayName: "Unrealized gain",
          dimension: "21",
          type: "EQUITY",
          debit: "0",
          credit: "2000",
          balance: "-2000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    };
    // Fixture JSON types empty `partnerCut` as `never[]`. A named cut is
    // a wider return than the mock's inferred Book — same `unknown`
    // bridge the budget/envelope mocks already use.
    wire.getBook = (async () => harbour) as unknown as typeof wire.getBook;
    wire.listAccounts = (async (...args: unknown[]) => {
      if (args[3] === "nav") return period;
      return capitalAccountsFixture;
    }) as typeof wire.listAccounts;
    try {
      const Capital = (await import("./books/[book]/views/[view]/capital/page"))
        .default;
      await renderAsync(
        Capital({
          params: params({ book: "harbourline-global-value", view: "abor" }),
          searchParams: params({ filter: "capital-2026-03" }),
        }),
      );
      // 80/20 of 30.00 is 24.00 / 6.00, not 15.00 / 15.00.
      expect(screen.getByText("24.00")).toBeDefined();
      expect(screen.getByText("6.00")).toBeDefined();
      expect(screen.queryByText("15.00")).toBeNull();
      expect(
        screen.getAllByText(
          /this partner's share of period income under the named cut/,
        ).length,
      ).toBeGreaterThan(0);
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = realAccounts;
    }
  });

  it("refuses capital activity on a personal book", async () => {
    const Capital = (await import("./books/[book]/views/[view]/capital/page"))
      .default;
    await renderAsync(
      Capital({
        params: params({ book: "household", view: "book" }),
        searchParams: params({}),
      }),
    );
    expect(screen.getByText(/Capital activity is an Investment figure/)).toBeDefined();
    expect(screen.getByText(/Personal/)).toBeDefined();
    expect(screen.queryByLabelText("Capital activity")).toBeNull();
  });

  it("cites an unset NAV roll-forward rather than a measured zero", async () => {
    const harbour = booksFixture.books.find((b) => b.kind === "INVESTMENT")!;
    const Nav = (await import("./books/[book]/views/[view]/nav/page")).default;
    const realBook = wire.getBook;
    const realAccounts = wire.listAccounts;
    wire.getBook = (async () => harbour) as typeof wire.getBook;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/harbourline-global-value/views/abor/accounts/2",
          displayName: "Cash and equivalents",
          dimension: "2",
          type: "ASSET",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/50",
          displayName: "Partner capital — LP",
          dimension: "50",
          type: "EQUITY",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/21",
          displayName: "Unrealized gain",
          dimension: "21",
          type: "EQUITY",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as typeof wire.listAccounts;
    try {
      await renderAsync(
        Nav({
          params: params({ book: "harbourline-global-value", view: "abor" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(
        screen.getByText(/Beginning and ending stay unset — not a measured zero NAV/),
      ).toBeDefined();
      expect(screen.getByLabelText("NAV roll-forward")).toBeDefined();
      expect(screen.getAllByText("—").length).toBeGreaterThan(2);
      expect(screen.queryByText("0.00")).toBeNull();
      expect(
        screen.getByText(/unset — Unrealized gain did not move this window/),
      ).toBeDefined();
      expect(
        screen.getByText(/unset — no subscription has posted units, not a fake zero/),
      ).toBeDefined();
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = realAccounts;
    }
  });

  it("cites ΔNAV against capital plugs and leaves commitment off the identity", async () => {
    const harbour = booksFixture.books.find((b) => b.kind === "INVESTMENT")!;
    const Nav = (await import("./books/[book]/views/[view]/nav/page")).default;
    const realBook = wire.getBook;
    const realAccounts = wire.listAccounts;
    wire.getBook = (async () => harbour) as typeof wire.getBook;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/harbourline-global-value/views/abor/accounts/2",
          displayName: "Cash and equivalents",
          dimension: "2",
          type: "ASSET",
          debit: "6000",
          credit: "1000",
          balance: "15000",
          abnormal: false,
          postingCount: "3",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/1",
          displayName: "Investments at fair value",
          dimension: "1",
          type: "ASSET",
          debit: "2000",
          credit: "0",
          balance: "2000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/40",
          displayName: "Management fee payable",
          dimension: "40",
          type: "LIABILITY",
          debit: "0",
          credit: "500",
          balance: "-500",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/50",
          displayName: "Partner capital — LP",
          dimension: "50",
          type: "EQUITY",
          debit: "1000",
          credit: "4000",
          balance: "-13000",
          abnormal: false,
          postingCount: "2",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/30",
          displayName: "Dividend income",
          dimension: "30",
          type: "REVENUE",
          debit: "0",
          credit: "2000",
          balance: "-2000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/10",
          displayName: "Management fee expense",
          dimension: "10",
          type: "EXPENSE",
          debit: "500",
          credit: "0",
          balance: "500",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/21",
          displayName: "Unrealized gain",
          dimension: "21",
          type: "EQUITY",
          debit: "0",
          credit: "2000",
          balance: "-2000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/harbourline-global-value/views/abor/accounts/54",
          displayName: "Undrawn commitments — LP",
          dimension: "54",
          type: "EQUITY",
          debit: "0",
          credit: "0",
          balance: "5000",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as typeof wire.listAccounts;
    try {
      await renderAsync(
        Nav({
          params: params({ book: "harbourline-global-value", view: "abor" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(screen.getByLabelText("NAV roll-forward")).toBeDefined();
      expect(
        screen.getByText(
          /beginning plus contributions minus distributions plus income minus expenses/,
        ),
      ).toBeDefined();
      expect(screen.getByText("100.00")).toBeDefined();
      expect(screen.getByText("40.00")).toBeDefined();
      expect(screen.getByText("10.00")).toBeDefined();
      expect(screen.getAllByText("20.00").length).toBeGreaterThanOrEqual(1);
      expect(screen.getByText("5.00")).toBeDefined();
      expect(screen.getAllByText("165.00").length).toBeGreaterThanOrEqual(1);
      expect(screen.getByText("65.00")).toBeDefined();
      expect(
        screen.getByText(/equity, so they cancel — remaining undrawn is on Capital/),
      ).toBeDefined();
      expect(screen.queryByText(/Beginning and ending stay unset/)).toBeNull();
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = realAccounts;
    }
  });

  it("asks ListAccounts for a period window on the NAV roll-forward", async () => {
    const harbour = booksFixture.books.find((b) => b.kind === "INVESTMENT")!;
    const calls: unknown[][] = [];
    const realBook = wire.getBook;
    const real = wire.listAccounts;
    wire.getBook = (async () => harbour) as typeof wire.getBook;
    wire.listAccounts = (async (...args: unknown[]) => {
      calls.push(args);
      return { accounts: [], nextPageToken: "" };
    }) as typeof wire.listAccounts;
    try {
      const Nav = (await import("./books/[book]/views/[view]/nav/page")).default;
      await renderAsync(
        Nav({
          params: params({ book: "harbourline-global-value", view: "abor" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(calls[0]?.slice(1)).toEqual([
        "harbourline-global-value",
        "abor",
        "nav",
        "2026-03",
      ]);
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = real;
    }
  });

  it("404s a NAV roll-forward on a book that is not Investment", async () => {
    const Nav = (await import("./books/[book]/views/[view]/nav/page")).default;
    await expect(
      Nav({
        params: params({ book: "household", view: "book" }),
        searchParams: params({ period: "2026-03" }),
      }),
    ).rejects.toThrow();
  });

  it("a personal book is not sent to project WIP or fund Exceptions", async () => {
    const Book = (await import("./books/[book]/page")).default;
    await renderAsync(Book({ params: params({ book: "household" }) }));
    expect(screen.getByRole("link", { name: "Budget vs actual" })).toBeDefined();
    expect(screen.queryByRole("link", { name: "WIP" })).toBeNull();
    expect(screen.queryByRole("link", { name: "Exceptions" })).toBeNull();
  });

  it("renders budget vs actual from the journal against a configuration total", async () => {
    const realBook = wire.getBook;
    const realAccounts = wire.listAccounts;
    const bridge = booksFixture.books.find((b) => b.kind === "PROJECT")!;
    wire.getBook = (async () => ({
      ...bridge,
      budget: "10000000",
    })) as typeof wire.getBook;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/bridge/views/book/accounts/1",
          displayName: "Cash",
          dimension: "1",
          type: "ASSET",
          debit: "500000",
          credit: "200000",
          balance: "300000",
          abnormal: false,
          postingCount: "2",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/bridge/views/book/accounts/2",
          displayName: "Work in progress",
          dimension: "2",
          type: "ASSET",
          debit: "400000",
          credit: "100000",
          balance: "300000",
          abnormal: false,
          postingCount: "2",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/bridge/views/book/accounts/10",
          displayName: "Project costs",
          dimension: "10",
          type: "EXPENSE",
          debit: "700000",
          credit: "400000",
          balance: "300000",
          abnormal: false,
          postingCount: "3",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/bridge/views/book/accounts/20",
          displayName: "Funding",
          dimension: "20",
          type: "EQUITY",
          debit: "0",
          credit: "900000",
          balance: "-900000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/bridge/views/book/accounts/30",
          displayName: "Project revenue",
          dimension: "30",
          type: "REVENUE",
          debit: "0",
          credit: "150000",
          balance: "-150000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/bridge/views/book/accounts/40",
          displayName: "Payables",
          dimension: "40",
          type: "LIABILITY",
          debit: "0",
          credit: "200000",
          balance: "-200000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as typeof wire.listAccounts;
    try {
      const Budget = (await import("./books/[book]/views/[view]/budget/page")).default;
      await renderAsync(
        Budget({
          params: params({ book: "bridge", view: "book" }),
          searchParams: params({}),
        }),
      );
      expect(screen.getAllByText("100,000.00").length).toBe(2);
      expect(screen.getAllByText("Project costs").length).toBeGreaterThan(0);
      expect(screen.getByText("Work in progress")).toBeDefined();
      // incurred 6,000.00; awarded unset; remaining to spend unset
      expect(screen.getByText("6,000.00")).toBeDefined();
      expect(screen.getByText("Original contract")).toBeDefined();
      expect(screen.getByText("Remaining to spend")).toBeDefined();
      expect(
        screen.getByText(/unset until a purchase order is awarded — not budget minus actual as fake headroom/),
      ).toBeDefined();
      expect(screen.getByText("Estimate at completion")).toBeDefined();
      expect(
        screen.getByText(/this page does not forecast/),
      ).toBeDefined();
      const remaining = screen.getByText("Remaining to spend").closest("[role=row]");
      expect(remaining?.textContent).toContain("—");
      expect(remaining?.textContent).not.toMatch(/92,000\.00/);
      expect(remaining?.textContent).not.toMatch(/0\.00/);
      expect(screen.getByText("Awarded")).toBeDefined();
      expect(
        screen.getByText(/unset — no purchase order has been awarded, not a fake zero committed/),
      ).toBeDefined();
      expect(screen.getByText("Approved change orders")).toBeDefined();
      expect(
        screen.getByText(/unset — no approved change order has posted, not a silent zero/),
      ).toBeDefined();
      expect(screen.getByText(/equals the original — no approved change order has posted/)).toBeDefined();
      expect(
        screen.queryByRole("link", { name: "Exceptions" }),
      ).toBeNull();
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = realAccounts;
    }
  });

  it("cites a revised contract from approved change orders without rewriting the baseline", async () => {
    const realBook = wire.getBook;
    const realAccounts = wire.listAccounts;
    const bridge = booksFixture.books.find((b) => b.kind === "PROJECT")!;
    wire.getBook = (async () => ({
      ...bridge,
      budget: "10000000",
    })) as typeof wire.getBook;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/bridge/views/book/accounts/10",
          displayName: "Project costs",
          dimension: "10",
          type: "EXPENSE",
          debit: "300000",
          credit: "0",
          balance: "300000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/bridge/views/book/accounts/20",
          displayName: "Funding",
          dimension: "20",
          type: "EQUITY",
          debit: "0",
          credit: "800000",
          balance: "-800000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/bridge/views/book/accounts/21",
          displayName: "Change-order authorization — Site and mobilization",
          dimension: "21",
          type: "EQUITY",
          debit: "500000",
          credit: "0",
          balance: "500000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/bridge/views/book/accounts/26",
          displayName: "Approved change orders — Site and mobilization",
          dimension: "26",
          type: "EQUITY",
          debit: "0",
          credit: "500000",
          balance: "-500000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/bridge/views/book/accounts/25",
          displayName: "Approved change orders",
          dimension: "25",
          type: "EQUITY",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as typeof wire.listAccounts;
    try {
      const Budget = (await import("./books/[book]/views/[view]/budget/page")).default;
      await renderAsync(
        Budget({
          params: params({ book: "bridge", view: "book" }),
          searchParams: params({}),
        }),
      );
      expect(screen.getAllByText("100,000.00").length).toBeGreaterThan(0);
      expect(screen.getByText("5,000.00")).toBeDefined();
      expect(screen.getByText("105,000.00")).toBeDefined();
      expect(screen.getByText(/original plus approved change orders/)).toBeDefined();
      expect(screen.queryByText("Change-order authorization — Site and mobilization")).toBeNull();
      expect(screen.getByText("Remaining to spend")).toBeDefined();
      expect(
        screen.getByText(/unset until a purchase order is awarded — not budget minus actual as fake headroom/),
      ).toBeDefined();
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = realAccounts;
    }
  });

  it("asks ListAccounts for a change-order window without folding the whole project into a month", async () => {
    const calls: unknown[][] = [];
    const real = wire.listAccounts;
    wire.listAccounts = (async (...args: unknown[]) => {
      calls.push(args);
      return { accounts: [], nextPageToken: "" };
    }) as typeof wire.listAccounts;
    try {
      const Budget = (await import("./books/[book]/views/[view]/budget/page")).default;
      await renderAsync(
        Budget({
          params: params({ book: "bridge", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(calls[0]?.slice(1)).toEqual(["bridge", "book"]);
      expect(calls[1]?.slice(1)).toEqual(["bridge", "book", "change", "2026-03"]);
      expect(screen.getByText(/Approved this window/)).toBeDefined();
      expect(
        screen.getByText(/unset — nothing approved in this window, not a fake zero/),
      ).toBeDefined();
    } finally {
      wire.listAccounts = real;
    }
  });

  it("cites remaining to spend from awarded commitments without treating an unposted award as zero", async () => {
    const realBook = wire.getBook;
    const realAccounts = wire.listAccounts;
    const bridge = booksFixture.books.find((b) => b.kind === "PROJECT")!;
    wire.getBook = (async () => ({
      ...bridge,
      budget: "10000000",
    })) as typeof wire.getBook;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/bridge/views/book/accounts/10",
          displayName: "Project costs",
          dimension: "10",
          type: "EXPENSE",
          debit: "300000",
          credit: "0",
          balance: "300000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/bridge/views/book/accounts/11",
          displayName: "Site and mobilization",
          dimension: "11",
          type: "EXPENSE",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/bridge/views/book/accounts/64",
          displayName: "Awarded commitments",
          dimension: "64",
          type: "EQUITY",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/bridge/views/book/accounts/65",
          displayName: "Awarded commitments — Site and mobilization",
          dimension: "65",
          type: "EQUITY",
          debit: "0",
          credit: "350000",
          balance: "-350000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as typeof wire.listAccounts;
    try {
      const Budget = (await import("./books/[book]/views/[view]/budget/page")).default;
      await renderAsync(
        Budget({
          params: params({ book: "bridge", view: "book" }),
          searchParams: params({}),
        }),
      );
      // revised 100,000 − incurred 3,000 − awarded 3,500 = 93,500
      expect(screen.getAllByText("3,500.00").length).toBeGreaterThan(0);
      expect(screen.getByText("93,500.00")).toBeDefined();
      expect(
        screen.getByText(/revised minus incurred minus awarded — the citeable leftover, not a forecast/),
      ).toBeDefined();
      expect(
        screen.getByText(/open award on this work package — same grain cost-by-package uses/),
      ).toBeDefined();
      expect(screen.getAllByText(/unset — no purchase order has been awarded on this work package/).length).toBeGreaterThan(0);
      expect(screen.queryByText("Commitment authorization")).toBeNull();
      expect(screen.getByText(/this page does not forecast/)).toBeDefined();
      const eac = screen.getByText("Estimate at completion").closest("[role=row]");
      expect(eac?.textContent).toContain("—");
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = realAccounts;
    }
  });

  it("renders WIP as cost then capitalized then recognized", async () => {
    const realAccounts = wire.listAccounts;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/bridge/views/book/accounts/2",
          displayName: "Work in progress",
          dimension: "2",
          type: "ASSET",
          debit: "400000",
          credit: "100000",
          balance: "300000",
          abnormal: false,
          postingCount: "2",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/bridge/views/book/accounts/10",
          displayName: "Project costs",
          dimension: "10",
          type: "EXPENSE",
          debit: "700000",
          credit: "400000",
          balance: "300000",
          abnormal: false,
          postingCount: "3",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as typeof wire.listAccounts;
    try {
      const Wip = (await import("./books/[book]/views/[view]/wip/page")).default;
      await renderAsync(
        Wip({ params: params({ book: "bridge", view: "book" }) }),
      );
      expect(screen.getByText("Currently capitalized")).toBeDefined();
      expect(screen.getByText("Recognized (out of WIP)")).toBeDefined();
      expect(screen.getByText("currently capitalized plus recognized")).toBeDefined();
      expect(screen.getByText("uncapitalized plus currently in WIP — not a second ledger")).toBeDefined();
    } finally {
      wire.listAccounts = realAccounts;
    }
  });

  it("opens a project book onto billing, not Exceptions or NAV", async () => {
    const real = wire.getBook;
    wire.getBook = (async () => booksFixture.books[2]) as typeof wire.getBook;
    try {
      const Book = (await import("./books/[book]/page")).default;
      await renderAsync(Book({ params: params({ book: "bridge" }) }));
      expect(screen.getByText("Project")).toBeDefined();
      const billing = screen.getByRole("link", { name: "Billing" });
      expect(billing.getAttribute("href")).toBe("/books/bridge/views/book/billing");
      expect(screen.queryByRole("link", { name: "Exceptions" })).toBeNull();
      expect(screen.queryByRole("link", { name: "NAV" })).toBeNull();
      expect(screen.queryByRole("link", { name: "Positions" })).toBeNull();
      expect(screen.queryByText("NAV, in")).toBeNull();
    } finally {
      wire.getBook = real;
    }
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
    expect(screen.getByRole("link", { name: "Net-worth bridge" }).getAttribute("href")).toBe(
      "/books/household/views/book/bridge",
    );
    expect(screen.getByRole("link", { name: "Cash flow" }).getAttribute("href")).toBe(
      "/books/household/views/book/cashflow",
    );
    expect(screen.getByRole("link", { name: "Period close" }).getAttribute("href")).toBe(
      "/books/household/views/book/close",
    );
    const trial = screen.getByRole("link", { name: "Trial balance" });
    expect(trial.getAttribute("href")).toBe(
      "/books/household/views/book/accounts",
    );
    const budget = screen.getByRole("link", { name: "Budget vs actual" });
    expect(budget.getAttribute("href")).toBe(
      "/books/household/views/book/budget",
    );
    const loans = screen.getByRole("link", { name: "Loan schedule" });
    expect(loans.getAttribute("href")).toBe(
      "/books/household/views/book/loans",
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

  it("cites household budget vs actual and does not invent a zero baseline", async () => {
    const Budget = (await import("./books/[book]/views/[view]/budget/page")).default;
    const real = wire.listAccounts;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/household/views/book/accounts/10",
          displayName: "Living expenses",
          dimension: "10",
          type: "EXPENSE",
          debit: "4000",
          credit: "0",
          balance: "4000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/11",
          displayName: "Taxes",
          dimension: "11",
          type: "EXPENSE",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as unknown as typeof wire.listAccounts;
    try {
      await renderAsync(
        Budget({
          params: params({ book: "household", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(screen.getByLabelText("Budget vs actual")).toBeDefined();
      expect(screen.getByText("Living expenses")).toBeDefined();
      expect(screen.getByText("Taxes")).toBeDefined();
      expect(
        screen.getByText("no [personal] budget on the configuration in force"),
      ).toBeDefined();
      expect(screen.getAllByText("—").length).toBeGreaterThan(0);
    } finally {
      wire.listAccounts = real;
    }
  });

  it("a set household budget is a figure against envelopes, not a second ledger", async () => {
    const Budget = (await import("./books/[book]/views/[view]/budget/page")).default;
    const realBook = wire.getBook;
    const realAccounts = wire.listAccounts;
    wire.getBook = (async () => ({
      ...(bookFixture as object),
      budget: "500000",
      envelopes: [
        { dimension: "10", budget: "400000" },
        { dimension: "11", budget: "100000" },
      ],
    })) as unknown as typeof wire.getBook;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/household/views/book/accounts/10",
          displayName: "Living expenses",
          dimension: "10",
          type: "EXPENSE",
          debit: "4000",
          credit: "0",
          balance: "4000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as unknown as typeof wire.listAccounts;
    try {
      await renderAsync(
        Budget({
          params: params({ book: "household", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(screen.getByText("5,000.00")).toBeDefined();
      // Living expenses and the spent total are the same figure here —
      // one category, one window. Two cells, not a second ledger.
      expect(screen.getAllByText("40.00").length).toBe(2);
      expect(screen.getByText(/envelope 4,000\.00/)).toBeDefined();
      expect(screen.getByText(/remaining authorization, not annualized/)).toBeDefined();
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = realAccounts;
    }
  });

  it("does not offer a loan schedule on an investment book hub", async () => {
    const Book = (await import("./books/[book]/page")).default;
    await renderAsync(Book({ params: params({ book: FUND }) }));
    expect(screen.queryByRole("link", { name: "Loan schedule" })).toBeNull();
    expect(screen.queryByRole("link", { name: "Net-worth bridge" })).toBeNull();
    expect(screen.queryByRole("link", { name: "Cash flow" })).toBeNull();
  });

  it("cites an unset household loan schedule rather than a roll-forward of zeros", async () => {
    const Loans = (await import("./books/[book]/views/[view]/loans/page")).default;
    await renderAsync(
      Loans({
        params: params({ book: "household", view: "book" }),
        searchParams: params({ period: "2026-03" }),
      }),
    );
    expect(screen.getByText("No loan schedule is configured.")).toBeDefined();
    expect(screen.queryByLabelText("Loan schedule")).toBeNull();
    expect(screen.queryByText("Mortgage")).toBeNull();
  });

  it("keys a configured loan roll-forward by liability, not a single debt bucket", async () => {
    const Loans = (await import("./books/[book]/views/[view]/loans/page")).default;
    const realBook = wire.getBook;
    const realAccounts = wire.listAccounts;
    wire.getBook = (async () => ({
      ...bookFixture,
      loans: [
        { dimension: "41", interest: "12" },
        { dimension: "42", interest: "13" },
      ],
    })) as unknown as typeof wire.getBook;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/household/views/book/accounts/41",
          displayName: "Mortgage",
          dimension: "41",
          type: "LIABILITY",
          debit: "80000",
          credit: "0",
          balance: "-9920000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/12",
          displayName: "Mortgage interest",
          dimension: "12",
          type: "EXPENSE",
          debit: "20000",
          credit: "0",
          balance: "20000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/42",
          displayName: "Auto loan",
          dimension: "42",
          type: "LIABILITY",
          debit: "35000",
          credit: "0",
          balance: "-1765000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/13",
          displayName: "Auto loan interest",
          dimension: "13",
          type: "EXPENSE",
          debit: "4500",
          credit: "0",
          balance: "4500",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/40",
          displayName: "Credit cards",
          dimension: "40",
          type: "LIABILITY",
          debit: "8900",
          credit: "0",
          balance: "-8900",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as unknown as typeof wire.listAccounts;
    try {
      await renderAsync(
        Loans({
          params: params({ book: "household", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(screen.getByLabelText("Loan schedule")).toBeDefined();
      expect(screen.getByText("Mortgage")).toBeDefined();
      expect(screen.getByText("Auto loan")).toBeDefined();
      expect(screen.queryByText("Credit cards")).toBeNull();
      expect(screen.getByText("100,000.00")).toBeDefined();
      expect(screen.getByText("800.00")).toBeDefined();
      expect(screen.getByText("200.00")).toBeDefined();
      expect(screen.getByText("99,200.00")).toBeDefined();
      expect(screen.queryByText("No loan schedule is configured.")).toBeNull();
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = realAccounts;
    }
  });

  it("404s a loan schedule on a book that is not Personal", async () => {
    const Loans = (await import("./books/[book]/views/[view]/loans/page")).default;
    await expect(
      Loans({
        params: params({ book: FUND, view: VIEW }),
        searchParams: params({ period: "2026-03" }),
      }),
    ).rejects.toThrow();
  });

  it("cites an unset net-worth bridge rather than a measured zero", async () => {
    const Bridge = (await import("./books/[book]/views/[view]/bridge/page")).default;
    const real = wire.listAccounts;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/household/views/book/accounts/1",
          displayName: "Cash and bank",
          dimension: "1",
          type: "ASSET",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/2",
          displayName: "Investments",
          dimension: "2",
          type: "ASSET",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/30",
          displayName: "Income",
          dimension: "30",
          type: "REVENUE",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as unknown as typeof wire.listAccounts;
    try {
      await renderAsync(
        Bridge({
          params: params({ book: "household", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(
        screen.getByText(/Beginning and ending stay unset — not a measured zero/),
      ).toBeDefined();
      expect(screen.getByLabelText("Net-worth bridge")).toBeDefined();
      expect(screen.getAllByText("—").length).toBeGreaterThan(2);
      expect(screen.queryByText("0.00")).toBeNull();
      expect(screen.getByText("no purchase account distinct from a transfer")).toBeDefined();
    } finally {
      wire.listAccounts = real;
    }
  });

  it("cites ΔNW against income and expense and leaves principal off the identity", async () => {
    const Bridge = (await import("./books/[book]/views/[view]/bridge/page")).default;
    const realBook = wire.getBook;
    const realAccounts = wire.listAccounts;
    wire.getBook = (async () => ({
      ...bookFixture,
      loans: [{ dimension: "41", interest: "12" }],
    })) as unknown as typeof wire.getBook;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/household/views/book/accounts/1",
          displayName: "Cash and bank",
          dimension: "1",
          type: "ASSET",
          debit: "3000",
          credit: "2100",
          balance: "10000900",
          abnormal: false,
          postingCount: "5",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/2",
          displayName: "Investments",
          dimension: "2",
          type: "ASSET",
          debit: "500",
          credit: "0",
          balance: "500",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/41",
          displayName: "Mortgage",
          dimension: "41",
          type: "LIABILITY",
          debit: "800",
          credit: "0",
          balance: "-9999200",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/30",
          displayName: "Income",
          dimension: "30",
          type: "REVENUE",
          debit: "0",
          credit: "3000",
          balance: "-3000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/10",
          displayName: "Living expenses",
          dimension: "10",
          type: "EXPENSE",
          debit: "600",
          credit: "0",
          balance: "600",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/12",
          displayName: "Mortgage interest",
          dimension: "12",
          type: "EXPENSE",
          debit: "200",
          credit: "0",
          balance: "200",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/40",
          displayName: "Credit cards",
          dimension: "40",
          type: "LIABILITY",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as unknown as typeof wire.listAccounts;
    try {
      await renderAsync(
        Bridge({
          params: params({ book: "household", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(screen.getByLabelText("Net-worth bridge")).toBeDefined();
      expect(screen.getByText("beginning plus income minus expenses")).toBeDefined();
      expect(screen.getAllByText("0.00").length).toBeGreaterThan(0);
      expect(screen.getByText("30.00")).toBeDefined();
      expect(screen.getAllByText("8.00").length).toBe(2);
      expect(screen.getAllByText("22.00").length).toBeGreaterThanOrEqual(1);
      expect(screen.getByText("5.00")).toBeDefined();
      expect(screen.getByText("no purchase account distinct from a transfer")).toBeDefined();
      expect(
        screen.getByText(/Principal, transfers and purchases move the sheet, not net worth/),
      ).toBeDefined();
      expect(screen.queryByText(/Beginning and ending stay unset/)).toBeNull();
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = realAccounts;
    }
  });

  it("asks ListAccounts for a period window on the net-worth bridge", async () => {
    const calls: unknown[][] = [];
    const real = wire.listAccounts;
    wire.listAccounts = (async (...args: unknown[]) => {
      calls.push(args);
      return { accounts: [], nextPageToken: "" };
    }) as typeof wire.listAccounts;
    try {
      const Bridge = (await import("./books/[book]/views/[view]/bridge/page")).default;
      await renderAsync(
        Bridge({
          params: params({ book: "household", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(calls[0]?.slice(1)).toEqual(["household", "book", "bridge", "2026-03"]);
    } finally {
      wire.listAccounts = real;
    }
  });

  it("404s a net-worth bridge on a book that is not Personal", async () => {
    const Bridge = (await import("./books/[book]/views/[view]/bridge/page")).default;
    await expect(
      Bridge({
        params: params({ book: FUND, view: VIEW }),
        searchParams: params({ period: "2026-03" }),
      }),
    ).rejects.toThrow();
  });

  it("cites an unset cash-flow rather than a measured zero cash", async () => {
    const CashFlow = (await import("./books/[book]/views/[view]/cashflow/page")).default;
    const real = wire.listAccounts;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/household/views/book/accounts/1",
          displayName: "Cash and bank",
          dimension: "1",
          type: "ASSET",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/2",
          displayName: "Investments",
          dimension: "2",
          type: "ASSET",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/30",
          displayName: "Income",
          dimension: "30",
          type: "REVENUE",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as unknown as typeof wire.listAccounts;
    try {
      await renderAsync(
        CashFlow({
          params: params({ book: "household", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(
        screen.getByText(/Beginning and ending stay unset — not a measured zero cash/),
      ).toBeDefined();
      expect(screen.getByLabelText("Cash flow")).toBeDefined();
      expect(screen.getAllByText("—").length).toBeGreaterThan(2);
      expect(screen.queryByText("0.00")).toBeNull();
      expect(screen.getByText("no purchase account distinct from a transfer")).toBeDefined();
    } finally {
      wire.listAccounts = real;
    }
  });

  it("cites an unset period close rather than a measured zero close", async () => {
    const Close = (await import("./books/[book]/views/[view]/close/page")).default;
    const realAccounts = wire.listAccounts;
    const realCloses = wire.listPeriodCloses;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/household/views/book/accounts/25",
          displayName: "Retained earnings",
          dimension: "25",
          type: "EQUITY",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/30",
          displayName: "Income",
          dimension: "30",
          type: "REVENUE",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as unknown as typeof wire.listAccounts;
    wire.listPeriodCloses = (async () => ({
      periodCloses: [],
      nextPageToken: "",
    })) as typeof wire.listPeriodCloses;
    try {
      await renderAsync(
        Close({
          params: params({ book: "household", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(
        screen.getByText(/Beginning and ending stay unset — not a measured zero close/),
      ).toBeDefined();
      expect(screen.getByLabelText("Period close")).toBeDefined();
      expect(screen.getByText(/provisional — not a closing entry/)).toBeDefined();
      expect(screen.getByText(/unset — no named closing adjustment this window/)).toBeDefined();
      expect(screen.getAllByText("—").length).toBeGreaterThan(2);
    } finally {
      wire.listAccounts = realAccounts;
      wire.listPeriodCloses = realCloses;
    }
  });

  it("asks ListAccounts for a close window and lists period closes", async () => {
    const calls: unknown[][] = [];
    const real = wire.listAccounts;
    const realCloses = wire.listPeriodCloses;
    wire.listAccounts = (async (...args: unknown[]) => {
      calls.push(args);
      return { accounts: [], nextPageToken: "" };
    }) as typeof wire.listAccounts;
    wire.listPeriodCloses = (async () => ({
      periodCloses: [],
      nextPageToken: "",
    })) as typeof wire.listPeriodCloses;
    try {
      const Close = (await import("./books/[book]/views/[view]/close/page")).default;
      await renderAsync(
        Close({
          params: params({ book: "household", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(calls[0]?.slice(1)).toEqual(["household", "book", "close", "2026-03"]);
    } finally {
      wire.listAccounts = real;
      wire.listPeriodCloses = realCloses;
    }
  });

  it("cites operating, investing and financing and leaves the cash tie visible", async () => {
    const CashFlow = (await import("./books/[book]/views/[view]/cashflow/page")).default;
    const realBook = wire.getBook;
    const realAccounts = wire.listAccounts;
    wire.getBook = (async () => ({
      ...bookFixture,
      loans: [{ dimension: "41", interest: "12" }],
    })) as unknown as typeof wire.getBook;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/household/views/book/accounts/1",
          displayName: "Cash and bank",
          dimension: "1",
          type: "ASSET",
          debit: "3000",
          credit: "2100",
          balance: "10000900",
          abnormal: false,
          postingCount: "5",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/2",
          displayName: "Investments",
          dimension: "2",
          type: "ASSET",
          debit: "500",
          credit: "0",
          balance: "500",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/41",
          displayName: "Mortgage",
          dimension: "41",
          type: "LIABILITY",
          debit: "800",
          credit: "0",
          balance: "-9999200",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/30",
          displayName: "Income",
          dimension: "30",
          type: "REVENUE",
          debit: "0",
          credit: "3000",
          balance: "-3000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/10",
          displayName: "Living expenses",
          dimension: "10",
          type: "EXPENSE",
          debit: "600",
          credit: "0",
          balance: "600",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/12",
          displayName: "Mortgage interest",
          dimension: "12",
          type: "EXPENSE",
          debit: "200",
          credit: "0",
          balance: "200",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/40",
          displayName: "Credit cards",
          dimension: "40",
          type: "LIABILITY",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/20",
          displayName: "Opening equity",
          dimension: "20",
          type: "EQUITY",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as unknown as typeof wire.listAccounts;
    try {
      await renderAsync(
        CashFlow({
          params: params({ book: "household", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(screen.getByLabelText("Cash flow")).toBeDefined();
      expect(
        screen.getByText("beginning plus operating plus investing plus financing"),
      ).toBeDefined();
      expect(screen.getByText("100,000.00")).toBeDefined();
      expect(screen.getByText("100,009.00")).toBeDefined();
      expect(screen.getByText("9.00")).toBeDefined();
      expect(screen.getByText("30.00")).toBeDefined();
      expect(screen.getByText("22.00")).toBeDefined();
      expect(screen.getAllByText("-5.00").length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText("-8.00").length).toBeGreaterThanOrEqual(1);
      expect(screen.getByText("no purchase account distinct from a transfer")).toBeDefined();
      expect(
        screen.getByText(/Inflow is positive, outflow is negative — cash, not net worth/),
      ).toBeDefined();
      expect(screen.queryByText(/Beginning and ending stay unset/)).toBeNull();
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = realAccounts;
    }
  });

  it("names unnamed loan activity on cash-flow instead of absorbing it", async () => {
    const CashFlow = (await import("./books/[book]/views/[view]/cashflow/page")).default;
    const realBook = wire.getBook;
    const realAccounts = wire.listAccounts;
    wire.getBook = (async () => ({
      ...bookFixture,
      loans: [],
    })) as unknown as typeof wire.getBook;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/household/views/book/accounts/1",
          displayName: "Cash and bank",
          dimension: "1",
          type: "ASSET",
          debit: "0",
          credit: "800",
          balance: "99200",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/household/views/book/accounts/41",
          displayName: "Mortgage",
          dimension: "41",
          type: "LIABILITY",
          debit: "800",
          credit: "0",
          balance: "-99200",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as unknown as typeof wire.listAccounts;
    try {
      await renderAsync(
        CashFlow({
          params: params({ book: "household", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(screen.getByText("Unclassified")).toBeDefined();
      expect(screen.getByRole("link", { name: "Mortgage" })).toBeDefined();
      expect(
        screen.getByText("not a named loan and not a transfer — open the account"),
      ).toBeDefined();
      expect(
        screen.getAllByText("no [personal.loan] on the configuration in force").length,
      ).toBeGreaterThanOrEqual(1);
    } finally {
      wire.getBook = realBook;
      wire.listAccounts = realAccounts;
    }
  });

  it("asks ListAccounts for a period window on the cash-flow statement", async () => {
    const calls: unknown[][] = [];
    const real = wire.listAccounts;
    wire.listAccounts = (async (...args: unknown[]) => {
      calls.push(args);
      return { accounts: [], nextPageToken: "" };
    }) as typeof wire.listAccounts;
    try {
      const CashFlow = (await import("./books/[book]/views/[view]/cashflow/page")).default;
      await renderAsync(
        CashFlow({
          params: params({ book: "household", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(calls[0]?.slice(1)).toEqual(["household", "book", "cashflow", "2026-03"]);
    } finally {
      wire.listAccounts = real;
    }
  });

  it("404s a cash-flow statement on a book that is not Personal or Operating", async () => {
    const CashFlow = (await import("./books/[book]/views/[view]/cashflow/page")).default;
    await expect(
      CashFlow({
        params: params({ book: FUND, view: VIEW }),
        searchParams: params({ period: "2026-03" }),
      }),
    ).rejects.toThrow();
  });

  it("cites billed vs earned, retainage, and cost by phase without a fake zero", async () => {
    const Billing = (await import("./books/[book]/views/[view]/billing/page"))
      .default;
    await renderAsync(
      Billing({ params: params({ book: "bridge", view: "book" }) }),
    );
    expect(screen.getByText("Billed to date")).toBeDefined();
    expect(screen.getByText("1,000.00")).toBeDefined();
    expect(screen.getByText("Earned to date")).toBeDefined();
    expect(screen.getByText("800.00")).toBeDefined();
    expect(screen.getByText("200.00")).toBeDefined();
    expect(screen.getByText("Retainage outstanding")).toBeDefined();
    expect(screen.getByText("100.00")).toBeDefined();
    const payable = screen.getByText("Payable").closest("[role=row]");
    expect(payable?.textContent).toContain("—");
    expect(screen.getByText("Site and mobilization")).toBeDefined();
    expect(screen.getByText("authorized 4,000.00 — no approved change order")).toBeDefined();
    expect(screen.getByText("250.00")).toBeDefined();
    expect(screen.getAllByText("budget unset — not a silent zero").length).toBeGreaterThan(0);
    const site = screen.getByText("Site and mobilization").closest("a");
    expect(site?.getAttribute("href")).toBe(
      "/books/bridge/views/book/accounts/11",
    );
    expect(screen.getByText("Original contract")).toBeDefined();
    expect(screen.getByText("Billing basis")).toBeDefined();
    expect(
      screen.getByText(/unset — no approved change order has posted, not a silent zero/),
    ).toBeDefined();
    expect(screen.getByText("Remaining to bill")).toBeDefined();
    expect(
      screen.getByText(/unset until \[project\] budget is set — not a priced remainder/),
    ).toBeDefined();
    expect(screen.getByText("Collections vs billed")).toBeDefined();
    expect(screen.getByText("Collected")).toBeDefined();
    expect(
      screen.getByText(/unset — accounts receivable has not posted, so cash against AR cannot be cited/),
    ).toBeDefined();
    const remaining = screen.getByText("Remaining to bill").closest("[role=row]");
    expect(remaining?.textContent).toContain("—");
    expect(remaining?.textContent).not.toMatch(/0\.00/);
    const collected = screen.getByText("Collected").closest("[role=row]");
    expect(collected?.textContent).toContain("—");
    expect(collected?.textContent).not.toMatch(/0\.00/);
  });

  it("keeps billed-minus-earned unset when either side has not posted", async () => {
    const real = wire.projectProgress;
    wire.projectProgress = (async () => ({
      ...projectProgressFixture,
      billed: "100000",
      earned: "",
      billedMinusEarned: "",
      retainageReceivable: "",
      retainagePayable: "",
    })) as typeof wire.projectProgress;
    try {
      const Billing = (await import("./books/[book]/views/[view]/billing/page"))
        .default;
      await renderAsync(
        Billing({ params: params({ book: "bridge", view: "book" }) }),
      );
      expect(
        screen.getByText(/unset until both billed and earned have posted/),
      ).toBeDefined();
      const variance = screen.getByText("Billed minus earned").closest("[role=row]");
      expect(variance?.textContent).toContain("—");
      expect(variance?.textContent).not.toMatch(/0\.00/);
    } finally {
      wire.projectProgress = real;
    }
  });

  it("keys a phase change order to cost-by-phase rather than a lump CO bucket", async () => {
    const realAccounts = wire.listAccounts;
    const realBook = wire.getBook;
    const bridge = booksFixture.books.find((b) => b.kind === "PROJECT")!;
    wire.getBook = (async () => ({
      ...bridge,
      budget: "10000000",
    })) as typeof wire.getBook;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/bridge/views/book/accounts/26",
          displayName: "Approved change orders — Site and mobilization",
          dimension: "26",
          type: "EQUITY",
          debit: "0",
          credit: "50000",
          balance: "-50000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as typeof wire.listAccounts;
    try {
      const Billing = (await import("./books/[book]/views/[view]/billing/page"))
        .default;
      await renderAsync(
        Billing({ params: params({ book: "bridge", view: "book" }) }),
      );
      expect(
        screen.getByText(/original 4,000.00 · approved changes 500.00 · revised 4,500.00/),
      ).toBeDefined();
      expect(screen.getByText("Billing basis")).toBeDefined();
      expect(screen.getByText("100,500.00")).toBeDefined();
      expect(screen.getByText("Remaining to bill")).toBeDefined();
      expect(screen.getByText("99,500.00")).toBeDefined();
      expect(
        screen.getByText(/revised minus billed — the citeable leftover/),
      ).toBeDefined();
    } finally {
      wire.listAccounts = realAccounts;
      wire.getBook = realBook;
    }
  });

  it("keeps remaining-to-bill unset when billed has not posted, even with a revised contract", async () => {
    const realProgress = wire.projectProgress;
    const realBook = wire.getBook;
    const bridge = booksFixture.books.find((b) => b.kind === "PROJECT")!;
    wire.getBook = (async () => ({
      ...bridge,
      budget: "10000000",
    })) as typeof wire.getBook;
    wire.projectProgress = (async () => ({
      ...projectProgressFixture,
      billed: "",
      earned: "",
      billedMinusEarned: "",
      retainageReceivable: "",
      retainagePayable: "",
    })) as typeof wire.projectProgress;
    try {
      const Billing = (await import("./books/[book]/views/[view]/billing/page"))
        .default;
      await renderAsync(
        Billing({ params: params({ book: "bridge", view: "book" }) }),
      );
      expect(
        screen.getByText(/unset until a progress bill posts — not the whole contract as a fake remainder/),
      ).toBeDefined();
      const remaining = screen.getByText("Remaining to bill").closest("[role=row]");
      expect(remaining?.textContent).toContain("—");
      expect(remaining?.textContent).not.toMatch(/100,000\.00/);
      expect(remaining?.textContent).not.toMatch(/0\.00/);
      expect(
        screen.getByText(/unset until a progress bill posts — not a fake zero collected/),
      ).toBeDefined();
      const collected = screen.getByText("Collected").closest("[role=row]");
      expect(collected?.textContent).toContain("—");
      expect(collected?.textContent).not.toMatch(/0\.00/);
    } finally {
      wire.projectProgress = realProgress;
      wire.getBook = realBook;
    }
  });

  it("cites collections vs billed as cash against AR when the journal can support the cut", async () => {
    const realAccounts = wire.listAccounts;
    const realBook = wire.getBook;
    const bridge = booksFixture.books.find((b) => b.kind === "PROJECT")!;
    wire.getBook = (async () => ({
      ...bridge,
      budget: "10000000",
    })) as typeof wire.getBook;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/bridge/views/book/accounts/3",
          displayName: "Accounts receivable",
          dimension: "3",
          type: "ASSET",
          debit: "100000",
          credit: "60000",
          balance: "40000",
          abnormal: false,
          postingCount: "2",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as typeof wire.listAccounts;
    try {
      const Billing = (await import("./books/[book]/views/[view]/billing/page"))
        .default;
      await renderAsync(
        Billing({ params: params({ book: "bridge", view: "book" }) }),
      );
      // billed 1,000.00 − AR 400.00 − retainage 100.00 = collected 500.00
      const collectedRow = screen.getByText("Collected").closest("[role=row]");
      expect(collectedRow?.textContent).toContain("500.00");
      const outstandingRow = screen
        .getByText("Outstanding receivable")
        .closest("[role=row]");
      expect(outstandingRow?.textContent).toContain("500.00");
      expect(
        screen.getByText(/AR plus retainage receivable — the uncollected billed/),
      ).toBeDefined();
      expect(screen.getByText("Remaining to bill")).toBeDefined();
      // revised 100,000.00 − billed 1,000.00
      expect(screen.getByText("99,000.00")).toBeDefined();
    } finally {
      wire.listAccounts = realAccounts;
      wire.getBook = realBook;
    }
  });

  it("shows a real zero collected when the job is billed and nothing has come in", async () => {
    const realAccounts = wire.listAccounts;
    const realProgress = wire.projectProgress;
    wire.projectProgress = (async () => ({
      ...projectProgressFixture,
      retainageReceivable: "",
    })) as typeof wire.projectProgress;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/bridge/views/book/accounts/3",
          displayName: "Accounts receivable",
          dimension: "3",
          type: "ASSET",
          debit: "100000",
          credit: "0",
          balance: "100000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as typeof wire.listAccounts;
    try {
      const Billing = (await import("./books/[book]/views/[view]/billing/page"))
        .default;
      await renderAsync(
        Billing({ params: params({ book: "bridge", view: "book" }) }),
      );
      const collectedRow = screen.getByText("Collected").closest("[role=row]");
      expect(collectedRow?.textContent).toContain("0.00");
      expect(collectedRow?.textContent).toMatch(/cash against AR/);
      const outstandingRow = screen
        .getByText("Outstanding receivable")
        .closest("[role=row]");
      expect(outstandingRow?.textContent).toContain("1,000.00");
    } finally {
      wire.listAccounts = realAccounts;
      wire.projectProgress = realProgress;
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
    expect(screen.getByRole("link", { name: "Net-worth bridge" }).getAttribute("href")).toBe(
      "/books/household/views/hearth/bridge",
    );
    expect(screen.getByRole("link", { name: "Cash flow" }).getAttribute("href")).toBe(
      "/books/household/views/hearth/cashflow",
    );
    expect(screen.getByRole("link", { name: "Period close" }).getAttribute("href")).toBe(
      "/books/household/views/hearth/close",
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
        "Credit cards",
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

describe("an operating-business statement", () => {
  async function withOperatingAccounts<T>(fn: () => Promise<T>): Promise<T> {
    const real = wire.listAccounts;
    wire.listAccounts = (async () =>
      operatingAccountsFixture) as typeof wire.listAccounts;
    try {
      return await fn();
    } finally {
      wire.listAccounts = real;
    }
  }

  it("renders chart_for(Operating) on a citable balance sheet", async () => {
    await withOperatingAccounts(async () => {
      const Sheet = (await import("./books/[book]/views/[view]/sheet/page")).default;
      await renderAsync(
        Sheet({
          params: params({ book: "studio", view: "book" }),
          searchParams: params({}),
        }),
      );
      expect(screen.getByLabelText("Balance sheet")).toBeDefined();
      for (const name of [
        "Cash",
        "Accounts receivable",
        "Accounts payable",
        "Owner equity",
      ]) {
        expect(screen.getByText(name)).toBeDefined();
      }
      expect(screen.queryByText("Cash and bank")).toBeNull();
      expect(screen.queryByText("Living expenses")).toBeNull();
      expect(screen.queryByText("Work in progress")).toBeNull();
      expect(screen.getByRole("link", { name: "Income statement" })).toBeDefined();
      expect(screen.getByRole("link", { name: "Cash flow" })).toBeDefined();
      expect(screen.queryByRole("link", { name: "Transfer" })).toBeNull();
      expect(screen.getByRole("link", { name: "AR/AP aging" })).toBeDefined();
      expect(screen.getByText(/control-account balances/)).toBeDefined();
      expect(screen.getByText(/Assets equal liabilities, equity and surplus/)).toBeDefined();
    });
  });

  it("renders a period income statement and says it is not since inception", async () => {
    await withOperatingAccounts(async () => {
      const PnL = (await import("./books/[book]/views/[view]/pnl/page")).default;
      await renderAsync(
        PnL({
          params: params({ book: "studio", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(screen.getByLabelText("Period income statement")).toBeDefined();
      expect(screen.getByText("Operating expenses")).toBeDefined();
      expect(screen.getAllByText("Operating revenue").length).toBeGreaterThan(0);
      expect(screen.getByText(/not since inception/)).toBeDefined();
      expect(screen.queryByText("Cash")).toBeNull();
      expect(screen.getByRole("link", { name: "Balance sheet" })).toBeDefined();
      expect(screen.getByRole("link", { name: "Cash flow" })).toBeDefined();
      expect(screen.queryByRole("link", { name: "Transfer" })).toBeNull();
    });
  });

  it("cites an unset operating cash-flow rather than a measured zero cash", async () => {
    const CashFlow = (await import("./books/[book]/views/[view]/cashflow/page")).default;
    const real = wire.listAccounts;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/studio/views/book/accounts/1",
          displayName: "Cash",
          dimension: "1",
          type: "ASSET",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/studio/views/book/accounts/2",
          displayName: "Accounts receivable",
          dimension: "2",
          type: "ASSET",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/studio/views/book/accounts/30",
          displayName: "Operating revenue",
          dimension: "30",
          type: "REVENUE",
          debit: "0",
          credit: "0",
          balance: "0",
          abnormal: false,
          postingCount: "0",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as unknown as typeof wire.listAccounts;
    try {
      await renderAsync(
        CashFlow({
          params: params({ book: "studio", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(
        screen.getByText(/Beginning and ending stay unset — not a measured zero cash/),
      ).toBeDefined();
      expect(screen.getByLabelText("Cash flow")).toBeDefined();
      expect(screen.getAllByText("—").length).toBeGreaterThan(2);
      expect(screen.queryByText("0.00")).toBeNull();
      expect(screen.getByText(/no investing account on this chart/)).toBeDefined();
      expect(screen.queryByText("Credit cards")).toBeNull();
      expect(screen.queryByText("Loan draws")).toBeNull();
    } finally {
      wire.listAccounts = real;
    }
  });

  it("cites operating and financing and leaves investing unset", async () => {
    const CashFlow = (await import("./books/[book]/views/[view]/cashflow/page")).default;
    const real = wire.listAccounts;
    wire.listAccounts = (async () => ({
      accounts: [
        {
          name: "funds/studio/views/book/accounts/1",
          displayName: "Cash",
          dimension: "1",
          type: "ASSET",
          debit: "25000",
          credit: "10000",
          balance: "115000",
          abnormal: false,
          postingCount: "5",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/studio/views/book/accounts/2",
          displayName: "Accounts receivable",
          dimension: "2",
          type: "ASSET",
          debit: "40000",
          credit: "15000",
          balance: "25000",
          abnormal: false,
          postingCount: "2",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/studio/views/book/accounts/10",
          displayName: "Operating expenses",
          dimension: "10",
          type: "EXPENSE",
          debit: "10000",
          credit: "0",
          balance: "10000",
          abnormal: false,
          postingCount: "2",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/studio/views/book/accounts/20",
          displayName: "Owner equity",
          dimension: "20",
          type: "EQUITY",
          debit: "5000",
          credit: "0",
          balance: "-95000",
          abnormal: false,
          postingCount: "1",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/studio/views/book/accounts/30",
          displayName: "Operating revenue",
          dimension: "30",
          type: "REVENUE",
          debit: "0",
          credit: "50000",
          balance: "-50000",
          abnormal: false,
          postingCount: "2",
          currencyTotals: [],
          units: "",
        },
        {
          name: "funds/studio/views/book/accounts/40",
          displayName: "Accounts payable",
          dimension: "40",
          type: "LIABILITY",
          debit: "3000",
          credit: "8000",
          balance: "-5000",
          abnormal: false,
          postingCount: "2",
          currencyTotals: [],
          units: "",
        },
      ],
      nextPageToken: "",
    })) as unknown as typeof wire.listAccounts;
    try {
      await renderAsync(
        CashFlow({
          params: params({ book: "studio", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(screen.getByLabelText("Cash flow")).toBeDefined();
      expect(
        screen.getByText("beginning plus operating plus financing"),
      ).toBeDefined();
      expect(screen.getByText("1,000.00")).toBeDefined();
      expect(screen.getByText("1,150.00")).toBeDefined();
      expect(screen.getByText("150.00")).toBeDefined();
      expect(screen.getByText("500.00")).toBeDefined();
      expect(screen.getByText("200.00")).toBeDefined();
      // ⛔ BOTH THE OWNER-EQUITY PLUG AND THE FINANCING FOOT ARE -50.00.
      // getByText("-50.00") finds both; the right figure is the one on
      // its row. Investing stays unset on this chart — not a silent 0.00.
      const owner = screen.getByText("Owner equity").closest("[role=row]");
      expect(owner?.textContent).toContain("-50.00");
      const financing = screen
        .getAllByRole("row")
        .find((r) => r.className.includes("tbfoot") && r.textContent?.includes("Financing"));
      expect(financing?.textContent).toContain("-50.00");
      const investing = screen
        .getAllByRole("row")
        .find((r) => r.className.includes("tbfoot") && r.textContent?.includes("Investing"));
      expect(investing?.textContent).toContain("—");
      expect(investing?.textContent).not.toMatch(/0\.00/);
      expect(screen.getByText("working capital — an invoice is not a cash inflow")).toBeDefined();
      expect(screen.getByText("working capital — a vendor bill is not a cash outflow")).toBeDefined();
      expect(screen.getByText(/no investing account on this chart/)).toBeDefined();
      expect(screen.getByText("Owner equity")).toBeDefined();
      expect(screen.queryByText("Credit cards")).toBeNull();
      expect(screen.queryByText("Net-worth bridge")).toBeNull();
      expect(screen.getByRole("link", { name: "Income statement" })).toBeDefined();
      expect(screen.queryByText(/Beginning and ending stay unset/)).toBeNull();
    } finally {
      wire.listAccounts = real;
    }
  });

  it("asks ListAccounts for a period window on the operating cash-flow statement", async () => {
    const calls: unknown[][] = [];
    const real = wire.listAccounts;
    wire.listAccounts = (async (...args: unknown[]) => {
      calls.push(args);
      return { accounts: [], nextPageToken: "" };
    }) as typeof wire.listAccounts;
    try {
      const CashFlow = (await import("./books/[book]/views/[view]/cashflow/page")).default;
      await renderAsync(
        CashFlow({
          params: params({ book: "studio", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(calls[0]?.slice(1)).toEqual(["studio", "book", "cashflow", "2026-03"]);
    } finally {
      wire.listAccounts = real;
    }
  });

  it("asks ListAccounts for a period window on the income statement", async () => {
    const calls: unknown[][] = [];
    const real = wire.listAccounts;
    wire.listAccounts = (async (...args: unknown[]) => {
      calls.push(args);
      return operatingAccountsFixture;
    }) as typeof wire.listAccounts;
    try {
      const PnL = (await import("./books/[book]/views/[view]/pnl/page")).default;
      await renderAsync(
        PnL({
          params: params({ book: "studio", view: "book" }),
          searchParams: params({ period: "2026-03" }),
        }),
      );
      expect(calls[0]?.slice(1)).toEqual(["studio", "book", "pnl", "2026-03"]);
    } finally {
      wire.listAccounts = real;
    }
  });

  it("cites aged AR/AP open items and says the buckets sum to the control", async () => {
    const Aging = (await import("./books/[book]/views/[view]/aging/page")).default;
    await renderAsync(
      Aging({
        params: params({ book: "studio", view: "book" }),
        searchParams: params({}),
      }),
    );
    expect(screen.getByLabelText("AR/AP aging")).toBeDefined();
    expect(screen.getByText("Accounts receivable")).toBeDefined();
    expect(screen.getByText("Accounts payable")).toBeDefined();
    expect(screen.getByText("300.00")).toBeDefined();
    expect(screen.getByText("200.00")).toBeDefined();
    expect(screen.getByText("500.00")).toBeDefined();
    expect(screen.getAllByText("80.00").length).toBeGreaterThan(0);
    expect(screen.getAllByText("buckets sum to the control").length).toBeGreaterThan(0);
    expect(screen.getByText(/A missing due date is not current/)).toBeDefined();
    expect(screen.getByText(/not Project billing/)).toBeDefined();
    expect(screen.getByRole("link", { name: "Balance sheet" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Trial balance" })).toBeDefined();
    expect(screen.queryByRole("link", { name: "Billing" })).toBeNull();
  });

  it("shows an em dash when aging cannot be cited, not a current-bucket zero", async () => {
    const real = wire.operatingAging;
    wire.operatingAging = (async () => ({
      name: "funds/studio/views/book",
      receivable: {
        current: "",
        daysThirty: "",
        daysSixty: "",
        daysNinety: "",
        daysOlder: "",
        undated: "",
        control: "40000",
      },
      payable: {
        current: "",
        daysThirty: "",
        daysSixty: "",
        daysNinety: "",
        daysOlder: "",
        undated: "",
        control: "",
      },
      journalPosition: "1",
    })) as typeof wire.operatingAging;
    try {
      const Aging = (await import("./books/[book]/views/[view]/aging/page")).default;
      await renderAsync(
        Aging({
          params: params({ book: "studio", view: "book" }),
          searchParams: params({}),
        }),
      );
      expect(screen.getAllByText("unset — not current").length).toBeGreaterThan(0);
      expect(screen.getByText("400.00")).toBeDefined();
      expect(screen.queryByText("0.00")).toBeNull();
    } finally {
      wire.operatingAging = real;
    }
  });

  it("asks OperatingAging for the as-of cut the chips name", async () => {
    const calls: unknown[][] = [];
    const real = wire.operatingAging;
    wire.operatingAging = (async (...args: unknown[]) => {
      calls.push(args);
      return operatingAgingFixture;
    }) as typeof wire.operatingAging;
    try {
      const Aging = (await import("./books/[book]/views/[view]/aging/page")).default;
      await renderAsync(
        Aging({
          params: params({ book: "studio", view: "book" }),
          searchParams: params({ period: "2026-04-15" }),
        }),
      );
      expect(calls[0]?.slice(1)).toEqual(["studio", "book", "2026-04-15"]);
    } finally {
      wire.operatingAging = real;
    }
  });

  it("404s aging on a household rather than wearing an operating label", async () => {
    const Aging = (await import("./books/[book]/views/[view]/aging/page")).default;
    await expect(
      Aging({
        params: params({ book: "household", view: "book" }),
        searchParams: params({}),
      }),
    ).rejects.toThrow();
  });
});

describe("a journal entry", () => {
  it("lists the journal and links each row to the entry", async () => {
    const Entries = (await import("./books/[book]/entries/page")).default;
    await renderAsync(Entries({ params: params({ book: FUND }) }));
    expect(screen.getByLabelText("Journal")).toBeDefined();
    expect(screen.getByText("journal order")).toBeDefined();
    expect(
      screen.getByRole("link", { name: /Purchase of 1,000 ACME/ }).getAttribute("href"),
    ).toBe(`/books/${FUND}/entries/e-0004`);
    for (const a of document.querySelectorAll("a[href]")) {
      expect(a.getAttribute("href")).not.toMatch(/\/funds\//);
    }
  });

  it("gives an entry a page of its own", async () => {
    // ⭐ #52. The posting screen printed `entry {id}` as text because this
    // URL 404'd. The page is the citation — the memo, the configuration,
    // the postings — and a render without the layout still has to show them.
    const Entry = (await import("./books/[book]/entries/[entry]/page")).default;
    await renderAsync(Entry({ params: params({ book: FUND, entry: "e-0004" }) }));
    expect(screen.getByRole("heading", { name: "Purchase of 1,000 ACME" })).toBeDefined();
    expect(screen.getByLabelText("Journal entry")).toBeDefined();
    expect(screen.getByText("The postings it produced")).toBeDefined();
    expect(screen.getByText("Identified lots")).toBeDefined();
    expect(screen.getByText(/this sale does not name lots/)).toBeDefined();
    expect(screen.queryByText(/named on this sale/)).toBeNull();
    expect(screen.getByText("Cash and equivalents")).toBeDefined();
    expect(screen.getByText("-332,880.00")).toBeDefined();
    expect(
      screen.getByRole("link", { name: "Cash and equivalents" }).getAttribute("href"),
    ).toBe(`/books/${FUND}/views/${VIEW}/accounts/1010`);
    expect(screen.getByRole("link", { name: "Journal" }).getAttribute("href")).toBe(
      `/books/${FUND}/entries`,
    );
    expect(screen.getByRole("link", { name: "Book" }).getAttribute("href")).toBe(
      `/books/${FUND}`,
    );
    for (const a of document.querySelectorAll("a[href]")) {
      expect(a.getAttribute("href")).not.toMatch(/\/funds\//);
    }
  });

  it("cites named lots on a SpecID sale and does not invent FIFO", async () => {
    const real = wire.getEntry;
    wire.getEntry = (async () => ({
      ...entryFixture,
      memo: "SpecID walk-through sale",
      identifiedLots: ["3"],
      identifiedLotsDeclared: true,
    })) as typeof wire.getEntry;
    try {
      const Entry = (await import("./books/[book]/entries/[entry]/page")).default;
      await renderAsync(Entry({ params: params({ book: FUND, entry: "e-0004" }) }));
      expect(screen.getByText("Identified lots")).toBeDefined();
      expect(screen.getByText("3")).toBeDefined();
      expect(screen.getByText(/named on this sale — not a lot method/)).toBeDefined();
      expect(screen.queryByText(/this sale does not name lots/)).toBeNull();
    } finally {
      wire.getEntry = real;
    }
  });

  it("links posting provenance to the entry, not to plain text", async () => {
    const Posting = (
      await import("./books/[book]/views/[view]/accounts/[account]/postings/[posting]/page")
    ).default;
    await renderAsync(
      Posting({
        params: params({
          book: FUND,
          view: VIEW,
          account: "1010",
          posting: "e-0004.0",
        }),
      }),
    );
    const link = screen.getByRole("link", { name: "entry e-0004" });
    expect(link.getAttribute("href")).toBe(`/books/${FUND}/entries/e-0004`);
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
    // ⭐ EUR opens the rate fact the translation cited. USD has no rate fact —
    // a fund does not record what a dollar is worth in dollars — so it is not
    // a link. Take rateFact off the EUR row and this goes red.
    const eur = screen.getByRole("link", { name: "EUR" });
    expect(eur.getAttribute("href")).toBe(
      `/books/${FUND}/data/facts/aabbccddeeff0011-2`,
    );
    expect(screen.queryByRole("link", { name: "USD" })).toBeNull();
  });
});

describe("positions", () => {
  it("shows the open-lot count on the list", async () => {
    const Positions = (await import("./books/[book]/views/[view]/positions/page")).default;
    await renderAsync(Positions({ params: params({ book: FUND, view: VIEW }) }));
    expect(screen.getByText(/40 open lots/)).toBeDefined();
  });

  it("opens the price fact a marked position cites", async () => {
    const Position = (
      await import("./books/[book]/views/[view]/positions/[position]/page")
    ).default;
    await renderAsync(
      Position({ params: params({ book: FUND, view: VIEW, position: "ACME" }) }),
    );
    // ⛔ Take priceFact off the fixture and this goes red — a figure that
    // cannot be opened is a figure that can only be trusted.
    expect(screen.getByText(/Price from/)).toBeDefined();
    expect(
      screen.getByRole("link", { name: /aabbccddeeff/ }).getAttribute("href"),
    ).toBe(`/books/${FUND}/data/facts/aabbccddeeff0011-3`);
  });

  it("renders the lot book, and says when a lot has no trade date", async () => {
    const { LotBook } = await import("@/components/LotBook");
    await renderAsync(LotBook({ fund: FUND, view: VIEW, position: "ACME" }));
    expect(document.querySelectorAll(".lotrow").length).toBe(2);
    // A lot the engine cannot classify says so rather than showing a guess.
    expect(screen.getByText("no trade date")).toBeDefined();
  });
});

describe("the fact plane", () => {
  it("lists recorded facts and marks the one a correction superseded", async () => {
    const Facts = (await import("./books/[book]/data/facts/page")).default;
    await renderAsync(Facts({ params: params({ book: FUND }) }));
    expect(screen.getByText("EUR at 1.08")).toBeDefined();
    expect(screen.getByText("EUR at 1.07")).toBeDefined();
    expect(screen.getByText("superseded")).toBeDefined();
    expect(screen.getByText(/a correction is a new row/)).toBeDefined();
  });

  it("opens one fact with the config digest the ingest run pinned", async () => {
    const Fact = (await import("./books/[book]/data/facts/[fact]/page")).default;
    await renderAsync(Fact({ params: params({ book: FUND, fact: "aabbccddeeff0011-2" }) }));
    expect(screen.getByText("EUR at 1.08")).toBeDefined();
    expect(screen.getByText("fx")).toBeDefined();
    // ⛔ Take configDigest off the fixture and the config link vanishes.
    const first = factsFixture.facts[0];
    if (!first) throw new Error("facts fixture is empty");
    expect(
      screen.getByRole("link", { name: /9f2c1ab7de40/ }).getAttribute("href"),
    ).toBe(`/books/${FUND}/config/${first.configDigest}`);
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
    // Lot method and wash window share the elected-term claim on the
    // harbourline fixture (min-tax and average-cost stay unset). getByText
    // would fail the moment a second term is cited, which is the opposite
    // of what this assertion is for.
    expect(
      screen.getAllByText(/a term of the administration agreement/).length,
    ).toBe(2);
  });

  it("cites a declared wash window and leaves the US transfer in force", async () => {
    const Overview = (await import("./funds/[fund]/page")).default;
    await renderAsync(Overview({ params: params({ fund: FUND }) }));
    expect(screen.getByText("Wash window")).toBeDefined();
    // ⛔ THE DAYS, AND THE CLAIM THAT SOMEBODY WROTE THEM. A silent 30 is the
    // lot-method trap for this field: every in-window loss on every existing
    // book would have been restated.
    expect(screen.getByText("30 days")).toBeDefined();
    expect(screen.getByText("Wash holding period")).toBeDefined();
    expect(screen.getByText("US transfer stays in force")).toBeDefined();
    expect(screen.getByText(/nobody wrote keep/)).toBeDefined();
    expect(screen.queryByText("replacement keeps its own date")).toBeNull();
    expect(screen.queryByText(/declares no wash window/)).toBeNull();
  });

  it("does not invent a wash window when nobody elected one", async () => {
    const real = wire.getFund;
    wire.getFund = (async () => ({
      ...fundFixture,
      washWindowDays: "0",
      washWindowDeclared: false,
      washKeepHoldingPeriod: false,
    })) as typeof wire.getFund;
    try {
      const Overview = (await import("./funds/[fund]/page")).default;
      await renderAsync(Overview({ params: params({ fund: FUND }) }));
      expect(screen.getByText("Wash window")).toBeDefined();
      expect(screen.getByText(/this configuration declares no wash window/)).toBeDefined();
      expect(screen.queryByText("30 days")).toBeNull();
      expect(screen.queryByText("Wash holding period")).toBeNull();
      expect(screen.queryByText("US transfer stays in force")).toBeNull();
    } finally {
      wire.getFund = real;
    }
  });

  it("does not invent a min-tax weight when nobody elected one", async () => {
    const Overview = (await import("./funds/[fund]/page")).default;
    await renderAsync(Overview({ params: params({ fund: FUND }) }));
    expect(screen.getByText("Min-tax short weight")).toBeDefined();
    // ⛔ NOT A SILENT 2. The fixture leaves the weight unset, so the row
    // must say so — printing 2 over that is the lot-method trap again.
    expect(screen.getByText(/this configuration declares no min-tax weight/)).toBeDefined();
  });

  it("cites a declared min-tax weight and does not invent a silent two", async () => {
    const real = wire.getFund;
    wire.getFund = (async () => ({
      ...fundFixture,
      minTaxShortWeight: "2",
      minTaxDeclared: true,
    })) as typeof wire.getFund;
    try {
      const Overview = (await import("./funds/[fund]/page")).default;
      await renderAsync(Overview({ params: params({ fund: FUND }) }));
      expect(screen.getByText("Min-tax short weight").nextElementSibling?.textContent).toMatch(
        /^2/,
      );
      expect(screen.queryByText(/declares no min-tax weight/)).toBeNull();
      // Lot method, wash window, and the min-tax weight now share the claim.
      expect(
        screen.getAllByText(/a term of the administration agreement/).length,
      ).toBe(3);
    } finally {
      wire.getFund = real;
    }
  });

  it("does not invent an average-cost pool when nobody elected one", async () => {
    const Overview = (await import("./funds/[fund]/page")).default;
    await renderAsync(Overview({ params: params({ fund: FUND }) }));
    expect(screen.getByText("Average cost")).toBeDefined();
    expect(
      screen.getByText(/this configuration declares no average-cost pool/),
    ).toBeDefined();
    expect(screen.queryByText("pooled basis")).toBeNull();
  });

  it("cites the average-cost pool and does not invent a silent true", async () => {
    const real = wire.getFund;
    wire.getFund = (async () => ({
      ...fundFixture,
      averageCost: true,
    })) as typeof wire.getFund;
    try {
      const Overview = (await import("./funds/[fund]/page")).default;
      await renderAsync(Overview({ params: params({ fund: FUND }) }));
      expect(screen.getByText("pooled basis")).toBeDefined();
      expect(screen.queryByText(/declares no average-cost pool/)).toBeNull();
      expect(
        screen.getAllByText(/a term of the administration agreement/).length,
      ).toBe(3);
    } finally {
      wire.getFund = real;
    }
  });

  it("cites the keep-holding-period election and does not invent a third meaning", async () => {
    const real = wire.getFund;
    wire.getFund = (async () => ({
      ...fundFixture,
      washWindowDays: "30",
      washWindowDeclared: true,
      washKeepHoldingPeriod: true,
    })) as typeof wire.getFund;
    try {
      const Overview = (await import("./funds/[fund]/page")).default;
      await renderAsync(Overview({ params: params({ fund: FUND }) }));
      expect(screen.getByText("replacement keeps its own date")).toBeDefined();
      expect(screen.queryByText("US transfer stays in force")).toBeNull();
      expect(screen.queryByText(/nobody wrote keep/)).toBeNull();
    } finally {
      wire.getFund = real;
    }
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
    measured: false,
  };

  const XFER_CASH_INV = {
    name: "funds/household/rules/xfer_cash_investments",
    ruleId: "xfer_cash_investments",
    kind: "TRADE" as const,
    description: "Move cash to investments",
    form: "debit Investments, credit Cash and bank",
    accounts: ["Investments", "Cash and bank"],
    measured: false,
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
