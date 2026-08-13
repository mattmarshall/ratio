import Link from "next/link";
import { caller } from "@/lib/caller";
import { money } from "@/lib/format";
import { listNavStrikes } from "@/wire/client";

export const dynamic = "force-dynamic";

/** Every NAV this fund has struck, newest first. */
export default async function Strikes({
  params,
}: {
  params: Promise<{ fund: string; view: string }>;
}) {
  const { fund, view } = await params;
  const c = await caller();
  const { navStrikes } = await listNavStrikes(c, fund, view);

  return (
    <section className="log" aria-label="NAV strikes">
      <div className="loghead">
        <span>NAV strikes</span>
        <span className="sortnote">
          {navStrikes.length ? "each one re-derivable" : ""}
        </span>
      </div>
      {navStrikes.length === 0 ? (
        <div className="empty">
          No NAV struck yet. <code>ratio strike</code> takes one.
        </div>
      ) : null}
      {navStrikes.map((s) => {
        const id = s.name.split("/").pop()!;
        return (
          <Link className="logrow strike" key={s.name} href={`/funds/${fund}/views/${view}/strikes/${id}`}>
            <span className="t num">{s.journalDigest.slice(0, 7)}</span>
            <span className="w">
              <b>{money(s.netAssetValue)}</b>
              <div className="cfg">
                {s.valuationTime.slice(0, 16).replace("T", " ")} · {s.actor} ·
                journal position {s.journalPosition}
              </div>
              {/* ⛔ THE QUALIFICATION BEFORE THE FIGURE IS READ, NOT BEHIND A
                  CLICK. A qualification a reader reaches only by opening the
                  strike is one they will read the figure without. */}
              {s.qualification.length ? (
                <div className="why">{s.qualification.join(" · ")}</div>
              ) : null}
            </span>
            <span className="amt num">
              {money(s.trialBalanceDifference)}
              <small>difference</small>
            </span>
          </Link>
        );
      })}
    </section>
  );
}
