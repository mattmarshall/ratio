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

export interface Screen {
  /** The URL segment, which is also the segment `useSelectedLayoutSegments` reports. */
  readonly segment: string;
  /** What the tab and the palette call it. */
  readonly label: string;
  /** Whether the screen belongs to a book of record. See the ⚠ below. */
  readonly scoped: boolean;
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
  { segment: "breaks", label: "Exceptions", scoped: true },
  { segment: "accounts", label: "Trial balance", scoped: true },
  { segment: "positions", label: "Positions", scoped: true },
  { segment: "data", label: "Data", scoped: false },
  { segment: "strikes", label: "NAV", scoped: true },
  { segment: "config", label: "Configuration", scoped: false },
  { segment: "rules", label: "Rules", scoped: false },
  { segment: "changes", label: "Change log", scoped: false },
];

/**
 * Where a screen lives for one fund and one book of record.
 *
 * ⛔ A LITERAL `views` SEGMENT, never `/funds/{fund}/{view}/…`. Next resolves
 * static segments before dynamic ones, so a view a fund happens to name `config`
 * or `rules` would silently shadow that screen. `routes.ts` makes the same point
 * where it declares the view layer.
 */
export function screenHref(fund: string, view: string, s: Screen): string {
  return s.scoped
    ? `/funds/${fund}/views/${view}/${s.segment}`
    : `/funds/${fund}/${s.segment}`;
}
