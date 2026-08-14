import { fireEvent, render, screen } from "@testing-library/react";
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
  listTemplates: async () => templatesFixture,
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

  async function ticket(rules = [TRADE_RULE]) {
    const { TradeTicket } = await import("./funds/[fund]/trade/TradeTicket");
    render(<TradeTicket fund={FUND} rules={rules} />);
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
    const { RecordForm } = await import("./funds/[fund]/record/RecordForm");
    const { MarkForm } = await import("./funds/[fund]/mark/MarkForm");
    const { IngestForm } = await import("./funds/[fund]/ingest/IngestForm");
    const { TradeTicket } = await import("./funds/[fund]/trade/TradeTicket");

    for (const [what, el] of [
      ["record", <RecordForm key="r" fund={FUND} rules={rulesFixture.rules as Rule[]} />],
      ["mark", <MarkForm key="m" fund={FUND} />],
      [
        "ingest",
        <IngestForm key="i" fund={FUND} templates={templatesFixture.templates} />,
      ],
      [
        "trade",
        <TradeTicket key="t" fund={FUND} rules={[TRADE_RULE]} />,
      ],
    ] as const) {
      const { unmount } = render(el);
      // The tree, the two views, and a way forward — the same three on each.
      expect(document.querySelector(".steps"), what).not.toBeNull();
      expect(screen.getByRole("button", { name: "Guided" }), what).toBeDefined();
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

  it("says so when the configuration in force declares no trade rule", async () => {
    // ⛔ AND OFFERS NOTHING INSTEAD. A rule is authored and approved at a
    // terminal; a screen that let one be chosen from somewhere else, or invented
    // a default, would be a way round the fence `ratio approve` is.
    const Trade = (await import("./funds/[fund]/trade/page")).default;
    await renderAsync(Trade({ params: params({ fund: FUND }) }));
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

  it("names the three things this write cannot carry, before it is written", async () => {
    // ⛔ NOT BEHIND THE PREVIEW. `ApplyEventRequest` has no field for any of
    // them, so the answer is known before the server is asked — and an operator
    // who posts without previewing would otherwise never see it.
    await asForm();
    expect(screen.getByText("Not carried:")).toBeDefined();
    expect(
      screen.getByText(/instrument, units, trade date/),
    ).toBeDefined();
    // The consequence is on the same line as the claim; the argument for it is
    // one click away, which is where the eight lines of prose went.
    expect(screen.getByText(/No tax lot opens/)).toBeDefined();
    expect(screen.getByText("What that costs")).toBeDefined();
    expect(screen.getByText("no trade date")).toBeDefined();
  });

  it("says what a disposal gives up, which is not what a purchase does", async () => {
    await asForm();
    fireEvent.click(screen.getByRole("button", { name: "Sell" }));
    // ⛔ THE WORSE HALF. A purchase that opens no lot is a lot missing; a
    // disposal that relieves no lot reports a realized gain computed against no
    // basis — the figure with no counterparty, which nobody catches.
    expect(screen.getByText(/No lot is relieved/)).toBeDefined();
    expect(screen.getByText("Unclassified")).toBeDefined();
    expect(screen.queryByText(/No tax lot opens/)).toBeNull();
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
    const { RecordForm } = await import("./funds/[fund]/record/RecordForm");
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
    const { RecordForm } = await import("./funds/[fund]/record/RecordForm");
    render(<RecordForm fund={FUND} rules={rulesFixture.rules as Rule[]} />);
    fireEvent.click(screen.getByRole("button", { name: "Form" }));
    fireEvent.change(screen.getByLabelText("Amount"), {
      target: { value: "1.005" },
    });
    expect(screen.getByText(/more than two decimal places/)).toBeDefined();
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
