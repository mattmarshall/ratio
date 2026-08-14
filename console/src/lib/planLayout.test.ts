import { describe, expect, it } from "vitest";

import { NODE_H, NODE_W, layout, share, visible } from "./planLayout";
import type { ExplainNavStrikeResponse, PlanEdge, PlanNode } from "@/wire/types";

// ⛔ NEGATIVE-TEST EVERY CASE HERE. Break the property and watch it go red
// before believing it — CONTRIBUTING.md records three suites in this repository
// that were green, covered the code, and tested nothing.

const node = (id: string, over: Partial<PlanNode> = {}): PlanNode => ({
  id,
  operator: id,
  detail: "",
  group: "RECORDED",
  role: "CHOSEN",
  cites: "a theorem",
  note: [],
  estimatedReads: "",
  estimatedNanos: "",
  actualRows: "",
  actualNanos: "",
  ...over,
});

const edge = (source: string, target: string): PlanEdge => ({
  source,
  target,
  kind: "FLOW",
  rows: "",
});

describe("the plan layout", () => {
  it("puts every step to the right of everything that feeds it", () => {
    // ⛔ THE PROPERTY THE LONGEST-PATH RANK BUYS, ON THE GRAPH THAT SHOWS IT.
    // `d` is reachable from `a` in ONE hop and in THREE. A shortest-path rank
    // puts it at column 1, to the LEFT of `c` at column 2 — so the c→d edge is
    // drawn running backwards, and a plan diagram with a backwards arrow reads
    // as a cycle.
    //
    // ⚠ This is the shape of the real plan, not a contrivance: `Classify by
    // Account Type` is two hops from `Open Book` via the chart and five via the
    // journal walk. The first version of this test used a graph both rankings
    // agreed on and passed against a deliberately broken rank.
    const nodes = [node("a"), node("b"), node("c"), node("d")];
    const edges = [edge("a", "b"), edge("b", "c"), edge("c", "d"), edge("a", "d")];
    const l = layout(nodes, edges);
    const x = (id: string) => l.nodes.find((n) => n.id === id)!.x;
    for (const e of edges) expect(x(e.target)).toBeGreaterThan(x(e.source));
  });

  it("gives the same coordinates for the same input, every time", () => {
    // Two people on one URL have to see one diagram, or they cannot describe a
    // step to each other — which is the whole argument for this console having
    // addresses at all.
    const nodes = [node("a"), node("b"), node("c")];
    const edges = [edge("a", "b"), edge("a", "c")];
    expect(layout(nodes, edges)).toEqual(layout(nodes, edges));
  });

  it("never overlaps two boxes", () => {
    const nodes = ["a", "b", "c", "d", "e"].map((i) => node(i));
    const edges = [edge("a", "b"), edge("a", "c"), edge("a", "d"), edge("d", "e")];
    const l = layout(nodes, edges);
    for (const p of l.nodes) {
      for (const o of l.nodes) {
        if (p.id === o.id) continue;
        const apart =
          p.x + NODE_W <= o.x ||
          o.x + NODE_W <= p.x ||
          p.y + NODE_H <= o.y ||
          o.y + NODE_H <= p.y;
        expect(apart, `${p.id} overlaps ${o.id}`).toBe(true);
      }
    }
  });

  it("drops an edge whose endpoints are not both on the page", () => {
    // What happens when the rejected steps are collapsed: their edges must not
    // be drawn into empty space.
    //
    // ⚠ TWO GUARDS STAND BEHIND THIS — the edge filter and the `at.get` miss —
    // so removing either one alone leaves the test green. That is the behaviour
    // being asserted rather than a line, and it is deliberate; it also means a
    // sabotage run has to take out both before believing the case.
    const l = layout([node("a"), node("b")], [edge("a", "b"), edge("a", "gone")]);
    expect(l.edges).toHaveLength(1);
    expect(l.nodes).toHaveLength(2);
  });

  it("sizes the canvas to hold everything it placed", () => {
    const l = layout([node("a"), node("b")], [edge("a", "b")]);
    for (const p of l.nodes) {
      expect(p.x + NODE_W).toBeLessThanOrEqual(l.width);
      expect(p.y + NODE_H).toBeLessThanOrEqual(l.height);
    }
  });
});

describe("what a reader has asked to see", () => {
  const plan = {
    nodes: [
      node("chosen"),
      node("scan-lots", { role: "REJECTED" }),
      node("unread-lots", { role: "UNREAD" }),
      node("refuse", { role: "REFUSAL" }),
    ],
  } as ExplainNavStrikeResponse;

  it("hides the rejected steps by default and shows them when asked", () => {
    expect(visible(plan, false).map((n) => n.id)).toEqual([
      "chosen",
      "unread-lots",
      "refuse",
    ]);
    expect(visible(plan, true)).toHaveLength(4);
  });

  it("keeps the unread lots on the page even when the rejected steps are hidden", () => {
    // ⛔ `UNREAD` IS NOT `REJECTED`. The lots nothing reads are the claim the
    // system rests on; collapsing them with the plans not taken would hide the
    // one thing on this screen that is a theorem.
    expect(visible(plan, false).some((n) => n.role === "UNREAD")).toBe(true);
  });
});

describe("a step's share of its group", () => {
  it("is the fraction of the group's reads it accounts for", () => {
    expect(share(node("x", { estimatedReads: "250" }), 500)).toBe(0.5);
    expect(share(node("x", { estimatedReads: "500" }), 500)).toBe(1);
  });

  it("does not run off the end of its bar, or divide by a total of nothing", () => {
    expect(share(node("x", { estimatedReads: "9000" }), 500)).toBe(1);
    expect(share(node("x", { estimatedReads: "1" }), 0)).toBe(0);
    expect(share(node("x", { estimatedReads: "" }), 0)).toBe(0);
  });
});
