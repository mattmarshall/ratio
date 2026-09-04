import Link from "next/link";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { caller } from "@/lib/caller";
import {
  citeOf,
  defaultPin,
  parsePin,
  pinKey,
  restated,
  strikeId,
  washCites,
} from "@/lib/asof";
import { closedYmd } from "@/lib/close";
import { periodLabel, previousMonth, utcMonth, utcYear } from "@/lib/dates";
import { gain } from "@/lib/format";
import { getView, listNavStrikes, listPeriodCloses } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/**
 * Point-in-time / restatement browser: pick a pinned prefix and read
 * the cites the core already has.
 *
 * ⭐ NOT A SECOND ANSWER TO A VALUATION DAY. `Ratio.Period` refuses
 * restating a NAV in place. This page browses closes and strikes that
 * already pin a prefix + config digest, and WashRestatement when a
 * later wash moved a struck realized gain. The struck NAV is not
 * rewritten.
 *
 * ⛔ UNSET STAYS UNSET. A window with no close and no strike does not
 * invent a digest. A book that never restated a strike does not invent
 * a moved figure. Kind still selects one chrome list (`screensFor`).
 */
async function AsOfPage({
  params,
  searchParams,
}: {
  params: Promise<{ book: string; view: string }>;
  searchParams: Promise<{ period?: string; pin?: string }>;
}) {
  const { book, view } = await params;
  const month = utcMonth();
  const year = utcYear();
  const last = previousMonth(month);
  const { period = month, pin: rawPin } = await searchParams;
  const window = period || month;
  const c = await caller();
  const [v, listed, strikes] = await Promise.all([
    getView(c, book, view),
    listPeriodCloses(c, book, view),
    listNavStrikes(c, book, view),
  ]);
  const pin = parsePin(rawPin) ?? defaultPin(listed.periodCloses, window);
  const cite = citeOf(
    pin,
    v.journalPosition,
    listed.periodCloses,
    strikes.navStrikes,
    window,
  );
  const washes = washCites(strikes.navStrikes);
  const anyRestatement = washes.some(restated);
  const anyQualified = washes.some((w) => w.qualified);

  const periods: readonly Filter[] = [
    { key: month, label: periodLabel(month) },
    { key: last, label: periodLabel(last) },
    { key: year, label: year },
  ];

  const prefixes: readonly Filter[] = [
    { key: "now", label: "Now" },
    ...listed.periodCloses.map((cl) => {
      const id = closedYmd(cl.closedDate);
      return { key: `close:${id}`, label: `Close ${id}` };
    }),
    ...strikes.navStrikes.map((s) => {
      const id = strikeId(s);
      return { key: `strike:${id}`, label: `Strike ${id}` };
    }),
  ];

  const digestShown = (d: string | null) =>
    d ? d.slice(0, 12) : "unset — no pinned digest";
  const configShown = (d: string | null) =>
    d ? d.slice(0, 7) : "unset — no pinned config";

  return (
    <>
      <FilterChips
        filters={periods}
        active={window}
        param="period"
        keep={{ pin: pinKey(pin) }}
        label="Period"
        note={`${periodLabel(window)} — dated closes and strikes only`}
      />
      <FilterChips
        filters={prefixes}
        active={pinKey(pin)}
        param="pin"
        keep={{ period: window }}
        label="Pinned prefix"
        note={cite.label}
      />

      <div className="tb" role="table" aria-label="Point in time">
        <div className="posgroup">
          <div className="posacct">Pinned prefix</div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Journal prefix
              <span className="at">
                {cite.journalPosition === null
                  ? "unset — no pinned prefix"
                  : `${cite.journalPosition} entries this figure folded`}
              </span>
            </span>
            <span role="cell" className="num">
              {cite.journalPosition ?? "—"}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Journal digest
              <span className="at">
                {cite.journalDigest
                  ? "SHA-256 of exactly those entries"
                  : "unset — the maintained fold is not a pin"}
              </span>
            </span>
            <span role="cell" className="num">
              {digestShown(cite.journalDigest)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Config digest
              <span className="at">
                {cite.configDigest
                  ? "the configuration in force at the last folded entry"
                  : "unset — no pinned config"}
              </span>
            </span>
            <span role="cell" className="num">
              {configShown(cite.configDigest)}
            </span>
          </div>
        </div>

        <div className="posgroup">
          <div className="posacct">WashRestatement</div>
          {!anyRestatement && !anyQualified ? (
            <div className="tbrow static" role="row">
              <span role="cell">
                Unset — no wash restatement
                <span className="at">
                  a restatement cites a strike; it does not rewrite the
                  struck figure
                </span>
              </span>
              <span role="cell" className="num">
                —
              </span>
            </div>
          ) : null}
          {washes.map((w) => {
            if (restated(w)) {
              return (
                <div className="tbrow static" role="row" key={w.strikeId}>
                  <span role="cell">
                    WashRestatement cites this strike
                    <span className="at">
                      {w.strikeId} — the struck figure is not rewritten
                    </span>
                  </span>
                  <span role="cell" className="num">
                    {gain(w.original!)} → {gain(w.movedTo!)}
                  </span>
                </div>
              );
            }
            if (w.qualified) {
              return (
                <div className="tbrow static" role="row" key={w.strikeId}>
                  <span role="cell">
                    Wash window was open
                    <span className="at">
                      {w.strikeId} — this realized gain can still move
                    </span>
                  </span>
                  <span role="cell" className="num">
                    qualified
                  </span>
                </div>
              );
            }
            return null;
          })}
        </div>
      </div>

      <p className="note">
        <Link
          href={`/books/${book}/views/${view}/close?period=${encodeURIComponent(window)}`}
        >
          Period close
        </Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/strikes`}>NAV strikes</Link>
        {" · "}a restatement cites the strike; it does not rewrite the struck
        figure
      </p>
    </>
  );
}

export default withRefusal(AsOfPage);
