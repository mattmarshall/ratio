import Link from "next/link";
import { notFound } from "next/navigation";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { caller } from "@/lib/caller";
import { periodLabel, previousMonth, utcMonth, utcYear } from "@/lib/dates";
import { debitShown, liabilityShown, loanRollup } from "@/lib/loans";
import { getBook, listAccounts } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/**
 * Loan roll-forward for one personal book of record.
 *
 * ⭐ NOT A SECOND AMORTIZATION TABLE. Which liabilities have a schedule is
 * `[personal.loan]` on the configuration this book pins (`Book.loans`).
 * Beginning, principal, interest and ending are the trial balance in the
 * period the chips name. A second ledger would be a second answer to a
 * question the journal already answers.
 *
 * ⛔ EMPTY `Book.loans` IS UNSET, NOT A ROLL-FORWARD OF ZEROS. CreateBook
 * seeds the posting pattern and does not invent a mortgage of nothing.
 * ListAccounts is skipped until someone names a liability — asking the
 * chart for zeros is how a silent schedule would get back in.
 *
 * ⛔ CUMULATIVE-ONLY IS THE ABOR-SHAPED VIEW. `filter=loan` without a
 * `YYYY-MM` / `YYYY` suffix is refused by the server; this page always
 * sends one. An entry with no date is not in any window.
 *
 * Project and investment books 404 rather than wearing a household label.
 */
async function Loans({
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

  const loans = b.loans ?? [];
  const { accounts } = loans.length
    ? await listAccounts(c, book, view, `loan-${window}`)
    : { accounts: [] };
  const r = loanRollup(accounts, loans);

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
        note={`${periodLabel(window)} — dated payments only, not since inception`}
      />

      {loans.length === 0 ? (
        <div className="empty">No loan schedule is configured.</div>
      ) : (
        <div className="tb" role="table" aria-label="Loan schedule">
          <div className="tbrow tbhead" role="row">
            <span role="columnheader">Liability</span>
            <span role="columnheader">Beginning</span>
            <span role="columnheader">Principal</span>
            <span role="columnheader">Interest</span>
            <span role="columnheader">Ending</span>
          </div>
          {r.rows.map((row) => (
            <Link
              key={row.dimension}
              className="tbrow"
              role="row"
              href={`/books/${book}/views/${view}/accounts/${row.dimension}`}
            >
              <span role="cell">
                {row.displayName}
                <span className="at">
                  {row.drawn > 0n
                    ? `drew ${debitShown(row.drawn)} this window`
                    : `interest ${debitShown(row.interestPaid)} · dim ${row.dimension}`}
                </span>
              </span>
              <span role="cell" className="num">
                {liabilityShown(row.beginning)}
              </span>
              <span role="cell" className="num">
                {debitShown(row.principalPaid)}
              </span>
              <span role="cell" className="num">
                {debitShown(row.interestPaid)}
              </span>
              <span role="cell" className="num">
                {liabilityShown(row.ending)}
              </span>
            </Link>
          ))}
        </div>
      )}
      <p className="note">
        <Link href={`/books/${book}/ingest`}>Ingest a loan payment</Link>
        {" · "}
        <Link href={`/books/${book}/record`}>Record a payment</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/accounts`}>Trial balance</Link>
      </p>
    </>
  );
}

export default withRefusal(Loans);
