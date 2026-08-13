import { caller } from "@/lib/caller";
import { or404 } from "@/lib/or404";
import { getRule } from "@/wire/client";

export const dynamic = "force-dynamic";

/** One rule, as it will post. */
export default async function RuleDetail({
  params,
}: {
  params: Promise<{ fund: string; rule: string }>;
}) {
  const { fund, rule } = await params;
  const c = await caller();
  const r = await or404(getRule(c, fund, rule));

  return (
    <aside className="detail" aria-label="Rule">
      <div className="dhead">
        <h2>{r.description || r.ruleId}</h2>
        <div className="sub">
          {r.ruleId} · {r.kind.toLowerCase()}
        </div>
      </div>
      <div className="dsec">
        <h3>What it does</h3>
        {/* The rule as the rules screen shows it, so a caller can read what it
            is about to invoke rather than trust its name. */}
        <p className="prov">{r.form || "—"}</p>
      </div>
      <div className="dsec">
        <h3>Where it posts</h3>
        {r.accounts.length === 0 ? (
          <div className="sub">No accounts named.</div>
        ) : (
          <ol className="note">
            {r.accounts.map((a, i) => (
              <li key={`${a}-${i}`}>{a}</li>
            ))}
          </ol>
        )}
        <p className="note">In leg order. Fund {fund}.</p>
      </div>
    </aside>
  );
}
