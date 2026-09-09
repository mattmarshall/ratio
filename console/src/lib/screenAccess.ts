import "server-only";

import { notFound } from "next/navigation";
import { offersScreen, offersTicket } from "./screens";
import type { BookKind } from "@/wire/types";

// A hidden tab cannot stop a saved URL from asking for another kind's figures.
export function requireScreen(kind: BookKind, segment: string): void {
  if (!offersScreen(kind, segment)) notFound();
}

export function requireTicket(kind: BookKind, segment: string): void {
  if (!offersTicket(kind, segment)) notFound();
}
