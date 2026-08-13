"use client";

import { useActionState } from "react";
import { money } from "@/lib/format";
import type { Rule } from "@/wire/types";
import { submit, type Result } from "./actions";

/**
 * Record an event, having first seen what it would do.
 *
 * ⛔ PREVIEW THEN COMMIT, AND THE PREVIEW IS THE DEFAULT. `validateOnly` runs
 * the identical code path on the server and records nothing, so what this shows
 * is what lands. A form whose primary button posts is a form that posts by
 * accident; here the plain submit previews, and committing takes a second,
 * named button.
 */
export function RecordForm({ fund, rules }: { fund: string; rules: Rule[] }) {
  const [result, action, pending] = useActionState<Result, FormData>(
    submit,
    null,
  );
  const preview = result?.ok ? result.response : null;

  return (
    <section className="record" aria-label="Record an event">
      <div className="loghead">
        <span>Record an event</span>
        <span className="sortnote">preview, then post</span>
      </div>

      <form action={action}>
        <input type="hidden" name="fund" value={fund} />
        <label>
          Rule
          <select name="ruleId" defaultValue="">
            <option value="" disabled>
              Choose a rule
            </option>
            {rules.map((r) => (
              <option key={r.name} value={r.ruleId}>
                {r.description || r.ruleId} ({r.kind.toLowerCase()})
              </option>
            ))}
          </select>
        </label>
        <label>
          Event id
          <input name="eventId" placeholder="left blank, the server names it" />
        </label>
        <label>
          Amount
          {/* ⛔ `type="text"`, NOT `type="number"`. A number input hands back a
              double and will happily reformat an int64 out of exactness. Money
              is digits here and stays digits all the way to the journal. */}
          <input name="amount" inputMode="numeric" placeholder="minor units" />
        </label>
        <label>
          Days
          <input name="days" inputMode="numeric" placeholder="accruals only" />
        </label>

        <div className="qbar">
          <button className="chip" type="submit" disabled={pending}>
            {pending ? "…" : "Preview"}
          </button>
          <button className="chip" type="submit" name="commit" value="1" disabled={pending}>
            Post
          </button>
        </div>
      </form>

      {result && !result.ok ? (
        <div className="empty err">{result.error}</div>
      ) : null}

      {preview ? (
        <div className="dsec">
          <h3>{preview.validateOnly ? "What this would do" : "What it did"}</h3>
          <dl className="kv">
            <dt>Net asset value</dt>
            <dd className="num">{money(preview.netAssetValue)}</dd>
            <dt>Was</dt>
            <dd className="num">{money(preview.previousNetAssetValue)}</dd>
          </dl>
          {preview.entry ? (
            <div className="postings">
              {preview.entry.postings.map((p, i) => (
                <div className="posting" key={`${p.account}-${i}`}>
                  <span>
                    <div className="p1">{p.displayName || p.account}</div>
                  </span>
                  <span className="num">{money(p.amount)}</span>
                </div>
              ))}
            </div>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
