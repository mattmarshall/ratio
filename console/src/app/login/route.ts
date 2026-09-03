import { startAuthKit } from "@/app/api/auth/start";

export const dynamic = "force-dynamic";

/**
 * Alias of `/sign-in`. The AuthKit-for-Next.js README allows either
 * `app/sign-in/route.ts` or `app/login/route.ts`. Ratio's WorkOS
 * application registers `/sign-in`.
 */
export const GET = startAuthKit;
