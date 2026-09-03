"use client";

import { useActionState } from "react";
import { BOOK_TEMPLATES } from "@/lib/templates";
import { submit, type Result } from "./actions";

export function CreateBookForm() {
  const [result, action, pending] = useActionState(submit, null as Result);

  return (
    <form action={action} className="form newbook">
      {result && !result.ok ? <p className="empty err">{result.error}</p> : null}
      <label>
        <span>Id</span>
        <input
          name="bookId"
          required
          pattern="[A-Za-z0-9_-]+"
          placeholder="household"
          autoComplete="off"
        />
      </label>
      <label>
        <span>Name</span>
        <input name="displayName" placeholder="Household" autoComplete="off" />
      </label>
      <fieldset className="templates">
        <legend>Template</legend>
        {BOOK_TEMPLATES.map((t) => (
          <label key={t.kind} className="template">
            <input
              type="radio"
              name="kind"
              value={t.kind}
              defaultChecked={t.kind === "PERSONAL"}
              required
            />
            <span className="template-name">{t.label}</span>
            <span className="template-blurb">{t.blurb}</span>
          </label>
        ))}
      </fieldset>
      <button type="submit" className="signin-btn" disabled={pending}>
        {pending ? "Creating…" : "Create book"}
      </button>
    </form>
  );
}
