"use client";

import { useActionState } from "react";
import { count, money } from "@/lib/format";
import { isoDate } from "@/lib/dates";
import { mark, type MarkResult } from "./actions";

/** Mark positions to market, having first seen the marks. */
export function MarkForm({ fund }: { fund: string }) {
  const [result, action, pending] = useActionState<MarkResult, FormData>(
    mark,
    null,
  );
  const r = result?.ok ? result.response : null;

  return (
    <section className="record" aria-label="Mark to market">
      <div className="loghead">
        <span>Mark to market</span>
        <span className="sortnote">preview, then post</span>
      </div>

      <form action={action}>
        <input type="hidden" name="fund" value={fund} />
        <label>
          Valuation date
          <input name="valuationDate" placeholder="2026-02-26" inputMode="numeric" />
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

      {result && !result.ok ? <div className="empty err">{result.error}</div> : null}

      {r ? (
        <>
          <div className="dsec">
            <h3>{r.validateOnly ? "What this would post" : "What it posted"}</h3>
            <dl className="kv">
              <dt>Marks</dt>
              <dd className="num">{r.marks.length}</dd>
              <dt>Posted</dt>
              <dd className="num">{count(r.postedCount)}</dd>
              <dt>Net asset value</dt>
              <dd className="num">{money(r.netAssetValue)}</dd>
              <dt>Was</dt>
              <dd className="num">{money(r.previousNetAssetValue)}</dd>
            </dl>
          </div>

          <div className="dsec postings">
            <h3>The marks</h3>
            {r.marks.map((m) => (
              <div className="posting" key={m.instrument}>
                <span>
                  <div className="p1">{m.instrumentLabel || m.instrument}</div>
                  <div className="p2 num">
                    {count(m.quantity)} at {money(m.price)} ·{" "}
                    {isoDate(m.priceDate)}
                  </div>
                </span>
                <span className="num">
                  {money(m.movement)}
                  <small>{money(m.market)}</small>
                </span>
              </div>
            ))}
          </div>

          {/* ⛔ SAY WHAT COULD NOT BE MARKED. A valuation that reports only what
              it priced looks exactly like one that priced everything, and the
              positions it skipped are the ones carrying a stale figure. */}
          {r.unpriced.length ? (
            <div className="dsec">
              <h3>No price</h3>
              <ul className="note">
                {r.unpriced.map((m) => (
                  <li key={m.instrument}>{m.instrumentLabel || m.instrument}</li>
                ))}
              </ul>
            </div>
          ) : null}

          {/* ⚠ FX IS THE ONE FIGURE THAT WILL NOT DIVIDE EXACTLY AT ANY FINITE
              PRECISION, and the server says so where it happens rather than
              rounding quietly. */}
          {r.inexact.length ? (
            <div className="dsec">
              <h3>Inexact</h3>
              <ul className="note">
                {r.inexact.map((x) => (
                  <li key={x}>{x}</li>
                ))}
              </ul>
            </div>
          ) : null}
        </>
      ) : null}
    </section>
  );
}
