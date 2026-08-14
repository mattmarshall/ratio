"use client";

import { useActionState, useState } from "react";
import { Commit, Field, Picker, Ticket, type Step } from "@/components/Ticket";
import { count, money } from "@/lib/format";
import { isoDate } from "@/lib/dates";
import {
  consideration,
  signature,
  suggestedReference,
  TRADE_DATE,
} from "@/lib/trade";
import type { Position, Rule } from "@/wire/types";
import { place, type TradeResult } from "./actions";

/** The picker's value for an instrument the fund does not hold yet. */
const NEW_HOLDING = "not-held";

/**
 * A trade ticket: what happened, how it books, what it would do, then post.
 *
 * ⭐ THE WORKFLOW IS THE PRODUCT HERE. `/record` already posts an event, and it
 * asks for a rule and an amount — which is the shape of the CONTRACT, not the
 * shape of the work. Nobody sells four hundred thousand pounds of something;
 * they sell 1,200 shares at 341.75 on the 26th, and the consideration is a
 * consequence. A form that asks for the consequence makes the operator do that
 * multiplication somewhere else and retype the answer into the one field on
 * this console that writes to a journal.
 *
 * ⛔ AND THE LAST STEP IS NOT A DISCLAIMER. `ApplyEventRequest` carries a rule,
 * an id and an amount; `Console::apply_event` builds its `ratio_rules::Event`
 * with `instrument: None, quantity: None` and its `JournalEntry` with
 * `trade_date: None`. So a trade recorded here moves value and moves nothing
 * else, and `ratio_project`'s walk skips any posting without both an instrument
 * and a quantity — no lot opens, no lot is relieved. The entry balances, the
 * trial balance ties, and the position's unit count is somebody else's. That is
 * the failure shape AGENTS.md puts first, and a screen that produced it
 * silently would be the strongest available argument against this product.
 */
export function TradeTicket({
  fund,
  rules,
  positions,
}: {
  fund: string;
  rules: Rule[];
  positions: Position[];
}) {
  const [result, action, pending] = useActionState<TradeResult, FormData>(
    place,
    null,
  );

  const [side, setSide] = useState<"buy" | "sell">("buy");
  const [picked, setPicked] = useState("");
  const [typed, setTyped] = useState("");
  const [units, setUnits] = useState("");
  const [price, setPrice] = useState("");
  const [tradeDate, setTradeDate] = useState("");
  const [ruleId, setRuleId] = useState("");
  const [reference, setReference] = useState("");

  const instrument = picked === NEW_HOLDING ? typed.trim() : picked;
  const held = positions.find((p) => p.instrument === instrument) ?? null;
  const label = held?.instrumentLabel || instrument;

  const worth = units && price ? consideration(units, price) : null;
  const rule = rules.find((r) => r.ruleId === ruleId) ?? null;

  // The reference the operator has not overridden. Typing one wins; leaving it
  // alone follows the ticket, which is what makes the field skippable.
  const suggested =
    instrument && TRADE_DATE.test(tradeDate)
      ? suggestedReference(side, instrument, tradeDate)
      : "";
  const ref = reference.trim() || suggested;

  const day = TRADE_DATE.test(tradeDate)
    ? {
        year: Number(tradeDate.slice(0, 4)),
        month: Number(tradeDate.slice(5, 7)),
        day: Number(tradeDate.slice(8, 10)),
      }
    : null;

  const complete =
    Boolean(instrument) &&
    Boolean(ruleId) &&
    Boolean(ref) &&
    TRADE_DATE.test(tradeDate) &&
    worth?.ok === true;

  const now = signature({
    side,
    instrument,
    units,
    price,
    tradeDate,
    ruleId,
    reference: ref,
  });
  // ⛔ POST IS OPEN ONLY FOR THE TICKET THAT WAS PREVIEWED. Change the price
  // after previewing and the preview on screen stops describing the button.
  const previewed =
    result?.ok === true &&
    result.response.validateOnly &&
    result.ticket.signature === now;

  const sideChips = (
    <div className="qbar" role="group" aria-label="Side">
      <button
        type="button"
        className="chip"
        aria-pressed={side === "buy"}
        onClick={() => setSide("buy")}
      >
        Buy
      </button>
      <button
        type="button"
        className="chip"
        aria-pressed={side === "sell"}
        onClick={() => setSide("sell")}
      >
        Sell
      </button>
      <span className="spacer" />
      <span className="sortnote">
        {side === "buy"
          ? "value comes in, cash goes out"
          : "the holding goes out, and a gain is realized against its basis"}
      </span>
    </div>
  );

  const instrumentPicker = (
    <>
      <Picker
        name="Instrument"
        value={picked}
        onValue={setPicked}
        empty="Choose an instrument"
        options={[
          ...positions
            .filter((p) => p.instrument)
            .map((p) => ({
              value: p.instrument,
              label: `${p.instrumentLabel || p.instrument} · ${count(p.quantity)} held`,
            })),
          { value: NEW_HOLDING, label: "Something not held yet…" },
        ]}
      />
      {picked === NEW_HOLDING ? (
        <Field
          name="Its identifier"
          value={typed}
          onValue={setTyped}
          hint="as the instrument master names it"
        />
      ) : null}
    </>
  );

  const booksPicker = (
    <Picker
      name="Books under"
      value={ruleId}
      onValue={setRuleId}
      empty={rules.length === 0 ? "No trade rule in force" : "Choose a rule"}
      options={rules.map((r) => ({
        value: r.ruleId,
        label: r.description || r.ruleId,
      }))}
    />
  );

  const worthBlock = <Consideration units={units} price={price} worth={worth} />;
  const notCarried = (
    <NotCarried side={side} label={label || instrument} date={tradeDate} />
  );

  const steps: Step[] = [
    {
      id: "side",
      label: "Side",
      ask: "Is this a purchase or a disposal?",
      why: "⛔ A side is a rule, not a sign. Both send a positive consideration and the configuration decides which accounts move and in which direction — a purchase rule handed a negative amount posts a purchase backwards, and the entry balances.",
      answer: side === "buy" ? "Buy" : "Sell",
      body: sideChips,
    },
    {
      id: "instrument",
      label: "Instrument",
      ask: "What was traded?",
      why: "What the fund already holds is offered first, because a disposal of something the book does not hold has no basis to give up.",
      answer: instrument ? label : null,
      body: (
        <>
          {instrumentPicker}
          {/* ⚠ WHAT A SALE WOULD BE GIVING UP, BEFORE IT GIVES IT UP. The engine
              relieves against open lots, and somebody selling more than the book
              holds should learn it here rather than from a break. */}
          {held ? (
            <p className="ruleform">
              <b>{label}</b>
              {count(held.quantity)} units held across{" "}
              {count(held.openLotCount)} open lot
              {held.openLotCount === "1" ? "" : "s"}, carried at{" "}
              <span className="num">{money(held.value)}</span>.
            </p>
          ) : null}
        </>
      ),
    },
    {
      id: "units",
      label: "Units",
      ask: "How many units?",
      why: "Whole units, or hundredths of one. A measure rather than a conserved quantity — buying 100 shares creates 100 in the book.",
      answer: units && worth?.ok !== false ? units : null,
      body: (
        <>
          <Field name="Units" value={units} onValue={setUnits} mode="decimal" hint="1000" />
          {worthBlock}
        </>
      ),
    },
    {
      id: "price",
      label: "Price",
      ask: "At what price?",
      why: "Per unit, as it was dealt. The consideration follows from this and is never typed.",
      answer: price && worth?.ok !== false ? price : null,
      body: (
        <>
          <Field name="Price" value={price} onValue={setPrice} mode="decimal" hint="341.75" />
          {worthBlock}
        </>
      ),
    },
    {
      id: "date",
      label: "Trade date",
      ask: "On what day?",
      why: "A day in the fund's own calendar, not an instant — a trade date that moves by one depending on where the reader is sitting is a different trade date.",
      answer: day ? isoDate(day) : null,
      body: (
        <Field
          name="Trade date"
          value={tradeDate}
          onValue={setTradeDate}
          mode="numeric"
          hint="2026-02-26"
        />
      ),
    },
    {
      id: "rule",
      label: "Books under",
      ask: "How does it book?",
      why: (
        <>
          A rule of the configuration in force, and this console does not guess
          which one a side means. On the data plane that mapping is declared in
          the template — <code>rules = {"{"} buy = &quot;equity_purchase&quot;{" "}
          {"}"}</code> — because which rule a counterparty&rsquo;s{" "}
          <code>B</code> means is a mapping decision. Nothing on this request
          carries such a declaration, so it is made here and can be read back.
        </>
      ),
      answer: ruleId || null,
      body: (
        <>
          {booksPicker}
          {rule ? <RuleForm rule={rule} /> : null}
          <Field
            name="Reference"
            value={reference}
            onValue={setReference}
            hint={suggested || "the counterparty's trade reference"}
          />
        </>
      ),
    },
    {
      id: "review",
      label: "Review",
      ask: "What this would do, and what it will not do.",
      why: "Preview runs the identical code path on the server and writes nothing, so what it shows is what lands.",
      answer: null,
      body: (
        <>
          {worthBlock}
          {notCarried}
        </>
      ),
    },
  ];

  return (
    <Ticket
      title="Place a trade"
      note="preview, then post"
      summary={
        complete && worth?.ok ? (
          <>
            {side === "buy" ? "Buy" : "Sell"} <b>{units}</b> {label} at {price}{" "}
            on {isoDate(day)} — consideration{" "}
            <b className="num">{money(worth.minor.toString())}</b>, booked under{" "}
            <code>{ruleId}</code> as <code>{ref}</code>.
          </>
        ) : (
          "A trade is an instrument, a number of units, a price and a day. The consideration follows from them."
        )
      }
      steps={steps}
      compact={
        <>
          {sideChips}
          <div className="form">
            {instrumentPicker}
            <Field name="Units" value={units} onValue={setUnits} mode="decimal" hint="1000" />
            <Field name="Price" value={price} onValue={setPrice} mode="decimal" hint="341.75" />
            <Field
              name="Trade date"
              value={tradeDate}
              onValue={setTradeDate}
              mode="numeric"
              hint="2026-02-26"
            />
            {booksPicker}
            <Field
              name="Reference"
              value={reference}
              onValue={setReference}
              hint={suggested || "the counterparty's reference"}
            />
          </div>
          {rule ? <RuleForm rule={rule} /> : null}
          {worthBlock}
          {notCarried}
        </>
      }
      actions={
        // ⛔ NOT ONE CONTROL OF THIS TICKET IS INSIDE THE `<form>`, AND THAT IS A
        // BUG FIX RATHER THAN A STYLE. React resets a form after a `<form
        // action>` submission — `form.reset()` over everything associated with
        // it. A controlled `<input>` is put back by the next render because its
        // `value` prop is reapplied; a controlled `<select>` is NOT, because the
        // prop has not changed, so React writes nothing and the element keeps
        // the reset. The instrument and the rule both fell back to "Choose…" the
        // moment a preview returned, while the state behind them, the ticket
        // sentence and the rule's own form all still said otherwise. State is
        // the one source of truth, and the form carries exactly state.
        <form action={action}>
          <input type="hidden" name="fund" value={fund} />
          <input type="hidden" name="side" value={side} />
          <input type="hidden" name="instrument" value={instrument} />
          <input type="hidden" name="units" value={units} />
          <input type="hidden" name="price" value={price} />
          <input type="hidden" name="tradeDate" value={tradeDate} />
          <input type="hidden" name="ruleId" value={ruleId} />
          <input type="hidden" name="reference" value={ref} />
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
      {rules.length === 0 ? (
        <div className="empty">
          The configuration in force declares no trade rule, so there is nothing
          here that can book one. A rule is authored and approved at a terminal,
          never from this screen.
        </div>
      ) : null}

      {result && !result.ok ? (
        <div className="empty err">{result.error}</div>
      ) : null}

      {result?.ok ? <Outcome result={result} /> : null}
    </Ticket>
  );
}

/** `Rule.form` exists so a caller can read what it is about to invoke rather
 *  than trust its name, and `Rule.accounts` is in leg order. */
function RuleForm({ rule }: { rule: Rule }) {
  return (
    <p className="ruleform">
      <b>{rule.ruleId}</b>
      {rule.form}
      {rule.accounts.length ? (
        <> Legs, in order: {rule.accounts.join(" · ")}.</>
      ) : null}
    </p>
  );
}

/** Units × price, derived, and refused rather than rounded. */
function Consideration({
  units,
  price,
  worth,
}: {
  units: string;
  price: string;
  worth: ReturnType<typeof consideration> | null;
}) {
  return (
    <div className="lotterms">
      <div className="lt">
        <span className="ltk">Units × price</span>
        <span className="ltv num">
          {units || "—"} × {price || "—"}
        </span>
        <span className="ltv">
          exact integer arithmetic, mirroring{" "}
          <code>ratio_ingest::posting_for</code>
        </span>
      </div>
      <div className={`lt${worth && !worth.ok ? " warn" : ""}`}>
        <span className="ltk">Consideration</span>
        <span className="ltv num strong">
          {worth?.ok ? money(worth.minor.toString()) : "—"}
        </span>
        {/* ⛔ REFUSED, NOT ROUNDED. Which way to round is a term of an
            administration agreement and the configuration has not declared one,
            so the data plane refuses this — and the ticket refuses it in the
            same words rather than booking a figure a file could not. */}
        <span className="ltv">
          {worth && !worth.ok
            ? worth.error
            : "sent as a decimal string in major units, and parsed on the server"}
        </span>
      </div>
    </div>
  );
}

/**
 * The three fields this write cannot carry, and what each one costs.
 *
 * ⛔ BEFORE THE PREVIEW, NOT INSIDE IT. This is a property of the METHOD, not of
 * the response: `ApplyEventRequest` has no field for an instrument, a quantity
 * or a trade date, so the answer is the same every time and is known before the
 * server is asked. Putting it behind a preview would mean an operator who posts
 * without previewing never learns any of it, and "preview first" is a discipline
 * the console asks for rather than one the contract enforces.
 *
 * ⚠ IT WEARS THE ATTENTION COLOUR, NOT THE FAILURE ONE. Nothing has gone wrong.
 * The entry balances, the trial balance ties, the NAV moves by the right amount,
 * and this is exactly the failure shape AGENTS.md puts first — the one that
 * surfaces years later in somebody's taxable income.
 */
function NotCarried({
  side,
  label,
  date,
}: {
  side: "buy" | "sell";
  label: string;
  date: string;
}) {
  const it = label || "the holding";
  return (
    <>
      <div className="qual">
        <b>Instrument.</b> ApplyEvent takes a rule and an amount, so the postings
        name accounts and nothing else. The value sits in the account&rsquo;s
        unattributed row rather than against {it}.<br />
        <b>Units.</b> No quantity moves, so {it}&rsquo;s unit count is unchanged
        and the cost per unit the book implies is now somebody else&rsquo;s.
        <br />
        <b>Trade date.</b> The entry carries none. {date || "The date"} reaches
        the journal inside the reference, where a person can read it and{" "}
        <code>Ratio.Lots.Relief</code> cannot.
      </div>
      <p className="cap">
        {side === "sell" ? (
          <>
            A disposal recorded this way relieves <b>no lot</b>. No basis is
            given up, so the realized gain is whatever this rule&rsquo;s own
            weights make of the consideration rather than proceeds less cost —
            and with no attributable sale in the entry it lands in{" "}
            <b>Unclassified</b>. The realized gain is the figure with no
            counterparty: a wrong NAV meets a reconciliation, and this meets
            nobody until a tax authority asks.
          </>
        ) : (
          <>
            A purchase recorded this way opens <b>no tax lot</b>. There is
            nothing for a later sale to relieve, and a lot opened by an entry
            with no trade date has <b>no trade date</b> — which the
            holding-period methods refuse rather than guess at, because both
            defaults are wrong in opposite directions.
          </>
        )}{" "}
        To carry all three, the trade has to arrive on the data plane, where a
        template declares the instrument, the quantity and the day as fields of a
        fact — or `ApplyEventRequest` has to grow the three fields.
      </p>
    </>
  );
}

/** What the server said this ticket would do, or what it did. */
function Outcome({ result }: { result: Extract<TradeResult, { ok: true }> }) {
  const { response, ticket } = result;

  return (
    <div className="prev">
      <div className="ph">
        <span>{response.validateOnly ? "What this would do" : "What it did"}</span>
        <span>{ticket.reference}</span>
      </div>

      {response.entry ? (
        <div className="postings">
          {response.entry.postings.map((p, i) => (
            <div className="posting" key={`${p.account}-${i}`}>
              <span>
                <div className="p1">{p.displayName || p.account}</div>
                {/* Signed, and a debit is positive. Named because a column of
                    signed figures with no convention printed beside it is a
                    column the reader has to guess the convention of. */}
                <div className="p2">
                  {p.amount.startsWith("-") ? "credit" : "debit"}
                </div>
              </span>
              <span className="num">{money(p.amount)}</span>
            </div>
          ))}
        </div>
      ) : null}

      {/* ⛔ A PAIR, NOT A DELTA. The contract reports the NAV with this entry and
          without it rather than the difference, because the NAV is the figure
          that matters and a delta invites reading the difference as the answer. */}
      <div className="navdelta">
        <span>Net asset value</span>
        <span className="num">{money(response.netAssetValue)}</span>
      </div>
      <div className="navdelta">
        <span>Was</span>
        <span className="num">{money(response.previousNetAssetValue)}</span>
      </div>
    </div>
  );
}
