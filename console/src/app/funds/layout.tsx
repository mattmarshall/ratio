import Link from "next/link";
import type { ReactNode } from "react";
import { Brand } from "@/components/Brand";
import { CommandHint } from "@/components/CommandHint";
import { FundRail } from "@/components/FundRail";
import { Palette } from "@/components/Palette";
import { Who } from "@/components/Who";
import { funds as fundsForRequest } from "@/lib/data";

// ⛔ Never prerendered. This reads a fund's state, and a page baked at build
// time would serve a NAV from whenever the build ran.
export const dynamic = "force-dynamic";

/**
 * The frame every fund-scoped screen renders inside: the header, and the rail.
 *
 * ⭐ THE COMMAND PALETTE MOUNTS HERE AND COSTS NOTHING TO FEED. `funds()` is
 * already awaited for the rail, so ⌘K reaches every fund without a second call —
 * and mounting one layout down from the root keeps it off `/`, `/signin` and the
 * error page, where there is nothing to navigate to and nobody signed in to do it.
 *
 * ⚠ `Palette` IS A CLIENT COMPONENT WRAPPING SERVER ONES. Everything below stays
 * server-rendered: `children` is an element created here and handed across the
 * boundary as a prop, not a module the client bundle has to pull in.
 */
export default async function FundsLayout({
  children,
}: {
  children: ReactNode;
}) {
  const funds = await fundsForRequest();

  return (
    <Palette funds={funds}>
      <div className="app">
        <header className="top">
          {/* The way back to the fund list. Below 860px the rail is hidden, so
              without this the only route out of a fund is the palette. */}
          <Link href="/funds" className="brandlink" aria-label="Your funds">
            <Brand />
          </Link>
          <span className="crumb">
            Operations <span aria-hidden="true">/</span> <b>NAV</b>
          </span>
          <span className="spacer" />
          <CommandHint />
          <Who />
        </header>
        <div className="body">
          <FundRail funds={funds} />
          {children}
        </div>
      </div>
    </Palette>
  );
}
