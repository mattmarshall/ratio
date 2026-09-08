import { Avatar } from "@/components/Avatar";
import { principal } from "@/lib/caller";
import { initialsOf } from "@/lib/initials";

/** The signed-in person, and the way out.
 *
 * ⚠ Sign-out is a POST, so it is a form rather than a link — see the route for
 * why. On a local run there is no principal and no control, because there is no
 * session to end.
 *
 * ⛔ `initialsOf` IS NOT IMPORTED FROM `Avatar`. That module is `"use client"`
 * for `onError`. Calling a function it exported during this server render
 * is digest `3404496738` — `Minified React error #441` — on `/books` once
 * the API accepted the session and this chip finally mounted.
 */
export async function Who() {
  const me = await principal();
  const label = me ? me.email || me.sub : "Operator";
  const picture = me?.profilePictureUrl ?? null;
  return (
    <span className="who">
      <Avatar src={picture} initials={me ? initialsOf(me) : "OP"} />
      {/* Wrapped so a phone can drop the words while keeping the avatar — and
          so a long email shrinks to an ellipsis instead of widening the header. */}
      <span className="wholabel">{label}</span>
      {me ? (
        <form action="/api/auth/logout" method="post">
          <button type="submit" className="signout">
            Sign out
          </button>
        </form>
      ) : null}
    </span>
  );
}
