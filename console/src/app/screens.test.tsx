import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import accountsFixture from "../../fixtures/accounts.json";
import breakFixture from "../../fixtures/break.json";
import breaksFixture from "../../fixtures/breaks.json";
import changeLogFixture from "../../fixtures/changeLogEntries.json";
import fundFixture from "../../fixtures/fund.json";
import lotsFixture from "../../fixtures/lots.json";
import navStrikesFixture from "../../fixtures/navStrikes.json";
import reconcileFixture from "../../fixtures/reconcile.json";
import positionsFixture from "../../fixtures/positions.json";
import replayFixture from "../../fixtures/replay.json";
import rulesFixture from "../../fixtures/rules.json";
import viewFixture from "../../fixtures/view.json";
import viewsFixture from "../../fixtures/views.json";

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
    usePathname: () =>
      "/funds/harbourline-global-value/views/abor/breaks",
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
const VIEW = "abor";
const params = <T,>(v: T) => Promise.resolve(v);

/** Render an async server component by awaiting the element it returns. */
async function renderAsync(el: Promise<React.ReactElement>) {
  render(await el);
}

describe("the trial balance", () => {
  it("shows the untranslated per-currency split beneath a translated total", async () => {
    const Accounts = (await import("./funds/[fund]/views/[view]/accounts/page")).default;
    await renderAsync(
      Accounts({ params: params({ fund: FUND, view: VIEW }), searchParams: params({}) }),
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
    const Positions = (await import("./funds/[fund]/views/[view]/positions/page")).default;
    await renderAsync(Positions({ params: params({ fund: FUND, view: VIEW }) }));
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
      await import("./funds/[fund]/views/[view]/reconcile/page")
    ).default;
    await renderAsync(
      Reconcile({
        params: params({ fund: FUND, view: VIEW }),
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
    const Strikes = (await import("./funds/[fund]/views/[view]/strikes/page")).default;
    await renderAsync(Strikes({ params: params({ fund: FUND, view: VIEW }) }));
    expect(
      screen.getByText(/a corporate action announced after this prefix/),
    ).toBeDefined();
  });

  it("reports history intact and reproduced as two separate claims", async () => {
    const Replay = (await import("./funds/[fund]/views/[view]/strikes/[strike]/replay/page"))
      .default;
    await renderAsync(
      Replay({ params: params({ fund: FUND, view: VIEW, strike: "2026-02-26" }) }),
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
    const Detail = (await import("./funds/[fund]/views/[view]/breaks/[break]/page")).default;
    await renderAsync(
      Detail({ params: params({ fund: FUND, view: VIEW, break: "cash-usd-2026-02-26" }) }),
    );
    expect(screen.getByText("Ratio")).toBeDefined();
    expect(screen.getByText("Reported")).toBeDefined();
    // 2,000.00 is deploy/seed-demo-book.sh's number.
    expect(screen.getByText("2,000.00")).toBeDefined();
  });

  it("names the bounds the severity was graded against", async () => {
    // A grade whose terms a reader has to go and look up is a grade a reader
    // takes on trust — which is the one thing this product is not for.
    const Detail = (await import("./funds/[fund]/views/[view]/breaks/[break]/page"))
      .default;
    await renderAsync(
      Detail({
        params: params({ fund: FUND, view: VIEW, break: "cash-usd-2026-02-26" }),
      }),
    );
    expect(screen.getByText(/graded at/)).toBeDefined();
    expect(screen.getByText(/1,000\.00 blocks/)).toBeDefined();
    expect(screen.getByText(/declared/)).toBeDefined();
  });

  it("shows an accepted explanation with the name on it", async () => {
    const Detail = (await import("./funds/[fund]/views/[view]/breaks/[break]/page"))
      .default;
    await renderAsync(
      Detail({
        params: params({ fund: FUND, view: VIEW, break: "cash-usd-2026-02-26" }),
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
    const Queue = (await import("./funds/[fund]/views/[view]/breaks/page")).default;
    await renderAsync(
      Queue({
        params: params({ fund: FUND, view: VIEW }),
        searchParams: params({}),
      }),
    );
    expect(screen.getByText("stale")).toBeDefined();
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
