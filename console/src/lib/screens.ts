// The screens under one book, and where each one lives.
//
// ⛔ THIS LIST WAS INSIDE `components/ScreenTabs.tsx` AND HAS TWO READERS NOW.
// The command palette offers the same screens the hub draws, and a
// second copy of them is a second answer to what `scoped` means for each. That
// is the shape of drift `funds/[fund]/layout.tsx` records about `defaultView`:
// "one source for a fact is the point of this whole feature".
//
// ⚠ It lives in `lib/` rather than beside the component because a `"use client"`
// module is a bundling boundary, not a home for data. `BASIS_LABEL` in
// `lib/format.ts` is the precedent.
//
// ⭐ KIND SELECTS THE LIST. A personal book that offered Exceptions / Positions
// / NAV would be a fake label on fund-ops screens — issue #65. The agreement
// screens (configuration, rules, change log, data) stay shared: a rule set is
// the same document whichever chart it posted.

import type { BookKind } from "@/wire/types";

export type ScreenGroup = "book" | "agreement";

export interface Screen {
  /** The URL segment, which is also the segment `useSelectedLayoutSegments` reports. */
  readonly segment: string;
  /** What the place and the palette call it. */
  readonly label: string;
  /** Whether the screen belongs to a book of record. See the ⚠ below. */
  readonly scoped: boolean;
  /**
   * Book figures vs what was agreed. Not cosmetic: mixing them in one
   * tab strip is how a personal book inherits a fund warehouse.
   */
  readonly group: ScreenGroup;
}

/**
 * ⚠ `scoped` IS NOT COSMETIC. A screen showing figures folded from a journal
 * prefix belongs to one view and lives under `views/<id>/`; one showing what was
 * AGREED — the configuration, the rules, the change log, the deliveries — does
 * not, because a rule set is the same document whichever way you recognise the
 * entries it produced. Getting this wrong in either direction is a URL that
 * lies about what it is showing.
 */
const AGREEMENT: readonly Screen[] = [
  { segment: "data", label: "Data", scoped: false, group: "agreement" },
  { segment: "config", label: "Configuration", scoped: false, group: "agreement" },
  { segment: "rules", label: "Rules", scoped: false, group: "agreement" },
  { segment: "changes", label: "Change log", scoped: false, group: "agreement" },
];

/** Fund / investment operations. NAV, breaks, positions. */
export const FUND_SCREENS: readonly Screen[] = [
  { segment: "breaks", label: "Exceptions", scoped: true, group: "book" },
  { segment: "accounts", label: "Trial balance", scoped: true, group: "book" },
  { segment: "positions", label: "Positions", scoped: true, group: "book" },
  { segment: "strikes", label: "NAV", scoped: true, group: "book" },
  ...AGREEMENT,
];

/**
 * Household figures. Balance sheet and a period P&L, not ABOR.
 *
 * Trial balance stays: it is the conservation view of the same accounts, and
 * a sheet that could not be checked against it would be a picture.
 */
export const PERSONAL_SCREENS: readonly Screen[] = [
  { segment: "sheet", label: "Balance sheet", scoped: true, group: "book" },
  { segment: "pnl", label: "Period P&L", scoped: true, group: "book" },
  { segment: "accounts", label: "Trial balance", scoped: true, group: "book" },
  ...AGREEMENT,
];

/** The fund list. Palette tests on an investment book read this. */
export const SCREENS: readonly Screen[] = FUND_SCREENS;

export const SCREEN_GROUPS: ReadonlyArray<{
  id: ScreenGroup;
  label: string;
}> = [
  { id: "book", label: "Book" },
  { id: "agreement", label: "Agreement" },
];

export function screensFor(kind: BookKind): readonly Screen[] {
  return kind === "PERSONAL" ? PERSONAL_SCREENS : FUND_SCREENS;
}

/** Where a newly opened book of record lands. */
export function defaultScreen(kind: BookKind): string {
  return kind === "PERSONAL" ? "sheet" : "breaks";
}

/** Title for the place currently open, across both kinds. */
export function placeOf(segment: string | undefined): Screen | undefined {
  if (!segment) return undefined;
  return (
    PERSONAL_SCREENS.find((s) => s.segment === segment) ??
    FUND_SCREENS.find((s) => s.segment === segment)
  );
}

export interface Ticket {
  readonly segment: string;
  readonly label: string;
  readonly keywords: string;
}

const FUND_TICKETS: readonly Ticket[] = [
  {
    segment: "trade",
    label: "Trade ticket",
    keywords: "trade,buy,sell,instrument,units,price",
  },
  { segment: "record", label: "Record an event", keywords: "record,event,rule,apply" },
  {
    segment: "ingest",
    label: "Ingest a delivery",
    keywords: "ingest,delivery,file,custodian,admit",
  },
  { segment: "mark", label: "Mark positions", keywords: "mark,price,valuation,marks" },
];

const PERSONAL_TICKETS: readonly Ticket[] = [
  {
    segment: "transfer",
    label: "Transfer",
    keywords: "transfer,move,cash,card,investments",
  },
  { segment: "record", label: "Record an event", keywords: "record,event,rule,apply" },
  {
    segment: "ingest",
    label: "Ingest a delivery",
    keywords: "ingest,delivery,file,custodian,admit",
  },
];

export function ticketsFor(kind: BookKind): readonly Ticket[] {
  return kind === "PERSONAL" ? PERSONAL_TICKETS : FUND_TICKETS;
}

/**
 * Where a screen lives for one book and one book of record.
 *
 * ⛔ A LITERAL `views` SEGMENT, never `/books/{book}/{view}/…`. Next resolves
 * static segments before dynamic ones, so a view a book happens to name `config`
 * or `rules` would silently shadow that screen. `routes.ts` makes the same point
 * where it declares the view layer.
 */
export function screenHref(
  id: string,
  view: string,
  s: Screen,
  root: "books" | "funds" = "books",
): string {
  return s.scoped
    ? `/${root}/${id}/views/${view}/${s.segment}`
    : `/${root}/${id}/${s.segment}`;
}
