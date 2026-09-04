import type { ReactElement, ReactNode } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Avatar, initialsOf } from "./Avatar";

/**
 * Header chip — photo when WorkOS has one, initials when it does not.
 *
 * ⛔ NO INVENTED FACE. Tests that need a src pass a host the CSP names
 * (`workoscdn.com`); they do not mint a person. Tests that check the
 * fallback omit the URL.
 */

const me = vi.hoisted(() => ({
  current: {
    sub: "u-1",
    email: "e.marsh@example.com",
    profilePictureUrl: null as string | null,
    firstName: null as string | null,
    lastName: null as string | null,
  },
}));

vi.mock("@/lib/caller", () => ({
  principal: async () => me.current,
}));

async function renderAsync(el: Promise<ReactNode>) {
  render((await el) as ReactElement);
}

describe("initialsOf", () => {
  it("uses the given name and family name when WorkOS has both", () => {
    expect(
      initialsOf({
        email: "matthew@example.com",
        sub: "u-1",
        firstName: "Matthew",
        lastName: "Marshall",
      }),
    ).toBe("MM");
  });

  it("falls back to the email local-part, not the first two characters of the address", () => {
    expect(
      initialsOf({ email: "e.marsh@example.com", sub: "u-1" }),
    ).toBe("EM");
  });
});

describe("Avatar", () => {
  it("renders the photo when a URL is present", () => {
    render(
      <Avatar src="https://workoscdn.com/images/v1/test" initials="MM" />,
    );
    const img = document.querySelector("img.avatar");
    expect(img?.getAttribute("src")).toBe("https://workoscdn.com/images/v1/test");
    expect(screen.queryByText("MM")).toBeNull();
  });

  it("falls back to initials when the image errors", () => {
    render(
      <Avatar src="https://workoscdn.com/images/v1/test" initials="MM" />,
    );
    fireEvent.error(document.querySelector("img.avatar")!);
    expect(document.querySelector("img.avatar")).toBeNull();
    expect(screen.getByText("MM")).toBeDefined();
  });

  it("renders initials when no URL is supplied", () => {
    render(<Avatar src={null} initials="EM" />);
    expect(document.querySelector("img.avatar")).toBeNull();
    expect(screen.getByText("EM")).toBeDefined();
  });
});

describe("Who", () => {
  it("shows the WorkOS photo when principal carries one", async () => {
    me.current = {
      sub: "u-1",
      email: "e.marsh@example.com",
      profilePictureUrl: "https://workoscdn.com/images/v1/test",
      firstName: "Matthew",
      lastName: "Marshall",
    };
    const { Who } = await import("./Who");
    await renderAsync(Who());
    expect(
      document.querySelector("img.avatar")?.getAttribute("src"),
    ).toBe("https://workoscdn.com/images/v1/test");
    expect(screen.getByText("e.marsh@example.com")).toBeDefined();
  });
});
