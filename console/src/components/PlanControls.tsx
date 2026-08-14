"use client";

import { usePathname, useRouter } from "next/navigation";
import { useTransition } from "react";

/**
 * The two dials on the plan screen, in the URL.
 *
 * ⛔ A GET AND A QUERY STRING, NOT A FORM. The console's standing assertion is
 * that no screen has grown a form, and the `/scale` amendment kept it by making
 * its dials "a plain `div` producing a query string on a GET, precisely so that
 * fence did not have to be widened to admit a read-only form". Same here.
 *
 * ⛔ AND THE STATE IS THE ADDRESS. A `useState` toggle would make "the plan with
 * the rejected steps shown" something a person can see and cannot send, which is
 * the 1801-line console's defect in miniature — the one this whole application
 * exists to have fixed.
 *
 * ⚠ `startTransition`, for `FilterChips`' reason: without it, flipping a dial
 * blanks the diagram through `loading.tsx` on every click. Turning `analyze` on
 * folds a journal, so that pause is real and the previous plan should stay on
 * screen while it happens.
 *
 * ⚠ `replace`, not `push`. Looking at a plan two ways is not two places.
 */
export function PlanControls({
  analyze,
  rejected,
}: {
  analyze: boolean;
  rejected: boolean;
}) {
  const router = useRouter();
  const path = usePathname();
  const [pending, start] = useTransition();

  const go = (next: { analyze: boolean; rejected: boolean }) => {
    const q = new URLSearchParams();
    if (next.analyze) q.set("analyze", "true");
    if (next.rejected) q.set("rejected", "true");
    const s = q.toString();
    start(() => router.replace(s ? `${path}?${s}` : path));
  };

  return (
    <div className="qbar" role="group" aria-label="Plan detail">
      <button
        className="chip"
        type="button"
        aria-pressed={rejected}
        onClick={() => go({ analyze, rejected: !rejected })}
      >
        Plans not taken
      </button>
      {/* ⛔ THE EXPENSIVE ONE SAYS SO ON ITS FACE. Pressing this re-derives the
          strike — a full fold, the slowest thing the API does. A control that
          spends a period end's work should not be indistinguishable from one
          that hides four boxes. */}
      <button
        className="chip"
        type="button"
        aria-pressed={analyze}
        onClick={() => go({ analyze: !analyze, rejected })}
      >
        Measure it (re-folds the journal)
      </button>
      <span className="spacer" />
      <span className="sortnote">
        {pending
          ? analyze
            ? "folding…"
            : "refreshing…"
          : analyze
            ? "measured now, on this machine"
            : "estimated from the proved model"}
      </span>
    </div>
  );
}
