import { describe, expect, it } from "vitest";
import { figure } from "./project";

describe("project figures", () => {
  it("renders unset as a dash and zero as zero", () => {
    expect(figure("")).toBe("—");
    expect(figure("0")).toBe("0.00");
    expect(figure("10000")).toBe("100.00");
    expect(figure("-20000")).toBe("-200.00");
  });
});
