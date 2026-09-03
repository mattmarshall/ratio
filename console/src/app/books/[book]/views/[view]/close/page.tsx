import Link from "next/link";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { caller } from "@/lib/caller";
import {
  closeRollForward,
  closedYmd,
  coveringClose,
  equityShown,
} from "@/lib/close";
import { isoDate, periodLabel, previousMonth, utcMonth, utcYear } from "@/lib/dates";
import { getPeriodClose, listAccounts, listPeriodCloses } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/**
 * Period close evidence and retained-earnings roll-forward.
 *
 * ⭐ NOT THE CLOSE VERB. The operator close is `ratio close` at a terminal.
 * This page is the evidence an operator opens: closed date, journal prefix,
 * configuration digest, actor, time — and the roll-forward the close
 * produced. `Ratio.Close`. `//tla:period_close_check`.
 *
 * ⛔ UNSET STAYS UNSET. Beginning retained earnings is unset when no dated
 * prefix precedes the window. Surplus is unset when no close recorded a
 * posting and the period I/E did not move. A missing destination is not
 * Opening equity. Kind still selects one chrome list (`screensFor`).
 *
 * A preview of an open period may show unclosed surplus; it says
 * provisional rather than presenting the period as closed.
 */
async function PeriodClosePage({
  params,
  searchParams,
}: {
  params: Promise<{ book: string; view: string }>;
  searchParams: Promise<{ period?: string }>;
}) {
  const { book, view } = await params;
  const month = utcMonth();
  const year = utcYear();
  const last = previousMonth(month);
  const { period = month } = await searchParams;
  const window = period || month;
  const c = await caller();
  const [{ accounts }, listed] = await Promise.all([
    listAccounts(c, book, view, "close", window),
    listPeriodCloses(c, book, view),
  ]);
  const cover = coveringClose(listed.periodCloses, window);
  const evidence = cover
    ? await getPeriodClose(c, book, view, closedYmd(cover.closedDate))
    : null;
  const r = closeRollForward(accounts, evidence);
  const bothUnset = r.beginning === null && r.ending === null && r.surplus === null;

  const filters: readonly Filter[] = [
    { key: month, label: periodLabel(month) },
    { key: last, label: periodLabel(last) },
    { key: year, label: year },
  ];

  const stateNote = r.closed
    ? "this window is closed — surplus is in retained earnings"
    : "provisional — not a closing entry";

  const tieNote =
    r.residual === null
      ? r.beginning === null
        ? "beginning stays unset until a dated prefix precedes this window"
        : "ending stays unset until a close posts to equity"
      : r.residual === 0n
        ? "beginning plus surplus plus named adjustments"
        : "does not tie — residual is not absorbed";

  const when = evidence?.createTime
    ? evidence.createTime.slice(0, 10)
    : null;

  return (
    <>
      <FilterChips
        filters={filters}
        active={window}
        param="period"
        label="Period"
        note={`${periodLabel(window)} — dated entries only, ${stateNote}`}
      />

      {bothUnset ? (
        <div className="empty">
          Beginning and ending stay unset — not a measured zero close. No
          dated journal prefix supports this period, and no close has been
          recorded.
        </div>
      ) : null}

      <div className="tb" role="table" aria-label="Period close">
        <div className="posgroup">
          <div className="posacct">Retained earnings</div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Beginning
              <span className="at">
                {r.beginning === null
                  ? "no dated prefix before this window"
                  : "as-of the day before this window, dated entries only"}
              </span>
            </span>
            <span role="cell" className="num">
              {equityShown(r.beginning)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Surplus
              <span className="at">
                {r.closed
                  ? r.surplus === null
                    ? "unset — no income or expense to roll, not a measured zero"
                    : "income less expenses, from the closing posting"
                  : r.surplus === null
                    ? "unset — the period I/E did not move"
                    : "provisional — not a closing entry"}
              </span>
            </span>
            <span role="cell" className="num">
              {equityShown(r.surplus)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Adjustments
              <span className="at">
                {r.adjustments === null
                  ? "unset — no named closing adjustment this window"
                  : "dest activity that is not the surplus"}
              </span>
            </span>
            <span role="cell" className="num">
              {equityShown(r.adjustments)}
            </span>
          </div>
          <div className="tbfoot static" role="row">
            <span role="cell">
              Ending
              <small>{tieNote}</small>
            </span>
            <span role="cell" className="num">
              {equityShown(r.ending)}
            </span>
          </div>
        </div>

        {evidence ? (
          <div className="posgroup">
            <div className="posacct">Close evidence</div>
            <div className="tbrow static" role="row">
              <span role="cell">
                Closed through
                <span className="at">{isoDate(evidence.closedDate)}</span>
              </span>
              <span role="cell" className="num">
                {evidence.view}
              </span>
            </div>
            <div className="tbrow static" role="row">
              <span role="cell">
                Journal prefix
                <span className="at">
                  {evidence.journalPosition} entries · {evidence.journalDigest.slice(0, 12)}
                </span>
              </span>
              <span role="cell" className="num">
                {evidence.configDigest.slice(0, 7)}
              </span>
            </div>
            <div className="tbrow static" role="row">
              <span role="cell">
                {evidence.closingEntry ? (
                  <Link href={`/books/${book}/entries/${encodeURIComponent(evidence.closingEntry)}`}>
                    Closing posting
                  </Link>
                ) : (
                  "Closing posting"
                )}
                <span className="at">
                  {evidence.closingEntry || "unset — the door holds and nothing rolled"}
                </span>
              </span>
              <span role="cell" className="num">
                {evidence.actor}
                {when ? ` · ${when}` : ""}
              </span>
            </div>
          </div>
        ) : (
          <div className="posgroup">
            <div className="posacct">Close evidence</div>
            <div className="tbrow static" role="row">
              <span role="cell">
                No close recorded for this window
                <span className="at">
                  the operator close is `ratio close --through` at a terminal
                </span>
              </span>
              <span role="cell" className="num">
                —
              </span>
            </div>
          </div>
        )}
      </div>

      <p className="note">
        <Link
          href={`/books/${book}/views/${view}/pnl?period=${encodeURIComponent(window)}`}
        >
          Period P&L
        </Link>
        {" · "}
        <Link
          href={`/books/${book}/views/${view}/sheet?period=${encodeURIComponent(window)}`}
        >
          Balance sheet
        </Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/accounts`}>Trial balance</Link>
      </p>
    </>
  );
}

export default withRefusal(PeriodClosePage);
