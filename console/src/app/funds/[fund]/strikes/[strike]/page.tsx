import Link from "next/link";
import { caller } from "@/lib/caller";
import { money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { getNavStrike } from "@/wire/client";

export const dynamic = "force-dynamic";

/** One strike, and the journal prefix it was folded from. */
export default async function StrikeDetail({
  params,
}: {
  params: Promise<{ fund: string; strike: string }>;
}) {
  const { fund, strike } = await params;
  const c = await caller();
  const s = await or404(getNavStrike(c, fund, strike));

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
        <Link className="signin-btn" href={`/funds/${fund}/strikes/${strike}/replay`}>
          Replay this strike
        </Link>
      </div>
    </aside>
  );
}
