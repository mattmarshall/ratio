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
    blurb: "Cash and bank, living expenses, income, and a credit-card liability.",
  },
  {
    kind: "INVESTMENT",
    label: "Investment / Fund",
    blurb: "Positions at fair value, contributions, realized and unrealized gain. Does not file a fund.",
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
