import Link from "next/link";
import { caller } from "@/lib/caller";
import { money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { requireFundOps } from "@/lib/requireFundOps";
import { getNavStrike } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/** One strike, and the journal prefix it was folded from. */
async function StrikeDetail({
  params,
}: {
  params: Promise<{ book: string; view: string; strike: string }>;
}) {
  const { book: fund, view, strike } = await params;
  await requireFundOps(fund);
  const c = await caller();
  const s = await or404(getNavStrike(c, fund, view, strike));

  return (
    <aside className="detail" aria-label="NAV strike">
      <div className="dhead">
        <h2>{money(s.netAssetValue)}</h2>
        <div className="sub">{s.valuationTime.slice(0, 16).replace("T", " ")}</div>
      </div>

      {/* ⛔ BEFORE THE FIGURES, NOT AFTER. A qualification is what makes a
          reader treat the number differently, and one placed below the number
          is one they have already accepted. */}
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

      <div className="dsec">
        <h3>What it was folded from</h3>
        <dl className="kv">
          <dt>Journal position</dt>
          <dd className="num">{s.journalPosition}</dd>
          <dt>Journal digest</dt>
          <dd className="num">{s.journalDigest.slice(0, 12)}…</dd>
          <dt>Trial balance</dt>
          <dd className="num">{money(s.trialBalanceDifference)}</dd>
          <dt>Struck by</dt>
          <dd>{s.actor}</dd>
        </dl>
      </div>

      <div className="dsec">
        <h3>The proof</h3>
        <p className="note">
          A strike pins the journal prefix it read, so it can be re-derived: the
          same prefix folds to the same answer or the history is not what it was.
          Replaying is something you ask for — a page that asserted the proof on
          load would be asserting it rather than offering it.
        </p>
        <Link className="signin-btn" href={`/books/${fund}/views/${view}/strikes/${strike}/replay`}>
          Replay this strike
        </Link>
      </div>

      {/* ⛔ THE DERIVATION IS A SECOND CITATION, AND A DIFFERENT ONE. A replay
          says the figure re-derives; this says what deriving it DOES — which
          steps ran, what each one read, and what the plans not taken would have
          cost. A number that reproduces and a number somebody can follow are
          two separate things to be able to send. */}
      <div className="dsec">
        <h3>The derivation</h3>
        <p className="note">
          A NAV is not read off the tax lots — it is folded from a journal
          prefix, and answered again off totals somebody maintains. Both costs
          are on the plan, because quoting only the flat one is the overclaim{" "}
          <code>ratio bench</code> exists to make hard.
        </p>
        <Link className="signin-btn" href={`/books/${fund}/views/${view}/strikes/${strike}/plan`}>
          How this NAV was computed
        </Link>
      </div>
    </aside>
  );
}

export default withRefusal(StrikeDetail);
