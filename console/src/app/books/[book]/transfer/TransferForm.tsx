"use client";

import { useActionState, useMemo, useState } from "react";
import {
  Commit,
  Derived,
  Field,
  Picker,
  Ticket,
  type Step,
} from "@/components/Ticket";
import { money } from "@/lib/format";
import { hundredths, TRADE_DATE } from "@/lib/trade";
import type { Rule } from "@/wire/types";
import { submit, type Result } from "./actions";

/**
 * Move value between two personal accounts.
 *
 * ⛔ PREVIEW THEN COMMIT. Same door as `/record` and `/trade`.
 *
 * ⚠ THE RULE IS LOOKED UP FROM THE TWO ACCOUNTS, not chosen by id. Each
 * `xfer_*` rule is a directed pair in the household template; a pair this
 * configuration does not name is a refusal rather than a synthesized
 * posting that would bypass the control plane.
 */
export function TransferForm({ fund, rules }: { fund: string; rules: Rule[] }) {
  const [result, action, pending] = useActionState<Result, FormData>(
    submit,
    null,
  );

  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [amount, setAmount] = useState("");
  const [date, setDate] = useState("");
  const [eventId, setEventId] = useState("");

  const accounts = useMemo(() => {
    const names = new Set<string>();
    for (const r of rules) {
      for (const a of r.accounts) names.add(a);
    }
    return [...names];
  }, [rules]);

  const rule =
    from && to && from !== to
      ? (rules.find((r) => r.accounts[0] === to && r.accounts[1] === from) ??
        null)
      : null;

  const parsed = amount ? hundredths(amount, "an amount") : null;
  const dateOk = TRADE_DATE.test(date);
  const complete =
    Boolean(rule) && parsed?.ok === true && dateOk && from !== to;

  const named = eventId.trim() || (dateOk ? `xfer-${date}` : "");
  const now = [rule?.ruleId ?? "", amount.trim(), date.trim(), named].join(" ");
  const previewed =
    result?.ok === true && result.response.validateOnly && result.signature === now;

  const fromPicker = (
    <Picker
      name="From"
      value={from}
      onValue={setFrom}
      empty={accounts.length === 0 ? "No transfer rule in force" : "Choose an account"}
      options={accounts.map((a) => ({ value: a, label: a }))}
    />
  );
  const toPicker = (
    <Picker
      name="To"
      value={to}
      onValue={setTo}
      empty={accounts.length === 0 ? "No transfer rule in force" : "Choose an account"}
      options={accounts
        .filter((a) => a !== from)
        .map((a) => ({ value: a, label: a }))}
    />
  );
  const amountField = (
    <Field
      name="Amount"
      value={amount}
      onValue={setAmount}
      mode="decimal"
      hint="250.00"
    />
  );
  const dateField = (
    <Field
      name="Date"
      value={date}
      onValue={setDate}
      hint="2026-03-15"
    />
  );
  const idField = (
    <Field
      name="Event id"
      value={eventId}
      onValue={setEventId}
      hint="left blank, named from the date"
    />
  );

  const echo = parsed ? (
    <>
      <Derived
        k="Amount"
        v={parsed.ok ? money(parsed.minor.toString()) : "—"}
        bad={!parsed.ok}
        from={parsed.ok ? "as the server will parse it" : parsed.error}
      />
      <Derived
        k="Date"
        v={dateOk ? date : "—"}
        bad={Boolean(date) && !dateOk}
        from={
          dateOk
            ? "the period this entry belongs to"
            : date
              ? "YYYY-MM-DD"
              : "required — an undated entry is in no period"
        }
      />
      {from && to && from !== to && !rule ? (
        <Derived
          k="Route"
          v="—"
          bad
          from="this configuration has no transfer between those accounts"
        />
      ) : null}
    </>
  ) : null;

  const steps: Step[] = [
    {
      id: "from",
      label: "From",
      ask: "Where does the money leave?",
      why: [
        "A transfer is two legs of one conserved entry: credit here, debit there.",
        "Cash, investments and a card are the household chart's asset and liability accounts.",
      ],
      answer: from || null,
      body: fromPicker,
    },
    {
      id: "to",
      label: "To",
      ask: "Where does it arrive?",
      why: [
        "Paying a card credits cash and debits the liability — it is not a sale.",
        "Moving cash to investments does not open a tax lot. That would be a trade.",
      ],
      answer: to || null,
      body: (
        <>
          {toPicker}
          {from && to && from !== to && !rule ? (
            <p className="ruleform">
              No approved transfer between {from} and {to}.
            </p>
          ) : null}
        </>
      ),
    },
    {
      id: "amount",
      label: "Amount",
      ask: "For how much?",
      why: [
        <>
          Major units, with a point — <code>250.00</code>.
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
      ask: "Which day did this happen?",
      why: [
        "A period P&L is a window over dated entries.",
        "An entry with no date is in no month and no year — the same refusal as a lot with no acquisition date.",
      ],
      answer: dateOk ? date : null,
      body: dateField,
    },
    {
      id: "id",
      label: "Event id",
      ask: "What is this transfer called?",
      why: [
        "Recording the same id twice is a refusal rather than a second entry.",
        "Left blank, the date names it. A second transfer that day needs an id of its own.",
      ],
      answer: eventId.trim() || "named from the date",
      body: idField,
    },
    {
      id: "review",
      label: "Review",
      ask: "What this would do.",
      answer: null,
      body: (
        <>
          {rule ? (
            <p className="ruleform">
              <b>{rule.ruleId}</b>
              {rule.description || rule.form}
            </p>
          ) : null}
          {echo}
        </>
      ),
    },
  ];

  return (
    <Ticket
      title="Transfer"
      summary={
        complete && parsed?.ok && rule ? (
          <>
            Move <b className="num">{money(parsed.minor.toString())}</b> from{" "}
            {from} to {to} on <code>{date}</code>.
          </>
        ) : (
          "Two personal accounts, an amount, and a day. No instrument — this is not a trade."
        )
      }
      steps={steps}
      compact={
        <>
          <div className="form">
            {fromPicker}
            {toPicker}
            {amountField}
            {dateField}
            {idField}
          </div>
          {rule ? (
            <p className="ruleform">
              <b>{rule.ruleId}</b>
              {rule.description || rule.form}
            </p>
          ) : null}
          {echo}
        </>
      }
      actions={
        <form action={action}>
          <input type="hidden" name="fund" value={fund} />
          <input type="hidden" name="ruleId" value={rule?.ruleId ?? ""} />
          <input type="hidden" name="amount" value={amount} />
          <input type="hidden" name="date" value={date} />
          <input type="hidden" name="eventId" value={eventId} />
          <Commit
            preview="Preview"
            commit="Post"
            previewed={previewed}
            pending={pending}
            ready={complete && rules.length > 0}
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
          <div className="navdelta">
            <span>Net worth</span>
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
