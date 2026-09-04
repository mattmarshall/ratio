"use client";

import { useActionState, useState } from "react";
import {
  Commit,
  Derived,
  Field,
  Ticket,
  type Step,
} from "@/components/Ticket";
import { billingRulesInForce, COLLECT_RECEIVABLE } from "@/lib/billingPost";
import { money } from "@/lib/format";
import { hundredths, TRADE_DATE } from "@/lib/trade";
import type { Rule } from "@/wire/types";
import { submit, type Result } from "./actions";

/**
 * Post a cash application from Project `/billing`.
 *
 * ⭐ PREVIEW THEN COMMIT, SAME DOOR AS `/record`. `validateOnly` runs the
 * identical `ApplyEvent` path and records nothing. The rule is
 * `collect_receivable` CreateBook already seeded — not a new journal
 * kind, not a payment processor.
 *
 * ⛔ UNSET UNTIL BILLED AND AR CAN SUPPORT THE CUT. This form does not
 * plug a zero. Collected stays `—` while billed is empty or AR has
 * never posted. Billed but uncollected is a real zero after the bill,
 * not after this form appears.
 */
export function BillingPostForm({ fund, rules }: { fund: string; rules: Rule[] }) {
  const [result, action, pending] = useActionState<Result, FormData>(
    submit,
    null,
  );

  const offered = billingRulesInForce(rules);
  const rule = offered.find((r) => r.ruleId === COLLECT_RECEIVABLE) ?? null;
  const ruleId = rule?.ruleId ?? "";

  const [amount, setAmount] = useState("");
  const [dated, setDated] = useState("");
  const [eventId, setEventId] = useState("");

  const parsed = amount ? hundredths(amount, "an amount") : null;
  const dateOk = !dated || TRADE_DATE.test(dated);

  const complete =
    Boolean(rule) &&
    parsed?.ok === true &&
    Boolean(eventId.trim()) &&
    dateOk;

  const now = [ruleId, amount.trim(), dated.trim(), eventId.trim()].join(" ");
  const previewed =
    result?.ok === true && result.response.validateOnly && result.signature === now;

  const amountField = (
    <Field
      name="Amount"
      value={amount}
      onValue={setAmount}
      mode="decimal"
      hint="400.00"
    />
  );
  const dateField = (
    <Field
      name="Date"
      value={dated}
      onValue={setDated}
      hint="2026-03-15 — blank is undated"
    />
  );
  const idField = (
    <Field
      name="Reference"
      value={eventId}
      onValue={setEventId}
      hint="COL-1"
    />
  );
  const ruleForm = rule ? (
    <p className="ruleform">
      <b>{rule.ruleId}</b>
      {rule.description || rule.form}
      {rule.accounts.length ? (
        <> Legs, in order: {rule.accounts.join(" · ")}.</>
      ) : null}
    </p>
  ) : (
    <p className="ruleform">No collection rule in force</p>
  );

  const echo = parsed ? (
    <Derived
      k="Amount"
      v={parsed.ok ? money(parsed.minor.toString()) : "—"}
      bad={!parsed.ok}
      from={parsed.ok ? "as the server will parse it" : parsed.error}
    />
  ) : null;

  const steps: Step[] = [
    {
      id: "amount",
      label: "Amount",
      ask: "How much cash applied to AR?",
      why: [
        "The same collect_receivable rule `/record` already posts.",
        "This page does not invent a collection kind, and it does not open a lot.",
        <>
          Major units, with a point — <code>400.00</code>, not{" "}
          <code>40000</code>.
        </>,
        "Parsed on the server by splitting on the point, never by parsing a float.",
      ],
      answer: parsed?.ok ? money(parsed.minor.toString()) : null,
      body: (
        <>
          {ruleForm}
          {amountField}
          {echo}
        </>
      ),
    },
    {
      id: "date",
      label: "Date",
      ask: "When was the cash received?",
      why: [
        "Optional. The as-of cite still moves when the entry posts.",
        "Blank is undated — unset, not a fake collection in March.",
      ],
      answer: dated.trim() ? dated.trim() : "undated",
      body: dateField,
    },
    {
      id: "id",
      label: "Reference",
      ask: "What is this collection called?",
      why: [
        "Carried onto the journal entry, so it can be traced back to the cash.",
        "Letters, digits, - _ . and at most sixty-four of them.",
      ],
      answer: eventId.trim() || null,
      body: idField,
    },
    {
      id: "review",
      label: "Review",
      ask: "What this would do.",
      answer: null,
      body: (
        <>
          {ruleForm}
          {echo}
        </>
      ),
    },
  ];

  return (
    <Ticket
      title="Post a collection"
      summary={
        complete && parsed?.ok ? (
          <>
            Post <code>{ruleId}</code> for{" "}
            <b className="num">{money(parsed.minor.toString())}</b> as{" "}
            <code>{eventId.trim()}</code>
            {dated.trim() ? <> dated {dated.trim()}</> : <>, undated</>}.
          </>
        ) : (
          "Cash against AR. Collected stays unset on this page until billed and receivable can support the cut — not a silent zero collected."
        )
      }
      steps={steps}
      compact={
        <>
          <div className="form">
            {amountField}
            {dateField}
            {idField}
          </div>
          {ruleForm}
          {echo}
        </>
      }
      actions={
        <form action={action}>
          <input type="hidden" name="fund" value={fund} />
          <input type="hidden" name="ruleId" value={rule?.ruleId ?? ""} />
          <input type="hidden" name="amount" value={amount} />
          <input type="hidden" name="dated" value={dated} />
          <input type="hidden" name="eventId" value={eventId} />
          <Commit
            preview="Preview"
            commit="Post"
            previewed={previewed}
            pending={pending}
            ready={complete && offered.length > 0}
          />
        </form>
      }
    >
      {result && !result.ok ? (
        <div className="empty err">{result.error}</div>
      ) : null}

      {result?.ok ? (
        <div className="prev">
          <div className="ph">
            <span>
              {result.response.validateOnly ? "What this would do" : "What it did"}
            </span>
            <span>{result.response.entry?.entryId}</span>
          </div>
          {result.response.entry ? (
            <div className="postings">
              {result.response.entry.postings.map((p, i) => (
                <div className="posting" key={`${p.account}-${i}`}>
                  <span>
                    <div className="p1">{p.displayName || p.account}</div>
                    <div className="p2">
                      {p.amount.startsWith("-") ? "credit" : "debit"}
                    </div>
                  </span>
                  <span className="num">{money(p.amount)}</span>
                </div>
              ))}
            </div>
          ) : null}
        </div>
      ) : null}
    </Ticket>
  );
}
