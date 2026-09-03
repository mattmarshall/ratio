import { caller } from "@/lib/caller";
import { or404 } from "@/lib/or404";
import { diffConfigVersions, getConfigVersion } from "@/wire/client";

export const dynamic = "force-dynamic";

const CHANGE_LABEL: Record<string, string> = {
  ADDED: "added",
  CHANGED: "changed",
  REMOVED: "removed",
  UNSPECIFIED: "",
};

/**
 * What one configuration changed.
 *
 * ⚠ `?base=` NAMES THE OTHER SIDE. Without it the server diffs against the
 * version before this one, which is the question people mean; with it, any two
 * versions can be compared. Both are the same URL shape, so either is a link.
 */
export default async function Diff({
  params,
  searchParams,
}: {
  params: Promise<{ book: string; version: string }>;
  searchParams: Promise<{ base?: string }>;
}) {
  const { book: fund, version } = await params;
  const { base } = await searchParams;
  const c = await caller();
  const v = await or404(getConfigVersion(c, fund, version));
  const d = await diffConfigVersions(c, fund, version, base);

  return (
    <aside className="detail" aria-label="Configuration diff">
      <div className="dhead">
        <h2>What changed</h2>
        <div className="sub">
          {d.baseDigest.slice(0, 7)} → {d.digest.slice(0, 7)} · sequence{" "}
          {v.sequence}
        </div>
      </div>

      <div className="dsec">
        <h3>Rules</h3>
        {d.changes.length === 0 ? (
          <div className="sub">
            Nothing changed between these two configurations.
          </div>
        ) : (
          d.changes.map((ch) => (
            <div className="posting" key={ch.ruleId + ch.kind}>
              <span>
                <div className="p1">
                  {ch.ruleId} <span className="tagx">{CHANGE_LABEL[ch.kind]}</span>
                </div>
                {/* Both sides, because "changed" without the old form is a
                    claim rather than a diff. */}
                {ch.baseForm ? <div className="p2">was {ch.baseForm}</div> : null}
                {ch.form ? <div className="p2">now {ch.form}</div> : null}
              </span>
            </div>
          ))
        )}
      </div>
    </aside>
  );
}
