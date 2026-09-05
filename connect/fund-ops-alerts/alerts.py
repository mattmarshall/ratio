#!/usr/bin/env python3
"""Fund ops alerts — a WorkOS Connect app, not a kernel notifier.

Unexplained breaks, unpriced marks, and NAV gate blocking reasons
live **here** as a read of cites. They do not live in `ratio watch`,
the operations console, a chatbot, or a notification service inside
Ratio. Core already has breaks, `nav_gate`, and unpriced cites
(#188). This app polls them.

⭐ SCOPES ARE THE FROZEN CATALOG NAMES. Subscribe via
`webhooks:journal` (reserved — the kernel surface is not built).
Poll `breaks:read` + `nav:read` + `views:read`. Membership via
`books:read`. Aliases (`journal:read`, `journal:append`) and
invented strings are refused. See docs/connect-scopes.md.

⭐ THIS APP IS READ-ONLY RELATIVE TO THE JOURNAL. It does not
request `journals:post`. An empty `journals:post` allowlist refuses
every post. An alert is a read of cites, not a rewrite.

⭐ UNSET STAYS UNSET. A missing BreakReport is unset, not a silent
empty list that looks reconciled. A cited report with no lines is
cited-empty — the period reconciled, and the pack says so. A
missing `nav_gate` is unset, not an all-clear gate. A cited gate
with no reasons is cited-empty — nothing blocks, and the pack
says so. Unpriced stays empty unless a valuation date was named.
A missing NAV strike is unset, not NAV 0.00. An empty journal
digest is unset, not history-intact. A posted `"0.00"` is a figure.

⭐ THREE FIRST-CLASS REASONS, THE SAME FOLD THE BADGE READS.
`nav_gate` copies `Console::blocking_at`: unexplained break,
unresolved trade, unpriced. This app does not invent a fourth
kind, a break explanation, or a rewritten strike.

⭐ MONEY IS MINOR UNITS, PARSED BY SPLITTING ON THE POINT. A float
is how a cent disappears. A third decimal place is refused.

⭐ NO NEW METHOD / ORDER / LOT_METHOD VARIANT. MinTax, SpecID,
average cost, and wash stay elections.

⭐ KIND-AWARE CITES, NOT A CHROME FORK. Fund-ops cites stay unset
on kinds that do not wear Exceptions / Positions / NAV.
`screensFor` is not forked. UNSPECIFIED is the proto default,
not a fifth kind.

⭐ THE GRANT PATH CALLS CONNECTAPIURL. `fetch_cites` and `deliver`
present a verified Connect access token and pull against the
Connect HTTP API. Membership is still required. A Connect token
never takes `RATIO_DEMO_OPEN` and never matches `org:{id}`.
WorkOS dashboard registration stays leftover #22. A green cite
is not a live walk-through.

⚠ NO KERNEL NOTIFIER, NO CHATBOT, NO HTML ALERT UI, NO LIVE
SLACK / EMAIL / PAGERDUTY WITHOUT A CONFIGURED DESTINATION.
`subscribe`, `kernel_notify`, `chatbot`, `html_alerts`,
`explain_break`, `rewrite_strike`, `slack`, `email`, and
`pagerduty` refuse. `dry_run` / `deliver` write a local cite
pack after a ConnectApiUrl pull.
"""

from __future__ import annotations

import csv
import io
import json
from dataclasses import dataclass, field
from datetime import date
from typing import Any, Mapping, Sequence

import grant as _grant

# i64 bounds. Lean's Int is unbounded; every money figure here is i64.
I64_MIN = -(2**63)
I64_MAX = 2**63 - 1

REQUIRED_SCOPES = frozenset(
    {
        "webhooks:journal",
        "breaks:read",
        "nav:read",
        "views:read",
        "books:read",
    }
)
OPTIONAL_SCOPES: frozenset[str] = frozenset()
CANONICAL_SCOPES = REQUIRED_SCOPES | OPTIONAL_SCOPES
REFUSED_ALIASES = frozenset({"journal:append", "journal:read"})

# ⛔ NOT METHODS. Each is an election with its own shape. The TLA
# probes that treat them as a Method exist so that mistake goes red.
REFUSED_LOT_METHODS = frozenset(
    {
        "min_tax",
        "specific_id",
        "average_cost",
        "wash",
        "wash_sale",
        "wash_sales",
    }
)

# Wire / proto field names this pack copies. A rename here that
# the proto does not share is how a pack invents a cite.
NAV_GATE_PROTO_FIELDS = (
    "unexplained_breaks",
    "unresolved_trades",
    "unpriced",
)
BREAK_PROTO_FIELDS = (
    "ratio_amount",
    "reported_amount",
    "difference",
    "config_digest",
    "explained",
    "explanation",
)
STRIKE_PROTO_FIELDS = (
    "valuation_time",
    "journal_position",
    "journal_digest",
    "net_asset_value",
    "trial_balance_difference",
    "config_digest",
)


class Refuse(Exception):
    """The alert is not emitted. Message is the reason, not a workaround."""


@dataclass(frozen=True)
class Client:
    client_id: str
    scopes: frozenset[str]


@dataclass(frozen=True)
class Book:
    """`books:read` membership. An org_id claim is not membership."""

    book_id: str
    kind: str
    member: bool = True
    org_id: str | None = None


@dataclass(frozen=True)
class BreakCite:
    """One `breaks:read` Break. Explanation is never invented here."""

    name: str
    account: str
    severity: str = ""
    explained: bool = False
    cause: str = ""
    ratio_amount: int | None = None
    reported_amount: int | None = None
    difference: int | None = None
    config_digest: str | None = None


@dataclass(frozen=True)
class NavGateCite:
    """`nav:read` / `views:read` copy of GetFund / GetView `nav_gate`.

    The same `blocking_at` fold the console badge reads (#188).
    `None` on the whole cite is unset (no gate was handed over).
    Empty tuples are cited-empty — nothing blocks — and the pack
    says so. Unpriced stays empty unless `valuation_date` is set.
    """

    unexplained_breaks: tuple[str, ...] = ()
    unresolved_trades: tuple[str, ...] = ()
    unpriced: tuple[str, ...] = ()
    valuation_date: date | None = None


@dataclass(frozen=True)
class StrikeCite:
    """One `nav:read` NavStrike. NAV empty is unset, not 0.00."""

    name: str
    view: str
    net_asset_value: int | None = None
    journal_digest: str | None = None
    journal_position: int | None = None
    config_digest: str | None = None


@dataclass(frozen=True)
class Section:
    """One pack section. `cited-empty` is a real empty, not unset."""

    status: str
    note: str
    rows: int = 0


@dataclass(frozen=True)
class AlertPack:
    """A cite pack of fund-ops alerts. Not a live Slack delivery."""

    book: Book
    breaks: tuple[BreakCite, ...] | None
    nav_gate: NavGateCite | None
    strike: StrikeCite | None
    sections: dict[str, Section]
    unset: tuple[str, ...]
    manifest: dict[str, Any] = field(default_factory=dict)


def load_app(path: str) -> dict[str, Any]:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def client_from_app(app: Mapping[str, Any]) -> Client:
    connect = app.get("workos_connect") or {}
    scopes = connect.get("scopes") or []
    allow = app.get("journals_post_allowlist") or {}
    return Client(
        client_id=str(
            allow.get("client_id")
            or connect.get("client_id")
            or app.get("name")
            or "ratio-fund-ops-alerts"
        ),
        scopes=frozenset(scopes),
    )


def parse_minor(text: str, *, allow_signed: bool = False) -> int:
    """Split on the point. Never parse a float.

    Same door as `ratio_common::parse_minor`. A third decimal place is
    refused rather than dropped. Overflow is refused rather than wrapped.
    Break difference may be signed; inventing a sign is how a loss and
    a gain swap.
    """
    if not isinstance(text, str):
        raise Refuse(
            f"{text!r} is not an amount string — a number typed as a float "
            "is how a cent disappears"
        )
    t = text.strip().replace(",", "").replace("$", "")
    sign = 1
    if t.startswith("-"):
        if not allow_signed:
            raise Refuse(
                f"{text!r} is signed; this figure is a magnitude, and a "
                "signed-amount inference is how a hold and a release swap"
            )
        sign = -1
        t = t[1:]
    if t.startswith("+"):
        t = t[1:]
    if not t:
        raise Refuse("an amount is required")
    if "." in t:
        whole, _, frac = t.partition(".")
        if "." in frac:
            raise Refuse(f"{text!r} is not an amount")
    else:
        whole, frac = t, ""
    if frac and len(frac) > 2:
        raise Refuse(
            f"{text!r} has more than two decimal places; the books are kept "
            "in minor units"
        )
    if (whole and not whole.isdigit()) or (frac and not frac.isdigit()):
        raise Refuse(f"{text!r} is not an amount")
    if not whole and not frac:
        raise Refuse(f"{text!r} is not an amount")
    try:
        major = int(whole) if whole else 0
        if len(frac) == 0:
            minor = 0
        elif len(frac) == 1:
            minor = int(frac) * 10
        else:
            minor = int(frac)
    except ValueError as e:
        raise Refuse(f"{text!r} is not an amount") from e
    if major > I64_MAX // 100:
        raise Refuse(f"{text!r} does not fit in i64 minor units")
    v = major * 100 + minor
    if sign < 0:
        v = -v
    if v > I64_MAX or v < I64_MIN:
        raise Refuse(f"{text!r} does not fit in i64 minor units")
    return v


def parse_optional_minor(text: Any, *, allow_signed: bool = False) -> int | None:
    """Empty / omitted is unset. `"0.00"` is a set figure of nothing."""
    if text is None:
        return None
    if isinstance(text, str) and not text.strip():
        return None
    return parse_minor(str(text), allow_signed=allow_signed)


def format_minor(n: int) -> str:
    """Decimal string — never a float, never scientific."""
    sign = "-" if n < 0 else ""
    n = abs(n)
    if n > I64_MAX:
        raise Refuse("amount does not fit in i64 minor units")
    whole, frac = divmod(n, 100)
    return f"{sign}{whole}.{frac:02d}"


def parse_optional_digest(text: Any) -> str | None:
    """Empty is unset. An empty digest is not history-intact."""
    if text is None:
        return None
    if not isinstance(text, str):
        raise Refuse(
            f"{text!r} is not a digest string — inventing a hash is how "
            "an empty digest looks like success"
        )
    t = text.strip()
    return t or None


def parse_optional_int(text: Any) -> int | None:
    if text is None:
        return None
    if isinstance(text, bool):
        raise Refuse("a journal position is a number, not a flag")
    if isinstance(text, str) and not text.strip():
        return None
    try:
        v = int(text)
    except (TypeError, ValueError) as e:
        raise Refuse(f"{text!r} is not a journal position") from e
    if v > I64_MAX or v < 0:
        raise Refuse("journal position does not fit in i64")
    return v


def parse_optional_day(text: Any) -> date | None:
    if text is None:
        return None
    if isinstance(text, Mapping):
        year = text.get("year")
        month = text.get("month")
        day = text.get("day")
        if year in (None, 0, "") or month in (None, 0, "") or day in (None, 0, ""):
            return None
        try:
            return date(int(year), int(month), int(day))
        except ValueError as e:
            raise Refuse(f"{text!r} is not a calendar day") from e
    if not isinstance(text, str) or not text.strip():
        return None
    t = text.strip()
    try:
        y, m, d = t.split("-")
        return date(int(y), int(m), int(d))
    except ValueError as e:
        raise Refuse(f"{text!r} is not a calendar day YYYY-MM-DD") from e


def _require_scopes(client: Client) -> None:
    aliases = client.scopes & REFUSED_ALIASES
    if aliases:
        raise Refuse(
            "refused alias scope "
            + ", ".join(sorted(aliases))
            + " — catalogs use journals:read / journals:post, not "
            "journal:read / journal:append"
        )
    extra = client.scopes - CANONICAL_SCOPES
    if extra:
        raise Refuse(
            "unknown scope "
            + ", ".join(sorted(extra))
            + " — a string that is not in docs/connect-scopes.md is refused"
        )
    if "journals:post" in client.scopes:
        raise Refuse(
            "this app is read-only relative to the journal — journals:post "
            "is a write grant this pack does not need. The empty allowlist "
            "would refuse every post anyway"
        )
    missing = REQUIRED_SCOPES - client.scopes
    if missing:
        raise Refuse(
            "this app needs "
            + ", ".join(sorted(REQUIRED_SCOPES))
            + f"; missing {', '.join(sorted(missing))}. "
            "webhooks:journal is the reserved subscribe grant; "
            "breaks:read / nav:read / views:read / books:read are the cites"
        )


def _require_membership(book: Book) -> None:
    if not book.book_id.strip():
        raise Refuse("books:read names a book")
    if not book.kind.strip():
        raise Refuse(
            "a book names a kind — UNSPECIFIED is the proto default, "
            "not a domain and not a hidden fifth kind"
        )
    if book.kind == "UNSPECIFIED":
        raise Refuse(
            "UNSPECIFIED is the proto default, not a domain and not a "
            "hidden fifth kind"
        )
    if not book.member:
        raise Refuse(
            f"book {book.book_id!r} is not in the subject's membership — "
            "an org_id claim is not membership. books:read lists books "
            "the subject can see (#151)"
        )


def _string_list(raw: Any) -> tuple[str, ...]:
    if raw is None:
        return ()
    if isinstance(raw, str):
        return (raw,) if raw.strip() else ()
    if not isinstance(raw, Sequence) or isinstance(raw, (bytes, bytearray)):
        raise Refuse("a nav_gate list is a sequence of cites, not invented text")
    out: list[str] = []
    for item in raw:
        s = str(item).strip()
        if s:
            out.append(s)
    return tuple(out)


def break_from_cite(row: Mapping[str, Any]) -> BreakCite:
    name = str(row.get("name") or "").strip()
    if not name:
        raise Refuse("a Break names itself")
    account = str(row.get("account") or "").strip()
    if not account:
        raise Refuse("a Break names an account")
    # breaks:explain stays off the grant. A person-attributed
    # explanation already on the Break is ignored here — unexplained
    # is the alert. Inventing one is refused at explain_break().
    return BreakCite(
        name=name,
        account=account,
        severity=str(row.get("severity") or "").strip(),
        explained=bool(row.get("explained", False)),
        cause=str(row.get("cause") or "").strip(),
        ratio_amount=parse_optional_minor(
            row.get("ratio_amount", row.get("ratioAmount")),
            allow_signed=True,
        ),
        reported_amount=parse_optional_minor(
            row.get("reported_amount", row.get("reportedAmount")),
            allow_signed=True,
        ),
        difference=parse_optional_minor(row.get("difference"), allow_signed=True),
        config_digest=parse_optional_digest(
            row.get("config_digest", row.get("configDigest"))
        ),
    )


def nav_gate_from_cite(raw: Mapping[str, Any] | None) -> NavGateCite | None:
    if raw is None:
        return None
    valuation = parse_optional_day(
        raw.get("valuation_date", raw.get("valuationDate", raw.get("as_of", raw.get("asOf"))))
    )
    unpriced = _string_list(raw.get("unpriced"))
    if unpriced and valuation is None:
        raise Refuse(
            "unpriced stays empty unless a valuation date was named — "
            "the same limit a bare ratio strike already has. Inventing "
            "an unpriced list without as-of is a silent mark"
        )
    return NavGateCite(
        unexplained_breaks=_string_list(
            raw.get("unexplained_breaks", raw.get("unexplainedBreaks"))
        ),
        unresolved_trades=_string_list(
            raw.get("unresolved_trades", raw.get("unresolvedTrades"))
        ),
        unpriced=unpriced,
        valuation_date=valuation,
    )


def strike_from_cite(row: Mapping[str, Any] | None) -> StrikeCite | None:
    if row is None:
        return None
    name = str(row.get("name") or "").strip()
    if not name:
        raise Refuse("a NavStrike names itself")
    return StrikeCite(
        name=name,
        view=str(row.get("view") or "").strip(),
        net_asset_value=parse_optional_minor(
            row.get("net_asset_value", row.get("netAssetValue")),
            allow_signed=True,
        ),
        journal_digest=parse_optional_digest(
            row.get("journal_digest", row.get("journalDigest"))
        ),
        journal_position=parse_optional_int(
            row.get("journal_position", row.get("journalPosition"))
        ),
        config_digest=parse_optional_digest(
            row.get("config_digest", row.get("configDigest"))
        ),
    )


def unexplained_breaks(breaks: Sequence[BreakCite] | None) -> tuple[BreakCite, ...]:
    """Alert lines: unexplained HIGH (or unnamed-severity) breaks.

    Explained breaks stay on the report; they are not the alert.
    This function does not invent a cause or an explainer.
    """
    if breaks is None:
        return ()
    out: list[BreakCite] = []
    for b in breaks:
        if b.explained:
            continue
        sev = b.severity.strip().upper()
        if sev and sev not in ("HIGH", "SEVERITY_HIGH"):
            continue
        out.append(b)
    return tuple(out)


def _section(
    *,
    cited: Sequence[Any] | None,
    name: str,
    cited_empty_note: str,
    unset_note: str,
) -> Section:
    if cited is None:
        return Section(status="unset", note=unset_note, rows=0)
    if len(cited) == 0:
        return Section(status="cited-empty", note=cited_empty_note, rows=0)
    return Section(status="cited", note=f"{len(cited)} {name}", rows=len(cited))


def build_pack(
    *,
    book: Book,
    client: Client,
    breaks: Sequence[BreakCite] | None = None,
    nav_gate: NavGateCite | None = None,
    strike: StrikeCite | None = None,
) -> AlertPack:
    """Read kernel cites into an alert pack.

    `None` on a sequence is unset (the cite was not handed over).
    An empty sequence is cited-empty — a BreakReport with no lines,
    or a `nav_gate` with no blocking reasons — and the pack says so.
    A silent empty list that looks reconciled is the defect this
    function exists to refuse.
    """
    _require_scopes(client)
    _require_membership(book)

    unset: list[str] = []
    sections: dict[str, Section] = {}

    sections["breaks"] = _section(
        cited=breaks,
        name="breaks",
        cited_empty_note=(
            "BreakReport cited with no lines — the period reconciled. "
            "That is not a missing report"
        ),
        unset_note=(
            "no breaks:read cite — not a silent reconciled-empty list"
        ),
    )
    if breaks is None:
        unset.append("breaks: unset — not a silent reconciled-empty list")
    elif len(breaks) == 0:
        unset.append(
            "breaks: cited-empty — BreakReport with no lines means the "
            "period reconciled, and the pack says so"
        )
    else:
        for b in breaks:
            if b.config_digest is None:
                unset.append(
                    f"breaks/{b.name}/config_digest: unset — empty is "
                    "not history-intact"
                )
        open_lines = unexplained_breaks(breaks)
        sections["unexplained"] = Section(
            status="cited" if open_lines else "cited-empty",
            note=(
                f"{len(open_lines)} unexplained break(s)"
                if open_lines
                else "no unexplained HIGH breaks on a cited report"
            ),
            rows=len(open_lines),
        )

    if nav_gate is None:
        sections["nav_gate"] = Section(
            status="unset",
            note=(
                "no nav_gate cite — not an all-clear gate. GetFund / "
                "GetView carry the same blocking_at fold the badge reads"
            ),
        )
        unset.append("nav_gate: unset — not an all-clear gate")
    else:
        reasons = (
            len(nav_gate.unexplained_breaks)
            + len(nav_gate.unresolved_trades)
            + len(nav_gate.unpriced)
        )
        if reasons == 0:
            sections["nav_gate"] = Section(
                status="cited-empty",
                note=(
                    "nav_gate cited with no blocking reasons — nothing "
                    "blocks. That is not a missing gate"
                ),
                rows=0,
            )
            unset.append(
                "nav_gate: cited-empty — nothing blocks, and the pack says so"
            )
        else:
            sections["nav_gate"] = Section(
                status="cited",
                note=f"{reasons} nav_gate reason(s)",
                rows=reasons,
            )
        if nav_gate.valuation_date is None:
            if nav_gate.unpriced:
                raise Refuse(
                    "unpriced stays empty unless a valuation date was named"
                )
            unset.append(
                "nav_gate.unpriced: empty without as-of — not a silent "
                "priced book"
            )
        elif not nav_gate.unpriced:
            unset.append(
                "nav_gate.unpriced: cited-empty on a named day — every "
                "position has a price on or before that day"
            )

    if strike is None:
        sections["strike"] = Section(
            status="unset",
            note="no nav:read strike — not a silent NAV of 0.00",
        )
        unset.append("strike: unset — not NAV 0.00")
    else:
        if strike.net_asset_value is None:
            sections["strike"] = Section(
                status="unset",
                note="NavStrike without net_asset_value — not NAV 0.00",
            )
            unset.append(f"strike/{strike.name}/net_asset_value: unset — not NAV 0.00")
        else:
            sections["strike"] = Section(
                status="cited",
                note="nav:read NavStrike",
                rows=1,
            )
        if strike.journal_digest is None:
            unset.append(
                f"strike/{strike.name}/journal_digest: unset — empty is "
                "not history-intact and not reproduced"
            )

    manifest = {
        "book": book.book_id,
        "kind": book.kind,
        "issue": 162,
        "grant_path": (
            "built — first-party Connect apps call ConnectApiUrl; "
            "leftover #22 is WorkOS dashboard registration"
        ),
        "note": (
            "A green cite is not a live walk-through. Missing cites "
            "stay unset here; they are not a silent reconciled list, "
            "an all-clear gate, or NAV 0.00. Destinations stay local "
            "dry-run until leftover #22 / product Slack / email / "
            "PagerDuty land."
        ),
        "sections": {
            name: {"status": s.status, "note": s.note, "rows": s.rows}
            for name, s in sections.items()
        },
        "unset": list(unset),
    }

    return AlertPack(
        book=book,
        breaks=tuple(breaks) if breaks is not None else None,
        nav_gate=nav_gate,
        strike=strike,
        sections=sections,
        unset=tuple(unset),
        manifest=manifest,
    )


def fetch_cites(
    *,
    token: str | None = None,
    book_id: str | None = None,
    view: str = "book",
    transport: _grant.Transport | None = None,
) -> dict[str, Any]:
    """Pull live cites from ConnectApiUrl. Membership still required.

    A missing token is a missing token, not "the grant path is not
    built". There is no kernel notifier — this is a read of cites.
    """
    cites: dict[str, Any] = {
        "book": _grant.pull(
            token=token,
            book_id=book_id,
            transport=transport,
            error=Refuse,
        )
    }
    named = (book_id or "").strip().strip("/")
    if named.startswith("books/"):
        named = named[len("books/") :]
    if named.startswith("funds/"):
        named = named[len("funds/") :]
    if named:
        for key, suffix in (
            ("fund", f"funds/{named}"),
            ("view", f"funds/{named}/views/{view}"),
            ("breaks", f"funds/{named}/views/{view}/breaks"),
            ("strikes", f"funds/{named}/views/{view}/navStrikes"),
        ):
            cites[key] = _grant.pull(
                token=token,
                path=f"/v1/{suffix}",
                transport=transport,
                error=Refuse,
            )
    return cites


def deliver(
    pack: AlertPack,
    *,
    token: str | None = None,
    transport: _grant.Transport | None = None,
) -> dict[str, str]:
    """Write the local cite pack after the grant can read ConnectApiUrl.

    This is the dry-run destination. It does not POST to Slack,
    email, or PagerDuty, and it does not grow a kernel notifier.
    """
    _grant.pull(
        token=token,
        book_id=pack.book.book_id,
        transport=transport,
        error=Refuse,
    )
    return as_files(pack)


def dry_run(
    pack: AlertPack,
    *,
    token: str | None = None,
    transport: _grant.Transport | None = None,
) -> dict[str, str]:
    """Same door as deliver — a local cite pack, not a live destination."""
    return deliver(pack, token=token, transport=transport)


def subscribe(*_a: Any, **_k: Any) -> None:
    """Refuse. webhooks:journal is reserved; the kernel surface is not built."""
    raise Refuse(
        "webhooks:journal is a reserved catalog grant — the kernel "
        "webhook surface is not built. This app polls breaks:read + "
        "nav:read + views:read. It does not grow a kernel notifier "
        "and it does not close #162 by pretending a webhook landed"
    )


def kernel_notify(*_a: Any, **_k: Any) -> None:
    """Refuse. Ops notification is Connect breadth."""
    raise Refuse(
        "no kernel notification service — ops alerts live in this "
        "Connect app. They do not live inside ratio watch"
    )


def chatbot(*_a: Any, **_k: Any) -> None:
    raise Refuse(
        "no chatbot — ops notification is Connect breadth, not a "
        "kernel conversation"
    )


def html_alerts(*_a: Any, **_k: Any) -> None:
    raise Refuse(
        "no HTML alert UI inside ratio watch or the console binary — "
        "screensFor is not forked. This app is the door"
    )


def explain_break(*_a: Any, **_k: Any) -> None:
    raise Refuse(
        "this app does not invent a BreakExplanation — the explainer "
        "is a person. breaks:explain stays off the grant"
    )


def rewrite_strike(*_a: Any, **_k: Any) -> None:
    raise Refuse(
        "this app does not rewrite a NavStrike — WashRestatement "
        "cites the strike, it does not move net_asset_value"
    )


def slack(*_a: Any, **_k: Any) -> None:
    raise Refuse(
        "Slack product destination is leftover — dry_run writes a "
        "local cite pack. Do not pretend delivery succeeded without "
        "a configured destination"
    )


def email(*_a: Any, **_k: Any) -> None:
    raise Refuse(
        "email product destination is leftover — dry_run writes a "
        "local cite pack. Do not pretend delivery succeeded without "
        "a configured destination"
    )


def pagerduty(*_a: Any, **_k: Any) -> None:
    raise Refuse(
        "PagerDuty product destination is leftover — dry_run writes a "
        "local cite pack. Do not pretend delivery succeeded without "
        "a configured destination"
    )


def _csv(columns: Sequence[str], rows: Sequence[Sequence[str]]) -> str:
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(columns)
    for row in rows:
        w.writerow(row)
    return buf.getvalue()


def csv_breaks(pack: AlertPack) -> str | None:
    if pack.breaks is None or len(pack.breaks) == 0:
        return None
    rows = []
    for b in pack.breaks:
        rows.append(
            [
                b.name,
                b.account,
                b.severity,
                "true" if b.explained else "false",
                b.cause,
                "" if b.ratio_amount is None else format_minor(b.ratio_amount),
                "" if b.reported_amount is None else format_minor(b.reported_amount),
                "" if b.difference is None else format_minor(b.difference),
                b.config_digest or "",
            ]
        )
    return _csv(
        (
            "Name",
            "Account",
            "Severity",
            "Explained",
            "Cause",
            "Ratio amount",
            "Reported amount",
            "Difference",
            "Config digest",
        ),
        rows,
    )


def csv_nav_gate(pack: AlertPack) -> str | None:
    if pack.nav_gate is None:
        return None
    g = pack.nav_gate
    if (
        not g.unexplained_breaks
        and not g.unresolved_trades
        and not g.unpriced
        and g.valuation_date is None
    ):
        # Cited-empty gate without as-of lives in manifest + unset.csv.
        # Emitting a sheet that looks all-clear-and-priced is the defect.
        return None
    rows: list[tuple[str, str, str]] = []
    for line in g.unexplained_breaks:
        rows.append(("unexplained_break", line, "same blocking_at fold as the badge"))
    for line in g.unresolved_trades:
        rows.append(("unresolved_trade", line, "pending fact that does not resolve"))
    if g.valuation_date is not None:
        if g.unpriced:
            for line in g.unpriced:
                rows.append(
                    (
                        "unpriced",
                        line,
                        f"no price on or before {g.valuation_date.isoformat()}",
                    )
                )
        else:
            rows.append(
                (
                    "unpriced",
                    "",
                    f"cited-empty on {g.valuation_date.isoformat()} — every position priced",
                )
            )
    if not rows:
        return None
    return _csv(("Reason", "Cite", "Note"), rows)


def csv_strike(pack: AlertPack) -> str | None:
    if pack.strike is None or pack.strike.net_asset_value is None:
        return None
    if pack.strike.journal_digest is None:
        # A strike without a digest is named in unset.csv. Emitting a
        # strike.csv that looks pinned would be empty-digest-as-success.
        return None
    s = pack.strike
    return _csv(
        ("Field", "Value", "Note"),
        (
            ("name", s.name, "NavStrike; this app does not rewrite it"),
            ("view", s.view, "views:read"),
            (
                "net_asset_value",
                format_minor(s.net_asset_value),
                "a missing strike is unset, not NAV 0.00",
            ),
            (
                "journal_digest",
                s.journal_digest,
                "empty is unset, not history-intact",
            ),
            (
                "journal_position",
                "" if s.journal_position is None else str(s.journal_position),
                "the pin",
            ),
            (
                "config_digest",
                s.config_digest or "",
                "empty is unset, not a pin",
            ),
        ),
    )


def csv_unset(pack: AlertPack) -> str:
    """Named missing cites. Silence on a sheet is the honesty, not a zero."""
    return _csv(("Unset",), [(line,) for line in pack.unset])


def manifest_json(pack: AlertPack) -> str:
    return json.dumps(pack.manifest, indent=2, sort_keys=True) + "\n"


def as_files(pack: AlertPack) -> dict[str, str]:
    """Named sheets that have a cite. Missing cites are not empty files.

    ⛔ A SILENT EMPTY breaks.csv LOOKS RECONCILED. Unset and
    cited-empty stay in manifest.json + unset.csv only. deliver()
    still refuses a live destination — this dict is the dry-run
    pack, not a Slack post.
    """
    files: dict[str, str] = {
        "manifest.json": manifest_json(pack),
        "unset.csv": csv_unset(pack),
    }
    breaks = csv_breaks(pack)
    if breaks is not None:
        files["breaks.csv"] = breaks
    gate = csv_nav_gate(pack)
    if gate is not None:
        files["nav_gate.csv"] = gate
    strike = csv_strike(pack)
    if strike is not None:
        files["strike.csv"] = strike
    return files


def as_json(pack: AlertPack) -> dict[str, Any]:
    """JSON cite. Missing figures stay null, never 0.00."""
    return {
        "book": pack.book.book_id,
        "kind": pack.book.kind,
        "issue": 162,
        "breaks": (
            None
            if pack.breaks is None
            else [
                {
                    "name": b.name,
                    "account": b.account,
                    "severity": b.severity,
                    "explained": b.explained,
                    "cause": b.cause,
                    "ratio_amount": (
                        None if b.ratio_amount is None else format_minor(b.ratio_amount)
                    ),
                    "difference": (
                        None if b.difference is None else format_minor(b.difference)
                    ),
                }
                for b in pack.breaks
            ]
        ),
        "unexplained": [
            {"name": b.name, "account": b.account, "cause": b.cause}
            for b in unexplained_breaks(pack.breaks)
        ],
        "nav_gate": (
            None
            if pack.nav_gate is None
            else {
                "unexplained_breaks": list(pack.nav_gate.unexplained_breaks),
                "unresolved_trades": list(pack.nav_gate.unresolved_trades),
                "unpriced": list(pack.nav_gate.unpriced),
                "valuation_date": (
                    pack.nav_gate.valuation_date.isoformat()
                    if pack.nav_gate.valuation_date
                    else None
                ),
            }
        ),
        "strike": (
            None
            if pack.strike is None
            else {
                "name": pack.strike.name,
                "net_asset_value": (
                    None
                    if pack.strike.net_asset_value is None
                    else format_minor(pack.strike.net_asset_value)
                ),
                "journal_digest": pack.strike.journal_digest,
            }
        ),
        "unset": list(pack.unset),
        "sections": {
            name: {"status": s.status, "note": s.note, "rows": s.rows}
            for name, s in pack.sections.items()
        },
    }
