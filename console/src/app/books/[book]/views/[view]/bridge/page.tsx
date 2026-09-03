import Link from "next/link";
import { notFound } from "next/navigation";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { caller } from "@/lib/caller";
import { periodLabel, previousMonth, utcMonth, utcYear } from "@/lib/dates";
import {
  debitShown,
  expenseShown,
  incomeShown,
  netWorthBridge,
  nwShown,
} from "@/lib/bridge";
import { getBook, listAccounts } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/**
 * Net-worth bridge for one personal book of record.
 *
 * ⭐ NOT A SECOND LEDGER. Beginning, ending, income, expense and the
 * named plugs are the trial balance in the period the chips name, under
 * `filter=bridge-YYYY[-MM]`. Sheet is one as-of; P&L is the surplus;
 * this page is why the two disagree with a side spreadsheet.
 *
 * ⛔ A ZERO NW ON AN EMPTY JOURNAL IS A FAKE. Beginning stays unset when
 * every account's beginning balance is 0. Ending stays unset when nothing
 * dated has landed on or before the window end. Origination that nets to
 * zero is a real zero — some account moved.
 *
 * ⛔ PRINCIPAL AND TRANSFERS DO NOT MOVE NET WORTH. They belong in the
 * story so an operator does not invent one. Adding them to ΔNW is the
 * defect. Asset purchases stay unset: chart_for(Personal) has no purchase
 * account distinct from a cash↔investments transfer.
 *
 * Project and investment books 404 rather than wearing a household label.
 */
async function Bridge({
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
  if (b.kind !== "PERSONAL") notFound();

  const { accounts } = await listAccounts(c, book, view, "bridge", window);
  const r = netWorthBridge(accounts, b.loans ?? []);
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
        note={`${periodLabel(window)} — dated entries only, not since inception`}
      />

      {bothUnset ? (
        <div className="empty">
          Beginning and ending stay unset — not a measured zero. No dated
          journal prefix supports this period.
        </div>
      ) : null}

      <div className="tb" role="table" aria-label="Net-worth bridge">
        <div className="posgroup">
          <div className="posacct">Net worth</div>
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
              {nwShown(r.beginning)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Income
              <span className="at">period P&amp;L, not since inception</span>
            </span>
            <span role="cell" className="num">
              {incomeShown(r.income)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Expenses
              <span className="at">living, tax, and loan interest this window</span>
            </span>
            <span role="cell" className="num">
              {expenseShown(r.expense)}
            </span>
          </div>
          {r.equity !== null && r.equity !== 0n ? (
            <div className="tbrow static" role="row">
              <span role="cell">
                Opening equity
                <span className="at">
                  period equity, not income — a contribution that is not a P&amp;L line
                </span>
              </span>
              <span role="cell" className="num">
                {incomeShown(r.equity)}
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
                    ? "beginning plus income minus expenses"
                    : "does not tie — residual is not absorbed"}
              </small>
            </span>
            <span role="cell" className="num">
              {nwShown(r.ending)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Change
              <span className="at">ΔNW this window</span>
            </span>
            <span role="cell" className="num">
              {nwShown(r.delta)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="posacct">Did not move net worth</div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Principal paid on loans
              <span className="at">
                {r.principalPaid === null
                  ? "no [personal.loan] on the configuration in force"
                  : "liability down and cash down — the sheet moves, NW does not"}
              </span>
            </span>
            <span role="cell" className="num">
              {debitShown(r.principalPaid)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Transfers
              <span className="at">
                {r.transfers === null
                  ? "no Investments or Credit cards account on this chart"
                  : "investments activity plus credit-card payments"}
              </span>
            </span>
            <span role="cell" className="num">
              {debitShown(r.transfers)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Asset purchases
              <span className="at">
                no purchase account distinct from a transfer
              </span>
            </span>
            <span role="cell" className="num">
              {debitShown(r.assetPurchases)}
            </span>
          </div>
        </div>
      </div>
      <p className="note">
        Principal, transfers and purchases move the sheet, not net worth.
        {" · "}
        <Link
          href={`/books/${book}/views/${view}/sheet?period=${encodeURIComponent(window)}`}
        >
          Balance sheet
        </Link>
        {" · "}
        <Link
          href={`/books/${book}/views/${view}/pnl?period=${encodeURIComponent(window)}`}
        >
          Period P&L
        </Link>
        {" · "}
        <Link
          href={`/books/${book}/views/${view}/loans?period=${encodeURIComponent(window)}`}
        >
          Loan schedule
        </Link>
        {" · "}
        <Link href={`/books/${book}/transfer`}>Transfer</Link>
      </p>
    </>
  );
}

export default withRefusal(Bridge);
