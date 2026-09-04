"use client";

import { useState } from "react";

/**
 * The header chip's face.
 *
 * A WorkOS / Google photo when the session has one; initials when it
 * does not, or when the remote image fails (CSP, a stale URL). The
 * fallback must not depend on the URL being reachable at render time.
 */
export function Avatar({
  src,
  initials,
}: {
  src: string | null;
  initials: string;
}) {
  const [failed, setFailed] = useState(false);
  if (src && !failed) {
    return (
      // ⚠ RAW <img>, NOT next/image. The URL is an IdP CDN the session
      // already named; a local optimizer would be a second fetch of a
      // face, and CSP already lists the hosts the photo may come from.
      <img
        className="avatar"
        src={src}
        alt=""
        referrerPolicy="no-referrer"
        onError={() => setFailed(true)}
      />
    );
  }
  return <span className="avatar">{initials}</span>;
}

/** Two letters from the name when WorkOS has one, else the email local-part. */
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
