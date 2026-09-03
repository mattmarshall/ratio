import Link from "next/link";
import { notFound } from "next/navigation";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { caller } from "@/lib/caller";
import { periodLabel, previousMonth, utcMonth, utcYear } from "@/lib/dates";
import { debitShown, householdRollup } from "@/lib/household";
import { getBook, listAccounts } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/**
 * Budget vs actual for one personal book of record.
 *
 * ⭐ NOT NAV RELABELLED. Baseline is `[personal] budget` on the configuration
 * this book pins. Actuals are the trial balance's expense accounts in the
 * period the chips name (living expenses, taxes). A second ledger would be
 * a second answer to a question the journal already answers.
 *
 * ⛔ CUMULATIVE-ONLY IS THE ABOR-SHAPED VIEW. `filter=budget` without a
 * `YYYY-MM` / `YYYY` suffix is refused by the server; this page always
 * sends one. An entry with no date is not in any window. The baseline is
 * not annualized: a monthly number cited against a year is the operator's
 * comparison, not twelve times a figure nobody wrote.
 *
 * Project books are #66 — this URL 404s for them rather than wearing a
 * household label. Investment books stay on fund-ops.
 */
async function Budget({
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
  const { accounts } = await listAccounts(c, book, view, `budget-${window}`);
  const r = householdRollup(accounts, b.budget, b.envelopes);

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
        note={`${periodLabel(window)} — dated spend only, not since inception`}
      />

      <div className="tb" role="table" aria-label="Budget vs actual">
        <div className="posgroup">
          <div className="posacct">Baseline</div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Authorized budget
              <span className="at">
                not a second ledger — journal expenses against a
                configuration total for the period this page names
              </span>
            </span>
            <span role="cell" className="num">
              {r.baseline === null ? "—" : debitShown(r.baseline)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="posacct">Actual</div>
          {r.envelopes.map((e) => (
            <Link
              key={e.dimension}
              className="tbrow"
              role="row"
              href={`/books/${book}/views/${view}/accounts/${e.dimension}`}
            >
              <span role="cell">
                {e.displayName}
                <span className="at">
                  {e.planned === null
                    ? "no [personal.envelope] for this category"
                    : `envelope ${debitShown(e.planned)}`}
                </span>
              </span>
              <span role="cell" className="num">
                {debitShown(e.actual)}
              </span>
            </Link>
          ))}
          <div className="tbfoot static" role="row">
            <span role="cell">
              Spent
              <small>living expenses and taxes this window</small>
            </span>
            <span role="cell" className="num">
              {debitShown(r.spent)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="tbfoot static" role="row">
            <span role="cell">
              Variance
              <small>
                {r.baseline === null
                  ? "no [personal] budget on the configuration in force"
                  : "baseline minus spent — remaining authorization, not annualized"}
              </small>
            </span>
            <span role="cell" className="num">
              {r.variance === null ? "—" : debitShown(r.variance)}
            </span>
          </div>
        </div>
      </div>
      <p className="note">
        <Link href={`/books/${book}/record`}>Record a spend</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/accounts`}>Trial balance</Link>
      </p>
    </>
  );
}

export default withRefusal(Budget);
