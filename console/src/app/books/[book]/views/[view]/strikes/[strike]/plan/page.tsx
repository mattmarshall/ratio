import { PlanControls } from "@/components/PlanControls";
import { PlanGraph } from "@/components/PlanGraph";
import { caller } from "@/lib/caller";
import { count, money, nanos, reads } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { visible } from "@/lib/planLayout";
import { explainNavStrike, getNavStrike } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/**
 * How this NAV was computed, and what the alternatives would have cost.
 *
 * ⛔ WHAT THIS IS NOT: a plan the engine chose. Nothing in Ratio selects between
 * the two paths below — `ratio strike` folds the journal, `Projection::nav`
 * reads maintained totals, and a caller picks one by calling it. `Ratio.Plan`
 * proves the two agree and is not emitted into Rust at all. This screen writes
 * down what each path does and attaches the cost the model proves or the fold
 * measures, and it says so in as many words rather than letting a diagram imply
 * a planner that does not exist.
 *
 * ⛔ AND BOTH CURVES ARE ON IT, ALWAYS. `ratio bench` "reports two curves and
 * both must be quoted"; the flat one is the persuasive number and the growing
 * one is what somebody actually waits for. Collapsing the plans not taken hides
 * sub-graphs and never the comparison — the strip below renders whatever the
 * dials say.
 */
async function Plan({
  params,
  searchParams,
}: {
  params: Promise<{ book: string; view: string; strike: string }>;
  searchParams: Promise<{ analyze?: string; rejected?: string }>;
}) {
  const { book: fund, view, strike } = await params;
  const qs = await searchParams;
  // ⛔ ONLY THE LITERAL `true` TURNS EITHER ON, and for `analyze` that matters:
  // the parameter decides whether this request folds a journal, so anything
  // ambiguous must fall to the cheap answer. The transcoder makes the same
  // judgement on the other side of the wire.
  const analyze = qs.analyze === "true";
  const rejected = qs.rejected === "true";

  const c = await caller();
  const s = await or404(getNavStrike(c, fund, view, strike));
  const plan = await explainNavStrike(c, fund, view, strike, analyze);
  const nodes = visible(plan, rejected);

  return (
    <aside className="detail" aria-label="How this NAV was computed">
      <div className="dhead">
        <h2>How this NAV was computed</h2>
        <div className="sub">
          {money(s.netAssetValue)} · {s.valuationTime.slice(0, 16).replace("T", " ")}
        </div>
      </div>

      {/* ⛔ BEFORE THE FIGURES, NOT AFTER, exactly as the strike screen places
          it. A qualification below the number is one the reader has already
          accepted. */}
      {s.qualification.length ? (
        <div className="dsec">
          <h3>Qualification</h3>
          <ul className="note">
            {s.qualification.map((q) => (
              <li key={q}>{q}</li>
            ))}
          </ul>
        </div>
      ) : null}

      <PlanControls analyze={analyze} rejected={rejected} />

      {/* ⛔ THE THREE COSTS, TOGETHER, WHATEVER THE DIALS SAY. This is the whole
          reason hiding the plans not taken is safe: what a collapse buys is a
          smaller picture, not a friendlier number. */}
      <div className="dsec">
        <h3>What it reads, three ways</h3>
        <dl className="kv plancompare">
          <dt>This plan</dt>
          <dd className="num good">{reads(plan.chosenReads)}</dd>
          <dt>Applying the open actions by rewriting the lots</dt>
          <dd className="num">{reads(plan.rewriteReads)}</dd>
          <dt>Folding every open tax lot</dt>
          <dd className="num">{reads(plan.scanReads)}</dd>
        </dl>
        <p className="note">
          Reads, not seconds — the counts are proved in <code>Ratio.Closure</code>{" "}
          and <code>Ratio.Plan</code>. None of the three includes capital
          activity: counting subscriptions and redemptions needs the chart roles,
          which the maintained projection does not have, so the term is excluded
          rather than guessed at.
        </p>
      </div>

      {plan.estimateRefusal ? (
        /* ⛔ THE SERVER'S OWN WORDS, RENDERED. The maintained projection folds
           the whole journal with no cut, so it cannot describe a trade- or
           settlement-basis view — and showing the recorded view's shape under
           this view's name is precisely the defect views exist to prevent. The
           reconcile screen already refuses in the same posture. */
        <div className="dsec">
          <h3>No estimate for this book of record</h3>
          <p className="note">{plan.estimateRefusal}</p>
          <p className="note">
            The measurement still works: the fold does cut, so “Measure it” above
            answers for this view even though the model cannot.
          </p>
        </div>
      ) : plan.dials ? (
        <div className="dsec">
          <h3>The fund this was costed over</h3>
          <dl className="kv">
            <dt>Securities</dt>
            <dd className="num">{count(plan.dials.securities)}</dd>
            {/* ⛔ SEPARATE FROM THE SECURITIES, AND NEVER MULTIPLIED INTO ONE
                FIGURE. 500 × 40,000 and 10,000 × 2,000 are both twenty million
                open lots and are not the same fund — one price is read per
                security, so they differ twentyfold in the term that grows with
                the chart. */}
            <dt>Open lots per security</dt>
            <dd className="num">{count(plan.dials.lotsPer)}</dd>
            <dt>Currencies</dt>
            <dd className="num">{count(plan.dials.currencies)}</dd>
            <dt>Open corporate actions</dt>
            <dd className="num">{count(plan.dials.openActions)}</dd>
            <dt>Open tax lots</dt>
            <dd className="num">{count(plan.dials.openLots)}</dd>
            <dt>Journal prefix</dt>
            <dd className="num">{count(s.journalPosition)}</dd>
          </dl>
        </div>
      ) : null}

      <PlanGraph plan={plan} nodes={nodes} />

      <div className="dsec prov">
        <h3>Where these numbers come from</h3>
        <p className="note">
          Reads are proved. Seconds are measured, and a rate is a property of a
          machine on a day: {nanos(plan.nanosPerRead)} per read —{" "}
          {plan.provenance}
        </p>
        <p className="note">
          {plan.analyzed
            ? "The measured figures are this machine re-deriving the pinned prefix now. They are not what the original strike cost: nothing was recorded at strike time, and a duration presented as the strike's own would be a fabrication."
            : "Nothing here has been measured. Every figure above is the proved model multiplied by the rate, and a step the model does not cost is blank rather than zero."}
        </p>
      </div>
    </aside>
  );
}

export default withRefusal(Plan);
