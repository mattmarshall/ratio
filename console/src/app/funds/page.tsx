import Link from "next/link";
import { funds as fundsForRequest } from "@/lib/data";
import { count, money, STATE_CLASS, STATE_LABEL } from "@/lib/format";

export const dynamic = "force-dynamic";

/** Every fund this operator administers, with what each one is waiting on. */
export default async function Funds() {
  const funds = await fundsForRequest();

  return (
    <main className="queue">
      <div className="qhead">
        <h1>Your funds</h1>
        <div className="subhead">
          <span>{count(String(funds.length))} administered</span>
        </div>
      </div>

      <ul className="rows">
        {funds.length === 0 ? (
          <li>
            <div className="empty">
              No funds are granted to you. Membership is data, not a group:
              a fund is granted in <code>MEMBERSHIP.tsv</code>.
            </div>
          </li>
        ) : null}
        {funds.map((f) => {
          const id = f.name.replace(/^funds\//, "");
          return (
            <li key={f.name}>
              <Link className="row" href={`/funds/${id}/breaks`}>
                <span className={`sev ${f.state === "BLOCKED" ? "high" : "low"}`} />
                <span>
                  <div className="title">{f.displayName}</div>
                  <div className="why">
                    <span className={`state ${STATE_CLASS[f.state]}`}>
                      {STATE_LABEL[f.state]}
                    </span>
                    {" · "}
                    {count(f.entryCount)} entries · configuration{" "}
                    {f.configDigest ? f.configDigest.slice(0, 7) : "none"}
                  </div>
                </span>
                <span className="amt num">
                  {money(f.netAssetValue)}
                  <small>{f.currencyCode}</small>
                </span>
              </Link>
            </li>
          );
        })}
      </ul>
    </main>
  );
}
