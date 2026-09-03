"use client";

import { setWorkspace } from "@/app/workspace/actions";
import { WORKSPACE_LABEL, type Workspace, WORKSPACES } from "@/lib/workspace";

/**
 * Which collection is home after sign-in.
 *
 * ⛔ NOT A THIRD LEDGER. Books, funds and projects are views of the same
 * Book. This only remembers which list to open.
 */
export function WorkspaceSwitch({ current }: { current: Workspace }) {
  return (
    <form action={setWorkspace} className="workspace">
      <label>
        <span className="workspace-label">Home</span>
        <select
          name="workspace"
          defaultValue={current}
          aria-label="Home workspace"
          onChange={(e) => e.currentTarget.form?.requestSubmit()}
        >
          {WORKSPACES.map((w) => (
            <option key={w} value={w}>
              {WORKSPACE_LABEL[w]}
            </option>
          ))}
        </select>
      </label>
    </form>
  );
}
