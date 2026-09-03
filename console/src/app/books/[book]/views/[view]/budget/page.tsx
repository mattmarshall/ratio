import Link from "next/link";
import { notFound } from "next/navigation";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { caller } from "@/lib/caller";
import { periodLabel, previousMonth, utcMonth, utcYear } from "@/lib/dates";
import { debitShown as householdDebit, householdRollup } from "@/lib/household";
import {
  creditShown,
  debitShown as projectDebit,
  ofType,
  projectRollup,
} from "@/lib/project";
import { getBook, listAccounts } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/**
 * Budget vs actual for one book of record.
 *
 * ⭐ NOT NAV RELABELLED, AND NOT TWO URLS. Kind selects the roll-up: a
 * personal book cites `[personal] budget` against period spend; a project
 * book cites `[project] budget` against cumulative costs, WIP and
 * payables. A second ledger would be a second answer to a question the
 * journal already answers. Investment books 404 — fund ABOR is untouched.
 *
 * ⚠ A PROJECT'S PERIOD IS THE PROJECT. Household chips name a month or
 * year because living expenses are a calendar figure; milestone-gated
 * close is still out of scope for a project — same period gap as #26,
 * named rather than faked with a NAV strike.
 *
 * ⛔ CUMULATIVE-ONLY IS THE ABOR-SHAPED VIEW FOR A HOUSEHOLD. The personal
 * path always sends `filter=budget-YYYY[-MM]`; bare `budget` is refused
 * by the server. An entry with no date is not in any window. The baseline
 * is not annualized.
 */
async function Budget({
  params,
  searchParams,
}: {
  params: Promise<{ book: string; view: string }>;
  searchParams: Promise<{ period?: string }>;
}) {
  const { book, view } = await params;
  const c = await caller();
  const b = await getBook(c, book);
  if (b.kind === "PERSONAL") {
    return householdBudget({ book, view, b, searchParams });
  }
  if (b.kind === "PROJECT") {
    return projectBudget({ book, view, b });
  }
  notFound();
}

async function householdBudget({
  book,
  view,
  b,
  searchParams,
}: {
  book: string;
  view: string;
  b: Awaited<ReturnType<typeof getBook>>;
  searchParams: Promise<{ period?: string }>;
}) {
  const month = utcMonth();
  const year = utcYear();
  const last = previousMonth(month);
  const { period = month } = await searchParams;
  const window = period || month;
  const c = await caller();
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
              {r.baseline === null ? "—" : householdDebit(r.baseline)}
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
                    : `envelope ${householdDebit(e.planned)}`}
                </span>
              </span>
              <span role="cell" className="num">
                {householdDebit(e.actual)}
              </span>
            </Link>
          ))}
          <div className="tbfoot static" role="row">
            <span role="cell">
              Spent
              <small>living expenses and taxes this window</small>
            </span>
            <span role="cell" className="num">
              {householdDebit(r.spent)}
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
              {r.variance === null ? "—" : householdDebit(r.variance)}
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

async function projectBudget({
  book,
  view,
  b,
}: {
  book: string;
  view: string;
  b: Awaited<ReturnType<typeof getBook>>;
}) {
  const c = await caller();
  const { accounts } = await listAccounts(c, book, view);
  const r = projectRollup(accounts, b.budget);
  const costs = ofType(accounts, "EXPENSE");
  const wip = ofType(accounts, "ASSET").filter((a) =>
    /work in progress/i.test(a.displayName),
  );
  const payables = ofType(accounts, "LIABILITY");
  const funding = ofType(accounts, "EQUITY");

  return (
    <>
      <div className="tb" role="table" aria-label="Budget vs actual">
        <div className="posgroup">
          <div className="posacct">Baseline</div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Authorized budget
              <span className="at">
                not a second ledger — journal costs, WIP and payables
                against a configuration total
              </span>
            </span>
            <span role="cell" className="num">
              {r.baseline === null ? "—" : projectDebit(r.baseline)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="posacct">Actual</div>
          {costs.map((a) => {
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
                  {projectDebit(BigInt(a.balance))}
                </span>
              </Link>
            );
          })}
          {wip.map((a) => {
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
                  {projectDebit(BigInt(a.balance))}
                </span>
              </Link>
            );
          })}
          <div className="tbfoot static" role="row">
            <span role="cell">
              Incurred
              <small>costs plus WIP — recognizing does not double-count</small>
            </span>
            <span role="cell" className="num">
              {projectDebit(r.incurred)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="posacct">Commitment</div>
          {payables.map((a) => {
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
                  {creditShown(BigInt(a.balance))}
                </span>
              </Link>
            );
          })}
          <div className="tbfoot static" role="row">
            <span role="cell">
              Committed
              <small>incurred plus unpaid payables</small>
            </span>
            <span role="cell" className="num">
              {projectDebit(r.committed)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="tbfoot static" role="row">
            <span role="cell">
              Variance
              <small>
                {r.baseline === null
                  ? "no [project] budget on the configuration in force"
                  : "baseline minus committed — remaining authorization"}
              </small>
            </span>
            <span role="cell" className="num">
              {r.variance === null ? "—" : projectDebit(r.variance)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="posacct">Funding</div>
          {funding.map((a) => {
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
                  {creditShown(BigInt(a.balance))}
                </span>
              </Link>
            );
          })}
          <div className="tbfoot static" role="row">
            <span role="cell">Funding received</span>
            <span role="cell" className="num">
              {creditShown(r.funding)}
            </span>
          </div>
        </div>
      </div>
      <p className="note">
        <Link href={`/books/${book}/views/${view}/wip`}>WIP capitalization</Link>
        {" · "}
        <Link href={`/books/${book}/record`}>Record a cost or capitalize WIP</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/accounts`}>Trial balance</Link>
      </p>
    </>
  );
}

export default withRefusal(Budget);
