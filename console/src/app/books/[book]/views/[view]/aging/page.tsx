import Link from "next/link";
import { notFound } from "next/navigation";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { caller } from "@/lib/caller";
import { periodLabel, previousMonth, utcMonth, utcYear } from "@/lib/dates";
import {
  agingShown,
  parseBucket,
  scheduleFoots,
  scheduleIsSet,
  type AgingSide,
} from "@/lib/aging";
import { getBook, operatingAging } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";
import type { AgingSchedule } from "@/wire/types";

export const dynamic = "force-dynamic";

/**
 * Aged AR/AP open items for one operating book of record.
 *
 * ⭐ NOT PROJECT `/billing`. Billing is one job's billed vs earned,
 * retainage, and collections. This page ages entity-wide control
 * accounts by due date. The journal carries the cuts — due date on
 * the invoice/bill, application on the collection/payment.
 *
 * ⛔ UNSET STAYS UNSET. A missing due date is not current. A collection
 * or payment that does not name its invoice or bill leaves that side
 * unset — remaining per item is unknown, and FIFO or an equal split
 * would make the buckets look exact while being somebody else's.
 * Empty dated buckets on a set schedule are `"0"` (nothing in that
 * window). `undated` empty is no residual line.
 *
 * Personal, project, and investment books 404 rather than wearing
 * an operating label.
 */
async function Aging({
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
  const [b, aged] = await Promise.all([
    getBook(c, book),
    operatingAging(c, book, view, period || undefined),
  ]);
  if (b.kind !== "OPERATING") notFound();

  const filters: readonly Filter[] = [
    { key: "", label: "Now" },
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
        label="As of"
        note={
          period
            ? `as of ${periodLabel(period)} — dated entries only`
            : "as of now, including undated entries"
        }
      />

      <div className="tb" role="table" aria-label="AR/AP aging">
        <Schedule
          title="Accounts receivable"
          side="receivable"
          schedule={aged.receivable}
        />
        <Schedule
          title="Accounts payable"
          side="payable"
          schedule={aged.payable}
        />
      </div>

      <p className="note">
        A missing due date is not current. Buckets sum to the control when
        every remaining item can be aged; a collection or payment that does
        not name its invoice or bill leaves that side unset — not an equal
        split. This is not Project billing.
      </p>
      <p className="note">
        <Link href={`/books/${book}/views/${view}/sheet${period ? `?period=${encodeURIComponent(period)}` : ""}`}>
          Balance sheet
        </Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/accounts`}>Trial balance</Link>
        {" · "}
        <Link href={`/books/${book}/record`}>Record a sale or an expense</Link>
      </p>
    </>
  );
}

const ROWS: ReadonlyArray<{ key: keyof AgingSchedule; label: string; hint: string }> = [
  { key: "current", label: "Current", hint: "due on or after the as-of day" },
  { key: "days130", label: "1–30 days", hint: "one to thirty days past due" },
  { key: "days3160", label: "31–60 days", hint: "thirty-one to sixty days past due" },
  { key: "days6190", label: "61–90 days", hint: "sixty-one to ninety days past due" },
  { key: "daysOver90", label: "Over 90 days", hint: "more than ninety days past due" },
  { key: "undated", label: "Undated", hint: "remaining, and no due date — not current" },
];

function Schedule({
  title,
  side,
  schedule,
}: {
  title: string;
  side: AgingSide;
  schedule: AgingSchedule | null;
}) {
  const set = scheduleIsSet(schedule);
  const foots = scheduleFoots(schedule);
  const control = parseBucket(schedule?.control ?? "");

  return (
    <div className="posgroup">
      <div className="posacct">{title}</div>
      {ROWS.map((row) => {
        const raw = set ? parseBucket(schedule![row.key]) : null;
        return (
          <div key={row.key} className="tbrow static" role="row">
            <span role="cell">
              {row.label}
              <span className="at">{row.hint}</span>
            </span>
            <span role="cell" className="num">
              {agingShown(raw, side)}
            </span>
          </div>
        );
      })}
      <div className="tbfoot static" role="row">
        <span role="cell">
          {set
            ? foots
              ? "buckets sum to the control"
              : "does not foot"
            : "unset — not current"}
          <small>
            {set
              ? "the control-account balance on the same cut"
              : "no due dates on remaining items, or a reduction that does not name its item"}
          </small>
        </span>
        <span role="cell" className="num">
          {agingShown(control, side)}
        </span>
      </div>
    </div>
  );
}

export default withRefusal(Aging);
