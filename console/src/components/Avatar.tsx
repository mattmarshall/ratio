"use client";

import { useState } from "react";

/**
 * The header chip's face.
 *
 * A WorkOS / Google photo when the session has one; initials when it
 * does not, or when the remote image fails (CSP, a stale URL). The
 * fallback must not depend on the URL being reachable at render time.
 *
 * ⛔ INITIALS ARE A PROP, NOT A FUNCTION THIS FILE EXPORTS. `Who`
 * computes them on the server (`lib/initials.ts`). Exporting
 * `initialsOf` from here made every signed-in `/books` render call a
 * client function during a Server Component render — production
 * digest `3404496738`.
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
