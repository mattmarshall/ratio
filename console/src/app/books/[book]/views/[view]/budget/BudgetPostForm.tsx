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
import {
  BUDGET_PHASES,
  BUDGET_POST_KINDS,
  budgetRuleId,
  budgetRulesInForce,
  kindsInForce,
  phasesInForce,
  type BudgetPhase,
  type BudgetPostKind,
} from "@/lib/budgetPost";
import { money } from "@/lib/format";
import { hundredths, TRADE_DATE } from "@/lib/trade";
import type { Rule } from "@/wire/types";
import { submit, type Result } from "./actions";

/**
 * Post an approved change order or award from Project `/budget`.
 *
 * ⭐ PREVIEW THEN COMMIT, SAME DOOR AS `/record`. `validateOnly` runs the
 * identical `ApplyEvent` path and records nothing. Kind × phase selects a
 * rule CreateBook already seeded — not a new journal kind, not a second
 * budget store.
 *
 * ⛔ UNSET UNTIL POSTED. This form does not plug a zero. The cites above
 * stay `—` while `postingCount === "0"`.
 */
export function BudgetPostForm({ fund, rules }: { fund: string; rules: Rule[] }) {
  const [result, action, pending] = useActionState<Result, FormData>(
    submit,
    null,
  );

  const offered = budgetRulesInForce(rules);
  const kinds = kindsInForce(rules);

  const [kind, setKind] = useState<BudgetPostKind | "">("");
  const [phase, setPhase] = useState<BudgetPhase | "">("");
  const [amount, setAmount] = useState("");
  const [dated, setDated] = useState("");
  const [eventId, setEventId] = useState("");

  const phases = phasesInForce(rules, kind);
  const ruleId = kind && phase ? budgetRuleId(kind, phase) : "";
  const rule = offered.find((r) => r.ruleId === ruleId) ?? null;
  const parsed = amount ? hundredths(amount, "an amount") : null;
  const dateOk = !dated || TRADE_DATE.test(dated);

  const complete =
    Boolean(kind) &&
    Boolean(rule) &&
    parsed?.ok === true &&
    Boolean(eventId.trim()) &&
    dateOk;

  const now = [ruleId, amount.trim(), dated.trim(), eventId.trim()].join(" ");
  const previewed =
    result?.ok === true && result.response.validateOnly && result.signature === now;

  const kindPicker = (
    <Picker
      name="Kind"
      value={kind}
      onValue={(v) => {
        setKind(v as BudgetPostKind);
        setPhase("");
      }}
      empty={kinds.length === 0 ? "No change-order or award rule in force" : "Choose a kind"}
      options={BUDGET_POST_KINDS.filter((k) => kinds.includes(k.id)).map((k) => ({
        value: k.id,
        label: k.label,
      }))}
    />
  );
  const phasePicker = (
    <Picker
      name="Phase"
      value={phase}
      onValue={(v) => setPhase(v as BudgetPhase)}
      empty={!kind ? "Choose a kind first" : "Choose a work package"}
      options={BUDGET_PHASES.filter((p) => phases.includes(p.id)).map((p) => ({
        value: p.id,
        label: p.label,
      }))}
    />
  );
  const amountField = (
    <Field
      name="Amount"
      value={amount}
      onValue={setAmount}
      mode="decimal"
      hint="5000.00"
    />
  );
  const dateField = (
    <Field
      name="Date"
      value={dated}
      onValue={setDated}
      hint="2026-03-15 — blank is undated, so the window chip cannot name it"
    />
  );
  const idField = (
    <Field
      name="Reference"
      value={eventId}
      onValue={setEventId}
      hint={kind === "award" || kind === "release" ? "PO-1" : "CO-1"}
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
  ) : kind && phase ? (
    <p className="ruleform">{ruleId} is not in force on this book.</p>
  ) : null;

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
      id: "kind",
      label: "Kind",
      ask: "Approve a change, or award a purchase order?",
      why: [
        "The same rules `/record` already posts — approve_co / deduct_co / award_commitment / release_commitment.",
        "This page does not invent a budget kind, and it does not open a lot.",
      ],
      answer: kind ? BUDGET_POST_KINDS.find((k) => k.id === kind)?.label ?? kind : null,
      body: kindPicker,
    },
    {
      id: "phase",
      label: "Phase",
      ask: "Which work package does this key to?",
      why: [
        "Site / structure / finishes, or the unpartitioned pair.",
        "The same grain cost-by-package and `/billing` already use. A lump CO bucket would hide the phase.",
      ],
      answer: phase ? BUDGET_PHASES.find((p) => p.id === phase)?.label ?? null : null,
      body: (
        <>
          {phasePicker}
          {ruleForm}
        </>
      ),
    },
    {
      id: "amount",
      label: "Amount",
      ask: "For how much?",
      why: [
        <>
          Major units, with a point — <code>5000.00</code>, not{" "}
          <code>500000</code>.
        </>,
        "Parsed on the server by splitting on the point, never by parsing a float.",
      ],
      answer: parsed?.ok ? money(parsed.minor.toString()) : null,
      body: (
        <>
          {amountField}
          {echo}
        </>
      ),
    },
    {
      id: "date",
      label: "Date",
      ask: "When was it approved or awarded?",
      why: [
        "Optional. The change-order window chip is which COs were approved in-period.",
        "Blank is undated — the as-of cite still moves; the window cannot name it. Unset, not a fake zero in March.",
      ],
      answer: dated.trim() ? dated.trim() : "undated",
      body: dateField,
    },
    {
      id: "id",
      label: "Reference",
      ask: "What is this change order or purchase order called?",
      why: [
        "Carried onto the journal entry, so it can be traced back to the award.",
        "The same grain the ingest templates call ChangeRef / PurchaseRef.",
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
      title="Post a change order or award"
      summary={
        complete && parsed?.ok ? (
          <>
            Post <code>{ruleId}</code> for{" "}
            <b className="num">{money(parsed.minor.toString())}</b> as{" "}
            <code>{eventId.trim()}</code>
            {dated.trim() ? <> dated {dated.trim()}</> : <>, undated</>}.
          </>
        ) : (
          "A kind, a work package, and the figure. Facts stay unset on this page until the journal has the entry — not a silent zero award."
        )
      }
      steps={steps}
      compact={
        <>
          <div className="form">
            {kindPicker}
            {phasePicker}
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
