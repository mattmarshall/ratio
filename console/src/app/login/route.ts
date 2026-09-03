import { startAuthKit } from "@/app/api/auth/start";

export const dynamic = "force-dynamic";

/**
 * Initiate login URL — the path AuthKit-for-Next.js documents
 * (`/app/login/route.ts` at https://workos.com/docs/authkit/nextjs).
 *
 * `/api/auth/login` is the same handler. Do not invent a third path.
 */
export const GET = startAuthKit;
