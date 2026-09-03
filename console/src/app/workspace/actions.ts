"use server";

import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { parseWorkspace, WORKSPACE_COOKIE } from "@/lib/workspace";

/** Remember which collection is home, and go there. */
export async function setWorkspace(form: FormData): Promise<void> {
  const workspace = parseWorkspace(String(form.get("workspace") ?? ""));
  const jar = await cookies();
  jar.set(WORKSPACE_COOKIE, workspace, {
    path: "/",
    sameSite: "lax",
    httpOnly: true,
    maxAge: 60 * 60 * 24 * 400,
  });
  redirect(`/${workspace}`);
}
