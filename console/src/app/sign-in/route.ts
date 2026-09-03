import { startAuthKit } from "@/app/api/auth/start";

export const dynamic = "force-dynamic";

/**
 * Initiate login URL — `app/sign-in/route.ts` in the AuthKit-for-Next.js
 * README, and the Sign-in URL already registered on the Ratio WorkOS
 * application (`http://localhost:3000/sign-in`,
 * `https://ratio-ims.vercel.app/sign-in`).
 *
 * `/login` and `/api/auth/login` are the same handler. Do not invent a
 * fourth path.
 */
export const GET = startAuthKit;
