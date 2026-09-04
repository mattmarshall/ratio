import Link from "next/link";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { caller } from "@/lib/caller";
import {
  activityOf,
  bookCapital,
  capitalShown,
  endingCapital,
  isPosted,
  factsInWindow,
  partnerCapitalAccounts,
  partnersOf,
  remainingUndrawn,
  undrawnFigure,
  undrawnOf,
  type AllocationFact,
  type AllocationKind,
  type PartnerCapitalAccount,
  type PartnerCut,
  type SpecialAllocation,
} from "@/lib/capital";
import { unitsShown } from "@/lib/nav";
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
 *
 * ⭐ THE CAPITAL ACCOUNT STATEMENT COMPOSES ON THIS URL. Beginning →
 * contributions → distributions → allocated plugs → ending, partner by
 * partner. Period rows read the Loan-shaped `nav-*` fold `/nav` already
 * cites — `capital-*` is Activity, and that fold makes every beginning
 * 0. Allocated income / expense / unrealized stay unset without a
 * named `[[partner_cut]]` — never a fake zero share, never a silent
 * equal split of book NAV. A written cut fills the plugs when the
 * figure divides. One chrome list (`screensFor`); `/strikes` stays
 * ABOR NAV.
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
          : b.kind === "OPERATING"
            ? "Operating"
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
  // Period statements need the Loan-shaped fold. `capital-*` is Activity
  // (beginning always 0); `nav-*` is the two-cut fold `/nav` already cites.
  const periodAccounts = dated
    ? (await listAccounts(c, book, view, "nav", window)).accounts
    : accounts;
  const statements = partnerCapitalAccounts(
    periodAccounts,
    dated ? "period" : "inception",
    wireCut(b.partnerCut),
    wireSpecials(b.specialAllocations),
    factsInWindow(wireFacts(b.allocationFacts), window),
  );

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

      <div className="tb" role="table" aria-label="Capital account statement">
        {statements.length === 0 ? (
          <div className="posgroup">
            <div className="posacct">Capital account</div>
            <div className="tbrow static" role="row">
              <span role="cell">
                No partner-capital accounts
                <span className="at">
                  a partner cut is unset, not an equal share of book NAV
                </span>
              </span>
              <span role="cell" className="num">
                —
              </span>
            </div>
          </div>
        ) : (
          statements.map((s) => (
            <Statement key={s.accountName} row={s} dated={dated} />
          ))
        )}
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

      <div className="tb" role="table" aria-label="Fee receivable">
        <div className="tbrow static" role="row">
          <span role="cell">
            Fee receivable
            <span className="at">
              {b.feeReceivable === ""
                ? "unset — no elected fee terms, not a silent zero receivable"
                : b.feeReceivable === "0"
                  ? "accrued then paid — a real zero, not unset"
                  : "accrued management fee on the journal — expense debit, receivable credit"}
            </span>
          </span>
          <span role="cell" className="num">
            {b.feeReceivable === "" ? "—" : money(b.feeReceivable)}
          </span>
        </div>
      </div>

      <p className="note">
        Allocated income, expense, and unrealized stay unset until a
        named partner-cut exists — not an equal share of book NAV, not a
        silent zero. CreateBook(Investment) writes
        <code>[[partner_cut]]</code> LP 80 / GP 20 so the live demo
        fills when the figure divides. A book that omits the table
        stays unset. Journal specials fold first; a remainder uses
        the cut. Unnamed <code>[]</code> refuses rather than inventing
        1/N. Book plugs remain on the NAV roll-forward. Not IRR, not
        a waterfall.
        Fee receivable stays unset without an elected
        <code>management_fee_accrual</code> — never a silent zero.
        Invoice and LP statements stay Connect.
        {" · "}
        <Link href={`/books/${book}/record`}>Record an event</Link>
        {" · "}
        <Link href={`/books/${book}/ingest`}>Ingest capital calls</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/accounts`}>Trial balance</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/nav`}>NAV roll-forward</Link>
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

function Statement({
  row: s,
  dated,
}: {
  row: PartnerCapitalAccount;
  dated: boolean;
}) {
  const beginningWhy = dated
    ? s.beginning === null
      ? "beginning stays unset until a dated prefix can support the cut — not a measured zero"
      : "as-of the day before this window — the same Loan-shaped fold /nav cites"
    : "since inception has no prior prefix — not a measured zero beginning";
  const endingWhy = dated
    ? s.ending === null
      ? "ending stays unset until a dated journal can support the cut"
      : "as-of this window's last day, credit-normal"
    : s.ending === null
      ? "unset — this partner has not posted, not a measured zero capital"
      : "inception-to-date ending — the same Ending the activity table cites";
  return (
    <div className="posgroup">
      <div className="posacct">Capital account — {s.grain}</div>
      <div className="tbrow static" role="row">
        <span role="cell">
          Beginning
          <span className="at">{beginningWhy}</span>
        </span>
        <span role="cell" className="num">
          {capitalShown(s.beginning)}
        </span>
      </div>
      <div className="tbrow static" role="row">
        <span role="cell">
          Contributions
          <span className="at">
            {s.contributions === null
              ? "unset — no partner cut this window"
              : "period credits on this partner — the same In above, not an equal share"}
          </span>
        </span>
        <span role="cell" className="num">
          {capitalShown(s.contributions)}
        </span>
      </div>
      <div className="tbrow static" role="row">
        <span role="cell">
          Distributions
          <span className="at">
            {s.distributions === null
              ? "unset — no partner cut this window"
              : "period debits on this partner — the same Out above"}
          </span>
        </span>
        <span role="cell" className="num">
          {capitalShown(s.distributions)}
        </span>
      </div>
      <div className="tbrow static" role="row">
        <span role="cell">
          Allocated income
          <span className="at">
            {s.allocatedIncome === null
              ? "unset — no partner-cut of period income, not an equal share of book NAV"
              : "this partner's share of period income under the named cut — not an equal split"}
          </span>
        </span>
        <span role="cell" className="num">
          {capitalShown(s.allocatedIncome)}
        </span>
      </div>
      <div className="tbrow static" role="row">
        <span role="cell">
          Allocated expense
          <span className="at">
            {s.allocatedExpense === null
              ? "unset — no partner-cut of period expense, not a silent zero share"
              : "this partner's share of period expense under the named cut — not a silent zero"}
          </span>
        </span>
        <span role="cell" className="num">
          {capitalShown(s.allocatedExpense)}
        </span>
      </div>
      <div className="tbrow static" role="row">
        <span role="cell">
          Unrealized
          <span className="at">
            {s.unrealized === null
              ? "unset — no partner-cut of Unrealized gain — not a silent equal allocation"
              : "this partner's share of Unrealized gain under the named cut — not an equal split"}
          </span>
        </span>
        <span role="cell" className="num">
          {capitalShown(s.unrealized)}
        </span>
      </div>
      <div className="tbrow static" role="row">
        <span role="cell">
          Units
          <span className="at">
            {s.units === null
              ? "unset — no units issued on this partner, not a fake zero"
              : s.units === 0n
                ? "fully redeemed — a real zero, not unset"
                : "ending units in issue on this partner — measured, not conserved"}
          </span>
        </span>
        <span role="cell" className="num">
          {unitsShown(s.units)}
        </span>
      </div>
      <div className="tbfoot static" role="row">
        <span role="cell">
          Ending
          <small>{endingWhy}</small>
        </span>
        <span role="cell" className="num">
          {capitalShown(s.ending)}
        </span>
      </div>
    </div>
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

function wireCut(
  rows: { partner: string; weight: string }[] | undefined,
): PartnerCut | null {
  if (!rows || rows.length === 0) return null;
  return rows.map((r) => ({ partner: r.partner, weight: BigInt(r.weight) }));
}

function wireSpecials(
  rows: { partner: string; kind: string; weight: string }[] | undefined,
): SpecialAllocation[] | null {
  if (!rows || rows.length === 0) return null;
  const out: SpecialAllocation[] = [];
  for (const r of rows) {
    if (r.kind !== "income" && r.kind !== "expense" && r.kind !== "unrealized") {
      continue;
    }
    out.push({
      partner: r.partner,
      kind: r.kind as AllocationKind,
      weight: BigInt(r.weight),
    });
  }
  return out.length === 0 ? null : out;
}

function wireFacts(
  rows:
    | { partner: string; kind: string; amount: string; tradeDate?: string }[]
    | undefined,
): AllocationFact[] | null {
  if (!rows || rows.length === 0) return null;
  const out: AllocationFact[] = [];
  for (const r of rows) {
    if (r.kind !== "income" && r.kind !== "expense" && r.kind !== "unrealized") {
      continue;
    }
    out.push({
      partner: r.partner,
      kind: r.kind as AllocationKind,
      amount: BigInt(r.amount),
      tradeDate: r.tradeDate ?? "",
    });
  }
  return out.length === 0 ? null : out;
}

export default withRefusal(Capital);
