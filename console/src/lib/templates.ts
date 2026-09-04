import type { BookKind } from "@/wire/types";

/**
 * What CreateBook offers. Kind is a template, not a label: it selects the
 * chart `chart_for` writes, not a fork of the kernel. No fund and no
 * organization are filed.
 *
 * ⭐ THE BLURBS NAME ACCOUNTS THAT `chart_for` ACTUALLY WRITES. A sentence
 * that promised a lot method or a currency the empty configuration does
 * not elect would be the stored-but-unread defect this repository refuses.
 */
export interface BookTemplate {
  readonly kind: Exclude<BookKind, "UNSPECIFIED">;
  readonly label: string;
  readonly blurb: string;
}

export const BOOK_TEMPLATES: readonly BookTemplate[] = [
  {
    kind: "PERSONAL",
    label: "Personal finance",
    blurb: "Cash and bank, living expenses, income, named loans, and a credit-card liability. A household budget is a configuration total, not a second ledger.",
  },
  {
    kind: "INVESTMENT",
    label: "Investment / Fund",
    blurb: "Positions at fair value, contributions, distributions, partner capital, and partner-scoped commitments. Does not file a fund.",
  },
  {
    kind: "PROJECT",
    label: "Project",
    blurb: "Cash, work in progress, project costs by work package, funding, revenue, progress billings, retainage, approved change orders, and awarded commitments. Budget vs actual and billed vs earned are two figures, not one URL. A change order adjusts the revised contract without rewriting the original baseline. Remaining to spend, remaining to bill, and collections vs billed stay unset until the journal can support them. The budget page does not forecast.",
  },
  {
    kind: "OPERATING",
    label: "Operating business",
    blurb: "Cash, Accounts receivable, Accounts payable, Operating revenue, Operating expenses, and Owner equity. Independent of a Fund and of an organization. Cites a balance sheet, a period income statement, a period cash-flow statement, and AR/AP aging when due dates and applications are on the journal. Investing stays unset — the chart has no PPE account. A missing due date is not current.",
  },
];

export const KIND_SHORT: Record<BookKind, string> = {
  PERSONAL: "Personal",
  INVESTMENT: "Investment",
  PROJECT: "Project",
  OPERATING: "Operating",
  UNSPECIFIED: "Book",
};

/**
 * Ingest templates CreateBook writes, keyed by the kind that owns them.
 *
 * ⭐ THE LIVE LIST IS THE BOOK'S CONFIGURATION. This catalog is the filter
 * that stops a mixed fixture — or a recapture that still only has the fund
 * snapshot — from offering `custodian-positions` on a Personal book. An id
 * that belongs to no kind (the demo book's `vendor_eod_prices`) is treated
 * as Investment: prices ride the same book as the trade loop.
 */
export const INGEST_TEMPLATE_KIND: Readonly<Record<string, Exclude<BookKind, "UNSPECIFIED">>> = {
  "bank-statement": "PERSONAL",
  "loan-payment": "PERSONAL",
  "custodian-positions": "INVESTMENT",
  "prime_equity_trades": "INVESTMENT",
  "capital-calls": "INVESTMENT",
  subscriptions: "INVESTMENT",
  "project-invoices": "PROJECT",
  "change-orders": "PROJECT",
  "purchase-orders": "PROJECT",
  "customer-invoices": "OPERATING",
  "vendor-bills": "OPERATING",
};

export function templatesForKind<T extends { templateId: string }>(
  kind: BookKind,
  listed: readonly T[],
): T[] {
  return listed.filter((t) => {
    const owner = INGEST_TEMPLATE_KIND[t.templateId] ?? "INVESTMENT";
    return owner === kind;
  });
}
