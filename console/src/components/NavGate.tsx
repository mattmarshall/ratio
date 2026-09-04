import type { NavGate } from "@/wire/types";

/**
 * Why a fund cannot strike a NAV, as the sentences `blocking_at` wrote.
 *
 * ⛔ THE SAME FOLD THE BADGE READS. A status of BLOCKED with no cite is
 * the bare-HTTP-400 defect: the operator sees a number and not the
 * unexplained break, the unpriced position, or the unresolved trade.
 *
 * ⚠ NULL MEANS THE LIST DID NOT LOOK. Empty lists mean it looked and
 * nothing blocks. Only a non-empty list is a refusal.
 */
export function NavGateCite({ gate }: { gate: NavGate | null | undefined }) {
  if (!gate) return null;
  const empty =
    gate.unexplainedBreaks.length === 0 &&
    gate.unresolvedTrades.length === 0 &&
    gate.unpriced.length === 0;
  if (empty) return null;

  return (
    <div className="empty err" role="status">
      <p>This fund is not ready to strike a NAV.</p>
      {gate.unexplainedBreaks.length ? (
        <>
          <h3>unexplained break</h3>
          <ul className="note">
            {gate.unexplainedBreaks.map((c) => (
              <li key={c}>{c}</li>
            ))}
          </ul>
        </>
      ) : null}
      {gate.unresolvedTrades.length ? (
        <>
          <h3>unresolved trade</h3>
          <ul className="note">
            {gate.unresolvedTrades.map((c) => (
              <li key={c}>{c}</li>
            ))}
          </ul>
        </>
      ) : null}
      {gate.unpriced.length ? (
        <>
          <h3>unpriced — not held at zero</h3>
          <ul className="note">
            {gate.unpriced.map((c) => (
              <li key={c}>{c}</li>
            ))}
          </ul>
        </>
      ) : null}
    </div>
  );
}
