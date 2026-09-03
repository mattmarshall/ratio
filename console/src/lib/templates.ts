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
    blurb: "Cash and bank, living expenses, income, and a credit-card liability. A household budget is a configuration total, not a second ledger.",
  },
  {
    kind: "INVESTMENT",
    label: "Investment / Fund",
    blurb: "Positions at fair value, contributions, distributions, and partner capital. Does not file a fund.",
  },
  {
    kind: "PROJECT",
    label: "Project",
    blurb: "Cash, work in progress, project costs, funding, and revenue. Budget vs actual is a figure over those accounts, not a second ledger.",
  },
];

export const KIND_SHORT: Record<BookKind, string> = {
  PERSONAL: "Personal",
  INVESTMENT: "Investment",
  PROJECT: "Project",
  UNSPECIFIED: "Book",
};

/**
 * Ingest templates CreateBook writes, keyed by the kind that owns them.
 *
 * ⭐ THE LIVE LIST IS THE BOOK'S CONFIGURATION. This catalog is the filter
 * that stops a mixed fixture — or a recapture that still only has the fund
 * snapshot — from offering `custodian-positions` on a Personal book. An id
 * that belongs to no kind (the demo book's `prime_equity_trades`) is treated
 * as Investment: that is the book that holds the closed loop.
 */
export const INGEST_TEMPLATE_KIND: Readonly<Record<string, Exclude<BookKind, "UNSPECIFIED">>> = {
  "bank-statement": "PERSONAL",
  "custodian-positions": "INVESTMENT",
  "project-invoices": "PROJECT",
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
