/** One figure in the header strip, with what it is and how it reads.
 *
 * ⚠ `sub` renders inside `.v` as a `<small>`, not beside it — `globals.css`
 * styles `.stat .v small` and nothing else. Moving it out is a silent
 * restyling. */
export function Stat({
  k,
  v,
  sub,
  tone,
}: {
  k: string;
  v: string;
  sub?: string | undefined;
  tone?: "tied" | "at-risk" | undefined;
}) {
  return (
    <div className={`stat${tone ? ` ${tone}` : ""}`}>
      <div className="k">{k}</div>
      <div className="v num">
        {v}
        {sub ? <small>{sub}</small> : null}
      </div>
    </div>
  );
}
