"use client";

import { KBarPortal } from "kbar/lib/KBarPortal";
import { KBarResults } from "kbar/lib/KBarResults";
import { KBarSearch } from "kbar/lib/KBarSearch";
import { useMatches } from "kbar/lib/useMatches";
import { VisualState, useKBar, type ActionImpl } from "@/lib/kbar";

// The palette's chrome, and everything expensive kbar has.
//
// ⛔ THIS MODULE IS LOADED ON FIRST ⌘K, NOT ON PAGE LOAD, and the imports above
// are why it is a separate file. `useMatches` requires `fuse.js` and
// `KBarResults` requires `@tanstack/react-virtual`; kbar is CommonJS, so nothing
// tree-shakes them away — they are only avoidable by not importing them until
// they are needed. See `@/lib/kbar` for the require graph and the measurement.
//
// ⚠ SO IT MUST NOT BE IMPORTED STATICALLY FROM ANYWHERE. `Palette.tsx` reaches
// it through `next/dynamic`, and an ordinary `import { PaletteUI }` in any file
// under `src/` undoes the split without changing a line of this one.

/** One result row. Sections arrive as plain strings in the same flat list. */
function PaletteRow({ item, active }: { item: ActionImpl | string; active: boolean }) {
  if (typeof item === "string") {
    return <div className="cmdsec">{item}</div>;
  }
  return (
    <div className={active ? "cmdrow on" : "cmdrow"}>
      <span className="k">{item.name}</span>
      {item.subtitle ? <span className="s num">{item.subtitle}</span> : null}
    </div>
  );
}

function PaletteResults() {
  const { results } = useMatches();
  return (
    <>
      {/* ⛔ ALWAYS RENDERED, EVEN WITH NOTHING IN IT. `KBarSearch` sets
          `aria-controls` to the id of the listbox this draws; swapping it for an
          empty-state element leaves a combobox pointing at an id that is not in
          the document. The message goes BESIDE it. */}
      <KBarResults
        items={results}
        maxHeight={380}
        onRender={({ item, active }) => <PaletteRow item={item} active={active} />}
      />
      {results.length === 0 ? (
        <p className="cmdempty" aria-live="polite">
          Nothing matches.
        </p>
      ) : null}
    </>
  );
}

export function PaletteUI() {
  const { query } = useKBar();

  return (
    <KBarPortal>
      <div
        className="cmdscrim"
        onPointerDown={(e) => {
          // Close on outer click, which is what `KBarAnimator` was doing for us.
          if (e.target === e.currentTarget) {
            query.setVisualState(VisualState.animatingOut);
          }
        }}
      >
        <div
          className="cmdbox"
          role="dialog"
          aria-modal="true"
          aria-label="Command palette"
          // ⭐ THE WHOLE FOCUS TRAP, AND IT IS COMPLETE. This dialog holds exactly
          // one focusable node — the input. The rows are divs that kbar points at
          // with `aria-activedescendant`, so there is nowhere for Tab to go, and
          // saying so costs less than a focus-trap library. kbar's own
          // `useFocusHandler` already restores focus to whatever had it on close.
          onKeyDown={(e) => {
            if (e.key === "Tab") e.preventDefault();
          }}
        >
          <KBarSearch
            className="cmdin"
            aria-label="Search screens, funds and ids"
            aria-autocomplete="list"
            defaultPlaceholder="Go to a screen, a fund, a book of record — or paste an id"
          />
          <PaletteResults />
        </div>
      </div>
    </KBarPortal>
  );
}
