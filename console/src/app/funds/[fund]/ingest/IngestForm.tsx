"use client";

import { useActionState } from "react";
import { count, money } from "@/lib/format";
import type { Template } from "@/wire/types";
import { admit, read, type AdmitResult, type ReadResult } from "./actions";

/**
 * Read a file, having first seen what it would produce.
 *
 * The same preview-then-commit shape as recording an event, and the same
 * reason: `validateOnly` runs the identical code path and records nothing, so a
 * mapping you have watched run beats one you have to imagine.
 */
export function IngestForm({
  fund,
  templates,
}: {
  fund: string;
  templates: Template[];
}) {
  const [readResult, readAction, reading] = useActionState<ReadResult, FormData>(
    read,
    null,
  );
  const [admitResult, admitAction, admitting] = useActionState<
    AdmitResult,
    FormData
  >(admit, null);

  const r = readResult?.ok ? readResult.response : null;
  const a = admitResult?.ok ? admitResult.response : null;

  return (
    <section className="record" aria-label="Read a file">
      <div className="loghead">
        <span>Read a file</span>
        <span className="sortnote">preview, then read</span>
      </div>

      <form action={readAction}>
        <input type="hidden" name="fund" value={fund} />
        <label>
          Template
          <select name="templateId" defaultValue="">
            <option value="" disabled>
              Choose a template
            </option>
            {templates.map((t) => (
              <option key={t.name} value={t.templateId}>
                {t.templateId} — {t.factKind}
              </option>
            ))}
          </select>
        </label>
        <label>
          Origin
          <input name="origin" placeholder="custodian/positions-2026-02-26.csv" />
        </label>
        <label>
          Content
          <textarea name="content" rows={8} placeholder="paste the file" />
        </label>
        <div className="qbar">
          <button className="chip" type="submit" disabled={reading}>
            {reading ? "…" : "Preview"}
          </button>
          <button className="chip" type="submit" name="commit" value="1" disabled={reading}>
            Read
          </button>
        </div>
      </form>

      {readResult && !readResult.ok ? (
        <div className="empty err">{readResult.error}</div>
      ) : null}

      {r ? (
        <div className="dsec">
          <h3>{r.validateOnly ? "What this would produce" : "What it produced"}</h3>
          <dl className="kv">
            <dt>Rows</dt>
            <dd className="num">{count(r.rowCount)}</dd>
            <dt>Facts</dt>
            <dd className="num">{count(r.factCount)}</dd>
            <dt>New</dt>
            <dd className="num">{count(r.newFactCount)}</dd>
            <dt>Ready to post</dt>
            <dd className="num">{count(r.readyCount)}</dd>
          </dl>
          {/* ⛔ SAY WHAT WAS REFUSED. A read that reports only its successes
              looks identical to one that dropped half the file. */}
          {r.rejected.length ? (
            <div className="postings">
              {r.rejected.map((row) => (
                <div className="posting" key={`${row.row}-${row.reason}`}>
                  <span>
                    <div className="p1">row {row.row}</div>
                    <div className="p2">{row.reason}</div>
                  </span>
                </div>
              ))}
            </div>
          ) : null}
        </div>
      ) : null}

      <form action={admitAction}>
        <input type="hidden" name="fund" value={fund} />
        <div className="loghead">
          <span>Admit</span>
          <span className="sortnote">post every fact that fully resolves</span>
        </div>
        <div className="qbar">
          <button className="chip" type="submit" disabled={admitting}>
            {admitting ? "…" : "Preview"}
          </button>
          <button className="chip" type="submit" name="commit" value="1" disabled={admitting}>
            Admit
          </button>
        </div>
      </form>

      {admitResult && !admitResult.ok ? (
        <div className="empty err">{admitResult.error}</div>
      ) : null}

      {a ? (
        <div className="dsec">
          <h3>{a.validateOnly ? "What admitting would do" : "What it did"}</h3>
          <dl className="kv">
            <dt>Posted</dt>
            <dd className="num">{count(a.postedCount)}</dd>
            <dt>Recorded</dt>
            <dd className="num">{count(a.recordedCount)}</dd>
            <dt>Still pending</dt>
            <dd className="num">{count(a.pendingCount)}</dd>
            <dt>Net asset value</dt>
            <dd className="num">{money(a.netAssetValue)}</dd>
            <dt>Was</dt>
            <dd className="num">{money(a.previousNetAssetValue)}</dd>
          </dl>
          {a.refused.length ? (
            <ul className="note">
              {a.refused.map((x) => (
                <li key={x}>{x}</li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
