import type { NavStrikePlan, PlanEdge, PlanNode } from "@/wire/types";

// Where the boxes go.
//
// ⛔ PURE, AND IT MEASURES NOTHING. Every coordinate below is arithmetic over
// the plan the server sent, so the SVG the server renders is byte-identical to
// the one a browser would draw. A layout that consulted the DOM — text metrics,
// `getBBox`, a ResizeObserver — would make this component client-only, and the
// whole reason this console server-renders is that a figure can be awaited.
//
// ⛔ AND IT IS DETERMINISTIC, which is the property the test asserts. A layout
// with a random tiebreak reflows on every request, so two people looking at one
// URL see two diagrams and cannot describe a step to each other — which is the
// same argument the routes file makes about the console having addresses at all.
//
// ⚠ NOT A GENERAL GRAPH LAYOUT. It is a layered left-to-right placement for a
// DAG that is known to be small (about twenty nodes) and known to be shallow.
// Reaching for dagre or elkjs would add a client-only dependency to a five-
// package application for a graph this size, and the colours would still have to
// be forced back through the design tokens.

/** How wide a step's box is, and how much room the columns get. */
export const NODE_W = 210;
export const NODE_H = 76;
const X_GAP = 62;
const Y_GAP = 20;
const PAD = 12;

export interface Placed extends PlanNode {
  x: number;
  y: number;
}

export interface Routed extends PlanEdge {
  /** An orthogonal path: out of the right edge, along a channel, into the left. */
  d: string;
  /** Where a row count sits, when there is one to show. */
  labelX: number;
  labelY: number;
}

export interface Layout {
  nodes: Placed[];
  edges: Routed[];
  width: number;
  height: number;
}

/**
 * Rank every node by its LONGEST path from a source.
 *
 * ⛔ LONGEST, NOT SHORTEST. `Classify by Account Type` is reachable from `Open
 * Book` in two hops via the chart and in five via the walk; ranking it at two
 * would draw an edge running backwards, and a plan diagram with a backwards
 * arrow reads as a cycle. The longest path puts every node to the right of
 * everything that feeds it, which is the only property that makes the picture
 * true.
 *
 * ⚠ Iterated to a fixed point rather than topologically sorted, because a
 * fixed point needs no separate cycle check: the bound is the node count, and
 * a graph that has not settled by then is not a DAG and is drawn as it is
 * rather than throwing. The server builds these and a test asserts the shape;
 * a render is not the place to discover it.
 */
function ranks(nodes: PlanNode[], edges: PlanEdge[]): Map<string, number> {
  const rank = new Map<string, number>(nodes.map((n) => [n.id, 0]));
  for (let pass = 0; pass < nodes.length; pass++) {
    let moved = false;
    for (const e of edges) {
      const from = rank.get(e.from);
      const to = rank.get(e.to);
      if (from === undefined || to === undefined) continue;
      if (to < from + 1) {
        rank.set(e.to, from + 1);
        moved = true;
      }
    }
    if (!moved) break;
  }
  return rank;
}

/**
 * Place one group's steps and route its edges.
 *
 * Within a column, order is the order the server declared the nodes in — which
 * is the order `ratio_nav::explain` writes them, i.e. the order the code runs.
 */
export function layout(nodes: PlanNode[], edges: PlanEdge[]): Layout {
  const ids = new Set(nodes.map((n) => n.id));
  const inner = edges.filter((e) => ids.has(e.from) && ids.has(e.to));
  const rank = ranks(nodes, inner);

  const column = new Map<number, PlanNode[]>();
  for (const n of nodes) {
    const r = rank.get(n.id) ?? 0;
    const col = column.get(r);
    if (col) col.push(n);
    else column.set(r, [n]);
  }

  const at = new Map<string, Placed>();
  const placed: Placed[] = [];
  for (const [r, col] of [...column.entries()].sort((a, b) => a[0] - b[0])) {
    col.forEach((n, i) => {
      const p: Placed = {
        ...n,
        x: PAD + r * (NODE_W + X_GAP),
        y: PAD + i * (NODE_H + Y_GAP),
      };
      at.set(n.id, p);
      placed.push(p);
    });
  }

  const routed: Routed[] = [];
  for (const e of inner) {
    const a = at.get(e.from);
    const b = at.get(e.to);
    if (!a || !b) continue;
    const x1 = a.x + NODE_W;
    const y1 = a.y + NODE_H / 2;
    const x2 = b.x;
    const y2 = b.y + NODE_H / 2;
    // The channel between the two columns. An edge that skips a column still
    // turns in the gap immediately after its source, so parallel edges share a
    // channel instead of crossing boxes.
    const mid = x1 + X_GAP / 2;
    const d =
      y1 === y2
        ? `M ${x1} ${y1} L ${x2} ${y2}`
        : `M ${x1} ${y1} L ${mid} ${y1} L ${mid} ${y2} L ${x2} ${y2}`;
    routed.push({ ...e, d, labelX: mid, labelY: (y1 + y2) / 2 - 6 });
  }

  const width =
    placed.reduce((w, n) => Math.max(w, n.x + NODE_W), 0) + PAD;
  const height =
    placed.reduce((h, n) => Math.max(h, n.y + NODE_H), 0) + PAD;
  return { nodes: placed, edges: routed, width, height };
}

/**
 * The steps a reader has asked to see.
 *
 * ⛔ THIS HIDES SUB-GRAPHS, NEVER THE COMPARISON. `NavStrikePlan.chosenReads`,
 * `rewriteReads` and `scanReads` are rendered whatever this returns, because
 * `ratio bench` "reports two curves and both must be quoted" and a plan screen
 * showing only the flat one would be that overclaim drawn as a diagram. What
 * collapsing buys is a smaller picture, not a friendlier number.
 */
export function visible(plan: NavStrikePlan, rejected: boolean): PlanNode[] {
  return plan.nodes.filter((n) => rejected || n.role !== "REJECTED");
}

/**
 * A step's share of what its group's chosen steps read, 0–1. For the bar.
 *
 * ⚠ AN EMPTY FIGURE AND A ZERO BOTH DRAW NOTHING, AND THAT IS RIGHT HERE — a
 * bar has no way to show "not estimated" that is different from "estimated at
 * nothing". The distinction is made in the LABEL, which renders `—` for the
 * first and `0` for the second, and `PlanGraph`'s render test is where it is
 * asserted. A guard in this function would look like it carried the
 * distinction while `Number("")` quietly made it zero anyway.
 */
export function share(node: PlanNode, of: number): number {
  const r = Number(node.estimatedReads);
  if (!Number.isFinite(r) || of <= 0) return 0;
  return Math.max(0, Math.min(1, r / of));
}
