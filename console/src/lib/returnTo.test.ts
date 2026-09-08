import { describe, expect, it } from "vitest";
import { isAuthFlowPath, safeReturnTo } from "./returnTo";

describe("isAuthFlowPath", () => {
  it("names the prompt, the initiate aliases, and the callback", () => {
    expect(isAuthFlowPath("/signin")).toBe(true);
    expect(isAuthFlowPath("/sign-in")).toBe(true);
    expect(isAuthFlowPath("/login")).toBe(true);
    expect(isAuthFlowPath("/callback")).toBe(true);
    expect(isAuthFlowPath("/api/auth/login")).toBe(true);
    expect(isAuthFlowPath("/books")).toBe(false);
  });
});

describe("safeReturnTo", () => {
  it("keeps a book URL, which is the whole point of the deep link", () => {
    expect(safeReturnTo("/books/harbourline-global-value")).toBe(
      "/books/harbourline-global-value",
    );
  });
});
