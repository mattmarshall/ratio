import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { View } from "@/wire/types";
import { PlaceHead } from "./PlaceHead";

const segments = vi.hoisted(() => ({ current: ["views", "abor", "breaks"] }));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: () => {}, replace: () => {}, refresh: () => {} }),
  usePathname: () => "/books/household/views/abor/breaks",
  useSelectedLayoutSegments: () => segments.current,
}));

const views = [
  {
    name: "funds/household/views/abor",
    displayName: "ABOR",
    basis: "TRADE_DATE",
    declared: true,
  },
  {
    name: "funds/household/views/book",
    displayName: "Book",
    basis: "RECORDED",
    declared: false,
  },
] as View[];

describe("PlaceHead", () => {
  beforeEach(() => {
    segments.current = ["views", "abor", "breaks"];
  });

  it("titles the open place and does not draw a tab strip", () => {
    const { container } = render(
      <PlaceHead
        fund="household"
        displayName="Household"
        views={views}
        defaultView="abor"
        meta={<span>USD</span>}
      />,
    );
    expect(screen.getByRole("heading", { name: "Exceptions" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Household" }).getAttribute("href")).toBe(
      "/books/household",
    );
    expect(container.querySelector(".screens")).toBeNull();
    expect(container.querySelectorAll(".places a").length).toBe(0);
  });

  it("keeps the book-of-record switch on a figure page", () => {
    render(
      <PlaceHead
        fund="household"
        displayName="Household"
        views={views}
        defaultView="abor"
        meta={<span>USD</span>}
      />,
    );
    expect(screen.getByLabelText("Book of record")).toBeDefined();
    expect(screen.getByText("ABOR")).toBeDefined();
  });

  it("titles capital activity on an investment figure page", () => {
    segments.current = ["views", "abor", "capital"];
    render(
      <PlaceHead
        fund="partners"
        displayName="Partners"
        views={views}
        defaultView="abor"
        meta={<span>USD</span>}
      />,
    );
    expect(screen.getByRole("heading", { name: "Capital activity" })).toBeDefined();
  });

  it("titles the period NAV roll-forward, not the ABOR strike", () => {
    segments.current = ["views", "abor", "nav"];
    render(
      <PlaceHead
        fund="partners"
        displayName="Partners"
        views={views}
        defaultView="abor"
        meta={<span>USD</span>}
      />,
    );
    expect(screen.getByRole("heading", { name: "NAV roll-forward" })).toBeDefined();
    expect(screen.queryByRole("heading", { name: /^NAV$/ })).toBeNull();
  });

  it("does not put a book of record on an agreement page", () => {
    segments.current = ["config"];
    render(
      <PlaceHead
        fund="household"
        displayName="Household"
        views={views}
        defaultView="abor"
        meta={<span>USD</span>}
      />,
    );
    expect(screen.getByRole("heading", { name: "Configuration" })).toBeDefined();
    expect(screen.queryByLabelText("Book of record")).toBeNull();
  });

  it("titles a personal balance sheet, not Exceptions", () => {
    segments.current = ["views", "book", "sheet"];
    render(
      <PlaceHead
        fund="household"
        displayName="Household"
        views={views}
        defaultView="book"
        meta={<span>USD</span>}
      />,
    );
    expect(screen.getByRole("heading", { name: "Balance sheet" })).toBeDefined();
    expect(screen.queryByRole("heading", { name: "Exceptions" })).toBeNull();
  });

  it("titles household cash flow, not the net-worth bridge", () => {
    segments.current = ["views", "book", "cashflow"];
    render(
      <PlaceHead
        fund="household"
        displayName="Household"
        views={views}
        defaultView="book"
        meta={<span>USD</span>}
      />,
    );
    expect(screen.getByRole("heading", { name: "Cash flow" })).toBeDefined();
    expect(screen.queryByRole("heading", { name: "Net-worth bridge" })).toBeNull();
  });

  it("titles household budget vs actual", () => {
    segments.current = ["views", "book", "budget"];
    render(
      <PlaceHead
        fund="household"
        displayName="Household"
        views={views}
        defaultView="book"
        meta={<span>USD</span>}
      />,
    );
    expect(screen.getByRole("heading", { name: "Budget vs actual" })).toBeDefined();
    expect(screen.queryByRole("heading", { name: "Exceptions" })).toBeNull();
  });

  it("titles a project figure the hub named, not Exceptions", () => {
    segments.current = ["views", "book", "budget"];
    render(
      <PlaceHead
        fund="bridge"
        displayName="Bridge"
        views={views}
        defaultView="book"
        meta={<span>USD</span>}
      />,
    );
    expect(screen.getByRole("heading", { name: "Budget vs actual" })).toBeDefined();
    expect(screen.queryByRole("heading", { name: "Exceptions" })).toBeNull();
  });

  it("titles the point-in-time browser, not Period close", () => {
    segments.current = ["views", "abor", "asof"];
    render(
      <PlaceHead
        fund="partners"
        displayName="Partners"
        views={views}
        defaultView="abor"
        meta={<span>USD</span>}
      />,
    );
    expect(screen.getByRole("heading", { name: "As-of" })).toBeDefined();
    expect(screen.queryByRole("heading", { name: "Period close" })).toBeNull();
  });

  it("titles billing on a project figure page", () => {
    segments.current = ["views", "book", "billing"];
    render(
      <PlaceHead
        fund="bridge"
        displayName="Bridge"
        views={views}
        defaultView="book"
        meta={<span>USD</span>}
      />,
    );
    expect(screen.getByRole("heading", { name: "Billing" })).toBeDefined();
    expect(screen.queryByRole("heading", { name: "Exceptions" })).toBeNull();
  });
});
