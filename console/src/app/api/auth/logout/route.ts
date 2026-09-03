import { signOut } from "@workos-inc/authkit-nextjs";
import { workosConfigured } from "@/lib/workos";
import { sameOrigin } from "../redirect";

export const dynamic = "force-dynamic";

/**
 * End the AuthKit session.
 *
 * POST, not GET: signing someone out from an `<img src>` is a small thing to
 * be able to do to them. `signOut()` is what the AuthKit Next.js docs name.
 */
export async function POST() {
  if (workosConfigured()) {
    await signOut({ returnTo: "/signin" });
  }
  return sameOrigin("/signin", 303);
}
