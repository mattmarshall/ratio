import { caller } from "@/lib/caller";
import { or404 } from "@/lib/or404";
import { getChangeLogEntry } from "@/wire/client";

export const dynamic = "force-dynamic";

/** One entry in the change log. */
export default async function ChangeDetail({
  params,
}: {
  params: Promise<{ fund: string; entry: string }>;
}) {
  const { fund, entry } = await params;
  const c = await caller();
  const e = await or404(getChangeLogEntry(c, fund, entry));

  return (
    <aside className="detail" aria-label="Change log entry">
      <div className="dhead">
        <h2>
          {e.action} {e.subject}
        </h2>
        <div className="sub">{e.actorKind === "MODEL" ? "a model" : "a person"}</div>
      </div>
      <div className="dsec">
        <h3>Who</h3>
        <dl className="kv">
          <dt>Actor</dt>
          <dd>{e.actor}</dd>
          <dt>Kind</dt>
          <dd>{e.actorKind.toLowerCase()}</dd>
          <dt>Configuration</dt>
          <dd className="num">{e.configDigest}</dd>
        </dl>
        {/* ⛔ THE ACTOR IS THE VERIFIED SUBJECT, NEVER THE REQUEST BODY. A NAV
            is signed once and read for years, so what is recorded has to outlive
            the session that produced it — and an audit trail that invents an
            actor is worse than one admitting it does not know. */}
        <p className="note">
          Recorded from the verified identity on the request, not from anything
          the caller supplied.
        </p>
      </div>
      {e.detail ? (
        <div className="dsec">
          <h3>Detail</h3>
          <p className="note">{e.detail}</p>
        </div>
      ) : null}
      <div className="dsec">
        <h3>Fund</h3>
        <div className="prov">{fund}</div>
      </div>
    </aside>
  );
}
