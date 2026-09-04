"use client";

import { useActionState, useState } from "react";
import {
  Commit,
  Derived,
  Field,
  Picker,
  Ticket,
  type Step,
} from "@/components/Ticket";
import { money } from "@/lib/format";
import { hundredths, wholeUnits } from "@/lib/trade";
import type { Rule } from "@/wire/types";
import { submit, type Result } from "./actions";

/** Rules whose amount is a day count as well as a figure. */
const ACCRUAL = "ACCRUAL";

/**
 * Record an event against one of this fund's approved rules.
 *
 * ⛔ PREVIEW THEN COMMIT, AND THE PREVIEW IS THE DEFAULT. `validateOnly` runs
 * the identical code path on the server and records nothing, so what this shows
 * is what lands. A form whose primary button posts is a form that posts by
 * accident; here the plain submit previews, and committing takes a second,
 * named button — and it stays shut until a preview has come back for exactly
 * these inputs.
 *
 * ⚠ THE PRIMITIVE, NOT THE WORKFLOW. This asks for a rule and an amount, which
 * is the shape of `ApplyEvent` itself. `/trade` asks for the trade and derives
 * the amount; that is the right screen for the thing an operator does daily,
 * and this is the one for everything else — a fee, a subscription, an accrual.
 */
export function RecordForm({ fund, rules }: { fund: string; rules: Rule[] }) {
  const [result, action, pending] = useActionState<Result, FormData>(
    submit,
    null,
  );

  const [ruleId, setRuleId] = useState("");
  const [amount, setAmount] = useState("");
  const [days, setDays] = useState("");
  const [units, setUnits] = useState("");
  const [eventId, setEventId] = useState("");

  const rule = rules.find((r) => r.ruleId === ruleId) ?? null;
  const accrual = rule?.kind === ACCRUAL;
  const measured = Boolean(rule?.measured);

  // ⛔ THE SAME PARSER THE SERVER USES, AND THIS FIELD USED TO LIE ABOUT ITSELF.
  // `ApplyEventRequest.amount` is "a DECIMAL string as a person types it", and
  // `ratio_common::parse_minor` reads `"42"` as forty-two POUNDS. This form
  // labelled the field "minor units" and validated `^-?\d+$`, so it refused the
  // decimal the contract documents and read everything else a hundred times too
  // large. Nothing downstream would have said a word: the entry balances at any
  // size, and the trial balance ties on it.
  const parsed = amount ? hundredths(amount, "an amount") : null;
  const unitsParsed = measured && units ? wholeUnits(units) : null;
  const daysOk = !days || /^\d+$/.test(days);

  const complete =
    Boolean(ruleId) &&
    parsed?.ok === true &&
    (!accrual || /^\d+$/.test(days)) &&
    (!measured || unitsParsed?.ok === true);

  const now = [ruleId, amount.trim(), days.trim(), units.trim(), eventId.trim()].join(
    " ",
  );
  const previewed =
    result?.ok === true && result.response.validateOnly && result.signature === now;

  const rulePicker = (
    <Picker
      name="Rule"
      value={ruleId}
      onValue={setRuleId}
      empty={rules.length === 0 ? "No rule in force" : "Choose a rule"}
      options={rules.map((r) => ({
        value: r.ruleId,
        label: `${r.description || r.ruleId} (${r.kind.toLowerCase()})`,
      }))}
    />
  );
  const amountField = (
    <Field
      name="Amount"
      value={amount}
      onValue={setAmount}
      mode="decimal"
      hint="250000.00"
    />
  );
  const daysField = (
    <Field name="Days" value={days} onValue={setDays} mode="numeric" hint="90" />
  );
  const unitsField = (
    <Field
      name="Units"
      value={units}
      onValue={setUnits}
      mode="numeric"
      hint="10"
    />
  );
  const idField = (
    <Field
      name="Event id"
      value={eventId}
      onValue={setEventId}
      hint="left blank, the server names it"
    />
  );
  const ruleForm = rule ? (
    <p className="ruleform">
      <b>{rule.ruleId}</b>
      {rule.form}
      {rule.accounts.length ? (
        <> Legs, in order: {rule.accounts.join(" · ")}.</>
      ) : null}
    </p>
  ) : null;
  // ⛔ THE FIGURE READ BACK AS THE SERVER WILL READ IT, in both views. A refusal
  // or a hundredfold that only appears on a review step is one the compact form
  // posts without ever showing.
  // ⛔ THE FIGURE READ BACK AS THE SERVER WILL READ IT, in both views. A
  // refusal or a hundredfold that only appears on a review step is one the
  // compact form posts without ever showing.
  const echo = parsed ? (
    <>
      <Derived
        k={accrual ? "Basis" : "Amount"}
        v={parsed.ok ? money(parsed.minor.toString()) : "—"}
        bad={!parsed.ok}
        from={parsed.ok ? "as the server will parse it" : parsed.error}
      />
      {accrual && parsed.ok ? (
        <Derived k="Days" v={days || "—"} from="at the rule's own rate" />
      ) : null}
      {measured && unitsParsed ? (
        <Derived
          k="Units"
          v={unitsParsed.ok ? unitsParsed.minor.toString() : "—"}
          bad={!unitsParsed.ok}
          from={
            unitsParsed.ok
              ? "whole units, issued or retired by the rule"
              : unitsParsed.error
          }
        />
      ) : null}
    </>
  ) : null;

  const steps: Step[] = [
    {
      id: "rule",
      label: "Rule",
      ask: "Which rule does this event fall under?",
      why: [
        "The rule decides which accounts move, and in which direction.",
        "Recording an event asserts that something happened; it does not decide how it books.",
        "Which is why this can be a write at all while approving a rule stays a person at a terminal.",
      ],
      answer: ruleId || null,
      body: (
        <>
          {rulePicker}
          {ruleForm}
        </>
      ),
    },
    {
      id: "amount",
      label: "Amount",
      ask: accrual ? "What basis does the rate apply to?" : "For how much?",
      why: [
        <>
          Major units, with a point — <code>250000.00</code>, not{" "}
          <code>25000000</code>.
        </>,
        "Parsed on the server by splitting on the point, never by parsing a float.",
        <>
          <code>parseFloat(&quot;1.005&quot;) * 100</code> is 100.49999999999999,
          and a product whose claim is exactness cannot lose it at the boundary.
        </>,
      ],
      answer: parsed?.ok ? money(parsed.minor.toString()) : null,
      body: (
        <>
          {amountField}
          {echo}
        </>
      ),
    },
    // ⛔ ONLY FOR AN ACCRUAL, because only an accrual has one. `Rule.Kind` says
    // what an event of each kind NEEDS, "which fixes what a caller must
    // supply", and a day count on a trade is a field the server will ignore.
    ...(accrual
      ? [
          {
            id: "days",
            label: "Days",
            ask: "How many days of the period does this cover?",
            why: [
              "The amount above is the basis.",
              "The rule's own rate and day-count convention turn the two into a figure.",
              "A convention is an enum in the configuration, so the denominator can never be a float.",
            ],
            answer: /^\d+$/.test(days) ? days : null,
            body: daysField,
          } satisfies Step,
        ]
      : []),
    ...(measured
      ? [
          {
            id: "units",
            label: "Units",
            ask: "How many units does this issue or retire?",
            why: [
              "A subscription names the units; a contribution does not.",
              "Whole units, refused rather than rounded. Quantity without an instrument opens no lot.",
              "The operator types a positive count. The rule decides whether they are issued or retired.",
            ],
            answer: unitsParsed?.ok ? unitsParsed.minor.toString() : null,
            body: unitsField,
          } satisfies Step,
        ]
      : []),
    {
      id: "id",
      label: "Event id",
      ask: "What is this event called?",
      why: [
        "Carried onto the journal entry, so it can be traced back to what caused it.",
        "It is what makes recording the same event twice a refusal rather than a second entry.",
        "Letters, digits, - _ . and at most sixty-four of them. Left blank, the server names it.",
      ],
      // ⚠ Optional, so it is answered even when empty — otherwise the tree
      // would block a ticket on a field the server fills in.
      answer: eventId.trim() || "the server names it",
      body: idField,
    },
    {
      id: "review",
      label: "Review",
      ask: "What this would do.",
      // ⚠ No `why` — the bar under the button already says it.
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
      title="Record an event"
      summary={
        complete && parsed?.ok ? (
          <>
            Record <code>{ruleId}</code> for{" "}
            <b className="num">{money(parsed.minor.toString())}</b>
            {accrual ? <> over {days} days</> : null}
            {measured && unitsParsed?.ok ? (
              <>
                {" "}
                · <b className="num">{unitsParsed.minor.toString()}</b> units
              </>
            ) : null}{" "}
            as <code>{eventId.trim() || "an id the server picks"}</code>.
          </>
        ) : (
          "A rule, and the figure it applies to. Which accounts move is the configuration's decision, not this form's."
        )
      }
      steps={steps}
      compact={
        <>
          <div className="form">
            {rulePicker}
            {amountField}
            {accrual ? daysField : null}
            {measured ? unitsField : null}
            {idField}
          </div>
          {ruleForm}
          {echo}
        </>
      }
      actions={
        <form action={action}>
          <input type="hidden" name="fund" value={fund} />
          <input type="hidden" name="ruleId" value={ruleId} />
          <input type="hidden" name="amount" value={amount} />
          <input type="hidden" name="days" value={accrual ? days : ""} />
          <input type="hidden" name="quantity" value={measured ? units : ""} />
          <input type="hidden" name="eventId" value={eventId} />
          <Commit
            preview="Preview"
            commit="Post"
            previewed={previewed}
            pending={pending}
            ready={complete && daysOk && rules.length > 0}
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
          {/* ⛔ A PAIR, NOT A DELTA — the NAV is the figure that matters, and a
              delta invites reading the difference as the answer. */}
          <div className="navdelta">
            <span>Net asset value</span>
            <span className="num">{money(result.response.netAssetValue)}</span>
          </div>
          <div className="navdelta">
            <span>Was</span>
            <span className="num">
              {money(result.response.previousNetAssetValue)}
            </span>
          </div>
        </div>
      ) : null}
    </Ticket>
  );
}
