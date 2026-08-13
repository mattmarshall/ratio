"use client";

import { usePathname, useRouter } from "next/navigation";
import { useTransition } from "react";

export interface Filter {
  readonly key: string;
  readonly label: string;
}

/**
 * Narrow a list, without blanking it.
 *
 * ⛔ THE TRANSITION IS THE WHOLE POINT OF THIS COMPONENT, AND LEAVING IT OUT IS
 * A REGRESSION THAT LOOKS LIKE A LOADING STATE. The console this replaces set
 * `placeholderData: (prev) => prev` on these queries and said why: "without it
 * every chip click blanks the queue, which reads as 'there is nothing here' for
 * as long as the round trip takes." On a queue of exceptions that is the worst
 * possible wrong answer.
 *
 * The naive port puts the filter in the URL and lets the route re-render, which
 * shows `loading.tsx` on every click — the identical defect, reintroduced by a
 * different mechanism. `startTransition` is what keeps the previous rows on
 * screen while the new ones are fetched: React holds the old UI until the new
 * one is ready, and `isPending` is how the page says so honestly.
 *
 * ⚠ `replace`, not `push`. Narrowing a list is not a place in history; a person
 * who filters three times and hits Back means to leave the screen.
 */
export function FilterChips({
  filters,
  active,
  label,
  note,
}: {
  filters: readonly Filter[];
  active: string;
  label: string;
  note: string;
}) {
  const router = useRouter();
  const path = usePathname();
  const [pending, start] = useTransition();

  return (
    <div className="qbar" role="group" aria-label={label}>
      {filters.map((f) => (
        <button
          key={f.key}
          className="chip"
          type="button"
          aria-pressed={active === f.key}
          onClick={() =>
            start(() => {
              router.replace(f.key ? `${path}?filter=${encodeURIComponent(f.key)}` : path);
            })
          }
        >
          {f.label}
        </button>
      ))}
      <span className="spacer" />
      <span className="sortnote">{pending ? "refreshing…" : note}</span>
    </div>
  );
}
