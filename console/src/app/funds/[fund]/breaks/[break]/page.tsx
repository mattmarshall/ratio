import { notFound } from "next/navigation";
import { isMoneyBreak } from "@/lib/breaks";
import { caller } from "@/lib/caller";
import { money } from "@/lib/format";
import { getBreak, NotFound } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * One break, at a URL.
 *
 * ⭐ THIS PAGE IS THE ARGUMENT FOR THE WHOLE MIGRATION. Ratio's claim is that a
 * figure can be checked: it names the configuration it ran under and the
 * postings that produced it. A console that could not produce a link to this
 * was asking a reader to take the citation on trust — which is the one thing
 * this product is not for.
 */
export default async function BreakDetail({
  params,
}: {
  params: Promise<{ fund: string; break: string }>;
}) {
  const { fund, break: id } = await params;
  const c = await caller();

  let brk;
  try {
    brk = await getBreak(c, fund, id);
  } catch (e) {
    // ⚠ A break this subject may not see is refused with the SAME error as one
    // that does not exist. That is deliberate in `Console::book_path`, and
    // rendering both as 404 is what keeps it true on this side too.
    if (e instanceof NotFound) notFound();
    throw e;
  }

  return (
    <aside className="detail" aria-label="Break detail">
      <div className="dhead">
        <h2>{brk.account}</h2>
        <div className="sub">{brk.cause}</div>
      </div>

      {isMoneyBreak(brk) ? (
        <div className="dsec">
          <h3>The two figures</h3>
          <dl className="kv">
            <dt>Ratio</dt>
            <dd className="num">{money(brk.ratioAmount)}</dd>
            <dt>Reported</dt>
            <dd className="num">{money(brk.reportedAmount)}</dd>
            <dt>Difference</dt>
            <dd className="num">{money(brk.difference)}</dd>
          </dl>
        </div>
      ) : (
        <div className="dsec">
          <h3>Why this blocks</h3>
          <p className="note">
            There are no two figures to compare. The lot engine could not
            relieve this sale, so the position and the lot book disagree — and
            what that corrupts is the realized gain, which no reconciliation
            reaches because it has no counterparty.
          </p>
        </div>
      )}

      <div className="dsec postings">
        <h3>What produced ours</h3>
        {brk.postings.length === 0 ? (
          <div className="sub">No postings on this account.</div>
        ) : (
          brk.postings.map((p) => (
            <div className="posting" key={p.entryId + p.amount}>
              <span>
                <div className="p1">{p.memo || "—"}</div>
                <div className="p2 num">{p.entryId}</div>
              </span>
              <span className="num">{money(p.amount)}</span>
            </div>
          ))
        )}
      </div>

      {/* What somebody decided about it, if anybody has.
          ⛔ THERE IS NO FORM HERE. Accepting an explanation is `ratio accept`
          at a terminal, for the same reason approving a rule is `ratio
          approve` — a screen offering a second way round the fence would make
          the fence worth nothing. This displays; it does not decide. */}
      {brk.explanation ? (
        <div className="dsec">
          <h3>Why this is acceptable</h3>
          {/* The qualification comes FIRST when there is one. A reader shown
              the words before being told the figure moved underneath them has
              been told something that was true last week. */}
          {brk.explanation.qualification.length > 0 && (
            <p className="qual">
              This note no longer stands: {brk.explanation.qualification.join("; ")}.
            </p>
          )}
          <p>{brk.explanation.text}</p>
          <div className="prov">
            accepted by <span className="g">{brk.explanation.actor || "—"}</span>
            <br />
            against a difference of {money(brk.explanation.difference)}
            <br />
            {brk.explanation.journalPosition} entries had been posted when it was
            accepted
          </div>
        </div>
      ) : null}

      {/* Provenance is the product: a figure that cannot name what produced it
          is worth no more than the one it disagrees with. */}
      <div className="dsec">
        <h3>Provenance</h3>
        <div className="prov">
          configuration <span className="g">{brk.configDigest.slice(0, 12)}…</span>
          <br />
          account dimension {brk.accountDimension}
          <br />
          {/* The bands the severity came from, beside the severity. A grade
              whose terms a reader has to go and look up is a grade a reader
              takes on trust. */}
          {brk.tolerance ? (
            <>
              graded at {money(brk.tolerance.blocksNav)} blocks,{" "}
              {money(brk.tolerance.belowNotice)} notice
              {brk.tolerance.declared ? " — declared" : " — by custom, not declared"}
            </>
          ) : (
            <>graded by what it means, not by how much</>
          )}
          <br />
          replays identically under this configuration
        </div>
      </div>
    </aside>
  );
}
