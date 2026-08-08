// The operations console.
//
// The design is the one published at claude.ai as a concept; this renders it
// from `ratio.v1.Console` instead of from a literal. Where the concept invented
// five funds, this shows the funds that exist — which is one on the demo, and
// however many books are on disk everywhere else.

import { useEffect, useState } from "react";
import {
  useAccounts,
  useAdmit,
  useApplyEvent,
  useIngest,
  useDeliveries,
  useBreaks,
  useChangeLog,
  useConfigDiff,
  useConfigVersions,
  useFunds,
  useNavStrikes,
  usePendingFacts,
  usePostings,
  useReplay,
  useRules,
  useTemplates,
} from "./api.js";
import {
  SEVERITY_CLASS,
  STATE_CLASS,
  STATE_LABEL,
  count,
  money,
} from "./format.js";
import type {
  Account,
  ApplyEventResponse,
  Break,
  ConfigVersion,
  Delivery,
  Fund,
  IngestDeliveryResponse,
  NavStrike,
  PendingFact,
} from "./types.js";

const ACCOUNT_FILTERS = [
  { key: "", label: "Whole chart" },
  { key: "posted", label: "Posted to" },
  { key: "abnormal", label: "Abnormal side" },
] as const;

const FILTERS = [
  { key: "", label: "All" },
  { key: "blocking", label: "Blocking" },
  { key: "unexplained", label: "Unexplained" },
] as const;

function Stat({
  k,
  v,
  sub,
  tone,
}: {
  k: string;
  v: string;
  sub?: string;
  tone?: "tied" | "at-risk";
}) {
  return (
    <div className={`stat${tone ? ` ${tone}` : ""}`}>
      <div className="k">{k}</div>
      <div className="v num">
        {v}
        {sub ? <small>{sub}</small> : null}
      </div>
    </div>
  );
}

/** One strike, with the proof available on demand rather than asserted. */
function Strike({ s }: { s: NavStrike }) {
  const [asked, setAsked] = useState(false);
  const replay = useReplay(s.name, asked);
  const proof = replay.data;

  return (
    <div className="logrow strike">
      <span className="t num">{s.valuationTime.slice(11, 16)}</span>
      <span className="w">
        <b className="num">{money(s.netAssetValue)}</b>{" "}
        <span className="cfg">
          {count(s.journalPosition)} entries · {s.configDigest.slice(0, 7)} · {s.actor}
        </span>
        {proof ? (
          <div className={`proof${proof.historyIntact && proof.reproduced ? "" : " bad"}`}>
            {proof.historyIntact
              ? "history intact — the journal prefix hashes as it did"
              : "HISTORY REWRITTEN — the prefix no longer hashes as it did"}
            <br />
            {proof.reproduced
              ? `re-derived ${money(proof.netAssetValue)} — identical`
              : `DIVERGED — re-derived ${money(proof.netAssetValue)}`}
          </div>
        ) : null}
        {replay.isError ? <div className="proof bad">{replay.error.message}</div> : null}
      </span>
      <button
        className="act"
        type="button"
        onClick={() => setAsked(true)}
        disabled={asked && replay.isFetching}
      >
        {replay.isFetching ? "re-deriving…" : asked ? "replayed" : "Replay"}
      </button>
    </div>
  );
}

function Detail({ brk }: { brk: Break | undefined }) {
  if (!brk) {
    return <aside className="detail"><div className="empty">Select a break.</div></aside>;
  }
  return (
    <aside className="detail" aria-label="Break detail">
      <div className="dhead">
        <h2>{brk.account}</h2>
        <div className="sub">{brk.cause}</div>
      </div>

      <div className="dsec">
        <h3>The two figures</h3>
        <dl className="kv">
          <dt>Ratio</dt><dd className="num">{money(brk.ratioAmount)}</dd>
          <dt>Reported</dt><dd className="num">{money(brk.reportedAmount)}</dd>
          <dt>Difference</dt><dd className="num">{money(brk.difference)}</dd>
        </dl>
      </div>

      <div className="dsec postings">
        <h3>What produced ours</h3>
        {brk.postings.length === 0 ? (
          <div className="sub">No postings on this account.</div>
        ) : (
          brk.postings.map((p) => (
            <div className="posting" key={p.entryId + p.amount}>
              <span>
                <div className="p1">{p.memo || "—"}</div>
                <div className="p2 num">{p.entryId}</div>
              </span>
              <span className="num">{money(p.amount)}</span>
            </div>
          ))
        )}
      </div>

      {/* Provenance is the product: a figure that cannot name what produced it
          is worth no more than the one it disagrees with. */}
      <div className="dsec">
        <h3>Provenance</h3>
        <div className="prov">
          configuration <span className="g">{brk.configDigest.slice(0, 12)}…</span>
          <br />
          account dimension {brk.accountDimension}
          <br />
          replays identically under this configuration
        </div>
      </div>
    </aside>
  );
}

const CHANGE_LABEL = {
  ADDED: "added",
  CHANGED: "changed",
  REMOVED: "removed",
  UNSPECIFIED: "",
} as const;

/** One configuration version, with what it changed available on demand. */
function Version({ v }: { v: ConfigVersion }) {
  const [asked, setAsked] = useState(false);
  // "" means the previous version — the server picks it, so the client never
  // has to know the ordering. On the first version there is no previous, and
  // every rule reads as added, which is what happened.
  const diff = useConfigDiff(asked ? v.name : undefined, "");

  return (
    <div className="logrow strike">
      <span className="t num">{v.digest.slice(0, 7)}</span>
      <span className="w">
        <b>v{v.sequence}</b>
        {v.active ? <span className="inforce">in force</span> : null}{" "}
        <span className="cfg">
          {v.rules.length} rule{v.rules.length === 1 ? "" : "s"}
          {v.actor ? ` · ${v.actor}` : " · no recorded approver"}
          {v.subject ? ` · ${v.subject}` : ""}
        </span>
        {diff.data ? (
          <div className="proof">
            {diff.data.changes.length === 0 ? (
              // A digest changes on any byte, and formatting is a byte.
              "no rule changed — the bytes differ, the rules do not"
            ) : (
              diff.data.changes.map((c) => (
                <div className="chg" key={c.ruleId + c.kind}>
                  <b>{CHANGE_LABEL[c.kind]}</b> {c.ruleId}
                  {c.baseForm ? <div className="was">{c.baseForm}</div> : null}
                  {c.form ? <div className="now">{c.form}</div> : null}
                </div>
              ))
            )}
          </div>
        ) : null}
        {diff.isError ? <div className="proof bad">{diff.error.message}</div> : null}
      </span>
      <button
        className="act"
        type="button"
        onClick={() => setAsked(!asked)}
        disabled={asked && diff.isFetching}
      >
        {diff.isFetching ? "diffing…" : asked ? "Hide" : "What changed"}
      </button>
    </div>
  );
}

/** The trial balance. Every account, whether or not anything touched it. */
function Balance({
  fund,
  accounts,
  selected,
  onSelect,
}: {
  fund: Fund | undefined;
  accounts: Account[];
  selected: string | undefined;
  onSelect: (name: string) => void;
}) {
  return (
    <div className="tb" role="table" aria-label="Trial balance">
      <div className="tbhead" role="row">
        <span role="columnheader">Account</span>
        <span role="columnheader" className="num">Debit</span>
        <span role="columnheader" className="num">Credit</span>
        <span role="columnheader" className="num">Balance</span>
      </div>
      {accounts.map((a) => (
        <button
          key={a.name}
          className="tbrow"
          role="row"
          aria-current={a.name === selected}
          onClick={() => onSelect(a.name)}
        >
          <span role="cell">
            <span className="an">{a.displayName}</span>
            <span className="at">
              {a.type.toLowerCase()}
              {/* An abnormal balance is legal. It is flagged, not scored. */}
              {a.abnormal ? <b className="abn">abnormal side</b> : null}
            </span>
          </span>
          <span role="cell" className="num">{a.debit === "0" ? "—" : money(a.debit)}</span>
          <span role="cell" className="num">{a.credit === "0" ? "—" : money(a.credit)}</span>
          <span role="cell" className="num bal">{money(a.balance)}</span>
        </button>
      ))}
      {/* The two columns that must agree, and the difference reported as a
          figure rather than as a green tick. */}
      <div className="tbfoot" role="row">
        <span role="cell">
          {accounts.length} account{accounts.length === 1 ? "" : "s"}
        </span>
        <span role="cell" className="num">{fund ? money(fund.totalDebit) : "—"}</span>
        <span role="cell" className="num">{fund ? money(fund.totalCredit) : "—"}</span>
        <span role="cell" className="num bal">
          {fund ? money(fund.trialBalanceDifference) : "—"}
          <small>difference</small>
        </span>
      </div>
    </div>
  );
}

/** The lines behind one figure, in journal order, with the running balance. */
function Lines({ account }: { account: Account | undefined }) {
  const postings = usePostings(account?.name);

  if (!account) {
    return <aside className="detail"><div className="empty">Select an account.</div></aside>;
  }
  return (
    <aside className="detail" aria-label="Account detail">
      <div className="dhead">
        <h2>{account.displayName}</h2>
        <div className="sub">
          {account.type.toLowerCase()} · dimension {account.dimension}
        </div>
      </div>

      <div className="dsec">
        <h3>What it holds</h3>
        <dl className="kv">
          <dt>Debit</dt><dd className="num">{money(account.debit)}</dd>
          <dt>Credit</dt><dd className="num">{money(account.credit)}</dd>
          <dt>Balance</dt><dd className="num">{money(account.balance)}</dd>
        </dl>
      </div>

      <div className="dsec postings">
        <h3>Every line behind it</h3>
        {/* The dimmed figure on each row is the balance AFTER that line. The
            last one is the balance above — which is the point of the pane, and
            worth saying rather than leaving a reader to notice. */}
        <div className="cap">
          in journal order · the second figure is the balance after that line,
          and the last one is the balance above
        </div>
        {postings.isError ? (
          <div className="empty err">{postings.error.message}</div>
        ) : null}
        {postings.data?.length === 0 ? (
          <div className="sub">Nothing has been posted to this account.</div>
        ) : null}
        {postings.data?.map((p) => (
          <div className="posting" key={p.name}>
            <span>
              <div className="p1">{p.memo || "—"}</div>
              <div className="p2 num">
                {p.entryId} · {p.configDigest.slice(0, 7)}
              </div>
            </span>
            <span className="num">
              {money(p.amount)}
              {/* The balance AFTER this line: where the figure came from, not
                  just what it ended at. */}
              <small>{money(p.runningBalance)}</small>
            </span>
          </div>
        ))}
      </div>
    </aside>
  );
}

/**
 * Record an event.
 *
 * The operator chooses a RULE and an amount. They do not choose accounts —
 * which accounts move, and in which direction, was decided by whoever approved
 * the configuration. That is the whole argument of the product expressed as a
 * form: you assert that something happened, and the thing that was proved
 * decides what it means.
 *
 * Nothing is written until the postings have been seen. `validateOnly` runs
 * the same code path and writes nothing, so what is previewed is what posts.
 */
function Record({ fund }: { fund: Fund | undefined }) {
  const rules = useRules(fund?.name);
  const apply = useApplyEvent(fund?.name);
  const [ruleId, setRuleId] = useState("");
  const [eventId, setEventId] = useState("");
  const [amount, setAmount] = useState("");
  const [days, setDays] = useState("");
  const [preview, setPreview] = useState<ApplyEventResponse | null>(null);

  const rule = rules.data?.find((r) => r.ruleId === ruleId) ?? rules.data?.[0];
  const needsDays = rule?.kind === "ACCRUAL";
  const ready = !!rule && eventId.trim() !== "" && amount.trim() !== "" &&
    (!needsDays || days.trim() !== "");

  // Any edit invalidates a preview. A form that keeps showing the postings for
  // an amount you have since changed is worse than one that shows none.
  function edit<T>(set: (v: T) => void) {
    return (v: T) => { setPreview(null); apply.reset(); set(v); };
  }

  function run(validateOnly: boolean) {
    if (!rule) return;
    apply.mutate(
      { ruleId: rule.ruleId, eventId: eventId.trim(), amount, days, validateOnly },
      {
        onSuccess: (res) => {
          if (res.validateOnly) { setPreview(res); return; }
          // Posted. Clear the form rather than leave an id that would now
          // conflict with itself.
          setPreview(null);
          setEventId("");
          setAmount("");
          setDays("");
        },
      },
    );
  }

  return (
    <section className="rec" aria-label="Record an event">
      <div className="loghead">
        <span>Record an event</span>
        <span className="sortnote">
          {fund?.configDigest
            ? `under configuration ${fund.configDigest.slice(0, 7)}`
            : ""}
        </span>
      </div>

      {rules.data?.length === 0 ? (
        <div className="empty">
          No rules are in force on this fund, so there is nothing to record
          against.
        </div>
      ) : null}

      {rules.data && rules.data.length > 0 ? (
        <>
          <div className="form">
            <label>
              <span>Rule</span>
              <select
                value={rule?.ruleId ?? ""}
                onChange={(e) => edit(setRuleId)(e.target.value)}
              >
                {rules.data.map((r) => (
                  <option key={r.ruleId} value={r.ruleId}>
                    {r.ruleId} — {r.kind.toLowerCase()}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>Reference</span>
              <input
                value={eventId}
                placeholder="t6"
                onChange={(e) => edit(setEventId)(e.target.value)}
              />
            </label>
            <label>
              <span>{needsDays ? "Basis" : "Amount"}</span>
              <input
                className="num"
                value={amount}
                inputMode="decimal"
                placeholder="250000.00"
                onChange={(e) => edit(setAmount)(e.target.value)}
              />
            </label>
            {needsDays ? (
              <label>
                <span>Days</span>
                <input
                  className="num"
                  value={days}
                  inputMode="numeric"
                  placeholder="30"
                  onChange={(e) => edit(setDays)(e.target.value)}
                />
              </label>
            ) : null}
          </div>

          {rule ? (
            <div className="ruleform">
              {rule.description ? <b>{rule.description}</b> : null}
              <div>posts to {rule.accounts.join(" and ")}</div>
            </div>
          ) : null}

          {apply.isError ? (
            <div className="empty err">{apply.error.message}</div>
          ) : null}

          {preview?.entry ? (
            <div className="prev">
              <div className="ph">
                What this would post
                <span>{preview.entry.configDigest.slice(0, 7)}</span>
              </div>
              {preview.entry.postings.map((p) => (
                <div className="posting" key={p.account + p.amount}>
                  <span>
                    <div className="p1">{p.displayName}</div>
                    <div className="p2 num">
                      {p.amount.startsWith("-") ? "credit" : "debit"}
                    </div>
                  </span>
                  <span className="num">{money(p.amount)}</span>
                </div>
              ))}
              <div className="navdelta">
                <span>Net asset value</span>
                <span className="num">
                  {money(preview.previousNetAssetValue)}
                  <b aria-hidden="true"> → </b>
                  {money(preview.netAssetValue)}
                </span>
              </div>
            </div>
          ) : null}

          <div className="formbar">
            <span className="sortnote">
              {preview
                ? "Nothing has been written. Post to record it."
                : "Preview runs the same code path and writes nothing."}
            </span>
            <button
              className="act"
              type="button"
              disabled={!ready || apply.isPending}
              onClick={() => run(true)}
            >
              {apply.isPending && !preview ? "checking…" : "Preview"}
            </button>
            <button
              className="act primary"
              type="button"
              disabled={!ready || !preview || apply.isPending}
              onClick={() => run(false)}
            >
              {apply.isPending && preview ? "posting…" : "Post"}
            </button>
          </div>
        </>
      ) : null}
    </section>
  );
}

/**
 * The data plane: what arrived, and what it could not resolve.
 *
 * A pending fact is an exception in the same sense a reconciliation break is —
 * a figure missing an input is not a figure — so it blocks the NAV and is
 * counted with them. What differs is the remedy, which is why ABSENT and
 * AMBIGUOUS are shown apart: one wants a new instrument, the other wants one
 * fewer.
 */
function Data({
  fund,
  deliveries,
  pending,
}: {
  fund: Fund | undefined;
  deliveries: Delivery[];
  pending: PendingFact[];
}) {
  return (
    <>
      <Deliver fund={fund} />
      <section className="log" aria-label="Files received">
        <div className="loghead">
          <span>Files received</span>
          <span className="sortnote">
            {deliveries.length ? "newest first · hashed on arrival" : ""}
          </span>
        </div>
        {deliveries.length === 0 ? (
          <div className="empty">
            Nothing delivered yet. <code>ratio ingest</code> reads a file.
          </div>
        ) : null}
        {deliveries.map((d) => (
          <div className="logrow strike" key={d.name + d.receiveTime}>
            <span className="t num">{d.digest.slice(0, 7)}</span>
            <span className="w">
              <b>{d.origin.split("/").pop()}</b>{" "}
              <span className="cfg">
                {count(d.factCount)} fact{d.factCount === "1" ? "" : "s"} ·{" "}
                {count(d.byteCount)} bytes · {d.receiveTime.slice(0, 16).replace("T", " ")}
              </span>
            </span>
            <span className={`amt num${d.pendingFactCount === "0" ? "" : " pos"}`}>
              {d.pendingFactCount === "0" ? "all clear" : `${d.pendingFactCount} pending`}
            </span>
          </div>
        ))}
      </section>

      <section className="log" aria-label="Pending facts">
        <div className="loghead">
          <span>Pending</span>
          <span className="sortnote">
            {pending.length ? "these clear when the master does" : ""}
          </span>
        </div>
        {pending.length === 0 ? (
          <div className="empty">
            Nothing pending — every fact read resolves against the master.
          </div>
        ) : null}
        {pending.map((p) => (
          <div className="logrow pend" key={p.name}>
            <span className={`sev ${p.blocker === "AMBIGUOUS" ? "med" : "high"}`} />
            <span className="w">
              <b>{p.reference}</b>{" "}
              <span className="tagx">{p.blocker.toLowerCase()}</span>
              <div className="why">{p.detail}</div>
              {/* The chain back to the bytes. A pending fact can say exactly
                  which row of which file it came from, and what mapped it. */}
              <div className="p2 num">
                row {p.row} of {p.deliveryDigest.slice(0, 12)} · {p.templateId}
              </div>
            </span>
          </div>
        ))}
      </section>
    </>
  );
}

/**
 * Read a file, having first seen what it would produce.
 *
 * The same preview-then-commit shape as recording an event, and the same
 * reason: `validateOnly` runs the identical code path and records nothing, so
 * what is shown is what lands. A mapping you have watched run beats one you
 * have to imagine.
 */
function Deliver({ fund }: { fund: Fund | undefined }) {
  const ingest = useIngest(fund?.name);
  const admit = useAdmit(fund?.name);
  const [templateId, setTemplateId] = useState("");
  const [origin, setOrigin] = useState("");
  const [content, setContent] = useState("");
  const [preview, setPreview] = useState<IngestDeliveryResponse | null>(null);
  const templates = (useTemplates(fund?.name).data ?? []).map((t) => t.templateId);

  const chosen = templateId || templates[0] || "";
  const ready = !!chosen && content.trim() !== "";

  function edit<T>(set: (v: T) => void) {
    return (v: T) => { setPreview(null); ingest.reset(); set(v); };
  }

  function run(validateOnly: boolean) {
    ingest.mutate(
      { templateId: chosen, content, origin: origin.trim() || "upload", validateOnly },
      {
        onSuccess: (res) => {
          if (res.validateOnly) { setPreview(res); return; }
          setPreview(null);
          setContent("");
          setOrigin("");
        },
      },
    );
  }

  return (
    <section className="rec" aria-label="Deliver a file">
      <div className="loghead">
        <span>Deliver a file</span>
        <span className="sortnote">
          {templates.length
            ? `${templates.length} template${templates.length === 1 ? "" : "s"} in force`
            : "no templates in force"}
        </span>
      </div>

      {templates.length === 0 ? (
        <div className="empty">
          No template is in force, so there is nothing to read a file with. A
          model drafts one with <code>propose_template</code>; a person approves it.
        </div>
      ) : (
        <>
          <div className="form">
            <label>
              <span>Template</span>
              <select value={chosen} onChange={(e) => edit(setTemplateId)(e.target.value)}>
                {templates.map((t) => (
                  <option key={t} value={t}>{t}</option>
                ))}
              </select>
            </label>
            <label>
              <span>Called</span>
              <input
                value={origin}
                placeholder="prime-trades.csv"
                onChange={(e) => edit(setOrigin)(e.target.value)}
              />
            </label>
          </div>
          <div className="form">
            <label>
              <span>The file</span>
              <textarea
                className="num"
                rows={4}
                value={content}
                spellCheck={false}
                placeholder="Paste the file — its header row and the rows under it."
                onChange={(e) => edit(setContent)(e.target.value)}
              />
            </label>
          </div>

          {ingest.isError ? (
            <div className="empty err">{ingest.error.message}</div>
          ) : null}
          {admit.isError ? <div className="empty err">{admit.error.message}</div> : null}

          {preview ? (
            <div className="prev">
              <div className="ph">
                What this would read
                <span>{preview.deliveryDigest.slice(0, 12)}</span>
              </div>
              <div className="why">
                {count(preview.rowCount)} row{preview.rowCount === "1" ? "" : "s"} ·{" "}
                {count(preview.factCount)} fact{preview.factCount === "1" ? "" : "s"} ·{" "}
                {count(preview.readyCount)} ready · {preview.pending.length} pending
                {preview.newFactCount !== preview.factCount
                  ? ` · ${count(preview.newFactCount)} new`
                  : ""}
              </div>
              {preview.rejected.map((r) => (
                <div className="posting" key={r.row}>
                  <span><div className="p1">row {r.row}</div></span>
                  <span className="num pos">{r.reason}</span>
                </div>
              ))}
              {preview.pending.map((p) => (
                <div className="posting" key={p.name}>
                  <span>
                    <div className="p1">{p.reference}</div>
                    <div className="p2">{p.detail}</div>
                  </span>
                  <span className="num">pending</span>
                </div>
              ))}
            </div>
          ) : null}

          {admit.data && !admit.data.validateOnly ? (
            <div className="prev">
              <div className="ph">Admitted</div>
              <div className="why">
                {count(admit.data.postedCount)} posted ·{" "}
                {count(admit.data.recordedCount)} recorded ·{" "}
                {count(admit.data.pendingCount)} pending
              </div>
              <div className="navdelta">
                <span>Net asset value</span>
                <span className="num">
                  {money(admit.data.previousNetAssetValue)}
                  <b aria-hidden="true"> → </b>
                  {money(admit.data.netAssetValue)}
                </span>
              </div>
            </div>
          ) : null}

          <div className="formbar">
            <span className="sortnote">
              {preview
                ? "Nothing has been recorded. Read it to record the facts."
                : "Preview runs the same code path and records nothing."}
            </span>
            <button
              className="act"
              type="button"
              disabled={!ready || ingest.isPending}
              onClick={() => run(true)}
            >
              {ingest.isPending && !preview ? "reading…" : "Preview"}
            </button>
            <button
              className="act"
              type="button"
              disabled={!ready || !preview || ingest.isPending}
              onClick={() => run(false)}
            >
              {ingest.isPending && preview ? "reading…" : "Read"}
            </button>
            {/* Posting is a separate act from reading. A file can be read and
                left unposted — that is a shadow run. */}
            <button
              className="act primary"
              type="button"
              disabled={admit.isPending}
              onClick={() => admit.mutate({ validateOnly: false })}
            >
              {admit.isPending ? "posting…" : "Admit"}
            </button>
          </div>
        </>
      )}
    </section>
  );
}

export default function App() {
  const funds = useFunds();
  const [fundName, setFundName] = useState<string>();
  const [filter, setFilter] = useState<string>("");
  const [brkName, setBrkName] = useState<string>();
  const [view, setView] = useState<"breaks" | "balance" | "data">("breaks");
  const [acctFilter, setAcctFilter] = useState<string>("");
  const [acctName, setAcctName] = useState<string>();

  // Select the first fund once the list arrives, and prefer one that needs
  // attention — an operator opening this wants the blocked fund, not the
  // alphabetically first one.
  useEffect(() => {
    if (fundName || !funds.data?.length) return;
    const blocked = funds.data.find((f) => f.state === "BLOCKED");
    setFundName((blocked ?? funds.data[0])!.name);
  }, [funds.data, fundName]);

  const breaks = useBreaks(fundName, filter);
  const log = useChangeLog(fundName);
  const strikes = useNavStrikes(fundName);
  const versions = useConfigVersions(fundName);
  // Only fetched on the trial-balance view: the exceptions queue does not
  // show a chart, and a query nobody reads is a round trip nobody asked for.
  const accounts = useAccounts(view === "balance" ? fundName : undefined, acctFilter);
  const deliveries = useDeliveries(view === "data" ? fundName : undefined);
  // Pending is fetched on every view, not just the data one: it is a blocking
  // condition, and a count an operator only sees after clicking a tab is a
  // count they will miss.
  const pendingFacts = usePendingFacts(fundName);
  const fund: Fund | undefined = funds.data?.find((f) => f.name === fundName);
  const shown = breaks.data ?? [];
  const selected = shown.find((b) => b.name === brkName) ?? shown[0];
  const rows = accounts.data ?? [];
  const account = rows.find((a) => a.name === acctName) ?? rows[0];

  return (
    <div className="app">
      <header className="top">
        <span className="brand">
          <svg viewBox="0 0 64 64" fill="currentColor" aria-hidden="true">
            <rect x="8" y="19" width="16.34" height="10" rx="2" />
            <rect x="29.34" y="19" width="26.66" height="10" rx="2" />
            <rect x="8" y="35" width="48" height="10" rx="2" />
          </svg>
          ratio
        </span>
        <span className="crumb">
          Operations <span aria-hidden="true">/</span> <b>NAV</b>
        </span>
        <nav className="views" aria-label="View">
          <button
            type="button"
            aria-pressed={view === "breaks"}
            onClick={() => setView("breaks")}
          >
            Exceptions
          </button>
          <button
            type="button"
            aria-pressed={view === "balance"}
            onClick={() => setView("balance")}
          >
            Trial balance
          </button>
          <button
            type="button"
            aria-pressed={view === "data"}
            onClick={() => setView("data")}
          >
            Data
            {fund && fund.pendingFactCount !== "0" ? (
              <b className="pip">{fund.pendingFactCount}</b>
            ) : null}
          </button>
        </nav>
        <span className="spacer" />
        <span className="who">
          <span className="avatar">OP</span>Operator
        </span>
      </header>

      <div className="body">
        <nav className="funds" aria-label="Funds">
          <div className="railhead">
            <span>Your funds</span>
            <span>{funds.data?.length ?? ""}</span>
          </div>
          {funds.isPending ? <div className="empty">Loading…</div> : null}
          {funds.isError ? (
            <div className="empty err">{funds.error.message}</div>
          ) : null}
          {funds.data?.map((f) => (
            <button
              key={f.name}
              className="fund"
              aria-current={f.name === fundName}
              onClick={() => {
                setFundName(f.name);
                setBrkName(undefined);
              }}
            >
              <div className="name">{f.displayName}</div>
              <div className="meta">
                <span className={`state ${STATE_CLASS[f.state]}`}>
                  {STATE_LABEL[f.state]}
                </span>
                <span className="num">
                  {f.openBreakCount === "0" ? "—" : `${f.openBreakCount} open`}
                </span>
              </div>
            </button>
          ))}
        </nav>

        <main className="queue">
          <div className="qhead">
            <h1>{fund?.displayName ?? "—"}</h1>
            <div className="subhead">
              <span>
                {fund ? `${fund.currencyCode} · ${count(fund.entryCount)} entries` : ""}
              </span>
              {fund?.configDigest ? (
                <span>
                  configuration <code>{fund.configDigest.slice(0, 7)}</code>
                </span>
              ) : null}
            </div>
          </div>

          <div className="stats">
            <Stat k="Net asset value" v={fund ? money(fund.netAssetValue) : "—"} sub={fund?.currencyCode} />
            <Stat
              k="Trial balance"
              v={fund ? money(fund.trialBalanceDifference) : "—"}
              sub="difference"
              tone={fund?.trialBalanceDifference === "0" ? "tied" : undefined}
            />
            <Stat
              k="Pending"
              v={fund ? count(fund.pendingFactCount) : "—"}
              sub={fund?.pendingFactCount === "1" ? "fact unresolved" : "facts unresolved"}
              tone={fund && fund.pendingFactCount !== "0" ? "at-risk" : "tied"}
            />
            <Stat
              k="Open difference"
              v={fund ? money(fund.openDifference) : "—"}
              sub={fund?.currencyCode}
              tone={fund && fund.openDifference !== "0" ? "at-risk" : undefined}
            />
          </div>

          {view === "data" ? (
            <Data
              fund={fund}
              deliveries={deliveries.data ?? []}
              pending={pendingFacts.data ?? []}
            />
          ) : view === "balance" ? (
            <>
              <div className="qbar" role="group" aria-label="Filter accounts">
                {ACCOUNT_FILTERS.map((f) => (
                  <button
                    key={f.key}
                    className="chip"
                    type="button"
                    aria-pressed={acctFilter === f.key}
                    onClick={() => setAcctFilter(f.key)}
                  >
                    {f.label}
                  </button>
                ))}
                <span className="spacer" />
                <span className="sortnote">
                  {accounts.isFetching ? "refreshing…" : "Chart order"}
                </span>
              </div>
              {accounts.isError ? (
                <div className="empty err">{accounts.error.message}</div>
              ) : null}
              {!accounts.isPending && rows.length === 0 ? (
                <div className="empty">
                  {acctFilter
                    ? "No account matches this filter."
                    : "This book has no chart of accounts yet."}
                </div>
              ) : null}
              {rows.length > 0 ? (
                <Balance
                  fund={fund}
                  accounts={rows}
                  selected={account?.name}
                  onSelect={setAcctName}
                />
              ) : null}
            </>
          ) : (
          <>
          <div className="qbar" role="group" aria-label="Filter breaks">
            {FILTERS.map((f) => (
              <button
                key={f.key}
                className="chip"
                type="button"
                aria-pressed={filter === f.key}
                onClick={() => setFilter(f.key)}
              >
                {f.label}
              </button>
            ))}
            <span className="spacer" />
            <span className="sortnote">
              {breaks.isFetching ? "refreshing…" : "Ordered by money at risk"}
            </span>
          </div>

          <ul className="rows">
            {breaks.isError ? (
              <li><div className="empty err">{breaks.error.message}</div></li>
            ) : null}
            {!breaks.isPending && shown.length === 0 ? (
              <li>
                <div className="empty">
                  {fund?.state === "STRUCK"
                    ? "No breaks. NAV struck."
                    : filter
                      ? "Nothing matches this filter."
                      : "No breaks — this period has not been reconciled yet."}
                </div>
              </li>
            ) : null}
            {shown.map((b) => (
              <li key={b.name}>
                <button
                  className="row"
                  aria-current={b.name === selected?.name}
                  onClick={() => setBrkName(b.name)}
                >
                  <span className={`sev ${SEVERITY_CLASS[b.severity]}`} />
                  <span>
                    <div className="title">
                      {b.account} — {money(b.difference)}
                    </div>
                    <div className="why">
                      {b.cause} · {b.postings.length} posting
                      {b.postings.length === 1 ? "" : "s"} under configuration{" "}
                      {b.configDigest.slice(0, 7)}
                    </div>
                  </span>
                  <span className={`amt num${b.explained ? "" : " pos"}`}>
                    {money(b.difference)}
                    <small>{b.explained ? "explained" : "open"}</small>
                  </span>
                </button>
              </li>
            ))}
          </ul>
          </>
          )}

          <Record fund={fund} />

          {/* The time axis. Every figure above is "now"; this is every NAV this
              fund has struck, and each one can be re-derived on demand. */}
          <section className="log" aria-label="NAV strikes">
            <div className="loghead">
              <span>NAV strikes</span>
              <span className="sortnote">
                {strikes.data?.length
                  ? "each folds a pinned journal prefix"
                  : ""}
              </span>
            </div>
            {strikes.data?.length === 0 ? (
              <div className="empty">
                No NAV struck yet. <code>ratio strike</code> takes one.
              </div>
            ) : null}
            {strikes.data?.map((s) => <Strike key={s.name} s={s} />)}
          </section>

          {/* The configuration axis. Every figure above was produced under one
              of these, and each one can say what it changed. */}
          <section className="log" aria-label="Configuration history">
            <div className="loghead">
              <span>Configuration</span>
              <span className="sortnote">
                {versions.data?.length ? "newest first" : ""}
              </span>
            </div>
            {versions.data?.length === 0 ? (
              <div className="empty">No configuration promoted yet.</div>
            ) : null}
            {versions.isError ? (
              <div className="empty err">{versions.error.message}</div>
            ) : null}
            {versions.data?.map((v) => <Version key={v.digest} v={v} />)}
          </section>

          <section className="log" aria-label="Change log">
            <div className="loghead">
              <span>Change log</span>
            </div>
            {log.data?.length === 0 ? (
              <div className="empty">Nothing has changed this configuration.</div>
            ) : null}
            {log.data?.map((e) => (
              <div className="logrow" key={e.name}>
                <span className="t num">
                  {e.configDigest === "proposal" ? "draft" : e.configDigest.slice(0, 7)}
                </span>
                <span className="w">
                  <span className={`by${e.actorKind === "MODEL" ? " model" : ""}`}>
                    {e.actor}
                  </span>{" "}
                  {e.action} <b>{e.subject}</b>
                  {e.detail ? ` — ${e.detail}` : ""}
                </span>
                {/* `detail` already carries "awaiting a person" for a model's
                    row, and printing it twice on one line read as emphasis
                    where it is just repetition. */}
                <span className="cfg num" />
              </div>
            ))}
          </section>
        </main>

        {view === "balance" ? <Lines account={account} /> : <Detail brk={selected} />}
      </div>
    </div>
  );
}
