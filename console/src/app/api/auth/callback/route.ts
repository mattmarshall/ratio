import { sameOrigin } from "../redirect";

export const dynamic = "force-dynamic";

/**
 * The Cognito callback is gone. AuthKit lands on `/callback`.
 *
 * Kept so an old bookmark does not 404; it cannot complete a sign-in.
 */
export async function GET() {
  return sameOrigin("/signin");
}
