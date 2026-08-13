import Link from "next/link";
import { caller } from "@/lib/caller";
import { or404 } from "@/lib/or404";
import { getConfigVersion } from "@/wire/client";

export const dynamic = "force-dynamic";

/** One configuration: what it contains, and who approved it. */
export default async function ConfigVersionDetail({
  params,
}: {
  params: Promise<{ fund: string; version: string }>;
}) {
  const { fund, version } = await params;
  const c = await caller();
  const v = await or404(getConfigVersion(c, fund, version));

  return (
    <aside className="detail" aria-label="Configuration version">
      <div className="dhead">
        <h2>{v.digest.slice(0, 12)}…</h2>
        <div className="sub">
          sequence {v.sequence}
          {v.active ? " · active" : ""}
        </div>
      </div>

      <div className="dsec">
        <h3>Who approved it</h3>
        <dl className="kv">
          <dt>Actor</dt>
          <dd>{v.actor}</dd>
          <dt>Approved</dt>
          <dd>{v.approveTime.slice(0, 16).replace("T", " ")}</dd>
          <dt>Subject</dt>
          <dd>{v.subject || "—"}</dd>
        </dl>
        {/* Every figure struck under this digest cites it. That is what makes a
            restatement a diff rather than an argument. */}
      </div>

      <div className="dsec">
        <h3>Rules</h3>
        {v.rules.length === 0 ? (
          <div className="sub">No rules in this configuration.</div>
        ) : (
          <ul className="note">
            {v.rules.map((r) => (
              <li key={r}>{r}</li>
            ))}
          </ul>
        )}
      </div>

      <div className="dsec">
        <h3>What changed</h3>
        <p className="note">
          A diff against the configuration before this one. Like a replay, it is
          an answer about the past: the same two digests always diff the same
          way, so it never goes stale.
        </p>
        <Link className="signin-btn" href={`/funds/${fund}/config/${version}/diff`}>
          Diff against the previous
        </Link>
      </div>
    </aside>
  );
}
