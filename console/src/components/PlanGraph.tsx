import { nanos, reads } from "@/lib/format";
import { NODE_H, NODE_W, layout, share } from "@/lib/planLayout";
import type { NavStrikePlan, PlanNode } from "@/wire/types";

// The plan, drawn.
//
// ⛔ A SERVER COMPONENT AND PLAIN SVG. There is no charting library in this
// application and this is not the place to add the first one: the layout is
// arithmetic (`lib/planLayout.ts`), the colours are the design tokens the site
// already publishes, and the whole thing renders on the server like every other
// figure here. A client-only graph library would also have to be forced back
// through `tokens_test` and past a CSP with no `unsafe-eval`.
//
// ⛔ SHAPE AS WELL AS COLOUR, which is the convention `globals.css` already
// states for `.state` and `.sev` — "so it survives a printout, a projector, and
// colour blindness". A chosen step is a solid box, a rejected one is dashed, a
// refusal carries a triangle, and the unread lots are dotted and detached.
//
// ⛔ AND THE SVG IS NOT THE ONLY COPY. Everything drawn is also written out as
// an ordinary list below it, hidden from sight and not from a screen reader —
// which is also what lets the render suite assert on the steps rather than on
// path geometry.

const BAR_W = NODE_W - 24;

/** SVG text does not wrap, and a step's detail is prose. */
function clip(s: string, n: number): string {
  return s.length <= n ? s : `${s.slice(0, n - 1)}…`;
}

function roleClass(role: string): string {
  return `pn-${role.toLowerCase()}`;
}

/**
 * One group of the plan.
 *
 * `total` is what this group's chosen steps read, and it is what the bars are a
 * share of — never the other group's total. The recorded fold and the
 * maintained read are two different questions with two different costs, and a
 * bar comparing one against the other would be a picture of a ratio nobody
 * computed.
 */
function Group({
  title,
  blurb,
  nodes,
  plan,
  total,
}: {
  title: string;
  blurb: string;
  nodes: PlanNode[];
  plan: NavStrikePlan;
  total: number;
}) {
  if (!nodes.length) return null;
  const l = layout(nodes, plan.edges);
  const heading = `${title} — ${blurb}`;

  return (
    <section className="plangroup">
      <h3>{title}</h3>
      <p className="note">{blurb}</p>
      <div className="planscroll">
        <svg
          className="plansvg"
          viewBox={`0 0 ${l.width} ${l.height}`}
          width={l.width}
          height={l.height}
          role="img"
          aria-label={heading}
        >
          <title>{heading}</title>
          <defs>
            <marker
              id={`arrow-${title.replace(/\W+/g, "")}`}
              viewBox="0 0 8 8"
              refX="7"
              refY="4"
              markerWidth="7"
              markerHeight="7"
              orient="auto-start-reverse"
            >
              <path className="pe-head" d="M 0 0 L 8 4 L 0 8 z" />
            </marker>
          </defs>

          {l.edges.map((e) => (
            <g key={`${e.from}-${e.to}`} className={`pe pe-${e.kind.toLowerCase()}`}>
              <path
                d={e.d}
                markerEnd={`url(#arrow-${title.replace(/\W+/g, "")})`}
                fill="none"
              />
              {e.rows ? (
                <text className="pe-rows" x={e.labelX} y={e.labelY} textAnchor="middle">
                  {reads(e.rows)}
                </text>
              ) : null}
            </g>
          ))}

          {l.nodes.map((n) => {
            const est = share(n, total);
            // ⛔ THE ACTUAL IS A SHARE OF THE SAME DENOMINATOR AS THE ESTIMATE,
            // so the two bars are comparable by length. Normalising each to its
            // own maximum would draw an estimate and a measurement that disagree
            // by two orders of magnitude as two bars of equal size.
            const act = share({ ...n, estimatedReads: n.actualRows }, total);
            return (
              <g key={n.id} className={`pn ${roleClass(n.role)}`}>
                <rect
                  className="pn-box"
                  x={n.x}
                  y={n.y}
                  width={NODE_W}
                  height={NODE_H}
                  rx="6"
                />
                {n.role === "REFUSAL" ? (
                  <path
                    className="pn-mark"
                    d={`M ${n.x + 12} ${n.y + 18} l 5 -9 l 5 9 z`}
                  />
                ) : null}
                <text
                  className="pn-op"
                  x={n.x + (n.role === "REFUSAL" ? 28 : 12)}
                  y={n.y + 18}
                >
                  {clip(n.operator, 29)}
                </text>
                <text className="pn-detail" x={n.x + 12} y={n.y + 33}>
                  {clip(n.detail, 38)}
                </text>

                <rect className="pn-track" x={n.x + 12} y={n.y + 42} width={BAR_W} height="5" rx="2" />
                <rect
                  className="pn-est"
                  x={n.x + 12}
                  y={n.y + 42}
                  width={Math.round(BAR_W * est)}
                  height="5"
                  rx="2"
                />
                <text className="pn-fig" x={n.x + 12} y={n.y + 60}>
                  {reads(n.estimatedReads)} reads
                </text>

                {plan.analyzed ? (
                  <>
                    <rect
                      className="pn-act"
                      x={n.x + 12}
                      y={n.y + 49}
                      width={Math.round(BAR_W * act)}
                      height="3"
                      rx="1"
                    />
                    <text className="pn-act-fig" x={n.x + NODE_W - 12} y={n.y + 60} textAnchor="end">
                      {nanos(n.actualNanos)}
                    </text>
                  </>
                ) : null}
              </g>
            );
          })}
        </svg>
      </div>

      {/* ⛔ THE SAME PLAN, IN WORDS. A diagram nothing can read aloud is a
          figure that cannot be cited, which is the defect this console exists
          to fix one level up. It is also what the render suite asserts on:
          testing that a step is on the page should not mean testing where its
          box landed. */}
      <ol className="planlist">
        {nodes.map((n) => (
          <li key={n.id}>
            <span className="plo">{n.operator}</span>
            <span className="pld">{n.detail}</span>
            <span className="plr">{ROLE_WORD[n.role] ?? n.role}</span>
            <span className="plf">
              estimated {reads(n.estimatedReads)} reads
              {n.estimatedNanos ? ` · ${nanos(n.estimatedNanos)}` : ""}
            </span>
            <span className="plf">
              {plan.analyzed
                ? `measured ${reads(n.actualRows)} rows · ${nanos(n.actualNanos)}`
                : "not measured"}
            </span>
            <span className="plc">{n.cites}</span>
            {n.note.map((t) => (
              <span className="pln" key={t}>
                {t}
              </span>
            ))}
          </li>
        ))}
      </ol>
    </section>
  );
}

const ROLE_WORD: Record<string, string> = {
  CHOSEN: "runs",
  REJECTED: "not taken",
  REFUSAL: "refuses",
  UNREAD: "never read",
};

/** What this group's chosen steps read, as a number for the bars. */
function totalOf(nodes: PlanNode[]): number {
  return nodes
    .filter((n) => n.role === "CHOSEN")
    .reduce((t, n) => t + (Number(n.estimatedReads) || 0), 0);
}

export function PlanGraph({
  plan,
  nodes,
}: {
  plan: NavStrikePlan;
  nodes: PlanNode[];
}) {
  const recorded = nodes.filter((n) => n.group === "RECORDED");
  const maintained = nodes.filter((n) => n.group === "MAINTAINED");

  return (
    <>
      <Group
        title="The strike as recorded"
        blurb="One streaming walk of the pinned prefix. This is the curve that grows with the journal, and the proved cost model has no term for it at all — what is measured here is measured, and what is not is blank."
        nodes={recorded}
        plan={plan}
        total={totalOf(recorded)}
      />
      <Group
        title="The same figure off the maintained totals"
        blurb="The chart and the currencies, and no tax lots. Flat in fragmentation — a fund's twentieth year strikes as fast as its first."
        nodes={maintained}
        plan={plan}
        total={totalOf(maintained)}
      />
    </>
  );
}
