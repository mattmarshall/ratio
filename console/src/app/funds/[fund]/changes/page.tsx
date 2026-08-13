import Link from "next/link";
import { caller } from "@/lib/caller";
import { listChangeLogEntries } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * Who changed what, and whether it was a person.
 *
 * ⭐ `actorKind` IS THE COLUMN THAT MATTERS. A model may draft a rule and may
 * not approve one; the log is where that distinction is visible after the fact.
 * An entry attributed to a model with `action: "drafted"` is a proposal waiting
 * for somebody — which is also where the rules screen gets its second list.
 */
export default async function Changes({
  params,
}: {
  params: Promise<{ fund: string }>;
}) {
  const { fund } = await params;
  const c = await caller();
  const { changeLogEntries } = await listChangeLogEntries(c, fund);

  return (
    <section className="log" aria-label="Change log">
      <div className="loghead">
        <span>Change log</span>
        <span className="sortnote">
          {changeLogEntries.length ? "newest first" : ""}
        </span>
      </div>
      {changeLogEntries.length === 0 ? (
        <div className="empty">Nothing has been recorded on this fund.</div>
      ) : null}
      {changeLogEntries.map((e) => {
        const id = e.name.split("/").pop()!;
        return (
          <Link className="logrow" key={e.name} href={`/funds/${fund}/changes/${id}`}>
            <span className="t num">{e.configDigest.slice(0, 7)}</span>
            <span className="w">
              <b>{e.action}</b> {e.subject}
              <div className="cfg">
                {e.actor} ·{" "}
                <span className="tagx">
                  {e.actorKind === "MODEL" ? "a model" : "a person"}
                </span>
                {e.detail ? ` · ${e.detail}` : ""}
              </div>
            </span>
          </Link>
        );
      })}
    </section>
  );
}
