/**
 * Two letters for the header chip when there is no photo.
 *
 * ⛔ NOT IN THE CLIENT `Avatar` MODULE. `Who` is a server component.
 * Calling a function exported from `"use client"` during its render is
 * how `/books` reached production as `Minified React error #441` (digest
 * `3404496738`) once AuthKit had a session the API accepted and the
 * chrome finally mounted. `Avatar` still needs `"use client"` for
 * `onError`; initials are a string it is handed, not a function it
 * exports. Same door as `BASIS_LABEL` living in `lib/format.ts` rather
 * than beside the client tabs.
 */
export function initialsOf(who: {
  email: string;
  sub: string;
  firstName?: string | null;
  lastName?: string | null;
}): string {
  const first = (who.firstName ?? "").trim();
  const last = (who.lastName ?? "").trim();
  if (first && last) return `${first[0]!}${last[0]!}`.toUpperCase();
  if (first.length >= 2) return first.slice(0, 2).toUpperCase();
  const local = (who.email || who.sub).split("@")[0] ?? "";
  const letters = local.replace(/[^a-zA-Z0-9]/g, "");
  return (letters.slice(0, 2) || "?").toUpperCase();
}
