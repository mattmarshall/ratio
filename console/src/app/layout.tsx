import { AuthKitProvider } from "@workos-inc/authkit-nextjs/components";
import type { Metadata } from "next";
import type { ReactNode } from "react";
import { workosConfigured } from "@/lib/workos";
import "./globals.css";

export const metadata: Metadata = {
  title: "Ratio — books",
  description:
    "An accounting book: personal finance, investment accounting, or project finance. Every figure cites the journal prefix it was folded from.",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>
        {/* Local mode has no AuthKit middleware or session. Mounting the SDK
            still invokes session Server Actions and produces background 500s.
            Client auth hooks are not used by local console children. */}
        {workosConfigured() ? (
          <AuthKitProvider>{children}</AuthKitProvider>
        ) : (
          children
        )}
      </body>
    </html>
  );
}
