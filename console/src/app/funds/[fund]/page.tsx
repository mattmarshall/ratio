import Link from "next/link";
import { caller } from "@/lib/caller";
import { count, gain, money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { getFund, listChangeLogEntries } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * What this fund is waiting on, and what happened to it lately.
 *
 * ⭐ A SCREEN THE OLD CONSOLE DID NOT HAVE. It opened straight onto the
 * exceptions queue, which answers "what is wrong" without ever answering "where
 * does this fund stand". The lot terms below are the part worth having in one
 * place: the method, whether it was elected or defaulted, and the gain it
 * produced.
 */
export default async function FundOverview({
  params,
}: {
  params: Promise<{ fund: string }>;
}) {
  const { fund } = await params;
  const c = await caller();
  const f = await or404(getFund(c, fund));
  const { changeLogEntries } = await listChangeLogEntries(c, fund);

  return (
    <>
      <section className="lots">
        <div className="loghead">
          <span>Lot terms</span>
          <span className="sortnote">from the administration agreement</span>
        </div>
        <dl className="kv">
          <dt>Lot method</dt>
          <dd>
            {f.lotMethod || "—"}
            {/* ⛔ BOTH CLAIMS THE METHOD ROW CAN MAKE, AND THEY ARE NOT THE
                SAME. A rule set that says nothing about lots is relieved
                oldest-first by CUSTOM, not by ELECTION — and a screen that
                prints "a term of the administration agreement" over a defaulted
                method is asserting something nobody agreed to. */}
            <span className="sub">
              {f.lotMethodDeclared
                ? " a term of the administration agreement"
                : " this configuration declares no method"}
            </span>
          </dd>
          <dt>Realized gain</dt>
          {/* Credit-normal: `gain` flips the sign in exactly one place, because
              applied per call site it gets applied twice somewhere and nowhere
              else — and both mistakes produce a plausible number. */}
          <dd className="num">{gain(f.realizedGain)}</dd>
          <dt>Short-term</dt>
          <dd className="num">{gain(f.shortTermGain)}</dd>
          <dt>Long-term</dt>
          <dd className="num">{gain(f.longTermGain)}</dd>
          <dt>Unclassified</dt>
          <dd className="num">{gain(f.unclassifiedGain)}</dd>
          <dt>Basis relieved</dt>
          <dd className="num">{money(f.basisRelieved)}</dd>
          <dt>Tax lots</dt>
          <dd className="num">
            {count(f.openLotCount)} open over {count(f.positionCount)} positions
          </dd>
        </dl>
        <p className="note">
          Long-term after {f.longTermDays} days. A disposal with no trade date
          cannot be classified and lands in unclassified rather than being
          guessed at.
        </p>
      </section>

      <section className="log" aria-label="Recent activity">
        <div className="loghead">
          <span>Recent activity</span>
          <Link className="sortnote" href={`/funds/${fund}/changes`}>
            the whole change log
          </Link>
        </div>
        {changeLogEntries.length === 0 ? (
          <div className="empty">Nothing has been recorded on this fund yet.</div>
        ) : null}
        {changeLogEntries.slice(0, 8).map((e) => (
          <div className="logrow" key={e.name}>
            <span className="t num">{e.configDigest.slice(0, 7)}</span>
            <span className="w">
              <b>{e.action}</b> {e.subject}
              <div className="cfg">
                {e.actor} · {e.actorKind === "MODEL" ? "a model" : "a person"}
              </div>
            </span>
          </div>
        ))}
      </section>
    </>
  );
}
