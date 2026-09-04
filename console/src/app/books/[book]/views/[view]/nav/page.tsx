import Link from "next/link";
import { notFound } from "next/navigation";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { caller } from "@/lib/caller";
import { periodLabel, previousMonth, utcMonth, utcYear } from "@/lib/dates";
import {
  bookUnits,
  expenseShown,
  incomeShown,
  navRollForward,
  navShown,
  outflowShown,
  perShareShown,
  unitsShown,
} from "@/lib/nav";
import { money } from "@/lib/format";
import { getBook, listAccounts } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/**
 * Period NAV roll-forward for one Investment book of record.
 *
 * ⭐ NOT A STRIKE, NOT A SECOND LEDGER. Beginning, ending, contributions,
 * distributions, income, expense and unrealized are the trial balance in
 * the period the chips name, under `filter=nav-YYYY[-MM]`. `/strikes` is
 * the ABOR NAV; `/capital` is who put money in. This page is why the two
 * disagree with a side spreadsheet.
 *
 * ⛔ A ZERO NAV ON AN EMPTY JOURNAL IS A FAKE. Beginning stays unset when
 * every account's beginning balance is 0. Ending stays unset when nothing
 * dated has landed on or before the window end. A commitment-only prefix
 * that nets to zero NAV is a real zero — some account moved.
 *
 * ⛔ COMMITMENT AND UNDRAWN DO NOT MOVE NAV. They are equity and cancel.
 * Remaining undrawn stays on `/capital`. Adding them here is the defect.
 *
 * Personal and project books 404 rather than wearing a fund label.
 */
async function NavRollForward({
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
  const b = await getBook(c, book);
  if (b.kind !== "INVESTMENT") notFound();

  const { accounts } = await listAccounts(c, book, view, "nav", window);
  const r = navRollForward(accounts);
  const units = bookUnits(accounts);
  const bothUnset = r.beginning === null && r.ending === null;

  const filters: readonly Filter[] = [
    { key: month, label: periodLabel(month) },
    { key: last, label: periodLabel(last) },
    { key: year, label: year },
  ];

  return (
    <>
      <FilterChips
        filters={filters}
        active={window}
        param="period"
        label="Period"
        note={`${periodLabel(window)} — dated entries only, not a strike, not IRR`}
      />

      {bothUnset ? (
        <div className="empty">
          Beginning and ending stay unset — not a measured zero NAV. No dated
          journal prefix supports this period.
        </div>
      ) : null}

      <div className="tb" role="table" aria-label="NAV roll-forward">
        <div className="posgroup">
          <div className="posacct">NAV</div>
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
              {navShown(r.beginning)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Contributions
              <span className="at">
                {r.contributions === null
                  ? "no partner-capital or contribution account on this chart"
                  : "period credits on Partner capital / Capital contributions — the same In /capital cites"}
              </span>
            </span>
            <span role="cell" className="num">
              {outflowShown(r.contributions)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Distributions
              <span className="at">
                {r.distributions === null
                  ? "no partner-capital or distribution account on this chart"
                  : "period debits on Partner capital / Distributions — the same Out /capital cites"}
              </span>
            </span>
            <span role="cell" className="num">
              {outflowShown(r.distributions)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Income
              <span className="at">period revenue, not since inception</span>
            </span>
            <span role="cell" className="num">
              {incomeShown(r.income)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Expenses
              <span className="at">management fee and other period expense</span>
            </span>
            <span role="cell" className="num">
              {expenseShown(r.expense)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Unrealized
              <span className="at">
                {r.unrealized === null
                  ? "unset — Unrealized gain did not move this window, not a silent zero mark"
                  : "period Unrealized gain, not a contribution"}
              </span>
            </span>
            <span role="cell" className="num">
              {incomeShown(r.unrealized)}
            </span>
          </div>
          {r.allocations !== null && r.allocations !== 0n ? (
            <div className="tbrow static" role="row">
              <span role="cell">
                Allocations
                <span className="at">
                  period Allocations — the same activity account /capital cites
                </span>
              </span>
              <span role="cell" className="num">
                {incomeShown(r.allocations)}
              </span>
            </div>
          ) : null}
          {r.transfers !== null && r.transfers !== 0n ? (
            <div className="tbrow static" role="row">
              <span role="cell">
                Transfers
                <span className="at">
                  period Capital transfers — intra-equity, the same account /capital cites
                </span>
              </span>
              <span role="cell" className="num">
                {incomeShown(r.transfers)}
              </span>
            </div>
          ) : null}
          <div className="tbfoot static" role="row">
            <span role="cell">
              Ending
              <small>
                {r.residual === null
                  ? r.beginning === null
                    ? "change stays unset until both cuts exist"
                    : "as-of this window's last day"
                  : r.residual === 0n
                    ? "beginning plus contributions minus distributions plus income minus expenses"
                    : "does not tie — residual is not absorbed"}
              </small>
            </span>
            <span role="cell" className="num">
              {navShown(r.ending)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Change
              <span className="at">ΔNAV this window — not IRR, not a waterfall</span>
            </span>
            <span role="cell" className="num">
              {navShown(r.delta)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Units in issue
              <span className="at">
                {units === null
                  ? "unset — no subscription has posted units, not a fake zero"
                  : units === 0n
                    ? "fully redeemed — a real zero, not unset"
                    : "ending units on partner capital / contributions / distributions"}
              </span>
            </span>
            <span role="cell" className="num">
              {unitsShown(units)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Issued
              <span className="at">
                {r.issued === null
                  ? "unset — no subscription posted units this window, not a silent zero issue"
                  : "period units issued — not the ending stock, not 1/N"}
              </span>
            </span>
            <span role="cell" className="num">
              {unitsShown(r.issued)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Redeemed
              <span className="at">
                {r.redeemed === null
                  ? "unset — no redemption posted this window, not a silent zero redemption"
                  : "period units redeemed — the plug, not the net"}
              </span>
            </span>
            <span role="cell" className="num">
              {unitsShown(r.redeemed)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Per-share NAV
              <span className="at">
                {r.perShare === null
                  ? units === 0n
                    ? "unset — no units in issue after a full redemption, not a divided-by-zero zero"
                    : "unset — no units in issue, not a fake zero per-share"
                  : r.perShare.residual === 0n
                    ? "ending NAV divides the units exactly — Ratio.Closure.perShare"
                    : `residual ${money(r.perShare.residual.toString())} stays with the fund — Ratio.Closure.residual_is_accounted`}
              </span>
            </span>
            <span role="cell" className="num">
              {perShareShown(r.perShare)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="posacct">Did not move NAV</div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Commitment / undrawn
              <span className="at">
                equity, so they cancel — remaining undrawn is on Capital
                activity, not a callable zero here
              </span>
            </span>
            <span role="cell" className="num">
              —
            </span>
          </div>
        </div>
      </div>
      <p className="note">
        Commitment and undrawn cancel in NAV. Remaining is on Capital activity.
        {" · "}
        <Link href={`/books/${book}/views/${view}/capital`}>Capital activity</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/strikes`}>NAV</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/accounts`}>Trial balance</Link>
        {" · "}
        <Link href={`/books/${book}/record`}>Record an event</Link>
      </p>
    </>
  );
}

export default withRefusal(NavRollForward);
