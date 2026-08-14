import { describe, expect, it } from "vitest";

import {
  consideration,
  decimalOf,
  hundredths,
  signature,
  suggestedReference,
} from "./trade";

/**
 * ⛔ THIS IS THE HALF OF THE TICKET THAT CAN BE WRONG BY A HUNDRED AND STILL
 * LOOK RIGHT. Every case below has a Rust counterpart —
 * `ratio_common::parse_minor`'s tests and `ratio_ingest::posting_for` — and the
 * point of writing them again here is that a mirror nothing compares is a mirror
 * that drifts. Same discipline as `//proto:mirrors_test`.
 *
 * ⛔ NEGATIVE-TEST THESE. Change `100n` to `1n` in the divisibility guard and
 * watch the refusal case go red before believing it.
 */
describe("a decimal string reaches minor units by splitting, not by floating", () => {
  it("agrees with parse_minor on every case parse_minor tests", () => {
    const at = (s: string) => {
      const r = hundredths(s, "an amount");
      return r.ok ? r.minor : r.error;
    };
    expect(at("250000.00")).toBe(25_000_000n);
    expect(at("0.10")).toBe(10n);
    expect(at("0.1")).toBe(10n);
    expect(at("1.5")).toBe(150n);
    expect(at("42")).toBe(4_200n);
    expect(at(".5")).toBe(50n);
    expect(at("-25,000.00")).toBe(-2_500_000n);
    expect(at("$1,204,880.11")).toBe(120_488_011n);
  });

  it("keeps the cent a double would lose", () => {
    // ⭐ THE CASE THE WHOLE APPROACH EXISTS FOR. `parseFloat("1.005") * 100` is
    // 100.49999999999999, which truncates to 100 — a cent short, on a figure
    // that becomes a journal entry.
    const r = hundredths("1.005", "an amount");
    expect(r.ok).toBe(false);
    expect(r.ok === false && r.error).toMatch(/more than two decimal places/);
    // And the two-place neighbour it would otherwise be confused with is exact.
    const ok = hundredths("1.00", "an amount");
    expect(ok.ok && ok.minor).toBe(100n);
  });

  it("refuses what is not an amount rather than reading part of it", () => {
    for (const bad of ["", "  ", "1.2.3", "twelve", "1e3", "-"]) {
      expect(hundredths(bad, "an amount").ok).toBe(false);
    }
  });
});

describe("the consideration", () => {
  const minor = (u: string, p: string) => {
    const r = consideration(u, p);
    return r.ok ? r.minor : r.error;
  };

  it("is units times price, exactly, at a size a double could not hold", () => {
    expect(minor("1000", "341.75")).toBe(34_175_000n);
    expect(minor("1", "0.01")).toBe(1n);
    // 2^53 minor units is where a double stops counting integers. A ticket that
    // big is absurd; an arithmetic that stops being exact before it is not.
    expect(minor("100000000", "999999.99")).toBe(9_999_999_900_000_000n);
  });

  it("refuses a product that is not a whole number of minor units", () => {
    // ⛔ REFUSED, NOT ROUNDED, and in `posting_for`'s own words. Which way to
    // round is a term of an administration agreement; a ticket that decided it
    // silently would book a figure the configuration never agreed to.
    const r = consideration("1.5", "0.01");
    expect(r.ok).toBe(false);
    expect(r.ok === false && r.error).toMatch(
      /not a whole number of minor units/,
    );
    expect(r.ok === false && r.error).toMatch(/rounding decision/);
  });

  it("refuses a negative side rather than letting a sign be the side", () => {
    // ⛔ A SALE IS A DIFFERENT RULE, NOT A NEGATIVE PURCHASE. Negating the
    // amount on a purchase rule posts a purchase backwards, and it balances.
    expect(consideration("-100", "10.00").ok).toBe(false);
    expect(consideration("100", "-10.00").ok).toBe(false);
  });
});

describe("what goes on the wire", () => {
  it("is major units with a point, which is what the contract asks for", () => {
    // ⛔ `ApplyEventRequest.amount` is "a DECIMAL string as a person types it".
    // Minor units here book every trade a hundred times too large, and the
    // entry balances either way.
    expect(decimalOf(34_175_000n)).toBe("341750.00");
    expect(decimalOf(1n)).toBe("0.01");
    expect(decimalOf(0n)).toBe("0.00");
    expect(decimalOf(-2_500n)).toBe("-25.00");
  });
});

describe("the ticket a preview answers for", () => {
  const base = {
    side: "buy",
    instrument: "ACME",
    units: "1000",
    price: "341.75",
    tradeDate: "2026-02-26",
    ruleId: "equity_purchase",
    reference: "BUY-ACME-2026-02-26",
  };

  it("changes when any field a posting depends on changes", () => {
    // ⭐ OTHERWISE "PREVIEW, THEN POST" IS A SUGGESTION. Preview, change the
    // price, post: the preview stays on screen describing an entry the button
    // will not write.
    for (const k of Object.keys(base) as (keyof typeof base)[]) {
      expect(signature({ ...base, [k]: `${base[k]}x` })).not.toBe(
        signature(base),
      );
    }
  });

  it("ignores whitespace, which changes no figure", () => {
    expect(signature({ ...base, units: " 1000 " })).toBe(signature(base));
  });
});

describe("the suggested reference", () => {
  it("is inside the charset and the length apply_event enforces", () => {
    // Letters, digits, `-`, `_`, `.`, at most 64 — the id reaches a
    // filename-shaped identifier and a screen other people read.
    const r = suggestedReference("sell", "GB00B ONE/TWO", "2026-02-26");
    expect(r).toMatch(/^[A-Za-z0-9._-]{1,64}$/);
    expect(r.startsWith("SEL-")).toBe(true);
    const long = suggestedReference("buy", "X".repeat(200), "2026-02-26");
    expect(long.length).toBeLessThanOrEqual(64);
  });
});
