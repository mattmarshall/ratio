import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import fundsFixture from "../../fixtures/funds.json";
import viewsFixture from "../../fixtures/views.json";
import { SCREENS, screenHref } from "@/lib/screens";
import type { Fund, View } from "@/wire/types";
import { CommandHint } from "./CommandHint";
import { FundActions } from "./FundActions";
import { Palette } from "./Palette";

// ⛔ THIS FILE IS `.test.tsx` AND MUST STAY THAT WAY. `//console:fields_test`
// excludes exactly `*.test.tsx` from the blob it greps; a needle that lived only
// in a `.test.ts` file would be found in the test asserting it and nowhere in the
// console, which is the vacuous pass that whole script directory exists to stop.

// ⛔ THE PLATFORM IS PINNED BEFORE ANY IMPORT, AND `beforeAll` WOULD BE TOO LATE.
// kbar vendors tinykeys, which resolves `$mod` ONCE at module load from
// `/Mac|iPod|iPhone|iPad/.test(navigator.platform)`. jsdom reports "", so `$mod`
// would be Control and every ⌘K below would fire nothing while the suite stayed
// green. `vi.hoisted` runs above the import of `kbar` itself.
const { push } = vi.hoisted(() => {
  Object.defineProperty(window.navigator, "platform", {
    value: "MacIntel",
    configurable: true,
  });
  return { push: vi.fn() };
});

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push, replace: () => {}, refresh: () => {} }),
  usePathname: () => "/funds/harbourline-global-value/views/abor/breaks",
  // ⚠ THE SEGMENTS A COMPONENT RENDERED BY `funds/[fund]/layout.tsx` SEES —
  // three, not four. `FundRail` sits one layout up and sees the fund id first;
  // `FundActions`, `ScreenTabs` and `ViewSwitch` all see this. A mock of the
  // wrong depth would let every action here build the wrong URL and stay green.
  useSelectedLayoutSegments: () => ["views", "abor", "breaks"],
}));

// ⛔ THE FENCE, AS A SPY. Nothing in the palette imports the wire client today,
// and this is what notices the day something does: a palette that could post a
// trade would be a second way round `Ticket`'s "preview, then post", which is
// enforced on the screen that owns each write.
const writes = vi.hoisted(() => ({
  applyEvent: vi.fn(),
  markPositions: vi.fn(),
  ingestDelivery: vi.fn(),
  admitFacts: vi.fn(),
}));
vi.mock("@/wire/client", () => writes);

const FUND = "harbourline-global-value";
const VIEW = "abor";
const funds = (fundsFixture as { funds: Fund[] }).funds;
const views = (viewsFixture as { views: View[] }).views;

beforeAll(() => {
  // ⛔ WITHOUT THIS THE RESULT LIST IS EMPTY AND NOTHING SAYS WHY.
  // `KBarResults` virtualizes with `@tanstack/react-virtual`, which measures its
  // scroll box through `offsetHeight` — and jsdom pins that at 0 for every
  // element. `calculateRange` returns `null` the moment `outerSize === 0`, so
  // `getVirtualIndexes()` returns NO indexes at all: a correct palette renders a
  // `role="listbox"` with nothing in it and every `getByText` fails.
  //
  // ⛔ AND THE ROWS MUST MEASURE SOMETHING, NOT ZERO. Zero was the first attempt
  // and it renders a WINDOW rather than the list — with every row at height 0,
  // every measurement has the same `start`, and the virtualizer's binary search
  // for the first visible index lands arbitrarily among them. It rendered items
  // 8 to 19 of 20: the tickets were findable and the screens above them were
  // not, which reads exactly like a registration bug. Distinct heights, and a
  // scroll box tall enough to hold every row, make the range the whole list.
  //
  // `data-index` is the attribute kbar puts on each row wrapper, which is what
  // lets one getter answer for both.
  Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
    configurable: true,
    get(this: HTMLElement) {
      return this.hasAttribute("data-index") ? 40 : 8000;
    },
  });
});

beforeEach(() => {
  push.mockClear();
  for (const w of Object.values(writes)) w.mockClear();
});

/** The chrome as `funds/layout.tsx` assembles it, with one fund open. */
function renderConsole() {
  return render(
    <Palette funds={funds}>
      <header>
        <CommandHint />
      </header>
      <FundActions fund={FUND} views={views} defaultView={VIEW} />
    </Palette>,
  );
}

async function open() {
  fireEvent.keyDown(window, { key: "k", metaKey: true });
  return screen.findByRole("combobox");
}

async function type(value: string) {
  const input = await open();
  fireEvent.change(input, { target: { value } });
  return input;
}

/** The row kbar draws, found by the text this console renders inside it. */
function row(name: string | RegExp) {
  return screen.findByText(name);
}

describe("the command palette", () => {
  it("costs a screen nobody opened it on nothing at all", () => {
    renderConsole();
    // ⛔ `KBarPortal` returns null while hidden. If that ever stops being true, a
    // fixed full-viewport scrim is sitting over every fund screen in the console.
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByRole("combobox")).toBeNull();
  });

  it("opens on the same chord the hint prints", async () => {
    renderConsole();
    // ⭐ ONE CASE FOR TWO THINGS WHOSE DISAGREEMENT IS INVISIBLE. kbar decides the
    // chord from `navigator.platform` at module load and `CommandHint` prints a
    // glyph from its own read of the same value. A hint that says ⌘ where Ctrl
    // fires is wrong in the one place a person looks to find out.
    await waitFor(() => expect(screen.getByText("⌘ K")).toBeTruthy());
    await open();
    expect(screen.getByRole("dialog")).toBeTruthy();
  });

  it("names the dialog for somebody who cannot see it", async () => {
    renderConsole();
    await open();
    // kbar names neither the dialog nor the input; both are ours.
    expect(screen.getByRole("dialog").getAttribute("aria-label")).toBe("Command palette");
    expect(screen.getByRole("combobox").getAttribute("aria-label")).toBeTruthy();
  });

  it("matches a fund by a word from the middle of its name", async () => {
    renderConsole();
    await type("global");
    expect(await row("Harbourline Global Value")).toBeTruthy();
    await waitFor(() => expect(screen.queryByText("Northstar Multi-Strategy")).toBeNull());
  });

  it("sends a fund exactly where the rail sends it", async () => {
    renderConsole();
    await type("northstar");
    fireEvent.click(await row("Northstar Multi-Strategy"));
    // ⛔ THE RAIL'S HREF, NOT THE LEGACY REDIRECT. `FundRail` links straight to
    // the fund's default view because "a redirect on every click is a round trip
    // nobody needs"; a palette using `/funds/{f}/breaks` would pay it every jump.
    const northstar = funds.find((f) => f.name.endsWith("northstar-multi-strategy"))!;
    expect(push).toHaveBeenCalledWith(
      `/funds/northstar-multi-strategy/views/${northstar.defaultView}/breaks`,
    );
  });

  // ⛔ WRITTEN OUT, NOT DERIVED FROM `SCREENS`, AND THE FIRST VERSION OF THIS WAS
  // A TAUTOLOGY. It looped over `SCREENS` and asserted each entry reached
  // `screenHref` of the same entry — so deleting a screen shortened the loop and
  // the suite stayed green. CONTRIBUTING.md records three suites here that were
  // green, covered the code and tested nothing; this was very nearly a fourth,
  // and the sabotage run is the only reason it is not.
  //
  // ⚠ THE VIEW SEGMENTS ARE THE POINT OF THE TABLE. Four of these are folded
  // from a journal prefix and belong to a book of record; four are what was
  // AGREED and do not. Flip one either way and this goes red — which is what
  // `scoped` is for, because getting it wrong is a URL that lies about which
  // book produced the figures on it.
  const EXPECTED_SCREENS: ReadonlyArray<readonly [string, string]> = [
    ["Exceptions", `/funds/${FUND}/views/${VIEW}/breaks`],
    ["Trial balance", `/funds/${FUND}/views/${VIEW}/accounts`],
    ["Positions", `/funds/${FUND}/views/${VIEW}/positions`],
    ["Data", `/funds/${FUND}/data`],
    ["NAV", `/funds/${FUND}/views/${VIEW}/strikes`],
    ["Configuration", `/funds/${FUND}/config`],
    ["Rules", `/funds/${FUND}/rules`],
    ["Change log", `/funds/${FUND}/changes`],
  ];

  it("offers all eight screens, each going where its tab goes", async () => {
    for (const [label, href] of EXPECTED_SCREENS) {
      const { unmount } = renderConsole();
      await type(label);
      fireEvent.click(await row(label));
      expect(push).toHaveBeenCalledWith(href);
      push.mockClear();
      unmount();
    }
  });

  it("offers the header's screens and no others", async () => {
    // ⭐ THE PALETTE AND THE HEADER ARE ONE LIST, said where it can be checked.
    // `ScreenTabs` and `FundActions` both read `@/lib/screens`, so this is what
    // notices a ninth tab arriving — or an eighth leaving — without the palette.
    expect(SCREENS.map((s) => s.label)).toEqual(EXPECTED_SCREENS.map(([l]) => l));
    for (const s of SCREENS) {
      const expected = EXPECTED_SCREENS.find(([l]) => l === s.label)![1];
      expect(screenHref(FUND, VIEW, s)).toBe(expected);
    }
  });

  it("keeps the screen when it switches the book of record", async () => {
    renderConsole();
    const view = views[0]!;
    const id = view.name.split("/").pop()!;
    await type(view.displayName);
    fireEvent.click(await row(new RegExp(view.displayName)));
    // ⭐ A view is a property of the answer, so switching one stays on the screen
    // whose figures it changes — `ViewSwitch` makes the same move.
    expect(push).toHaveBeenCalledWith(`/funds/${FUND}/views/${id}/breaks`);
  });

  it("opens the four tickets and posts none of them", async () => {
    for (const [label, segment] of [
      ["Trade ticket", "trade"],
      ["Record an event", "record"],
      ["Ingest a delivery", "ingest"],
      ["Mark positions", "mark"],
    ] as const) {
      const { unmount } = renderConsole();
      await type(label);
      fireEvent.click(await row(label));
      expect(push).toHaveBeenCalledWith(`/funds/${FUND}/${segment}`);
      // ⛔ THE PALETTE REACHES THE FORM AND NEVER GETS PAST IT. No form is
      // rendered here, and no write on the wire client was called.
      expect(document.querySelector("form")).toBeNull();
      for (const w of Object.values(writes)) expect(w).not.toHaveBeenCalled();
      push.mockClear();
      unmount();
    }
  });

  it("offers every route a pasted id could name and picks none of them", async () => {
    renderConsole();
    await type("cash-usd-2026-02-26");
    // ⛔ THE ASSERTION IS THE PLURAL. A break id contains a date and a NAV strike
    // id IS one, so nothing here can tell them apart — and there is no RPC to
    // ask. A heuristic that narrowed this to one would be the console asserting
    // a fact it has not checked, and it would turn this red.
    await waitFor(() =>
      expect(screen.getAllByText(/^Open “cash-usd-2026-02-26” as /).length).toBeGreaterThan(1),
    );
    fireEvent.click(await row(/as an exception$/));
    expect(push).toHaveBeenCalledWith(
      `/funds/${FUND}/views/${VIEW}/breaks/cash-usd-2026-02-26`,
    );
  });

  it("translates a pasted resource name exactly, and only it", async () => {
    renderConsole();
    await type(`funds/${FUND}/views/${VIEW}/navStrikes/2026-02-26`);
    // ⭐ ONE CANDIDATE, because the string itself says which resource it names —
    // and `navStrikes` is not what the URL calls it.
    await waitFor(() => expect(screen.getAllByText(/^Open funds\//)).toHaveLength(1));
    expect(screen.queryByText(/as a NAV strike$/)).toBeNull();
    fireEvent.click(await row(/^Open funds\//));
    expect(push).toHaveBeenCalledWith(
      `/funds/${FUND}/views/${VIEW}/strikes/2026-02-26`,
    );
  });

  it("says nothing matches rather than showing an empty box", async () => {
    renderConsole();
    // ⚠ A RESOURCE NAME IT CANNOT TRANSLATE, WHICH IS THE ONLY WAY TO EMPTY THIS
    // PALETTE. Any bare word of two characters or more is a plausible id, so
    // "Open by id" always has eleven routes to offer — a garbage query still
    // fills the list. A string with slashes in it is a NAME, and a name the
    // table does not know produces nothing at all.
    //
    // ⭐ This is exactly the entry resource of #52: declared in the contract,
    // carried in payloads, and given no screen. The palette says so instead of
    // inventing `/funds/{f}/entries/5`.
    await type(`funds/${FUND}/entries/5`);
    expect(await screen.findByText("Nothing matches.")).toBeTruthy();
    // ⛔ The listbox stays in the document even with nothing in it: `KBarSearch`
    // points `aria-controls` at its id, and removing it leaves a combobox naming
    // an element that is not there.
    expect(screen.getByRole("listbox")).toBeTruthy();
    expect(screen.getByRole("combobox").getAttribute("aria-controls")).toBe(
      screen.getByRole("listbox").getAttribute("id"),
    );
  });

  it("never lands the selection on a section heading", async () => {
    renderConsole();
    const input = await open();
    await row("Exceptions");
    // ⚠ Headings share the `role="option"` wrapper kbar gives every item in its
    // flat list, which this console cannot change without forking `KBarResults`.
    // What can be checked is that arrowing never selects one.
    for (let i = 0; i < 12; i++) {
      const id = input.getAttribute("aria-activedescendant");
      if (id) {
        const active = document.getElementById(id);
        expect(active?.querySelector(".cmdsec")).toBeNull();
        expect(active?.querySelector(".cmdrow")).toBeTruthy();
      }
      fireEvent.keyDown(window, { key: "ArrowDown" });
    }
  });

  it("gives focus back to what had it when Escape closes it", async () => {
    renderConsole();
    const hint = screen.getByRole("button", { name: /Search/ });
    hint.focus();
    await open();
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(hint));
  });

  it("traps Tab, because there is nowhere for it to go", async () => {
    renderConsole();
    await open();
    const dialog = screen.getByRole("dialog");
    const tab = fireEvent.keyDown(dialog, { key: "Tab" });
    // `fireEvent` returns false when a handler called preventDefault.
    expect(tab).toBe(false);
  });
});
