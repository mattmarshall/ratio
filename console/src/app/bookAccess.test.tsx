import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BookKind } from "@/wire/types";
import bookFixture from "../../fixtures/book.json";
import { offersScreen, offersTicket } from "@/lib/screens";
import { getBook, listAccounts, projectProgress, operatingAging, listRules, applyEvent, markPositions } from "@/wire/client";
import { revalidatePath } from "next/cache";
import { submit as transfer } from "./books/[book]/transfer/actions";
import { submit as budget } from "./books/[book]/views/[view]/budget/actions";
import { submit as billing } from "./books/[book]/views/[view]/billing/actions";
import { place as trade } from "./books/[book]/trade/actions";
import { mark } from "./books/[book]/mark/actions";

vi.mock("@/lib/caller", () => ({ caller: async () => ({ idToken: "test-session" }) }));
vi.mock("@workos-inc/authkit-nextjs", () => ({ withAuth: async () => ({ user: null, accessToken: null }) }));
vi.mock("next/cache", () => ({ revalidatePath: vi.fn() }));
vi.mock("@/wire/client", async (original) => ({
  ...await original<typeof import("@/wire/client")>(),
  getBook: vi.fn(), listAccounts: vi.fn(), listRules: vi.fn(), projectProgress: vi.fn(), operatingAging: vi.fn(),
  applyEvent: vi.fn().mockResolvedValue({}), markPositions: vi.fn().mockResolvedValue({}),
}));
const kinds: BookKind[] = ["PERSONAL", "PROJECT", "OPERATING", "INVESTMENT", "UNSPECIFIED"];
// Independent expectations: testing the policy against itself would miss drift.
const screens: [string, BookKind[]][] = [
  ["wip", ["PROJECT"]], ["billing", ["PROJECT"]],
  ["sheet", ["PERSONAL", "OPERATING"]], ["pnl", ["PERSONAL", "OPERATING"]],
  ["budget", ["PERSONAL", "PROJECT"]], ["bridge", ["PERSONAL"]],
  ["loans", ["PERSONAL"]], ["cashflow", ["PERSONAL", "OPERATING"]],
  ["aging", ["OPERATING"]], ["capital", ["INVESTMENT"]], ["nav", ["INVESTMENT"]],
];
beforeEach(() => { vi.clearAllMocks(); });
function bookOf(kind: BookKind) {
  vi.mocked(getBook).mockResolvedValue({ ...bookFixture, kind });
}
describe("a saved URL cannot borrow another book kind's figures", () => {
  for (const [segment, allowed] of screens) for (const kind of kinds) {
    it(`${kind} ${allowed.includes(kind) ? "offers" : "refuses"} ${segment}`, async () => {
      expect(offersScreen(kind, segment)).toBe(allowed.includes(kind));
      if (allowed.includes(kind)) return; // Allowed-page renders live in screens.test.tsx.
      bookOf(kind);
      const Page = (await import(`./books/[book]/views/[view]/${segment}/page.tsx`)).default;
      await expect(Page({ params: Promise.resolve({ book: "selected", view: "book" }), searchParams: Promise.resolve({}) }))
        .rejects.toThrow("NEXT_HTTP_ERROR_FALLBACK;404");
      expect(listAccounts).not.toHaveBeenCalled();
      expect(projectProgress).not.toHaveBeenCalled();
      expect(operatingAging).not.toHaveBeenCalled();
      expect(listRules).not.toHaveBeenCalled();
    });
  }
  for (const kind of kinds.filter((kind) => kind !== "PERSONAL")) {
    it(`${kind} refuses a direct household transfer URL before loading rules`, async () => {
      bookOf(kind);
      const Page = (await import("./books/[book]/transfer/page")).default;
      await expect(Page({ params: Promise.resolve({ book: "selected" }) })).rejects.toThrow("NEXT_HTTP_ERROR_FALLBACK;404");
      expect(listRules).not.toHaveBeenCalled();
    });
  }
});
const actions = [
  { name: "transfer", run: transfer, allowed: ["PERSONAL"], fields: { ruleId: "xfer_cash_card", date: "2026-09-09" } },
  { name: "budget", run: budget, allowed: ["PROJECT"], fields: { ruleId: "approve_co_site", dated: "2026-09-09" } },
  { name: "billing", run: billing, allowed: ["PROJECT"], fields: { ruleId: "collect_receivable", dated: "2026-09-09" } },
  { name: "trade", run: trade, allowed: ["INVESTMENT", "UNSPECIFIED"], fields: { ruleId: "equity_purchase", instrument: "TEST", units: "2", price: "3.00", tradeDate: "2026-09-09", reference: "test-event" } },
  { name: "mark", run: mark, allowed: ["INVESTMENT", "UNSPECIFIED"], fields: { valuationDate: "2026-09-09" } },
];
describe("direct Server Action calls enforce book kind before preview or commit", () => {
  for (const action of actions) for (const kind of kinds) for (const commit of [false, true]) {
    it(`${kind} ${action.name} ${commit ? "commit" : "preview"} respects the action's scope`, async () => {
      bookOf(kind);
      const form = new FormData();
      for (const [key, value] of Object.entries({ fund: "selected", eventId: "test-event", amount: "6.00", ...action.fields })) {
        if (value !== undefined) form.set(key, value);
      }
      if (commit) form.set("commit", "yes");
      const result = await action.run(null, form);
      expect(getBook).toHaveBeenCalledWith({ idToken: "test-session" }, "selected");
      const allowed = action.allowed.includes(kind);
      expect(result?.ok).toBe(allowed);
      if (action.name !== "budget" && action.name !== "billing") expect(offersTicket(kind, action.name)).toBe(allowed);
      const write = action.name === "mark" ? markPositions : applyEvent;
      if (allowed) {
        expect(write).toHaveBeenCalledExactlyOnceWith({ idToken: "test-session" }, "selected", expect.objectContaining({ validateOnly: !commit }));
        expect(revalidatePath).toHaveBeenCalledTimes(commit ? 1 : 0);
      } else {
        expect(result).toEqual({ ok: false, error: `This book does not support this ${action.name} action.` });
        expect(applyEvent).not.toHaveBeenCalled(); expect(markPositions).not.toHaveBeenCalled();
        expect(revalidatePath).not.toHaveBeenCalled();
      }
    });
  }
});
