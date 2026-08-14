import { Suspense } from "react";
import { LotBook } from "@/components/LotBook";
import { caller } from "@/lib/caller";
import { isoDate } from "@/lib/dates";
import { count, money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { getPosition } from "@/wire/client";

export const dynamic = "force-dynamic";

export default async function PositionDetail({
  params,
}: {
  params: Promise<{ fund: string; view: string; position: string }>;
}) {
  const { fund, view, position } = await params;
  const c = await caller();
  const p = await or404(getPosition(c, fund, view, position));

  return (
    <aside className="detail" aria-label="Position detail">
      <div className="dhead">
        <h2>{p.instrumentLabel || p.instrument}</h2>
        <div className="sub">{p.accountLabel || p.account}</div>
      </div>

      <div className="dsec">
        <h3>The holding</h3>
        <dl className="kv">
          <dt>Quantity</dt>
          <dd className="num">{count(p.quantity)}</dd>
          <dt>Value</dt>
          <dd className="num">{money(p.value)}</dd>
          <dt>Open lots</dt>
          <dd className="num">{count(p.openLotCount)}</dd>
          <dt>Marked</dt>
          <dd>{isoDate(p.markDate)}</dd>
        </dl>
      </div>

      <div className="dsec">
        <h3>Tax lots</h3>
        {/* ⭐ THE SCALE CLAIM, AND WHERE IT COMES FROM. A NAV is a fold over
            balances; the lot book is the history behind them. The fold that
            strikes a NAV never reads these, which is why twenty million lots
            still strike in microseconds. */}
        <p className="note">
          The fold that strikes a NAV never reads these — a NAV is a fold over
          balances, and the lot book is the history behind them.
        </p>
        {/* ⛔ The unbounded read, held behind the shell. See LotBook. */}
        <Suspense fallback={<div className="empty">Reading the lot book…</div>}>
          <LotBook fund={fund} view={view} position={position} />
        </Suspense>
      </div>
    </aside>
  );
}
