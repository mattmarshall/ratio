import { describe, expect, it } from "vitest";
import { parseWorkspace, workspaceHome } from "./workspace";

describe("the home workspace", () => {
  it("defaults a new operator to /books", () => {
    expect(parseWorkspace(undefined)).toBe("books");
    expect(parseWorkspace("")).toBe("books");
    expect(parseWorkspace("nope")).toBe("books");
    expect(workspaceHome(undefined)).toBe("/books");
  });

  it("honours a stored funds or projects preference", () => {
    expect(workspaceHome("funds")).toBe("/funds");
    expect(workspaceHome("projects")).toBe("/projects");
    expect(workspaceHome("books")).toBe("/books");
  });
});
