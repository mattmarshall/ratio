import Link from "next/link";
import { Unavailable } from "@/components/Unavailable";
import { WorkspaceSwitch } from "@/components/WorkspaceSwitch";
import { funds as fundsForRequest } from "@/lib/data";
import { count, STATE_CLASS, STATE_LABEL } from "@/lib/format";

export const dynamic = "force-dynamic";

/** Every fund this operator administers, with what each one is waiting on. */
export default async function Funds() {
  const fundsRead = await fundsForRequest();
  if (fundsRead.unavailable !== null) {
    return <Unavailable why={fundsRead.unavailable} />;
  }
  const funds = fundsRead.value;

  return (
    <main className="queue">
      <div className="qhead">
        <h1>Your funds</h1>
        <div className="subhead">
          <span>{count(String(funds.length))} administered</span>
          <Link href="/books">All books</Link>
          <Link href="/projects">Projects</Link>
          <WorkspaceSwitch current="funds" />
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
              <Link className="row" href={`/books/${id}/views/${f.defaultView}/breaks`}>
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
                {/* ⛔ NOT A NAV. A net asset value depends on which entries
                    are recognised, so it belongs to a view and this row has no
                    view selected. Printing one here would be a figure that
                    does not say which question it answers — which is exactly
                    the failure this split was made to prevent. */}
                <span className="amt num">
                  {count(f.openBreakCount)}
                  <small>
                    open in {f.defaultView || "the only view"}
                  </small>
                </span>
              </Link>
            </li>
          );
        })}
      </ul>
    </main>
  );
}
