import { startAuthKit } from "../start";

export const dynamic = "force-dynamic";

/**
 * Alias of `/login`. Kept so an existing sign-in button or bookmark still
 * starts AuthKit. Register `/login` as the Initiate login URL.
 */
export const GET = startAuthKit;
