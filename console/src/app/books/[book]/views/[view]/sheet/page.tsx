import Link from "next/link";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { caller } from "@/lib/caller";
import { periodLabel, previousMonth, utcMonth, utcYear } from "@/lib/dates";
import { money } from "@/lib/format";
import { ofType, sectionOf, sheetFoots, sheetTotals, shown } from "@/lib/statement";
import { listAccounts } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";
import type { Account } from "@/wire/types";

export const dynamic = "force-dynamic";

/**
 * A citable balance sheet for one book of record.
 *
 * ⭐ THE CHART IS `chart_for(Personal)` (or whichever kind this book is).
 * Grouping does not invent accounts. Surplus is income − expenses, the
 * residual that makes A = L + E + surplus while the books have not closed.
 *
 * Period chips are as-of a month or year end. Empty is now — the maintained
 * fold, including undated entries. A dated as-of skips those, because an
 * entry with no date has no period.
 */
async function Sheet({
  params,
  searchParams,
}: {
  params: Promise<{ book: string; view: string }>;
  searchParams: Promise<{ period?: string }>;
}) {
  const { book, view } = await params;
  const { period = "" } = await searchParams;
  const month = utcMonth();
  const year = utcYear();
  const last = previousMonth(month);
  const c = await caller();
  const { accounts } = await listAccounts(
    c,
    book,
    view,
    period ? "sheet" : undefined,
    period || undefined,
  );

  const filters: readonly Filter[] = [
    { key: "", label: "Now" },
    { key: month, label: periodLabel(month) },
    { key: last, label: periodLabel(last) },
    { key: year, label: year },
  ];

  const totals = sheetTotals(accounts);
  const foots = sheetFoots(totals);

  return (
    <>
      <FilterChips
        filters={filters}
        active={period}
        param="period"
        label="As of"
        note={
          period
            ? `as of ${periodLabel(period)} — dated entries only`
            : "as of now, including undated entries"
        }
      />

      {accounts.length === 0 ? (
        <div className="empty">This book has no chart of accounts yet.</div>
      ) : (
        <div className="tb" role="table" aria-label="Balance sheet">
          <Section
            book={book}
            view={view}
            title="Assets"
            accounts={ofType(accounts, "asset")}
            section="asset"
            foot={shown("asset", totals.assets)}
          />
          <Section
            book={book}
            view={view}
            title="Liabilities"
            accounts={ofType(accounts, "liability")}
            section="liability"
            foot={shown("liability", totals.liabilities)}
          />
          <Section
            book={book}
            view={view}
            title="Equity"
            accounts={ofType(accounts, "equity")}
            section="equity"
            foot={shown("equity", totals.equity)}
          />
          <div className="posgroup">
            <div className="posacct">Surplus</div>
            <div className="tbrow static" role="row">
              <span role="cell">
                Income less expenses
                <span className="at">
                  not a closing entry — the residual that makes the sheet foot
                </span>
              </span>
              <span role="cell" className="num">
                {shown("equity", totals.surplus)}
              </span>
            </div>
            <div className="tbfoot static" role="row">
              <span role="cell">
                {foots ? "Assets equal liabilities, equity and surplus" : "Does not foot"}
                <small>
                  {period
                    ? "An entry with no date is not in this as-of."
                    : "Checked in the book's own units, not a second ledger."}
                </small>
              </span>
              <span role="cell" className="num">
                {shown("asset", totals.assets)}
              </span>
            </div>
          </div>
        </div>
      )}

      <p className="note">
        <Link href={`/books/${book}/views/${view}/pnl?period=${encodeURIComponent(period || month)}`}>
          Period P&L
        </Link>
        {" · "}
        <Link href={`/books/${book}/transfer`}>Transfer</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/accounts`}>Trial balance</Link>
      </p>
    </>
  );
}

function Section({
  book,
  view,
  title,
  accounts,
  section,
  foot,
}: {
  book: string;
  view: string;
  title: string;
  accounts: Account[];
  section: "asset" | "liability" | "equity";
  foot: string;
}) {
  return (
    <div className="posgroup">
      <div className="posacct">{title}</div>
      {accounts.map((a) => {
        const id = a.name.split("/").pop()!;
        const s = sectionOf(a.type);
        return (
          <Link
            key={a.name}
            className="tbrow"
            role="row"
            href={`/books/${book}/views/${view}/accounts/${id}`}
          >
            <span role="cell">{a.displayName}</span>
            <span role="cell" className="num">
              {s ? shown(s, BigInt(a.balance)) : money(a.balance)}
            </span>
          </Link>
        );
      })}
      <div className="tbfoot static" role="row">
        <span role="cell">{title}</span>
        <span role="cell" className="num">
          {foot}
        </span>
      </div>
    </div>
  );
}

export default withRefusal(Sheet);
