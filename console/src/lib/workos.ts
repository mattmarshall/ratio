/**
 * Whether AuthKit can run in this process.
 *
 * ⭐ EMPTY MEANS LOCAL. `ratio watch` and `next dev` set none of these, so
 * the console skips sign-in and the API answers as `Subject::Local`. The
 * same shape as the Cognito path it replaces — one IdP, or none.
 *
 * Variables are the ones AuthKit-for-Next.js documents. Do not invent a
 * second cookie password or a Cognito client beside them.
 */
export function workosConfigured(): boolean {
  return Boolean(
    process.env.WORKOS_CLIENT_ID &&
      process.env.WORKOS_API_KEY &&
      process.env.WORKOS_COOKIE_PASSWORD,
  );
}
