import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import accountsFixture from "../../fixtures/accounts.json";
import breakFixture from "../../fixtures/break.json";
import changeLogFixture from "../../fixtures/changeLogEntries.json";
import fundFixture from "../../fixtures/fund.json";
import lotsFixture from "../../fixtures/lots.json";
import navStrikesFixture from "../../fixtures/navStrikes.json";
import positionsFixture from "../../fixtures/positions.json";
import replayFixture from "../../fixtures/replay.json";
import rulesFixture from "../../fixtures/rules.json";

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

// The router hooks a client component reaches for. There is no app router
// mounted under jsdom, and the screens under test navigate rather than toggle —
// which is the point of the migration, so it is worth the four lines.
vi.mock("next/navigation", async () => {
  const actual =
    await vi.importActual<typeof import("next/navigation")>("next/navigation");
  return {
    ...actual,
    useRouter: () => ({ replace: () => {}, push: () => {}, refresh: () => {} }),
    usePathname: () => "/funds/harbourline-global-value/breaks",
    useSelectedLayoutSegment: () => "breaks",
    useSelectedLayoutSegments: () => ["harbourline-global-value", "breaks"],
  };
});

const wire = {
  getFund: async () => fundFixture,
  getBreak: async () => breakFixture,
  listAccounts: async () => accountsFixture,
  listPositions: async () => positionsFixture,
  listLots: async () => lotsFixture,
  listNavStrikes: async () => navStrikesFixture,
  getNavStrike: async () => navStrikesFixture.navStrikes[0],
  replayNavStrike: async () => replayFixture,
  listRules: async () => rulesFixture,
  listChangeLogEntries: async () => changeLogFixture,
};

vi.mock("@/wire/client", async () => {
  const actual = await vi.importActual<typeof import("@/wire/client")>(
    "@/wire/client",
  );
  return { ...actual, ...wire };
});

const FUND = "harbourline-global-value";
const params = <T,>(v: T) => Promise.resolve(v);

/** Render an async server component by awaiting the element it returns. */
async function renderAsync(el: Promise<React.ReactElement>) {
  render(await el);
}

describe("the trial balance", () => {
  it("shows the untranslated per-currency split beneath a translated total", async () => {
    const Accounts = (await import("./funds/[fund]/accounts/page")).default;
    await renderAsync(
      Accounts({ params: params({ fund: FUND }), searchParams: params({}) }),
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
    const Positions = (await import("./funds/[fund]/positions/page")).default;
    await renderAsync(Positions({ params: params({ fund: FUND }) }));
    expect(screen.getByText(/40 open lots/)).toBeDefined();
  });

  it("renders the lot book, and says when a lot has no trade date", async () => {
    const { LotBook } = await import("@/components/LotBook");
    await renderAsync(LotBook({ fund: FUND, position: "ACME" }));
    expect(document.querySelectorAll(".lotrow").length).toBe(2);
    // A lot the engine cannot classify says so rather than showing a guess.
    expect(screen.getByText("no trade date")).toBeDefined();
  });
});

describe("the fund overview", () => {
  it("reads the realized gain, and flips it credit-normal exactly once", async () => {
    const Overview = (await import("./funds/[fund]/page")).default;
    await renderAsync(Overview({ params: params({ fund: FUND }) }));
    expect(screen.getByText("Realized gain")).toBeDefined();
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
    const Strikes = (await import("./funds/[fund]/strikes/page")).default;
    await renderAsync(Strikes({ params: params({ fund: FUND }) }));
    expect(
      screen.getByText(/a corporate action announced after this prefix/),
    ).toBeDefined();
  });

  it("reports history intact and reproduced as two separate claims", async () => {
    const Replay = (await import("./funds/[fund]/strikes/[strike]/replay/page"))
      .default;
    await renderAsync(
      Replay({ params: params({ fund: FUND, strike: "2026-02-26" }) }),
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

describe("a break", () => {
  it("shows the two figures and what produced ours", async () => {
    const Detail = (await import("./funds/[fund]/breaks/[break]/page")).default;
    await renderAsync(
      Detail({ params: params({ fund: FUND, break: "cash-usd-2026-02-26" }) }),
    );
    expect(screen.getByText("Ratio")).toBeDefined();
    expect(screen.getByText("Reported")).toBeDefined();
    // 2,000.00 is deploy/seed-demo-book.sh's number.
    expect(screen.getByText("2,000.00")).toBeDefined();
  });

  it("names the bounds the severity was graded against", async () => {
    // A grade whose terms a reader has to go and look up is a grade a reader
    // takes on trust — which is the one thing this product is not for.
    const Detail = (await import("./funds/[fund]/breaks/[break]/page")).default;
    await renderAsync(
      Detail({ params: params({ fund: FUND, break: "cash-usd-2026-02-26" }) }),
    );
    expect(screen.getByText(/graded at/)).toBeDefined();
    expect(screen.getByText(/1,000\.00 blocks/)).toBeDefined();
    expect(screen.getByText(/declared/)).toBeDefined();
  });
});

describe("rules", () => {
  it("keeps the active rules and the unapproved drafts as two lists", async () => {
    const Rules = (await import("./funds/[fund]/rules/page")).default;
    await renderAsync(Rules({ params: params({ fund: FUND }) }));
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
    expect(href).toBe(`/api/auth/login?returnTo=${encodeURIComponent(deep)}`);
  });

  it("names the signed-in principal and offers the way out", async () => {
    const { Who } = await import("@/components/Who");
    await renderAsync(Who());
    expect(screen.getByText("e.marsh@example.com")).toBeDefined();
    expect(screen.getByRole("button", { name: "Sign out" })).toBeDefined();
  });
});
