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
import { count, money } from "@/lib/format";
import { isoDate } from "@/lib/dates";
import {
  consideration,
  signature,
  suggestedReference,
  TRADE_DATE,
  wholeUnits,
} from "@/lib/trade";
import type { Position, Rule } from "@/wire/types";
import { place, type TradeResult } from "./actions";

/** The picker's value for an instrument the fund does not hold. */
const NOT_HELD = "not-held";

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
  view,
}: {
  fund: string;
  rules: Rule[];
  /** What the fund holds in `view`. Empty when no view was named — see page. */
  positions: Position[];
  /** The view the holdings were read in, or "" when none was named. */
  view: string;
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

  // ⚠ THE PICKER IS AN AFFORDANCE, THE TEXT FIELD IS THE CONTRACT. With no view
  // named there are no holdings to choose from, so the field is all there is —
  // and a trade in something the fund does not hold yet has to be typeable in
  // either case.
  const offered = positions.filter((p) => p.instrument);
  const choosing = offered.length > 0 && picked !== NOT_HELD;
  const instrument = choosing ? picked : typed.trim();
  const held = offered.find((p) => p.instrument === instrument) ?? null;
  const label = held?.instrumentLabel || instrument;

  const worth = units && price ? consideration(units, price) : null;
  // ⛔ WHOLE UNITS, BECAUSE A LOT IS KEPT IN THEM. `apply_event` refuses a
  // fractional quantity rather than carrying it as none — the drop is what
  // produced a lot-less entry before — so the form refuses it here rather than
  // collecting the refusal a round trip after computing a consideration.
  const qty = units ? wholeUnits(units) : null;
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
    qty?.ok === true &&
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

  const instrumentField = (
    <>
      {offered.length ? (
        <Picker
          name="Instrument"
          value={picked}
          onValue={setPicked}
          empty="Choose an instrument"
          options={[
            ...offered.map((p) => ({
              value: p.instrument,
              label: `${p.instrumentLabel || p.instrument} · ${count(p.quantity)} held`,
            })),
            { value: NOT_HELD, label: "Something not held yet…" },
          ]}
        />
      ) : null}
      {choosing ? null : (
        <Field
          name={offered.length ? "Its identifier" : "Instrument"}
          value={typed}
          onValue={setTyped}
          hint="as the instrument master names it"
        />
      )}
    </>
  );

  // ⚠ WHAT A SALE WOULD GIVE UP, BEFORE IT GIVES IT UP — AND WHICH BOOK SAYS SO.
  // The engine relieves against open lots, so somebody selling more than the
  // book holds should learn it here rather than from a break. ⛔ Both figures
  // are view-dependent, so the view is named beside them: a quantity that
  // depends on a recognition convention and does not say which is the defect the
  // view split exists to prevent.
  const holding = held ? (
    <p className="ruleform">
      <b>{label}</b>
      {count(held.quantity)} units across {count(held.openLotCount)} open lot
      {held.openLotCount === "1" ? "" : "s"}, carried at{" "}
      <span className="num">{money(held.value)}</span> — as{" "}
      <code>{view}</code> recognises it.
    </p>
  ) : null;

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
  const carried = (
    <WillCarry side={side} label={label || instrument} date={tradeDate} />
  );

  const steps: Step[] = [
    {
      id: "side",
      label: "Side",
      ask: "Is this a purchase or a disposal?",
      why: [
        "A side is a rule, not a sign: both send a positive consideration.",
        "The configuration decides which accounts move, and which way.",
        "A purchase rule handed a negative amount posts a purchase backwards — and balances.",
      ],
      answer: side === "buy" ? "Buy" : "Sell",
      body: sideChips,
    },
    {
      id: "instrument",
      label: "Instrument",
      ask: "What was traded?",
      why: [
        "What the fund holds is offered first.",
        "A disposal of something the book does not hold has no basis to give up.",
      ],
      answer: instrument || null,
      body: (
        <>
          {instrumentField}
          {holding}
        </>
      ),
    },
    {
      id: "units",
      label: "Units",
      ask: "How many units?",
      why: [
        "Whole units, or hundredths of one.",
        "A measure, not a conserved quantity: buying 100 shares creates 100 in the book.",
      ],
      answer: qty?.ok && worth?.ok !== false ? units : null,
      body: (
        <>
          <Field name="Units" value={units} onValue={setUnits} mode="numeric" hint="1000" />
          {qty && !qty.ok ? (
            <Derived k="Units" v="—" bad from={qty.error} />
          ) : null}
          {worthBlock}
        </>
      ),
    },
    {
      id: "price",
      label: "Price",
      ask: "At what price?",
      why: [
        "Per unit, as it was dealt.",
        "The consideration follows from it and is never typed.",
      ],
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
      why: [
        "A day in the fund's calendar, not an instant.",
        "A date that shifts with the reader's timezone is a different trade date.",
      ],
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
      why: [
        "A rule of the configuration in force, approved by a person at a terminal.",
        <>
          This console does not guess which rule a side means. On the data plane
          that mapping is declared — <code>rules = {"{"} buy ={" "}
          &quot;equity_purchase&quot; {"}"}</code> — because it is a mapping
          decision.
        </>,
        "Nothing on this request carries such a declaration, so it is made here and can be read back.",
      ],
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
      ask: "What this would do.",
      // ⚠ NO `why` HERE. It said preview runs the identical code path and writes
      // nothing, which is what the bar under the button already says — and the
      // same sentence twice on one screen is one of them being ignored.
      answer: null,
      body: (
        <>
          {worthBlock}
          {carried}
        </>
      ),
    },
  ];

  return (
    <Ticket
      title="Place a trade"
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
            {instrumentField}
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
          {holding}
          {rule ? <RuleForm rule={rule} /> : null}
          {qty && !qty.ok ? <Derived k="Units" v="—" bad from={qty.error} /> : null}
          {worthBlock}
          {carried}
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

/**
 * Units × price, derived, and refused rather than rounded.
 *
 * ⚠ ONE LINE. This was a three-column block explaining that the arithmetic is
 * exact integers mirroring `ratio_ingest::posting_for` and that the figure
 * travels as a decimal string — both true, both already said in the code that
 * does it, and neither of them something an operator placing their fortieth
 * ticket needs read to them again. What is on screen is the figure and what it
 * came from. The refusal, when there is one, is the exception that earns words.
 */
function Consideration({
  units,
  price,
  worth,
}: {
  units: string;
  price: string;
  worth: ReturnType<typeof consideration> | null;
}) {
  const bad = worth !== null && !worth.ok;
  return (
    <Derived
      k="Consideration"
      v={worth?.ok ? money(worth.minor.toString()) : "—"}
      bad={bad}
      // ⛔ REFUSED, NOT ROUNDED, and in the data plane's own words. Which way to
      // round is a term of an administration agreement and the configuration
      // has not declared one.
      from={bad && !worth.ok ? worth.error : `${units || "—"} × ${price || "—"}`}
    />
  );
}

/**
 * What the entry will carry, and what still turns on the operator getting it
 * right.
 *
 * ⭐ THIS PANEL USED TO BE AN APOLOGY. `ApplyEventRequest` carried a rule, an id
 * and an amount, so a trade recorded here opened no tax lot and relieved none —
 * the entry balanced, the trial balance tied, the NAV moved by the right amount,
 * and the position's unit count was somebody else's. The screen's job was to
 * admit that. The contract carries the instrument, the units and the day now, so
 * the panel says what happens instead.
 *
 * ⚠ IT DOES NOT BECOME A TICK. Two things still decide whether the lot is right,
 * and both are the operator's: the instrument has to be the one the master
 * knows, and the day has to be the day it was dealt. Neither is checkable from
 * here, and the second is the one that decides a holding period.
 */
function WillCarry({
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
    <div className="qual">
      <b>Carried:</b> instrument, units, trade date.{" "}
      {side === "sell"
        ? `Lots of ${it} are relieved under the fund's elected method, and the gain is proceeds less the basis given up.`
        : `A tax lot opens against ${it}, dated ${date || "the day you name"}.`}
      <details className="more">
        <summary>What still turns on you</summary>
        <ul className="pts">
          <li>
            The identifier has to be the one the entity master knows. Two
            spellings of one security are two positions, and the books tie
            either way.
          </li>
          <li>
            The day is the day it was <b>dealt</b>. It is what the
            holding-period methods read, so a month out is a short-term gain
            reported as long-term — and nothing reconciles a gain.
          </li>
          <li>
            Whole units only. A fractional quantity is refused rather than
            dropped, because dropping it is what produced a lot-less entry
            before.
          </li>
        </ul>
      </details>
    </div>
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
