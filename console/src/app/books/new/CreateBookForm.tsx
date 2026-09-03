"use client";

import { useActionState } from "react";
import { submit, type Result } from "./actions";

export function CreateBookForm() {
  const [result, action, pending] = useActionState(submit, null as Result);

  return (
    <form action={action} className="ticket">
      {result && !result.ok ? <p className="empty err">{result.error}</p> : null}
      <label>
        Id
        <input
          name="bookId"
          required
          pattern="[A-Za-z0-9_-]+"
          placeholder="household"
          autoComplete="off"
        />
      </label>
      <label>
        Name
        <input name="displayName" placeholder="Household" autoComplete="off" />
      </label>
      <label>
        Kind
        <select name="kind" defaultValue="PERSONAL" required>
          <option value="PERSONAL">Personal</option>
          <option value="INVESTMENT">Investment</option>
          <option value="PROJECT">Project</option>
        </select>
      </label>
      <button type="submit" className="signin-btn" disabled={pending}>
        {pending ? "Creating…" : "Create book"}
      </button>
    </form>
  );
}
