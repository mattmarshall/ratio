import Link from "next/link";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { caller } from "@/lib/caller";
import { periodLabel, previousMonth, utcMonth, utcYear } from "@/lib/dates";
import { ofType, sheetTotals, shown } from "@/lib/statement";
import { getBook, listAccounts } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";
import type { Account } from "@/wire/types";

export const dynamic = "force-dynamic";

/**
 * Period profit and loss. Month or year, not since inception.
 *
 * ⛔ CUMULATIVE-ONLY IS THE ABOR-SHAPED VIEW. `filter=pnl` without a
 * `YYYY-MM` / `YYYY` suffix is refused by the server; this page always
 * sends one. An entry with no date is not in any window.
 *
 * Accounts are `chart_for` income and expense — living expenses, taxes,
 * income — not a fund's realized-gain warehouse.
 */
async function PnL({
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
  const [b, { accounts }] = await Promise.all([
    getBook(c, book),
    listAccounts(c, book, view, "pnl", window),
  ]);
  const operating = b.kind === "OPERATING";

  const filters: readonly Filter[] = [
    { key: month, label: periodLabel(month) },
    { key: last, label: periodLabel(last) },
    { key: year, label: year },
  ];

  const totals = sheetTotals(accounts);
  const income = ofType(accounts, "income");
  const expenses = ofType(accounts, "expense");

  return (
    <>
      <FilterChips
        filters={filters}
        active={window}
        param="period"
        label="Period"
        note={`${periodLabel(window)} — dated entries only, not since inception`}
      />

      <div
        className="tb"
        role="table"
        aria-label={operating ? "Period income statement" : "Period profit and loss"}
      >
        <Rows
          book={book}
          view={view}
          title="Income"
          accounts={income}
          foot={shown("income", totals.income)}
          section="income"
        />
        <Rows
          book={book}
          view={view}
          title="Expenses"
          accounts={expenses}
          foot={shown("expense", totals.expenses)}
          section="expense"
        />
        <div className="posgroup">
          <div className="tbfoot static" role="row">
            <span role="cell">
              Surplus
              <small>income less expenses, this window</small>
            </span>
            <span role="cell" className="num">
              {shown("equity", totals.surplus)}
            </span>
          </div>
        </div>
      </div>

      <p className="note">
        <Link href={`/books/${book}/views/${view}/sheet?period=${encodeURIComponent(window)}`}>
          Balance sheet
        </Link>
        {" · "}
        <Link href={`/books/${book}/record`}>Record income or an expense</Link>
        {operating ? null : (
          <>
            {" · "}
            <Link href={`/books/${book}/transfer`}>Transfer</Link>
          </>
        )}
      </p>
    </>
  );
}

function Rows({
  book,
  view,
  title,
  accounts,
  foot,
  section,
}: {
  book: string;
  view: string;
  title: string;
  accounts: Account[];
  foot: string;
  section: "income" | "expense";
}) {
  return (
    <div className="posgroup">
      <div className="posacct">{title}</div>
      {accounts.length === 0 ? (
        <div className="tbrow static" role="row">
          <span role="cell">None in this chart.</span>
          <span role="cell" className="num">
            {shown(section, 0n)}
          </span>
        </div>
      ) : (
        accounts.map((a) => {
          const id = a.name.split("/").pop()!;
          return (
            <Link
              key={a.name}
              className="tbrow"
              role="row"
              href={`/books/${book}/views/${view}/accounts/${id}`}
            >
              <span role="cell">{a.displayName}</span>
              <span role="cell" className="num">
                {shown(section, BigInt(a.balance))}
              </span>
            </Link>
          );
        })
      )}
      <div className="tbfoot static" role="row">
        <span role="cell">{title}</span>
        <span role="cell" className="num">
          {foot}
        </span>
      </div>
    </div>
  );
}

export default withRefusal(PnL);
