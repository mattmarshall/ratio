import type { ReactNode } from "react";
import { Brand } from "@/components/Brand";
import { FundRail } from "@/components/FundRail";
import { Who } from "@/components/Who";
import { funds as fundsForRequest } from "@/lib/data";

// ⛔ Never prerendered. This reads a fund's state, and a page baked at build
// time would serve a NAV from whenever the build ran.
export const dynamic = "force-dynamic";

/** The frame every fund-scoped screen renders inside: the header, and the rail. */
export default async function FundsLayout({
  children,
}: {
  children: ReactNode;
}) {
  const funds = await fundsForRequest();

  return (
    <div className="app">
      <header className="top">
        <Brand />
        <span className="crumb">
          Operations <span aria-hidden="true">/</span> <b>NAV</b>
        </span>
        <span className="spacer" />
        <Who />
      </header>
      <div className="body">
        <FundRail funds={funds} />
        {children}
      </div>
    </div>
  );
}
