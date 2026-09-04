"use client";

import dynamic from "next/dynamic";
import { useRouter } from "next/navigation";
import { startTransition, useCallback, type ReactNode } from "react";
import {
  KBarProvider,
  Priority,
  VisualState,
  useKBar,
  useRegisterActions,
  type Action,
  type KBarOptions,
} from "@/lib/kbar";
import type { Fund } from "@/wire/types";

/**
 * ⌘K over the screens, the funds, the books of record and the four tickets.
 *
 * ⛔ IT NAVIGATES AND IT NEVER WRITES. Every action here ends in a `router.push`.
 * `Ticket`'s "preview, then post" is enforced on the screen that owns each write
 * — the commit stays shut until the previewed inputs match what the screen holds
 * — and an action that submitted from a command palette would be a second way
 * round a fence the README spends a paragraph on.
 *
 * ⛔ AND IT HOLDS NO FIGURE. The palette is a way to reach a URL, not a place
 * state lives. `routes.ts` argues that a screen must be citable; a palette that
 * accumulated filters or selections would be the 1801-line `useState` coming
 * back through a door marked "convenience".
 *
 * ⛔ THE PANEL IS THIS CONSOLE'S, AND kbar SHIPS TWO COMPONENTS IT DOES NOT USE.
 * `KBarAnimator` renders its outer element with an inline `opacity:0` and becomes
 * visible only once `Element.animate(…, {fill:"forwards"})` has run, so anywhere
 * the Web Animations API is missing — including the render suite — it draws a
 * palette that is present, focusable and invisible; it also constructs a
 * `ResizeObserver` unguarded. `KBarPositioner` is typed to accept only
 * `className` and `style`, so `role="dialog"` on it does not compile. What they
 * bought was an entrance and close-on-outer-click: the first is nine lines of
 * CSS in `globals.css`, the second is three lines in `PaletteUI.tsx`.
 */

// ⚠ MODULE SCOPE, BECAUSE kbar READS THIS ONCE. `useStore` captures the options
// on its first render and never looks again, so an object rebuilt per render
// would be silently ignored — which reads as "the option does not work".
const PALETTE_OPTIONS: KBarOptions = {
  // ⛔ THE DOCUMENT LOCK IS OFF, AND ITS SCROLLBAR MANAGEMENT IS THE REASON. kbar
  // sets `document.body.style.marginRight = getScrollbarWidth()` while it is
  // open. Nothing in this console scrolls the body — `.app` is `height:100%` and
  // the scrolling boxes are `.funds` and `.queue` — so the lock prevents nothing
  // and the margin shifts the whole three-column grid sideways and back on every
  // ⌘K.
  disableDocumentLock: true,
  // ⚠ Zero, because the entrance is a CSS animation that
  // `prefers-reduced-motion` can reach. These timers only delay mount/unmount.
  animations: { enterMs: 0, exitMs: 0 },
};

// ⛔ LOADED ON FIRST OPEN, AND THE GATE BELOW IS HALF OF WHAT MAKES THAT TRUE.
// `PaletteUI` carries `fuse.js` and `@tanstack/react-virtual` — 28.0 KB gzipped
// that kbar's CommonJS build will not let `next build` shake out. `next/dynamic`
// only defers a module nobody renders, so `PaletteMount` must not render this
// until the palette is actually showing; rendering it unconditionally and
// letting `KBarPortal` return null would fetch the chunk on every page load and
// change nothing.
//
// ⚠ `ssr: false` COSTS NOTHING HERE. The panel is hidden on first paint by
// definition, so there is no server render to lose. And the chunk load is CSP-
// clean: `script-src 'strict-dynamic'` with the nonce is precisely what lets the
// nonced bootstrap pull chunks — `proxy.ts` says so where it sets the policy.
const PaletteUI = dynamic(() => import("./PaletteUI").then((m) => m.PaletteUI), {
  ssr: false,
});

/** Every palette navigation goes through here, and nothing else may. */
export function usePaletteNavigator(): (href: string) => () => void {
  const router = useRouter();
  // ⛔ `useCallback`, AND IT IS NOT A MICRO-OPTIMISATION. Callers put this in a
  // `useRegisterActions` dependency list, which memoizes on identity — a
  // navigator rebuilt every render re-registers every action, which sets store
  // state, which re-renders the subscriber, which rebuilds the navigator. That is
  // a loop, not a slow render.
  return useCallback(
    (href: string) => () =>
      // ⛔ `push`, NOT `replace`. `FilterChips` replaces and says why — "narrowing
      // a list is not a place in history". This is the opposite act: every
      // destination the palette offers is one `FundRail`, `ScreenTabs` or
      // `ViewSwitch` already reaches with a <Link>, and a <Link> pushes. If this
      // replaced, the palette would be the one control here whose result Back
      // cannot undo.
      //
      // ⛔ AND INSIDE A TRANSITION, for FilterChips' reason reached by a
      // different door. `KBarResults` performs the action and closes the panel
      // immediately, so without this the operator watches `loading.tsx` blank the
      // queue they just left — "on a queue of exceptions that is the worst
      // possible wrong answer". Nothing renders `isPending`, so this is the
      // module-level `startTransition` rather than the hook.
      startTransition(() => router.push(href)),
    [router],
  );
}

/**
 * The funds, registered from inside the provider.
 *
 * ⛔ A CHILD, BECAUSE `useKBar` IN THE COMPONENT THAT RENDERS `KBarProvider`
 * READS THE CONTEXT ABOVE IT. kbar has the same problem and solves it the same
 * way, with its own `<InternalEvents/>`.
 *
 * ⚠ AND NOT THE PROVIDER'S `actions` PROP, which `useStore` also captures once.
 * A fund list that changed on a refresh would never reach the palette.
 */
function FundRegistrar({ funds }: { funds: Fund[] }) {
  const go = usePaletteNavigator();

  const actions: Action[] = funds.map((f) => {
    const id = f.name.replace(/^funds\//, "");
    return {
      id: `fund:${id}`,
      name: f.displayName,
      // ⛔ WHERE THE RAIL GOES, CHARACTER FOR CHARACTER. `FundRail` links
      // straight to the fund's default view rather than through the legacy
      // redirect, because "the rail is the commonest way in, and a redirect on
      // every click is a round trip nobody needs". A palette that used the old
      // URL would pay that cost on every jump.
      subtitle: `/books/${id}/views/${f.defaultView}/breaks`,
      // ⚠ COMMAS, NOT SPACES. kbar reads this field as `keywords.split(",")`, so
      // a space-separated string is one long token that matches nothing. The raw
      // id is here so `harbourline-global-value` finds it as well as the name.
      keywords: `${id},fund,book`,
      section: { name: "Funds", priority: Priority.HIGH },
      perform: go(`/books/${id}/views/${f.defaultView}/breaks`),
    };
  });

  // ⚠ A PRIMITIVE DEP, NOT THE ARRAY. `useRegisterActions` memoizes on this
  // list, and `funds` is a fresh array on every render of the layout above — so
  // depending on it directly would re-register on every render. `go` is stable
  // by `useCallback`, which is the other half of the same requirement.
  useRegisterActions(actions, [funds.map((f) => f.name).join(","), go]);
  return null;
}

/**
 * ⛔ NOT BARE RESOURCE IDS. DeepLinks takes the first search token as an
 * id and offers "Open … as an exception". "independent" is how a reader
 * reaches the book collection (the test cannot type "your books" —
 * that also matches break id `your`). Treating the keyword as a break
 * id sent that click to `/breaks/independent`.
 */
export const WORKSPACE_DOOR_TOKENS = new Set([
  "independent",
  "books",
  "personal",
  "project",
  "projects",
  "workspace",
]);

/**
 * The book collection, independent of any fund.
 *
 * ⭐ A PERSONAL OR PROJECT BOOK HAS NO `/funds/{fund}` URL. The rail only
 * lists books that carry a fund layer. These two actions are the door that
 * does not require one.
 */
function BookDoors() {
  const go = usePaletteNavigator();
  useRegisterActions(
    [
      {
        id: "books:list",
        name: "Your books",
        subtitle: "/books",
        keywords: "books,personal,project,independent",
        section: { name: "Books", priority: Priority.HIGH },
        perform: go("/books"),
      },
      {
        id: "books:new",
        name: "New book",
        subtitle: "/books/new",
        keywords: "create,new,independent,template",
        section: { name: "Books", priority: Priority.HIGH },
        perform: go("/books/new"),
      },
      {
        id: "projects:list",
        name: "Your projects",
        subtitle: "/projects",
        keywords: "projects,project,workspace",
        section: { name: "Books", priority: Priority.HIGH },
        perform: go("/projects"),
      },
    ],
    [go],
  );
  return null;
}

/** Renders the expensive half, and only once somebody has asked for it. */
function PaletteMount() {
  const { showing } = useKBar((state) => ({
    showing: state.visualState !== VisualState.hidden,
  }));
  return showing ? <PaletteUI /> : null;
}

export function Palette({ funds, children }: { funds: Fund[]; children: ReactNode }) {
  return (
    <KBarProvider options={PALETTE_OPTIONS}>
      <FundRegistrar funds={funds} />
      <BookDoors />
      {children}
      <PaletteMount />
    </KBarProvider>
  );
}
