import { describe, expect, it } from "vitest";
import {
  PROJECT_SCREENS,
  SCREENS,
  defaultScreen,
  screensFor,
  ticketsFor,
} from "./screens";

describe("kind selects the screens", () => {
  it("keeps investment books on fund-ops places", () => {
    expect(screensFor("INVESTMENT")).toBe(SCREENS);
    expect(defaultScreen("INVESTMENT")).toBe("breaks");
    expect(ticketsFor("INVESTMENT").some((t) => t.segment === "trade")).toBe(true);
  });

  it("offers billing on a project book, not Exceptions or NAV", () => {
    expect(defaultScreen("PROJECT")).toBe("billing");
    expect(PROJECT_SCREENS.map((s) => s.segment)).toContain("billing");
    expect(PROJECT_SCREENS.map((s) => s.label)).not.toContain("Exceptions");
    expect(PROJECT_SCREENS.map((s) => s.label)).not.toContain("NAV");
    expect(PROJECT_SCREENS.map((s) => s.label)).not.toContain("Positions");
    expect(ticketsFor("PROJECT").some((t) => t.segment === "trade")).toBe(false);
    expect(ticketsFor("PROJECT").some((t) => t.segment === "mark")).toBe(false);
  });
});
