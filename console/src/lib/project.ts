// Project billing presentation: unset stays a dash, not a fake zero.
//
// ⛔ INTEGER STRINGS, NEVER A NUMBER. `lib/format.ts` refuses to parse money
// into a double; a billing page that summed with `Number` would undo that
// on the one screen a project operator actually looks at.

import { money } from "./format";

/** Empty is unset — the #66 honesty. `"0"` is a set figure of nothing. */
export function figure(minor: string): string {
  return minor === "" ? "—" : money(minor);
}

/** Debit-normal magnitude as stored. */
export function debitShown(minor: string): string {
  return figure(minor);
}

/** Credit-normal magnitude already stored as a credit (progress billings, retainage payable). */
export function creditShown(minor: string): string {
  return figure(minor);
}
