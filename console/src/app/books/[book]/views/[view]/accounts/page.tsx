import Link from "next/link";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { caller } from "@/lib/caller";
import { money } from "@/lib/format";
import { listAccounts } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

const FILTERS: readonly Filter[] = [
  { key: "", label: "Whole chart" },
  { key: "posted", label: "Posted to" },
  { key: "abnormal", label: "Abnormal side" },
];

/** The trial balance: every account, whether or not anything touched it. */
async function Accounts({
  params,
  searchParams,
}: {
  params: Promise<{ book: string; view: string }>;
  searchParams: Promise<{ filter?: string }>;
}) {
  const { book: fund, view } = await params;
  const { filter = "" } = await searchParams;
  const c = await caller();
  const { accounts } = await listAccounts(c, fund, view, filter || undefined);

  return (
    <>
      <FilterChips
        filters={FILTERS}
        active={filter}
        label="Filter accounts"
        note="Chart order"
      />

      {accounts.length === 0 ? (
        <div className="empty">
          {filter
            ? "No account matches this filter."
            : "This book has no chart of accounts yet."}
        </div>
      ) : (
        <div className="tb" role="table" aria-label="Trial balance">
          <div className="tbrow tbhead" role="row">
            <span role="columnheader">Account</span>
            <span role="columnheader">Debit</span>
            <span role="columnheader">Credit</span>
            <span role="columnheader">Balance</span>
          </div>
          {accounts.map((a) => {
            const id = a.name.split("/").pop()!;
            return (
              <div key={a.name}>
                <Link
                  className="tbrow"
                  role="row"
                  href={`/books/${fund}/views/${view}/accounts/${id}`}
                >
                  <span role="cell">
                    {a.displayName}
                    {/* A balance on the side its type does not call normal. It
                        is not necessarily wrong; it is always worth reading. */}
                    {a.abnormal ? <b className="abn">*</b> : null}
                  </span>
                  <span role="cell" className="num">
                    {money(a.debit)}
                  </span>
                  <span role="cell" className="num">
                    {money(a.credit)}
                  </span>
                  <span role="cell" className="num">
                    {money(a.balance)}
                  </span>
                </Link>
                {/* ⛔ THE UNTRANSLATED PER-CURRENCY SPLIT, BENEATH THE
                    TRANSLATED TOTAL. The figure above is a conversion; a reader
                    checking it needs the facts it was converted from. Two
                    currencies are two independent conservation laws, not one law
                    over a sum — a flat total hides a currency mismatch, and this
                    repository has already shipped a NAV that did exactly that. */}
                {a.currencyTotals.length > 1
                  ? a.currencyTotals.map((t) => {
                      const factId = t.rateFact.split("/").pop();
                      return (
                        <div className="ccyrow" key={a.name + t.currencyCode}>
                          <span>
                            {t.rateFact && factId ? (
                              <Link href={`/books/${fund}/data/facts/${factId}`}>
                                {t.currencyCode}
                              </Link>
                            ) : (
                              t.currencyCode
                            )}
                          </span>
                          <span className="num">{money(t.debit)}</span>
                          <span className="num">{money(t.credit)}</span>
                          <span className="num">{money(t.balance)}</span>
                        </div>
                      );
                    })
                  : null}
              </div>
            );
          })}
        </div>
      )}
    </>
  );
}

export default withRefusal(Accounts);
