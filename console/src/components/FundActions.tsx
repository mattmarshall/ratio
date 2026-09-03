"use client";

import { useSelectedLayoutSegments } from "next/navigation";
import { Priority, useKBar, useRegisterActions, type Action } from "@/lib/kbar";
import { candidatesForId, hrefForResourceName } from "@/lib/deeplink";
import { BASIS_LABEL } from "@/lib/format";
import { defaultScreen, screenHref, screensFor, ticketsFor } from "@/lib/screens";
import type { BookKind, View } from "@/wire/types";
import { usePaletteNavigator } from "./Palette";

/**
 * What the palette can reach while a fund is open.
 *
 * ⚠ A CLIENT COMPONENT FOR ONE REASON, and it is `ViewSwitch`'s reason: to know
 * which view is open. The list of views is fetched on the server by
 * `books/[book]/layout.tsx` and handed down; nothing here calls the API, and the
 * palette costs this console no upstream request at all.
 *
 * ⛔ THE OPEN VIEW IS READ FROM THE SEGMENTS, NOT TAKEN AS A PROP. A layout
 * persists across navigation within its segment — that is the whole reason
 * `books/[book]/layout.tsx` is allowed one `GetBook` for every screen beneath it
 * — so it is NOT re-run when an operator switches `/views/abor/… → /views/ibor/…`.
 * A `view` prop captured at layout render would keep pointing at the book of
 * record they left, and every action here would quietly navigate to the wrong
 * one. `useSelectedLayoutSegments` re-reads on every navigation.
 *
 * ⚠ THE SEGMENTS ARE RELATIVE TO THE LAYOUT THAT RENDERED THE CALLER. This
 * component is rendered by `books/[book]/layout.tsx`, so it sees
 * `["views","abor","breaks"]` — the same three `ViewSwitch` reads. The
 * collection layout one level up sees the book id first instead.
 */
export function FundActions({
  fund,
  views,
  defaultView,
  kind = "INVESTMENT",
}: {
  fund: string;
  views: View[];
  defaultView: string;
  kind?: BookKind;
}) {
  const go = usePaletteNavigator();
  const segments = useSelectedLayoutSegments();
  // ⚠ `noUncheckedIndexedAccess`: every index into this is possibly undefined.
  const underView = segments[0] === "views";
  const view = (underView ? segments[1] : undefined) ?? defaultView;
  // Stay on the same screen when switching books, exactly as `ViewSwitch` does.
  // A control-plane screen is not view-scoped, so switching from one lands on
  // the kind's first figure.
  const places = screensFor(kind);
  const screen = (underView ? segments[2] : undefined) ?? defaultScreen(kind);

  const actions: Action[] = [
    // ⛔ NO `subtitle` ON A SCREEN, AND THE REASON IS THE SEARCH RATHER THAN THE
    // LAYOUT. kbar searches `subtitle` as well as `name` and `keywords`, with
    // `ignoreLocation`, so a substring anywhere in the field matches. Every one
    // of these URLs carries the fund id — and putting it here made typing any
    // fragment of a fund's name ("global") match all eight screens, all four
    // tickets and both books of record. The label is the answer; the URL is
    // where it goes.
    ...places.map((s) => ({
      id: `screen:${s.segment}`,
      name: s.label,
      keywords: `${s.segment},screen,tab`,
      section: {
        name: s.group === "book" ? "Book" : "Agreement",
        priority: Priority.HIGH,
      },
      perform: go(screenHref(fund, view, s, "books")),
    })),

    // ⭐ A VIEW IS A PROPERTY OF THE ANSWER, and the palette says so the way the
    // header does: switching one keeps the screen, because every figure on that
    // screen means something different under the other book.
    ...views.map((v) => {
      const id = v.name.split("/").pop() ?? defaultView;
      const href = `/books/${fund}/views/${id}/${screen}`;
      return {
        id: `view:${id}`,
        // ⛔ AN UNDECLARED VIEW SAYS SO, HERE TOO. A book whose configuration
        // declares nothing has exactly one view, recognising entries in the
        // journal's own order — and labelling that as an elected basis asserts a
        // decision nobody made. `ViewSwitch` renders the same caveat as a pip.
        name: v.declared ? v.displayName : `${v.displayName} (default)`,
        // ⚠ The basis, not the URL — see the ⛔ on the screens above. Which
        // convention recognises the entries is the thing a reader is choosing
        // between; the href would only make every view match every fund name.
        subtitle: BASIS_LABEL[v.basis],
        keywords: `${id},view,book of record,basis`,
        section: { name: "Book of record", priority: Priority.HIGH },
        perform: go(href),
      };
    }),

    // ⛔ THESE OPEN A TICKET AND NOTHING ELSE. `/record` and `/trade` are one RPC
    // asked for two ways, and all four screens enforce "preview, then post" on
    // themselves. The palette is a way to reach the form, never a way past it.
    ...ticketsFor(kind).map((t) => ({
      id: `write:${t.segment}`,
      name: t.label,
      keywords: t.keywords,
      section: { name: "Record", priority: Priority.NORMAL },
      perform: go(`/books/${fund}/${t.segment}`),
    })),
  ];

  useRegisterActions(actions, [
    fund,
    view,
    screen,
    kind,
    views.map((v) => `${v.name}:${v.declared}`).join(","),
    go,
  ]);

  return <DeepLinks fund={fund} view={view} />;
}

/**
 * A pasted identifier, turned into somewhere to go.
 *
 * ⛔ ITS OWN COMPONENT SO ONLY IT RE-RENDERS PER KEYSTROKE. This is the one place
 * in the palette that subscribes to the search query, and a subscriber re-renders
 * on every character typed. Keeping the static registrations above out of that
 * path is what stops eleven actions being rebuilt eleven times a word.
 */
function DeepLinks({ fund, view }: { fund: string; view: string }) {
  const go = usePaletteNavigator();
  const { search } = useKBar((state) => ({ search: state.searchQuery }));

  // ⚠ THE FIRST TOKEN IS THE ID; THE REST IS THE OPERATOR NARROWING. kbar
  // searches with `tokenMatch: "all"`, so typing `ACME position` keeps only the
  // candidate carrying both words — which is how a person disambiguates a
  // namespace this console cannot look up.
  const token = search.trim().split(/\s+/)[0] ?? "";

  const exact = hrefForResourceName(token);
  const actions: Action[] = exact
    ? [
        {
          id: "jump:exact",
          name: `Open ${token}`,
          subtitle: exact,
          keywords: "resource,name,paste,open",
          // ⭐ NOT A GUESS, SO IT OUTRANKS EVERYTHING. The string itself says
          // which resource it names.
          section: { name: "Open", priority: Priority.HIGH },
          perform: go(exact),
        },
      ]
    : candidatesForId(fund, view, token).map((c) => ({
        id: `jump:${c.key}`,
        name: c.label,
        // ⛔ THE URL, SHOWN RATHER THAN ASSERTED. Nothing here has checked that
        // this resource exists — there is no search RPC and the browser may not
        // call the API — so the row shows where it would go and lets the reader
        // judge. A row reading "Break cash-usd-2026-02-26" would be a claim about
        // the book.
        subtitle: c.href,
        keywords: c.keywords,
        section: { name: "Open by id", priority: Priority.LOW },
        perform: go(c.href),
      }));

  useRegisterActions(actions, [fund, view, token, go]);
  return null;
}
