import { renderToStaticMarkup } from "react-dom/server";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import RootLayout from "./layout";

// Keep the provider observable without invoking real WorkOS Server Actions in
// jsdom. The production-build phone test exercises the actual SDK mount.
vi.mock("@workos-inc/authkit-nextjs/components", () => ({
  AuthKitProvider: ({ children }: { children: ReactNode }) => (
    <div data-authkit-provider="mounted">{children}</div>
  ),
}));

afterEach(() => vi.unstubAllEnvs());

function configureWorkos(enabled: boolean) {
  vi.stubEnv("WORKOS_CLIENT_ID", enabled ? "client_test" : "");
  vi.stubEnv("WORKOS_API_KEY", enabled ? "test-key" : "");
  vi.stubEnv("WORKOS_COOKIE_PASSWORD", enabled ? "test-cookie-password" : "");
}

function layout() {
  return renderToStaticMarkup(RootLayout({ children: <main>Your books</main> }));
}

describe("the root layout follows the server's authentication mode", () => {
  it("renders local pages without mounting AuthKit session actions", () => {
    configureWorkos(false);
    const html = layout();
    expect(html).toContain("<main>Your books</main>");
    expect(html).not.toContain("data-authkit-provider");
  });

  it("keeps AuthKit around children when WorkOS is configured", () => {
    configureWorkos(true);
    expect(layout()).toContain(
      '<div data-authkit-provider="mounted"><main>Your books</main></div>',
    );
  });
});
