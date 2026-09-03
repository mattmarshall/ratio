import { caller } from "@/lib/caller";
import { isoDate } from "@/lib/dates";
import { or404 } from "@/lib/or404";
import { getCorporateAction } from "@/wire/client";

export const dynamic = "force-dynamic";

/** One corporate action, and which strikes it qualifies. */
export default async function ActionDetail({
  params,
}: {
  params: Promise<{ book: string; action: string }>;
}) {
  const { book: fund, action } = await params;
  const c = await caller();
  const a = await or404(getCorporateAction(c, fund, action));

  return (
    <aside className="detail" aria-label="Corporate action">
      <div className="dhead">
        <h2>{a.instrument}</h2>
        <div className="sub">
          {a.form} · {a.numerator}:{a.denominator}
        </div>
      </div>

      <div className="dsec">
        <h3>The announcement</h3>
        <dl className="kv">
          <dt>Ex date</dt>
          <dd>{isoDate(a.exDate)}</dd>
          <dt>Announced</dt>
          <dd>{a.announceTime.slice(0, 16).replace("T", " ")}</dd>
          <dt>Announce position</dt>
          <dd className="num">{a.announcePosition}</dd>
          <dt>Journal position</dt>
          <dd className="num">{a.journalPosition}</dd>
          <dt>Applied</dt>
          <dd className={a.applied ? "good" : ""}>{a.applied ? "yes" : "no"}</dd>
        </dl>
      </div>

      <div className="dsec">
        <h3>Why this is not a status</h3>
        {/* ⛔ APPLYING ONE TWICE DOUBLES A POSITION WHILE THE BOOKS GO ON
            TYING. `applied` is derived from the journal on every read — the
            applied action IS an entry — so nothing cached can disagree with the
            book about it. Nothing on this screen applies an action; that is
            `ratio action`, at a terminal. */}
        <p className="note">
          Applying an action twice doubles a position while the trial balance
          goes on tying, so this is read from the journal every time rather than
          held anywhere. Applying one is <code>ratio action</code>.
        </p>
      </div>

      <div className="dsec">
        <h3>Qualified strikes</h3>
        {a.qualifiedNavStrikes.length === 0 ? (
          <div className="sub">No strike carries a qualification from this.</div>
        ) : (
          <ul className="note">
            {a.qualifiedNavStrikes.map((s) => (
              <li key={s}>{s}</li>
            ))}
          </ul>
        )}
        <p className="note">Fund {fund}.</p>
      </div>
    </aside>
  );
}
