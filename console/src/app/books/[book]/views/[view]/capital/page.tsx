import Link from "next/link";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { caller } from "@/lib/caller";
import {
  activityOf,
  bookCapital,
  endingCapital,
  isPosted,
  partnersOf,
  remainingUndrawn,
  undrawnFigure,
  undrawnOf,
} from "@/lib/capital";
import { periodLabel, previousMonth, utcMonth, utcYear } from "@/lib/dates";
import { money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { listAccounts, getBook } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";
import type { Account } from "@/wire/types";

export const dynamic = "force-dynamic";

/**
 * Partner capital and named capital activity, for Investment books.
 *
 * ⛔ NOT A RETURN, NOT IRR, NOT ATTRIBUTION. In is every credit to the
 * account (contributions, income allocated, transfers in). Out is every
 * debit (distributions, fees allocated, transfers out). Ending is credit
 * less debit. PLAN.md refuses performance reporting; this is the ledger
 * of who put money in and took money out.
 *
 * ⭐ UNDRAWN IS A STOCK ON THE SAME JOURNAL. A commitment is equity that
 * cancels (Dr undrawn / Cr commitments). A call draws that pair while
 * cash and partner capital move. Remaining is debit-normal on the
 * undrawn account. `postingCount === "0"` is unset — not a callable
 * zero — and a fully-drawn line is a real zero.
 *
 * Period is an AIP-160 `filter` (`capital-2026-03`). Bare `capital` is
 * inception-to-date, including undated entries. A dated suffix drops them.
 * Remaining undrawn is the since-inception figure; a month chip is the
 * window's commits and calls, not outstanding.
 */
async function Capital({
  params,
  searchParams,
}: {
  params: Promise<{ book: string; view: string }>;
  searchParams: Promise<{ filter?: string }>;
}) {
  const { book, view } = await params;
  const { filter: raw = "capital" } = await searchParams;
  const c = await caller();
  const b = await or404(getBook(c, book));
  if (b.kind !== "INVESTMENT") {
    const kind =
      b.kind === "PERSONAL"
        ? "Personal"
        : b.kind === "PROJECT"
          ? "Project"
          : "not an investment book";
    return (
      <div className="empty err" role="status">
        Capital activity is an Investment figure — this book is {kind}.
      </div>
    );
  }

  const month = utcMonth();
  const year = utcYear();
  const last = previousMonth(month);
  const filter = raw.startsWith("capital") ? raw : "capital";
  const window = filter.startsWith("capital-") ? filter.slice("capital-".length) : "";
  const dated = window.length > 0;
  const { accounts } = await listAccounts(c, book, view, filter);

  const partners = partnersOf(accounts);
  const activity = activityOf(accounts);
  const undrawn = undrawnOf(accounts);
  const total = bookCapital(accounts);
  const remaining = dated ? null : undrawnFigure(accounts);
  const undrawnGap =
    undrawn.length === 0
      ? "this chart has no commitment accounts — undrawn is unset, not a callable zero"
      : remaining === null
        ? dated
          ? "outstanding undrawn is the since-inception figure — this window is commits and calls, not remaining"
          : "unset — no commitment has been posted, not a callable zero"
        : dated
          ? "net this window — not outstanding"
          : "remaining commitment, partner grain — not a return";

  const chips: readonly Filter[] = [
    { key: "capital", label: "Since inception" },
    { key: `capital-${month}`, label: periodLabel(month) },
    { key: `capital-${last}`, label: periodLabel(last) },
    { key: `capital-${year}`, label: year },
  ];

  return (
    <>
      <FilterChips
        filters={chips}
        active={filter}
        label="Capital window"
        note={
          dated
            ? `${periodLabel(window)} — dated entries only, not a return`
            : "since inception — who put money in and took money out, not IRR"
        }
      />

      <div className="tb" role="table" aria-label="Capital activity">
        <div className="tbrow tbhead" role="row">
          <span role="columnheader">Account</span>
          <span role="columnheader">In</span>
          <span role="columnheader">Out</span>
          <span role="columnheader">{dated ? "Net this window" : "Ending"}</span>
        </div>

        <Section title="Partners" empty="No partner-capital accounts in this chart.">
          {partners.map((a) => (
            <Row key={a.name} book={book} view={view} account={a} />
          ))}
        </Section>

        <Section
          title="Activity"
          empty="No contribution / distribution / allocation / transfer accounts."
        >
          {activity.map((a) => (
            <Row key={a.name} book={book} view={view} account={a} />
          ))}
        </Section>

        <div className="posgroup">
          <div className="tbfoot static" role="row">
            <span role="cell">
              {dated ? "Net capital this window" : "Ending capital"}
              <small>
                partners plus unallocated activity — not a return, not attribution
              </small>
            </span>
            <span role="cell" className="num" />
            <span role="cell" className="num" />
            <span role="cell" className="num">
              {money(total.toString())}
            </span>
          </div>
        </div>
      </div>

      <div className="tb" role="table" aria-label="Undrawn commitment">
        <div className="tbrow tbhead" role="row">
          <span role="columnheader">Account</span>
          <span role="columnheader">Committed</span>
          <span role="columnheader">Called</span>
          <span role="columnheader">{dated ? "Net this window" : "Undrawn"}</span>
        </div>

        <Section
          title="Commitments"
          empty="No commitment / undrawn accounts in this chart."
        >
          {undrawn.map((a) => (
            <UndrawnRow key={a.name} book={book} view={view} account={a} dated={dated} />
          ))}
        </Section>

        <div className="posgroup">
          <div className="tbfoot static" role="row">
            <span role="cell">
              {dated ? "Undrawn this window" : "Undrawn commitment"}
              <small>{undrawnGap}</small>
            </span>
            <span role="cell" className="num" />
            <span role="cell" className="num" />
            <span role="cell" className="num">
              {remaining === null ? "—" : money(remaining.toString())}
            </span>
          </div>
        </div>
      </div>

      <p className="note">
        Record a contribution, distribution, commitment, or capital call against
        the seeded capital rules.
        {" · "}
        <Link href={`/books/${book}/record`}>Record an event</Link>
        {" · "}
        <Link href={`/books/${book}/ingest`}>Ingest capital calls</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/accounts`}>Trial balance</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/strikes`}>NAV</Link>
      </p>
    </>
  );
}

function Section({
  title,
  empty,
  children,
}: {
  title: string;
  empty: string;
  children: React.ReactNode;
}) {
  const rows = Array.isArray(children) ? children : children ? [children] : [];
  return (
    <div className="posgroup">
      <div className="posacct">{title}</div>
      {rows.length === 0 ? (
        <div className="tbrow static" role="row">
          <span role="cell">{empty}</span>
          <span role="cell" className="num">
            —
          </span>
          <span role="cell" className="num">
            —
          </span>
          <span role="cell" className="num">
            —
          </span>
        </div>
      ) : (
        children
      )}
    </div>
  );
}

function Row({
  book,
  view,
  account: a,
}: {
  book: string;
  view: string;
  account: Account;
}) {
  const id = a.name.split("/").pop()!;
  return (
    <Link
      className="tbrow"
      role="row"
      href={`/books/${book}/views/${view}/accounts/${id}`}
    >
      <span role="cell">{a.displayName}</span>
      <span role="cell" className="num">
        {money(a.credit)}
      </span>
      <span role="cell" className="num">
        {money(a.debit)}
      </span>
      <span role="cell" className="num">
        {money(endingCapital(a).toString())}
      </span>
    </Link>
  );
}

function UndrawnRow({
  book,
  view,
  account: a,
  dated,
}: {
  book: string;
  view: string;
  account: Account;
  dated: boolean;
}) {
  const id = a.name.split("/").pop()!;
  const posted = isPosted(a);
  const remaining = remainingUndrawn(a);
  return (
    <Link
      className="tbrow"
      role="row"
      href={`/books/${book}/views/${view}/accounts/${id}`}
    >
      <span role="cell">{a.displayName}</span>
      <span role="cell" className="num">
        {posted ? money(a.debit) : "—"}
      </span>
      <span role="cell" className="num">
        {posted ? money(a.credit) : "—"}
      </span>
      <span role="cell" className="num">
        {posted ? money(remaining.toString()) : dated ? "—" : "unset"}
      </span>
    </Link>
  );
}

export default withRefusal(Capital);
