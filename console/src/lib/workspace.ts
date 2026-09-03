// Which collection is home after sign-in.
//
// ⭐ A COOKIE, NOT A DATABASE. Membership is still a line in MEMBERSHIP.tsv;
// this is only which list the operator wants to land on. Empty means /books.

export const WORKSPACES = ["books", "funds", "projects"] as const;
export type Workspace = (typeof WORKSPACES)[number];

export const WORKSPACE_COOKIE = "ratio-workspace";

export const WORKSPACE_LABEL: Record<Workspace, string> = {
  books: "Books",
  funds: "Funds",
  projects: "Projects",
};

export function parseWorkspace(raw: string | undefined | null): Workspace {
  if (raw === "funds" || raw === "projects" || raw === "books") return raw;
  return "books";
}

export function workspaceHome(raw: string | undefined | null): string {
  return `/${parseWorkspace(raw)}`;
}
