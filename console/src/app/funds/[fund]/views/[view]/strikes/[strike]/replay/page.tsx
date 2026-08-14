import { caller } from "@/lib/caller";
import { money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { getNavStrike, replayNavStrike } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * A URL for a proof.
 *
 * ⭐ THIS IS WHY THE CONSOLE NEEDED ROUTES. Replaying a strike re-folds the
 * journal prefix the strike pinned and reports two things: whether the history
 * is intact, and whether the figure reproduced. In the old console that was a
 * button and a piece of component state — a proof somebody had to be watching
 * to have seen. Here it is an address, which is what a citation is.
 *
 * ⚠ A REPLAY IS A FOLD, AND ON A LARGE BOOK IT IS THE SLOWEST THING THE API
 * DOES. Two 15-second ceilings stack behind it. That is the cost of it being
 * real rather than asserted.
 */
export default async function Replay({
  params,
}: {
  params: Promise<{ fund: string; view: string; strike: string }>;
}) {
  const { fund, view, strike } = await params;
  const c = await caller();
  const s = await or404(getNavStrike(c, fund, view, strike));
  const r = await replayNavStrike(c, fund, view, strike);

  const agrees = r.netAssetValue === s.netAssetValue;

  return (
    <aside className="detail" aria-label="Replay">
      <div className="dhead">
        <h2>Replay</h2>
        <div className="sub">{s.valuationTime.slice(0, 16).replace("T", " ")}</div>
      </div>

      <div className="dsec">
        <h3>What the replay says</h3>
        <dl className="kv">
          <dt>History intact</dt>
          <dd className={r.historyIntact ? "good" : "crit"}>
            {r.historyIntact ? "yes" : "no"}
          </dd>
          <dt>Reproduced</dt>
          <dd className={r.reproduced ? "good" : "crit"}>
            {r.reproduced ? "yes" : "no"}
          </dd>
          <dt>Recorded</dt>
          <dd className="num">{money(s.netAssetValue)}</dd>
          <dt>Re-derived</dt>
          <dd className="num">{money(r.netAssetValue)}</dd>
          <dt>Journal digest</dt>
          <dd className="num">{r.journalDigest.slice(0, 12)}…</dd>
        </dl>
      </div>

      {/* ⛔ TWO SEPARATE CLAIMS, AND THE SECOND IS THE DANGEROUS ONE. "History
          intact" says the prefix still hashes to what the strike recorded.
          "Reproduced" says folding it again gives the same figure. A book can
          report an intact history and a different number — that is what a
          changed configuration does — and reading the two as one claim is how a
          restatement goes unnoticed. */}
      <div className="dsec">
        <h3>What that means</h3>
        <p className="note">
          {r.historyIntact
            ? "The journal prefix this strike pinned still hashes to what it recorded — nothing behind it has been rewritten."
            : "The journal prefix this strike pinned no longer hashes to what it recorded. The history behind this figure is not the history it was struck from."}
        </p>
        <p className="note">
          {agrees
            ? "Re-folding that prefix gives the same net asset value. The figure is not asserted; it is derived, and it derives again."
            : "Re-folding that prefix gives a different net asset value. An intact history and a different figure is what a changed configuration produces — read the configuration version before reading the difference."}
        </p>
      </div>
    </aside>
  );
}
