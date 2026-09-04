import Link from "next/link";
import { notFound } from "next/navigation";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { IngestForm } from "@/app/books/[book]/ingest/IngestForm";
import { caller } from "@/lib/caller";
import { BUDGET_INGEST_TEMPLATES } from "@/lib/budgetPost";
import { periodLabel, previousMonth, utcMonth, utcYear } from "@/lib/dates";
import { debitShown as householdDebit, householdRollup } from "@/lib/household";
import {
  changeOrdersInWindow,
  creditShown,
  debitShown as projectDebit,
  isFundingAccount,
  ofType,
  phaseAwarded,
  projectRollup,
} from "@/lib/project";
import { getBook, listAccounts, listRules } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";
import { BudgetPostForm } from "./BudgetPostForm";

export const dynamic = "force-dynamic";

/**
 * Budget vs actual for one book of record.
 *
 * ⭐ NOT NAV RELABELLED, AND NOT TWO URLS. Kind selects the roll-up: a
 * personal book cites `[personal] budget` against period spend; a project
 * book cites `[project] budget` as the original contract against cumulative
 * costs, WIP, awarded commitments, and remaining to spend, plus approved
 * change orders as a journal fact that does not rewrite that key. A second
 * ledger would be a second answer to a question the journal already
 * answers. Investment books 404 — fund ABOR is untouched.
 *
 * ⚠ A PROJECT'S PERIOD IS THE PROJECT. Household chips name a month or
 * year because living expenses are a calendar figure; milestone-gated
 * close is still out of scope for a project — same period gap as #26,
 * named rather than faked with a NAV strike. The optional change-order
 * chip is which COs were approved in-window; incurred and awarded stay as-of.
 *
 * ⛔ THIS PAGE DOES NOT FORECAST. Estimate at completion and cost to
 * complete are not journal facts yet. Remaining to spend is a leftover,
 * not an EAC. Over/under-billing stays on `/billing` as billed minus
 * earned — this page does not invent costs-in-excess-of-billings.
 *
 * ⭐ A PROJECT POSTS A CHANGE ORDER OR AWARD HERE. Kind × phase selects
 * `approve_co_*` / `award_commitment_*` already in force. CSV ingest
 * uses the same `change-orders` / `purchase-orders` templates. Facts
 * stay unset until posted — not a silent zero on either line.
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
    return projectBudget({ book, view, b, searchParams });
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
  const { period = "" } = await searchParams;
  const c = await caller();
  const { accounts } = await listAccounts(c, book, view);
  const { rules } = await listRules(c, book);
  const windowed = period
    ? (await listAccounts(c, book, view, "change", period)).accounts
    : [];
  const r = projectRollup(accounts, b.budget);
  const inWindow = period ? changeOrdersInWindow(windowed) : null;
  const costs = ofType(accounts, "EXPENSE");
  const wip = ofType(accounts, "ASSET").filter((a) =>
    /work in progress/i.test(a.displayName),
  );
  const payables = ofType(accounts, "LIABILITY");
  const funding = accounts.filter(isFundingAccount);

  const filters: readonly Filter[] = [
    { key: "", label: "Since inception" },
    { key: month, label: periodLabel(month) },
    { key: last, label: periodLabel(last) },
    { key: year, label: year },
  ];

  return (
    <>
      <FilterChips
        filters={filters}
        active={period}
        param="period"
        label="Change-order window"
        note={
          period
            ? `${periodLabel(period)} — approved this window; incurred and awarded commitments are still as-of, because a project's period is the project`
            : "since inception — original contract against journal costs and awarded commitments; the chip is which COs were approved in-window"
        }
      />

      <div className="tb" role="table" aria-label="Budget vs actual">
        <div className="posgroup">
          <div className="posacct">Contract</div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Original contract
              <span className="at">
                not a second ledger — journal costs, WIP and payables
                against a configuration total. [project] budget is not rewritten when a change order posts
              </span>
            </span>
            <span role="cell" className="num">
              {r.baseline === null ? "—" : projectDebit(r.baseline)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Approved change orders
              <span className="at">
                {r.approved === null
                  ? "unset — no approved change order has posted, not a silent zero"
                  : "credit-normal on the work-package pair; does not mutate the baseline"}
              </span>
            </span>
            <span role="cell" className="num">
              {r.approved === null ? "—" : projectDebit(r.approved)}
            </span>
          </div>
          {period ? (
            <div className="tbrow static" role="row">
              <span role="cell">
                Approved this window
                <span className="at">
                  {inWindow === null
                    ? "unset — nothing approved in this window, not a fake zero"
                    : "dated approvals only; undated entries have no period"}
                </span>
              </span>
              <span role="cell" className="num">
                {inWindow === null ? "—" : projectDebit(inWindow)}
              </span>
            </div>
          ) : null}
          <div className="tbfoot static" role="row">
            <span role="cell">
              Revised contract
              <small>
                {r.revised === null
                  ? r.approved === null
                    ? "no [project] budget on the configuration in force"
                    : "cannot revise an unknown baseline — approved changes stay visible above"
                  : r.approved === null
                    ? "equals the original — no approved change order has posted"
                    : "original plus approved change orders — the billing basis when priced"}
              </small>
            </span>
            <span role="cell" className="num">
              {r.revised === null ? "—" : projectDebit(r.revised)}
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
          <div className="posacct">Unpaid</div>
          {payables
            .filter((a) => a.displayName === "Payables")
            .map((a) => {
              const id = a.name.split("/").pop()!;
              return (
                <Link
                  key={a.name}
                  className="tbrow"
                  role="row"
                  href={`/books/${book}/views/${view}/accounts/${id}`}
                >
                  <span role="cell">
                    {a.displayName}
                    <span className="at">
                      incurred on a vendor invoice and not yet paid — not an awarded purchase order
                    </span>
                  </span>
                  <span role="cell" className="num">
                    {creditShown(BigInt(a.balance))}
                  </span>
                </Link>
              );
            })}
        </div>
        <div className="posgroup">
          <div className="posacct">Committed cost</div>
          {costs.map((a) => {
            const id = a.name.split("/").pop()!;
            const awarded = phaseAwarded(accounts, a.displayName);
            return (
              <Link
                key={`award-${a.name}`}
                className="tbrow"
                role="row"
                href={`/books/${book}/views/${view}/accounts/${id}`}
              >
                <span role="cell">
                  {a.displayName}
                  <span className="at">
                    {awarded === null
                      ? "unset — no purchase order has been awarded on this work package, not a silent zero"
                      : "open award on this work package — same grain cost-by-package uses"}
                  </span>
                </span>
                <span role="cell" className="num">
                  {awarded === null ? "—" : projectDebit(awarded)}
                </span>
              </Link>
            );
          })}
          <div className="tbfoot static" role="row">
            <span role="cell">
              Awarded
              <small>
                {r.awarded === null
                  ? "unset — no purchase order has been awarded, not a fake zero committed"
                  : "credit-normal on the work-package pair; not incurred and not a payable"}
              </small>
            </span>
            <span role="cell" className="num">
              {r.awarded === null ? "—" : projectDebit(r.awarded)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="tbfoot static" role="row">
            <span role="cell">
              Remaining to spend
              <small>
                {r.remainingToSpend === null
                  ? r.revised === null
                    ? "unset until [project] budget is set — not a priced remainder"
                    : "unset until a purchase order is awarded — not budget minus actual as fake headroom"
                  : "revised minus incurred minus awarded — the citeable leftover, not a forecast"}
              </small>
            </span>
            <span role="cell" className="num">
              {r.remainingToSpend === null ? "—" : projectDebit(r.remainingToSpend)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="tbrow static" role="row">
            <span role="cell">
              Estimate at completion
              <span className="at">
                this page does not forecast — EAC and cost to complete are not a journal fact
              </span>
            </span>
            <span role="cell" className="num">—</span>
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
        A change order or award stays unset on this page until it posts —
        not a silent zero on the approved or awarded line. The same
        journal kinds <code>/record</code> already uses; this is not a
        second budget store.
      </p>
      <BudgetPostForm fund={book} rules={rules} />
      <IngestForm fund={book} templates={[...BUDGET_INGEST_TEMPLATES]} />
      <p className="note">
        <Link href={`/books/${book}/views/${view}/wip`}>WIP capitalization</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/billing`}>Remaining to bill and collections</Link>
        {" · "}
        <Link href={`/books/${book}/record`}>Record a cost or capitalize WIP</Link>
        {" · "}
        <Link href={`/books/${book}/ingest`}>Ingest another delivery</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/accounts`}>Trial balance</Link>
      </p>
    </>
  );
}

export default withRefusal(Budget);
