import { Avatar, initialsOf } from "@/components/Avatar";
import { principal } from "@/lib/caller";

/** The signed-in person, and the way out.
 *
 * ⚠ Sign-out is a POST, so it is a form rather than a link — see the route for
 * why. On a local run there is no principal and no control, because there is no
 * session to end. */
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
