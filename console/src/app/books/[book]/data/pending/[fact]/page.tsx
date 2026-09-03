import { caller } from "@/lib/caller";
import { or404 } from "@/lib/or404";
import { getPendingFact } from "@/wire/client";

export const dynamic = "force-dynamic";

/** One fact that cannot post, and what is stopping it. */
export default async function PendingFactDetail({
  params,
}: {
  params: Promise<{ book: string; fact: string }>;
}) {
  const { book: fund, fact } = await params;
  const c = await caller();
  const p = await or404(getPendingFact(c, fund, fact));

  return (
    <aside className="detail" aria-label="Pending fact">
      <div className="dhead">
        <h2>{p.reference}</h2>
        <div className="sub">
          {p.kind} · {p.blocker.toLowerCase()}
        </div>
      </div>
      <div className="dsec">
        <h3>Why it cannot post</h3>
        <p className="note">{p.detail}</p>
        {/* ⛔ ABSENT AND AMBIGUOUS ARE DIFFERENT PROBLEMS. Absent means the
            master has nothing under this reference; ambiguous means it has more
            than one. Resolving them takes opposite actions, and a screen that
            said only "pending" would send somebody the wrong way. */}
        <p className="note">
          {p.blocker === "AMBIGUOUS"
            ? "The master resolves this reference more than one way. It clears when the master names one."
            : "The master has nothing under this reference. It clears when the master gains one."}
        </p>
      </div>
      <div className="dsec">
        <h3>Where it came from</h3>
        <div className="prov">
          row {p.row} of{" "}
          <span className="g">{p.deliveryDigest.slice(0, 12)}…</span>
          <br />
          mapped by {p.templateId}
          <br />
          fund {fund}
        </div>
      </div>
    </aside>
  );
}
