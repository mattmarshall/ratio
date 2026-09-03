// The screens under one fund, and where each one lives.
//
// ⛔ THIS LIST WAS INSIDE `components/ScreenTabs.tsx` AND HAS TWO READERS NOW.
// The command palette offers the same eight screens the header draws, and a
// second copy of them is a second answer to what `scoped` means for each. That
// is the shape of drift `funds/[fund]/layout.tsx` records about `defaultView`:
// "one source for a fact is the point of the whole feature".
//
// ⚠ It lives in `lib/` rather than beside the component because a `"use client"`
// module is a bundling boundary, not a home for data. `BASIS_LABEL` in
// `lib/format.ts` is the precedent.

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
export const SCREENS: readonly Screen[] = [
  { segment: "breaks", label: "Exceptions", scoped: true, group: "book" },
  { segment: "accounts", label: "Trial balance", scoped: true, group: "book" },
  { segment: "positions", label: "Positions", scoped: true, group: "book" },
  { segment: "strikes", label: "NAV", scoped: true, group: "book" },
  { segment: "data", label: "Data", scoped: false, group: "agreement" },
  { segment: "config", label: "Configuration", scoped: false, group: "agreement" },
  { segment: "rules", label: "Rules", scoped: false, group: "agreement" },
  { segment: "changes", label: "Change log", scoped: false, group: "agreement" },
];

export const SCREEN_GROUPS: ReadonlyArray<{
  id: ScreenGroup;
  label: string;
}> = [
  { id: "book", label: "Book" },
  { id: "agreement", label: "Agreement" },
];

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
