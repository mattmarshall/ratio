import { describe, expect, it } from "vitest";
import { defaultScreen, screensFor } from "./screens";

describe("kind-aware places", () => {
  it("an investment book leads with capital activity and still offers NAV", () => {
    const labels = screensFor("INVESTMENT").filter((s) => s.group === "book").map((s) => s.label);
    expect(labels[0]).toBe("Capital activity");
    expect(labels).toContain("Exceptions");
    expect(labels).toContain("NAV");
    expect(labels).toContain("Positions");
    expect(defaultScreen("INVESTMENT")).toBe("capital");
  });

  it("a personal book is not sent to the investment capital figure", () => {
    const segs = screensFor("PERSONAL").map((s) => s.segment);
    expect(segs).not.toContain("capital");
    expect(defaultScreen("PERSONAL")).toBe("breaks");
  });

  it("a project book is not sent to the investment capital figure", () => {
    expect(screensFor("PROJECT").map((s) => s.segment)).not.toContain("capital");
  });
});
