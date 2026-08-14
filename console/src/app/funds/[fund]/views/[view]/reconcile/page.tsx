import { caller } from "@/lib/caller";
import { isoDate } from "@/lib/dates";
import { money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { reconcileViews } from "@/wire/client";
import type { RecognitionDifference } from "@/wire/types";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/**
 * What two books of record over one journal disagree about, entry by entry.
 *
 * ⭐ THE SCREEN THIS WHOLE FEATURE IS FOR, AND IT IS A DERIVATION RATHER THAN A
 * RECONCILIATION. `Ratio.Views.two_views_differ_by_exactly_what_is_in_flight`
 * says each view is the other plus the entries the other has not recognised
 * yet — so the two lists below account for the difference EXACTLY, and the
 * sums are rendered rather than asserted.
 *
 * ⚠ AND IT IS ONE UPSTREAM CALL, NOT TWO SUBTRACTED. Fetching two views and
 * differencing them in the browser would compare figures at two journal
 * positions, and the gap would be partly a settlement convention and partly one
 * view being behind, in one number, with nothing saying which part is which.
 * `//tla:views_at_two_prefixes_check` is that failure as a model run.
 */
async function Reconcile({
  params,
  searchParams,
}: {
  params: Promise<{ fund: string; view: string }>;
  searchParams: Promise<{ against?: string }>;
}) {
  const { fund, view } = await params;
  const { against } = await searchParams;
  const c = await caller();

  if (!against) {
    return (
      <div className="empty">
        Pick a view to compare against — this screen answers a question about
        two of them.
      </div>
    );
  }

  const r = await or404(reconcileViews(c, fund, view, against));
  const here = r.name.split("/").pop()!;
  const there = r.against.split("/").pop()!;

  return (
    <section className="log" aria-label="View reconciliation">
      <div className="loghead">
        <span>
          {here} against {there}
        </span>
        <span className="sortnote">
          folded from journal position {r.journalPosition}
        </span>
      </div>

      {/* The arithmetic on one line, so a reader is never asked to trust it. */}
      <div className="recsum">
        <span className="num">{money(r.netAssetValue)}</span>
        <span className="op">−</span>
        <span className="num">{money(r.againstNetAssetValue)}</span>
        <span className="op">=</span>
        <b className="num">{money(r.difference)}</b>
      </div>

      <Rows
        title={`Recognised in ${here}, not yet in ${there}`}
        rows={r.recognisedHere}
      />
      <Rows
        title={`Recognised in ${there}, not yet in ${here}`}
        rows={r.recognisedThere}
      />
      {/* ⛔ THE THIRD BLOCK IS NOT OPTIONAL. An entry NEITHER view can place —
          no trade date, or a pinned configuration that does not declare one of
          them — contributes to neither figure above. Leaving it off would make
          the difference look fully explained when it is not, which is the shape
          of every defect in HANDOFF.md's table. */}
      {r.unplaceable.length ? (
        <Rows
          title="Neither view can place these"
          rows={r.unplaceable}
        />
      ) : null}
    </section>
  );
}

function Rows({
  title,
  rows,
}: {
  title: string;
  rows: RecognitionDifference[];
}) {
  // Rendered so the render suite can assert the list adds to the difference —
  // a screen that shows a difference it cannot itemize is one to distrust.
  const total = rows.reduce((n, r) => n + BigInt(r.netAssetValueEffect), 0n);
  return (
    <>
      <div className="loghead">
        <span>{title}</span>
        <span className="sortnote">{rows.length || "none"}</span>
      </div>
      {rows.length === 0 ? (
        <div className="empty">Nothing in flight.</div>
      ) : null}
      {rows.map((r) => (
        <div className="logrow" key={r.entryId}>
          <span className="t num">
            {r.tradeDate ? isoDate(r.tradeDate) : "no trade date"}
          </span>
          <span className="w">
            <b>{r.memo || r.entryId}</b>
            <div className="cfg">
              here {isoDate(r.recognisedHere)} · there{" "}
              {isoDate(r.recognisedThere)}
            </div>
          </span>
          <span className="amt num">{money(r.netAssetValueEffect)}</span>
        </div>
      ))}
      {rows.length ? (
        <div className="logrow recfoot">
          <span className="t" />
          <span className="w">Contributing</span>
          <span className="amt num">
            <b>{money(total.toString())}</b>
          </span>
        </div>
      ) : null}
    </>
  );
}

export default withRefusal(Reconcile);
