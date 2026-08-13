import Link from "next/link";
import { caller } from "@/lib/caller";
import { count, money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { getAccount, listPostings } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * The lines behind one figure, in journal order, with the running balance.
 *
 * ⚠ `listPostings` IS UNBOUNDED. `nextPageToken` is declared on the response
 * and left empty by the server, so this reads every posting on the account. The
 * old console only fetched them when a row was opened; giving an account its own
 * URL makes the read unconditional. On a busy account against a cold Lambda
 * that is the request most likely to find the 15-second ceiling — which is why
 * the account's own figures render from `getAccount` above rather than being
 * summed from this list.
 */
export default async function AccountDetail({
  params,
}: {
  params: Promise<{ fund: string; account: string }>;
}) {
  const { fund, account } = await params;
  const c = await caller();
  const a = await or404(getAccount(c, fund, account));
  const { postings } = await listPostings(c, fund, account);

  return (
    <aside className="detail" aria-label="Account detail">
      <div className="dhead">
        <h2>{a.displayName}</h2>
        <div className="sub">
          {a.type.toLowerCase()} · dimension {a.dimension} ·{" "}
          {count(a.postingCount)} postings
        </div>
      </div>

      <div className="dsec">
        <h3>The figure</h3>
        <dl className="kv">
          <dt>Debit</dt>
          <dd className="num">{money(a.debit)}</dd>
          <dt>Credit</dt>
          <dd className="num">{money(a.credit)}</dd>
          <dt>Balance</dt>
          <dd className="num">{money(a.balance)}</dd>
        </dl>
        {a.abnormal ? (
          <p className="note">
            This balance sits on the side its account type does not call normal.
          </p>
        ) : null}
      </div>

      <div className="dsec postings">
        <h3>The lines behind it</h3>
        {postings.length === 0 ? (
          <div className="sub">Nothing has posted to this account.</div>
        ) : (
          postings.map((p) => {
            const id = p.name.split("/").pop()!;
            return (
              <Link
                className="posting"
                key={p.name}
                href={`/funds/${fund}/accounts/${account}/postings/${id}`}
              >
                <span>
                  <div className="p1">{p.memo || "—"}</div>
                  <div className="p2 num">
                    {p.entryId} · {p.configDigest.slice(0, 7)}
                  </div>
                </span>
                <span className="num">
                  {money(p.amount)}
                  <small>{money(p.runningBalance)}</small>
                </span>
              </Link>
            );
          })
        )}
      </div>
    </aside>
  );
}
