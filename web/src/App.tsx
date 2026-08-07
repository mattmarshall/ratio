// The operations console.
//
// The design is the one published at claude.ai as a concept; this renders it
// from `ratio.v1.Console` instead of from a literal. Where the concept invented
// five funds, this shows the funds that exist — which is one on the demo, and
// however many books are on disk everywhere else.

import { useEffect, useState } from "react";
import { useBreaks, useChangeLog, useFunds, useNavStrikes, useReplay } from "./api.js";
import {
  SEVERITY_CLASS,
  STATE_CLASS,
  STATE_LABEL,
  count,
  money,
} from "./format.js";
import type { Break, Fund, NavStrike } from "./types.js";

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

export default function App() {
  const funds = useFunds();
  const [fundName, setFundName] = useState<string>();
  const [filter, setFilter] = useState<string>("");
  const [brkName, setBrkName] = useState<string>();

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
  const fund: Fund | undefined = funds.data?.find((f) => f.name === fundName);
  const shown = breaks.data ?? [];
  const selected = shown.find((b) => b.name === brkName) ?? shown[0];

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
              k="Open difference"
              v={fund ? money(fund.openDifference) : "—"}
              sub={fund?.currencyCode}
              tone={fund && fund.openDifference !== "0" ? "at-risk" : undefined}
            />
          </div>

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
                <span className="cfg num">
                  {e.actorKind === "MODEL" ? "awaiting a person" : ""}
                </span>
              </div>
            ))}
          </section>
        </main>

        <Detail brk={selected} />
      </div>
    </div>
  );
}
