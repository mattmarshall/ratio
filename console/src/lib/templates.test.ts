import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import templatesFixture from "../../fixtures/templates.json";
import { BOOK_TEMPLATES, INGEST_TEMPLATE_KIND, templatesForKind } from "./templates";

const samples = join(dirname(fileURLToPath(import.meta.url)), "../../fixtures/samples");
const sampleHeader = (name: string) =>
  readFileSync(join(samples, name), "utf8").split("\n")[0];

describe("book templates", () => {
  it("are the three CreateBook kinds, not a second ledger", () => {
    expect(BOOK_TEMPLATES.map((t) => t.kind)).toEqual([
      "PERSONAL",
      "INVESTMENT",
      "PROJECT",
    ]);
  });

  it("names accounts chart_for actually writes", () => {
    const byKind = Object.fromEntries(BOOK_TEMPLATES.map((t) => [t.kind, t.blurb]));
    expect(byKind.PERSONAL).toMatch(/Cash and bank/);
    expect(byKind.PERSONAL).toMatch(/configuration total/);
    expect(byKind.PERSONAL).toMatch(/named loans/);
    expect(byKind.INVESTMENT).toMatch(/fair value/);
    expect(byKind.INVESTMENT).toMatch(/distributions/);
    expect(byKind.INVESTMENT).toMatch(/partner capital/);
    expect(byKind.INVESTMENT).toMatch(/commitments/);
    expect(byKind.INVESTMENT).toMatch(/Does not file a fund/);
    expect(byKind.PROJECT).toMatch(/work in progress/);
    expect(byKind.PROJECT).toMatch(/retainage/);
    expect(byKind.PROJECT).toMatch(/change orders/);
    expect(byKind.PROJECT).toMatch(/two figures/);
    expect(byKind.PROJECT).toMatch(/Remaining to bill/);
    expect(byKind.PROJECT).toMatch(/collections vs billed/);
    expect(byKind.PROJECT).toMatch(/awarded commitments/);
    expect(byKind.PROJECT).toMatch(/Remaining to spend/);
    expect(byKind.PROJECT).toMatch(/does not forecast/);
  });
});

describe("ingest templates", () => {
  it("seeds the CreateBook mappings, and the fixture carries each", () => {
    expect(INGEST_TEMPLATE_KIND["bank-statement"]).toBe("PERSONAL");
    expect(INGEST_TEMPLATE_KIND["loan-payment"]).toBe("PERSONAL");
    expect(INGEST_TEMPLATE_KIND["custodian-positions"]).toBe("INVESTMENT");
    expect(INGEST_TEMPLATE_KIND["prime_equity_trades"]).toBe("INVESTMENT");
    expect(INGEST_TEMPLATE_KIND["capital-calls"]).toBe("INVESTMENT");
    expect(INGEST_TEMPLATE_KIND["project-invoices"]).toBe("PROJECT");
    expect(INGEST_TEMPLATE_KIND["change-orders"]).toBe("PROJECT");
    expect(INGEST_TEMPLATE_KIND["purchase-orders"]).toBe("PROJECT");
    const ids = templatesFixture.templates.map((t) => t.templateId);
    for (const id of Object.keys(INGEST_TEMPLATE_KIND)) {
      expect(ids, `fixtures/templates.json drifted off CreateBook seed ${id}`).toContain(id);
    }
  });

  it("will not offer a fund snapshot or trade file on a Personal or Project book", () => {
    const listed = templatesFixture.templates;
    const personal = templatesForKind("PERSONAL", listed).map((t) => t.templateId);
    const project = templatesForKind("PROJECT", listed).map((t) => t.templateId);
    const investment = templatesForKind("INVESTMENT", listed).map((t) => t.templateId);
    expect(personal).toEqual(["bank-statement", "loan-payment"]);
    expect(project).toEqual(["project-invoices", "change-orders", "purchase-orders"]);
    expect(investment).toEqual([
      "custodian-positions",
      "prime_equity_trades",
      "capital-calls",
    ]);
    expect(personal).not.toContain("custodian-positions");
    expect(personal).not.toContain("prime_equity_trades");
    expect(personal).not.toContain("capital-calls");
    expect(project).not.toContain("custodian-positions");
    expect(project).not.toContain("prime_equity_trades");
    expect(project).not.toContain("capital-calls");
  });

  it("leaves an unlisted price file visible on the Investment book that holds it", () => {
    // ⚠ seed-demo-book.sh's vendor_eod_prices. Not a CreateBook seed; the
    // default-to-Investment filter is what keeps it on the fund that has it
    // without offering it to a household.
    const extra = [
      ...templatesFixture.templates,
      {
        name: "funds/harbourline-global-value/templates/vendor_eod_prices",
        templateId: "vendor_eod_prices",
        factKind: "price",
        form: "one price per row",
        posts: false,
      },
    ];
    expect(
      templatesForKind("INVESTMENT", extra).map((t) => t.templateId),
    ).toEqual([
      "custodian-positions",
      "prime_equity_trades",
      "capital-calls",
      "vendor_eod_prices",
    ]);
    expect(
      templatesForKind("PERSONAL", extra).map((t) => t.templateId),
    ).toEqual(["bank-statement", "loan-payment"]);
    expect(
      templatesForKind("PROJECT", extra).map((t) => t.templateId),
    ).toEqual(["project-invoices", "change-orders", "purchase-orders"]);
  });

  it("the fixture forms are the rendered mapping, not a slogan", () => {
    // ⛔ THE OLD FIXTURE SAID "one row, one holding". Template.form is
    // `render()`, and a slogan that is not that string is a third syntax.
    const byId = Object.fromEntries(
      templatesFixture.templates.map((t) => [t.templateId, t]),
    );
    const need = (id: string) => {
      const t = byId[id];
      expect(t, id).toBeDefined();
      return t!;
    };
    expect(need("bank-statement").factKind).toBe("statement");
    expect(need("bank-statement").posts).toBe(true);
    expect(need("bank-statement").form).toMatch(/template bank-statement \{/);
    expect(need("bank-statement").form).toMatch(/one statement per row/);
    expect(need("project-invoices").factKind).toBe("invoice");
    expect(need("project-invoices").posts).toBe(true);
    expect(need("project-invoices").form).toMatch(/template project-invoices \{/);
    expect(need("custodian-positions").factKind).toBe("position");
    expect(need("custodian-positions").posts).toBe(false);
    expect(need("custodian-positions").form).toMatch(/posts      nothing/);
    expect(need("prime_equity_trades").factKind).toBe("trade");
    expect(need("prime_equity_trades").posts).toBe(true);
    expect(need("prime_equity_trades").form).toMatch(/template prime_equity_trades \{/);
    expect(need("prime_equity_trades").form).toMatch(/one trade per row/);
    expect(need("prime_equity_trades").form).toMatch(/amount      consideration/);
    expect(need("prime_equity_trades").form).toMatch(/buy         -> equity_purchase/);
    expect(need("capital-calls").factKind).toBe("capital");
    expect(need("capital-calls").posts).toBe(true);
    expect(need("capital-calls").form).toMatch(/template capital-calls \{/);
    expect(need("capital-calls").form).toMatch(/one capital per row/);
    expect(need("capital-calls").form).toMatch(/commit_lp   -> commit_lp/);
    expect(need("capital-calls").form).toMatch(/call_lp     -> call_lp/);
    expect(need("change-orders").factKind).toBe("change");
    expect(need("change-orders").posts).toBe(true);
    expect(need("change-orders").form).toMatch(/template change-orders \{/);
    expect(need("change-orders").form).toMatch(/one change per row/);
    expect(need("change-orders").form).toMatch(/approve_co_site-> approve_co_site/);
    expect(need("change-orders").form).toMatch(/deduct_co_site-> deduct_co_site/);
    expect(need("purchase-orders").factKind).toBe("purchase");
    expect(need("purchase-orders").posts).toBe(true);
    expect(need("purchase-orders").form).toMatch(/template purchase-orders \{/);
    expect(need("purchase-orders").form).toMatch(/one purchase per row/);
    expect(need("purchase-orders").form).toMatch(/award_commitment_site-> award_commitment_site/);
    expect(need("purchase-orders").form).toMatch(/release_commitment_site-> release_commitment_site/);
  });

  it("sample CSVs name the columns the seeded templates read", () => {
    // ⛔ HEADER NAMES, NEVER POSITIONS. extract_csv locates columns by
    // name; a sample that drifted off the template would ingest blanks
    // and look like a working file.
    expect(sampleHeader("bank-statement.csv")).toBe(
      "Ref,Date,Amount,Ccy,Memo,Account,Kind",
    );
    expect(sampleHeader("project-invoices.csv")).toBe(
      "InvoiceRef,Date,Amount,Ccy,Vendor,Memo,Kind",
    );
    expect(sampleHeader("custodian-positions.csv")).toBe(
      "LineRef,AsOf,ISIN,Ticker,Exch,Quantity,MarketValue,Ccy",
    );
    expect(sampleHeader("prime_equity_trades.csv")).toBe(
      "TradeRef,ISIN,Symbol,Exch,Broker,B/S,Quantity,Price,Ccy,TradeDate",
    );
    expect(sampleHeader("capital-calls.csv")).toBe(
      "CallRef,Date,Amount,Ccy,Kind",
    );
    expect(sampleHeader("change-orders.csv")).toBe(
      "ChangeRef,Date,Amount,Ccy,Memo,Kind",
    );
    expect(sampleHeader("purchase-orders.csv")).toBe(
      "PurchaseRef,Date,Amount,Ccy,Memo,Kind",
    );
    const forms = Object.fromEntries(
      templatesFixture.templates.map((t) => [t.templateId, t.form]),
    );
    expect(forms["bank-statement"]).toMatch(/from "Memo"/);
    expect(forms["project-invoices"]).toMatch(/from "Vendor"/);
    expect(forms["custodian-positions"]).toMatch(/from "ISIN"/);
    expect(forms["prime_equity_trades"]).toMatch(/from "B\/S"/);
    expect(forms["prime_equity_trades"]).toMatch(/from "TradeDate"/);
    expect(forms["capital-calls"]).toMatch(/from "Kind"/);
    expect(forms["capital-calls"]).toMatch(/from "CallRef"/);
    expect(forms["change-orders"]).toMatch(/from "Kind"/);
    expect(forms["change-orders"]).toMatch(/from "ChangeRef"/);
    expect(forms["purchase-orders"]).toMatch(/from "Kind"/);
    expect(forms["purchase-orders"]).toMatch(/from "PurchaseRef"/);
  });
});
