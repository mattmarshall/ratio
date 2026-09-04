// Point-in-time / restatement cites: a pinned prefix + config digest,
// and WashRestatement when a later wash moved a struck realized gain.
//
// ⛔ UNSET STAYS UNSET. A window with no close and no strike does not
// invent a digest. A book that never restated a strike does not invent
// a moved figure. Printing 0 or "the head" in those holes is the silent
// rewrite `Ratio.Lots.WashRestatement` exists to refuse.
//
// ⛔ THE STRUCK NAV IS NOT REWRITTEN. A restatement is a new record that
// cites the strike. Putting `moved_to` on `netAssetValue` is
// `rewrite_in_place`.

import { closedYmd, coveringClose } from "./close";
import type { NavStrike, PeriodClose } from "@/wire/types";

export type Pin =
  | { readonly kind: "now" }
  | { readonly kind: "close"; readonly id: string }
  | { readonly kind: "strike"; readonly id: string };

/** `?pin=` → a pin, or `null` when the query does not name one. */
export function parsePin(raw: string | undefined): Pin | null {
  if (raw === undefined || raw === "") return null;
  if (raw === "now") return { kind: "now" };
  const close = /^close:(.+)$/.exec(raw);
  if (close?.[1]) return { kind: "close", id: close[1] };
  const strike = /^strike:(.+)$/.exec(raw);
  if (strike?.[1]) return { kind: "strike", id: strike[1] };
  return null;
}

export function pinKey(p: Pin): string {
  if (p.kind === "now") return "now";
  if (p.kind === "close") return `close:${p.id}`;
  return `strike:${p.id}`;
}

/** Covering close of the calendar window, else the maintained fold. */
export function defaultPin(
  closes: readonly PeriodClose[],
  period: string,
): Pin {
  const cover = coveringClose(closes, period);
  const id = cover ? closedYmd(cover.closedDate) : "";
  return id ? { kind: "close", id } : { kind: "now" };
}

export interface PrefixCite {
  readonly kind: "now" | "close" | "strike" | "unset";
  /** Journal entries folded. Null when no prefix is pinned. */
  readonly journalPosition: string | null;
  /** SHA-256 of that prefix. Null on `now` — the head is not a pin. */
  readonly journalDigest: string | null;
  /** Configuration in force at the last folded entry. Null when unset. */
  readonly configDigest: string | null;
  readonly label: string;
}

function unsetCite(label: string): PrefixCite {
  return {
    kind: "unset",
    journalPosition: null,
    journalDigest: null,
    configDigest: null,
    label,
  };
}

/**
 * The cite a pin names.
 *
 * ⭐ DIGEST AND CONFIG ARE UNSET ON `now`. GetView carries a journal
 * position; it does not pin a historical digest. A close or a strike
 * does. Filling those from the configuration in force now is the
 * lot-method trap applied to a prefix.
 */
export function citeOf(
  pin: Pin | null,
  nowPosition: string,
  closes: readonly PeriodClose[],
  strikes: readonly NavStrike[],
  period: string,
): PrefixCite {
  const resolved = pin ?? defaultPin(closes, period);
  if (resolved.kind === "now") {
    const pos = nowPosition && nowPosition !== "0" ? nowPosition : null;
    return {
      kind: pos ? "now" : "unset",
      journalPosition: pos,
      journalDigest: null,
      configDigest: null,
      label: pos
        ? "now — the maintained fold, not a pinned strike"
        : "unset — no pinned prefix",
    };
  }
  if (resolved.kind === "close") {
    const c = closes.find((x) => closedYmd(x.closedDate) === resolved.id);
    if (!c) return unsetCite("unset — no close pins this prefix");
    return {
      kind: "close",
      journalPosition: c.journalPosition || null,
      journalDigest: c.journalDigest || null,
      configDigest: c.configDigest || null,
      label: `close through ${resolved.id}`,
    };
  }
  const s = strikes.find((x) => strikeId(x) === resolved.id);
  if (!s) return unsetCite("unset — no strike pins this prefix");
  return {
    kind: "strike",
    journalPosition: s.journalPosition || null,
    journalDigest: s.journalDigest || null,
    configDigest: s.configDigest || null,
    label: `NAV strike ${resolved.id}`,
  };
}

export function strikeId(s: NavStrike): string {
  return s.name.split("/").pop() ?? s.name;
}

export interface WashCiteView {
  readonly strikeId: string;
  readonly qualified: boolean;
  /** Credit-normal original. Null when no restatement. */
  readonly original: string | null;
  readonly movedTo: string | null;
}

export function washCites(strikes: readonly NavStrike[]): WashCiteView[] {
  return strikes.map((s) => ({
    strikeId: strikeId(s),
    qualified: s.washQualified,
    original: s.washRestatementOriginal || null,
    movedTo: s.washRestatementMovedTo || null,
  }));
}

/** A restatement exists only when both figures are present. */
export function restated(w: WashCiteView): boolean {
  return w.original !== null && w.movedTo !== null;
}
