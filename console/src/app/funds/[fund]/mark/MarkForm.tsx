"use client";

import { useActionState, useState } from "react";
import { Commit, Derived, Field, Ticket, type Step } from "@/components/Ticket";
import { count, money } from "@/lib/format";
import { isoDate } from "@/lib/dates";
import { TRADE_DATE } from "@/lib/trade";
import { mark, type MarkResult } from "./actions";

/**
 * Mark positions to market, having first seen the marks.
 *
 * ⚠ A POSITION ALREADY AT MARKET MOVES BY NOTHING, so a second mark at one price
 * is a no-op rather than a second gain. That is a property of the server, not of
 * this form — but it is why previewing twice is safe.
 */
export function MarkForm({ fund }: { fund: string }) {
  const [result, action, pending] = useActionState<MarkResult, FormData>(
    mark,
    null,
  );
  const [valuationDate, setValuationDate] = useState("");
  const r = result?.ok ? result.response : null;

  const day = TRADE_DATE.test(valuationDate)
    ? {
        year: Number(valuationDate.slice(0, 4)),
        month: Number(valuationDate.slice(5, 7)),
        day: Number(valuationDate.slice(8, 10)),
      }
    : null;
  const previewed =
    result?.ok === true &&
    result.response.validateOnly &&
    result.signature === valuationDate.trim();

  const dateField = (
    <Field
      name="Valuation date"
      value={valuationDate}
      onValue={setValuationDate}
      mode="numeric"
      hint="2026-02-26"
    />
  );

  const steps: Step[] = [
    {
      id: "date",
      label: "Valuation date",
      ask: "As at what day?",
      why: "Each position moves by the difference between what the book holds it at and what it is worth on this day, with the contra in unrealized gain. A posting rather than an assignment — nothing is overwritten, so \"why is this worth more than we paid\" has an entry to point at.",
      answer: day ? isoDate(day) : null,
      body: dateField,
    },
    {
      id: "review",
      label: "Review",
      ask: "What this would post.",
      why: "A position with no price on or before the date is not marked at zero. It is reported unpriced, and it blocks — a NAV struck over a position nobody has priced is wrong in a way nobody can see.",
      answer: null,
      body: r ? <Marks r={r} /> : <p className="why">Preview to see the marks.</p>,
    },
  ];

  return (
    <Ticket
      title="Mark to market"
      summary={
        day
          ? `Mark every position this fund holds as at ${isoDate(day)}.`
          : "A valuation date, and the book moves to what it is worth on that day."
      }
      steps={steps}
      compact={<div className="form">{dateField}</div>}
      actions={
        <form action={action}>
          <input type="hidden" name="fund" value={fund} />
          <input type="hidden" name="valuationDate" value={valuationDate} />
          <Commit
            preview="Preview"
            commit="Post"
            previewed={previewed}
            pending={pending}
            ready={Boolean(day)}
          />
        </form>
      }
    >
      {result && !result.ok ? (
        <div className="empty err">{result.error}</div>
      ) : null}
      {r ? <Marks r={r} detail /> : null}
    </Ticket>
  );
}

/** The marks, what could not be priced, and what would not divide. */
function Marks({
  r,
  detail,
}: {
  r: NonNullable<Extract<MarkResult, { ok: true }>["response"]>;
  detail?: boolean;
}) {
  if (!detail) {
    return (
      <>
        <Derived
          k="Marks"
          v={String(r.marks.length)}
          from="priced on or before the date"
        />
        {/* ⛔ NOT MARKED AT ZERO. Zero says "worth what it cost"; these are
            unvalued, and they block the NAV. */}
        {r.unpriced.length ? (
          <Derived
            k="Unpriced"
            v={String(r.unpriced.length)}
            bad
            from="not marked at zero — unvalued, and they block the NAV"
          />
        ) : null}
      </>
    );
  }

  return (
    <>
      <div className="prev">
        <div className="ph">
          <span>{r.validateOnly ? "What this would post" : "What it posted"}</span>
          <span>{count(r.postedCount)} posted</span>
        </div>
        <div className="postings">
          {r.marks.map((m) => (
            <div className="posting" key={m.instrument}>
              <span>
                <div className="p1">{m.instrumentLabel || m.instrument}</div>
                <div className="p2 num">
                  {count(m.quantity)} at {money(m.price)} · {isoDate(m.priceDate)}
                </div>
              </span>
              <span className="num">
                {money(m.movement)}
                <small>{money(m.market)}</small>
              </span>
            </div>
          ))}
        </div>
        {/* ⛔ A PAIR, NOT A DELTA. */}
        <div className="navdelta">
          <span>Net asset value</span>
          <span className="num">{money(r.netAssetValue)}</span>
        </div>
        <div className="navdelta">
          <span>Was</span>
          <span className="num">{money(r.previousNetAssetValue)}</span>
        </div>
      </div>

      {/* ⛔ SAY WHAT COULD NOT BE MARKED. A valuation that reports only what it
          priced looks exactly like one that priced everything, and the positions
          it skipped are the ones carrying a stale figure. */}
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
  );
}
