#!/usr/bin/env python3
"""Audit evidence packer — a WorkOS Connect app, not a kernel RPC.

Period closes, NAV strikes, break reports / explanations, and
config / journal digests live **here** as a read-only ZIP of cites.
They do not live in `ratio watch`, the operations console, a new
kernel blob store, or a replacement for `ratio close`.

⭐ SCOPES ARE THE FROZEN CATALOG NAMES. `audit:export` plus the
read scopes this pack actually cites: `closes:read`, `breaks:read`,
`breaks:explain`, `nav:read`, `journals:read`, `config:read`,
`books:read`. Aliases (`journal:read`, `journal:append`) and
invented strings are refused. See docs/connect-scopes.md.

⭐ THIS APP IS READ-ONLY RELATIVE TO THE JOURNAL. It does not
request `journals:post`. An empty `journals:post` allowlist refuses
every post. A pack is a read of cites, not a rewrite.

⭐ UNSET STAYS UNSET. A missing cite is named in the pack manifest,
not a silent empty file that looks complete. An empty journal
digest is unset, not history-intact and not reproduced. A missing
NAV strike is unset, not NAV 0.00. A missing BreakReport is unset,
not a silent reconciled-empty ZIP entry. A cited report with no
lines is cited-empty — that is the kernel's "the period
reconciled", and the manifest says so. A posted `"0.00"` is a
figure.

⭐ MONEY IS MINOR UNITS, PARSED BY SPLITTING ON THE POINT. A float
is how a cent disappears. A third decimal place is refused.

⭐ NO NEW METHOD / ORDER / LOT_METHOD VARIANT. MinTax, SpecID,
average cost, and wash stay elections. This pack copies a
config digest / RuleSet pin; it does not invent one.

⭐ KIND-AWARE CITES, NOT A CHROME FORK. Closes and digests apply
to every kind. NAV strikes and breaks stay unset on kinds that
do not wear fund-ops. `screensFor` is not forked. UNSPECIFIED
is the proto default, not a fifth kind.

⚠ THE GRANT PATH IS NOT BUILT. `fetch_cites` and `deliver` refuse.
A Connect access token is not accepted on `/v1` (leftover #22 /
#150). A green cite is not a live token.

⚠ NO BLOB STORE, NO PERIOD-CLOSE REPLACEMENT, NO LP PORTAL, NO
E-SIGN, NO SECOND JOURNAL. `store_blob`, `close_period`,
`lp_portal`, `esign`, and `second_journal` refuse.
"""

from __future__ import annotations

import csv
import io
import json
import zipfile
from dataclasses import dataclass, field
from datetime import date
from typing import Any, Mapping, Sequence

# i64 bounds. Lean's Int is unbounded; every money figure here is i64.
I64_MIN = -(2**63)
I64_MAX = 2**63 - 1

CANONICAL_SCOPES = frozenset(
    {
        "audit:export",
        "closes:read",
        "breaks:read",
        "breaks:explain",
        "nav:read",
        "journals:read",
        "config:read",
        "books:read",
    }
)
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

# RuleSet fields this pack may name on a config pin. Names must
# stay the ones crates/ratio-rules already stores — a silent
# rename is how a pack invents an election.
CONFIG_PIN_FIELDS = (
    "lot_method",
    "long_term_days",
    "wash_window_days",
    "wash_keep_holding_period",
    "min_tax_short_weight",
    "average_cost",
)

# Wire / proto field names this pack copies. A rename here that
# the proto does not share is how a pack invents a cite.
CLOSE_PROTO_FIELDS = (
    "closed_date",
    "journal_position",
    "journal_digest",
    "config_digest",
    "closing_entry",
    "equity_destination",
    "surplus",
)
STRIKE_PROTO_FIELDS = (
    "valuation_time",
    "journal_position",
    "journal_digest",
    "net_asset_value",
    "trial_balance_difference",
    "config_digest",
    "qualification",
    "wash_qualified",
    "wash_restatement_original",
    "wash_restatement_moved",
)
BREAK_PROTO_FIELDS = (
    "ratio_amount",
    "reported_amount",
    "difference",
    "config_digest",
    "explained",
    "explanation",
)
EXPLANATION_PROTO_FIELDS = (
    "text",
    "actor",
    "accept_time",
    "difference",
    "config_digest",
    "journal_position",
    "journal_digest",
)


class Refuse(Exception):
    """The pack is not emitted. Message is the reason, not a workaround."""


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
class JournalCite:
    """`journals:read` prefix. Empty digest is unset, not success."""

    position: int | None = None
    digest: str | None = None


@dataclass(frozen=True)
class ConfigCite:
    """`config:read` RuleSet pin. Digest empty is unset, not a pin."""

    digest: str | None = None
    lot_method: str | None = None
    long_term_days: int | None = None
    wash_window_days: int | None = None
    wash_keep_holding_period: bool | None = None
    min_tax_short_weight: int | None = None
    average_cost: bool | None = None


@dataclass(frozen=True)
class CloseCite:
    """One `closes:read` PeriodClose. Surplus empty is unset."""

    name: str
    view: str
    closed_date: date | None = None
    actor: str = ""
    journal_position: int | None = None
    journal_digest: str | None = None
    config_digest: str | None = None
    closing_entry: str | None = None
    equity_destination: str | None = None
    surplus: int | None = None


@dataclass(frozen=True)
class StrikeCite:
    """One `nav:read` NavStrike. NAV empty is unset, not 0.00."""

    name: str
    view: str
    valuation_time: str | None = None
    actor: str = ""
    journal_position: int | None = None
    journal_digest: str | None = None
    net_asset_value: int | None = None
    trial_balance_difference: int | None = None
    config_digest: str | None = None
    qualification: tuple[str, ...] = ()
    wash_qualified: bool = False
    wash_restatement_original: int | None = None
    wash_restatement_moved: int | None = None


@dataclass(frozen=True)
class ExplanationCite:
    """One `breaks:explain` BreakExplanation. Copied, not invented."""

    text: str
    actor: str
    accept_time: str | None = None
    difference: int | None = None
    config_digest: str | None = None
    journal_position: int | None = None
    journal_digest: str | None = None
    qualification: tuple[str, ...] = ()


@dataclass(frozen=True)
class BreakCite:
    """One `breaks:read` Break. Explanation None is unset."""

    name: str
    account: str
    severity: str = ""
    explained: bool = False
    cause: str = ""
    ratio_amount: int | None = None
    reported_amount: int | None = None
    difference: int | None = None
    config_digest: str | None = None
    explanation: ExplanationCite | None = None


@dataclass(frozen=True)
class Section:
    """One pack section. `cited-empty` is a real empty, not unset."""

    status: str
    note: str
    rows: int = 0


@dataclass(frozen=True)
class Pack:
    """An evidence ZIP of cites. Not a live /v1 delivery."""

    book: Book
    journal: JournalCite | None
    config: ConfigCite | None
    closes: tuple[CloseCite, ...] | None
    strikes: tuple[StrikeCite, ...] | None
    breaks: tuple[BreakCite, ...] | None
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
            or "ratio-audit-export"
        ),
        scopes=frozenset(scopes),
    )


def parse_minor(text: str, *, allow_signed: bool = False) -> int:
    """Split on the point. Never parse a float.

    Same door as `ratio_common::parse_minor`. A third decimal place is
    refused rather than dropped. Overflow is refused rather than wrapped.
    Surplus, trial-balance difference, and break difference may be
    signed; inventing a sign is how a loss and a gain swap.
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


def parse_day(text: str | None) -> date:
    if not isinstance(text, str) or not text.strip():
        raise Refuse("a close names a calendar day YYYY-MM-DD")
    t = text.strip()
    try:
        y, m, d = t.split("-")
        return date(int(y), int(m), int(d))
    except ValueError as e:
        raise Refuse(f"{text!r} is not a calendar day YYYY-MM-DD") from e


def parse_optional_day(text: Any) -> date | None:
    if text is None:
        return None
    if isinstance(text, Mapping):
        # proto Date { year, month, day } as already exposed on PeriodClose.
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
    return parse_day(text)


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
    missing = CANONICAL_SCOPES - client.scopes
    if missing:
        raise Refuse(
            "this app needs "
            + ", ".join(sorted(CANONICAL_SCOPES))
            + f"; missing {', '.join(sorted(missing))}. "
            "audit:export is the pack; closes:read / nav:read / "
            "breaks:read / breaks:explain / journals:read / config:read "
            "/ books:read are the cites"
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


def _require_config(config: ConfigCite | None) -> None:
    if config is None:
        return
    if config.lot_method is not None:
        method = config.lot_method.strip().lower()
        if method in REFUSED_LOT_METHODS:
            raise Refuse(
                f'lot_method = "{config.lot_method}" stays refused — '
                "MinTax, SpecID, average cost, and wash are elections, "
                "not a Method, an Order, or a lot_method variant"
            )
    if config.average_cost is False:
        raise Refuse(
            "average_cost = false is not an election — omit the field. "
            "None is unset, not a silent true"
        )
    if config.wash_keep_holding_period is False:
        raise Refuse(
            "wash_keep_holding_period = false is not an election — omit "
            "the field. None is unset, not a silent keep"
        )


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


def _digest_unset(label: str, digest: str | None, unset: list[str]) -> None:
    if digest is None:
        unset.append(
            f"{label}: journal/config digest is unset — empty is not "
            "history-intact and not reproduced"
        )


def close_from_cite(row: Mapping[str, Any]) -> CloseCite:
    name = str(row.get("name") or "").strip()
    if not name:
        raise Refuse("a PeriodClose names itself")
    view = str(row.get("view") or "").strip()
    closed = parse_optional_day(row.get("closed_date") or row.get("closedDate"))
    return CloseCite(
        name=name,
        view=view,
        closed_date=closed,
        actor=str(row.get("actor") or "").strip(),
        journal_position=parse_optional_int(
            row.get("journal_position", row.get("journalPosition"))
        ),
        journal_digest=parse_optional_digest(
            row.get("journal_digest", row.get("journalDigest"))
        ),
        config_digest=parse_optional_digest(
            row.get("config_digest", row.get("configDigest"))
        ),
        closing_entry=(
            str(row.get("closing_entry") or row.get("closingEntry") or "").strip()
            or None
        ),
        equity_destination=(
            str(row.get("equity_destination") or row.get("equityDestination") or "").strip()
            or None
        ),
        surplus=parse_optional_minor(
            row.get("surplus"), allow_signed=True
        ),
    )


def strike_from_cite(row: Mapping[str, Any]) -> StrikeCite:
    name = str(row.get("name") or "").strip()
    if not name:
        raise Refuse("a NavStrike names itself")
    view = str(row.get("view") or "").strip()
    quals = row.get("qualification") or ()
    if isinstance(quals, str):
        quals = (quals,) if quals.strip() else ()
    return StrikeCite(
        name=name,
        view=view,
        valuation_time=(
            str(row.get("valuation_time") or row.get("valuationTime") or "").strip()
            or None
        ),
        actor=str(row.get("actor") or "").strip(),
        journal_position=parse_optional_int(
            row.get("journal_position", row.get("journalPosition"))
        ),
        journal_digest=parse_optional_digest(
            row.get("journal_digest", row.get("journalDigest"))
        ),
        net_asset_value=parse_optional_minor(
            row.get("net_asset_value", row.get("netAssetValue")),
            allow_signed=True,
        ),
        trial_balance_difference=parse_optional_minor(
            row.get(
                "trial_balance_difference",
                row.get("trialBalanceDifference"),
            ),
            allow_signed=True,
        ),
        config_digest=parse_optional_digest(
            row.get("config_digest", row.get("configDigest"))
        ),
        qualification=tuple(str(q) for q in quals),
        wash_qualified=bool(row.get("wash_qualified", row.get("washQualified", False))),
        wash_restatement_original=parse_optional_minor(
            row.get(
                "wash_restatement_original",
                row.get("washRestatementOriginal"),
            ),
            allow_signed=True,
        ),
        wash_restatement_moved=parse_optional_minor(
            row.get(
                "wash_restatement_moved",
                row.get("washRestatementMoved"),
            ),
            allow_signed=True,
        ),
    )


def explanation_from_cite(raw: Any) -> ExplanationCite | None:
    if raw is None:
        return None
    if not isinstance(raw, Mapping):
        raise Refuse("a BreakExplanation is a record, not invented text")
    text = str(raw.get("text") or "").strip()
    actor = str(raw.get("actor") or "").strip()
    if not text or not actor:
        raise Refuse(
            "breaks:explain is a person-attributed explanation — empty "
            "text or actor is unset, not a silent accept"
        )
    return ExplanationCite(
        text=text,
        actor=actor,
        accept_time=(
            str(raw.get("accept_time") or raw.get("acceptTime") or "").strip()
            or None
        ),
        difference=parse_optional_minor(raw.get("difference"), allow_signed=True),
        config_digest=parse_optional_digest(
            raw.get("config_digest", raw.get("configDigest"))
        ),
        journal_position=parse_optional_int(
            raw.get("journal_position", raw.get("journalPosition"))
        ),
        journal_digest=parse_optional_digest(
            raw.get("journal_digest", raw.get("journalDigest"))
        ),
        qualification=tuple(str(q) for q in (raw.get("qualification") or ())),
    )


def break_from_cite(row: Mapping[str, Any]) -> BreakCite:
    name = str(row.get("name") or "").strip()
    if not name:
        raise Refuse("a Break names itself")
    account = str(row.get("account") or "").strip()
    if not account:
        raise Refuse("a Break names an account")
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
        difference=parse_optional_minor(
            row.get("difference"), allow_signed=True
        ),
        config_digest=parse_optional_digest(
            row.get("config_digest", row.get("configDigest"))
        ),
        explanation=explanation_from_cite(row.get("explanation")),
    )


def journal_from_cite(raw: Mapping[str, Any] | None) -> JournalCite | None:
    if raw is None:
        return None
    return JournalCite(
        position=parse_optional_int(raw.get("position", raw.get("journal_position"))),
        digest=parse_optional_digest(raw.get("digest", raw.get("journal_digest"))),
    )


def config_from_cite(raw: Mapping[str, Any] | None) -> ConfigCite | None:
    if raw is None:
        return None

    def _opt_int(key: str) -> int | None:
        v = raw.get(key)
        if v is None or v == "":
            return None
        if isinstance(v, bool):
            raise Refuse(f"{key} is a number, not a flag")
        return int(v)

    def _opt_bool(key: str) -> bool | None:
        v = raw.get(key)
        if v is None or v == "":
            return None
        if not isinstance(v, bool):
            raise Refuse(
                f"{key} is a flag; omit the field rather than inventing "
                "a third meaning"
            )
        return v

    method = raw.get("lot_method")
    method_s = str(method).strip() if method not in (None, "") else None
    return ConfigCite(
        digest=parse_optional_digest(raw.get("digest", raw.get("config_digest"))),
        lot_method=method_s,
        long_term_days=_opt_int("long_term_days"),
        wash_window_days=_opt_int("wash_window_days"),
        wash_keep_holding_period=_opt_bool("wash_keep_holding_period"),
        min_tax_short_weight=_opt_int("min_tax_short_weight"),
        average_cost=_opt_bool("average_cost"),
    )


def build_pack(
    *,
    book: Book,
    client: Client,
    journal: JournalCite | None = None,
    config: ConfigCite | None = None,
    closes: Sequence[CloseCite] | None = None,
    strikes: Sequence[StrikeCite] | None = None,
    breaks: Sequence[BreakCite] | None = None,
) -> Pack:
    """Read kernel cites into an evidence pack.

    `None` on a sequence is unset (the cite was not handed over).
    An empty sequence is cited-empty — ListPeriodCloses / a
    BreakReport with no lines — and the manifest says so. A silent
    empty file that looks complete is the defect this function
    exists to refuse.
    """
    _require_scopes(client)
    _require_membership(book)
    _require_config(config)

    unset: list[str] = []
    sections: dict[str, Section] = {}

    if journal is None:
        sections["journal"] = Section(
            status="unset",
            note="no journals:read prefix — empty is not history-intact",
        )
        unset.append("journal: no journals:read cite")
    else:
        if journal.digest is None:
            sections["journal"] = Section(
                status="unset" if journal.position is None else "cited",
                note=(
                    "journals:read prefix without a digest — empty is "
                    "not history-intact and not reproduced"
                ),
                rows=0 if journal.position is None else 1,
            )
            unset.append("journal.digest: unset — empty is not success")
        else:
            sections["journal"] = Section(
                status="cited",
                note="journals:read prefix + digest",
                rows=1,
            )
        if journal.position is None:
            unset.append("journal.position: unset")

    if config is None:
        sections["config"] = Section(
            status="unset",
            note="no config:read pin — empty is not a RuleSet digest",
        )
        unset.append("config: no config:read cite")
    elif config.digest is None:
        sections["config"] = Section(
            status="unset",
            note="config:read without a digest — empty is not a pin",
        )
        unset.append("config.digest: unset — empty is not a pin")
    else:
        sections["config"] = Section(
            status="cited",
            note="config:read RuleSet pin",
            rows=1,
        )

    sections["closes"] = _section(
        cited=closes,
        name="period closes",
        cited_empty_note=(
            "ListPeriodCloses returned no records — not a silent "
            "closed period"
        ),
        unset_note="no closes:read cite — not a silent empty ZIP entry",
    )
    if closes is None:
        unset.append("closes: unset")
    elif len(closes) == 0:
        unset.append(
            "closes: cited-empty — no recorded PeriodClose, not a "
            "fake closed period"
        )
    else:
        for c in closes:
            _digest_unset(f"closes/{c.name}/journal_digest", c.journal_digest, unset)
            _digest_unset(f"closes/{c.name}/config_digest", c.config_digest, unset)
            if c.closed_date is None:
                unset.append(f"closes/{c.name}/closed_date: unset")
            if c.surplus is None:
                unset.append(
                    f"closes/{c.name}/surplus: unset — empty is not a "
                    "measured zero (Ratio.Close.missing_surplus_is_unset)"
                )

    sections["strikes"] = _section(
        cited=strikes,
        name="NAV strikes",
        cited_empty_note=(
            "ListNavStrikes returned no records — not NAV 0.00"
        ),
        unset_note="no nav:read cite — not a silent NAV of 0.00",
    )
    if strikes is None:
        unset.append("strikes: unset")
    elif len(strikes) == 0:
        unset.append("strikes: cited-empty — no recorded NavStrike, not NAV 0.00")
    else:
        for s in strikes:
            _digest_unset(f"strikes/{s.name}/journal_digest", s.journal_digest, unset)
            _digest_unset(f"strikes/{s.name}/config_digest", s.config_digest, unset)
            if s.net_asset_value is None:
                unset.append(
                    f"strikes/{s.name}/net_asset_value: unset — not NAV 0.00"
                )

    sections["breaks"] = _section(
        cited=breaks,
        name="breaks",
        cited_empty_note=(
            "BreakReport cited with no lines — the period reconciled. "
            "That is not a missing report"
        ),
        unset_note=(
            "no breaks:read cite — not a silent reconciled-empty file"
        ),
    )
    if breaks is None:
        unset.append("breaks: unset")
    elif len(breaks) == 0:
        unset.append(
            "breaks: cited-empty — BreakReport with no lines means the "
            "period reconciled, and the manifest says so"
        )
    else:
        for b in breaks:
            _digest_unset(f"breaks/{b.name}/config_digest", b.config_digest, unset)
            if b.explanation is None:
                unset.append(
                    f"breaks/{b.name}/explanation: unset — "
                    "breaks:explain does not invent a person"
                )
            else:
                _digest_unset(
                    f"breaks/{b.name}/explanation/journal_digest",
                    b.explanation.journal_digest,
                    unset,
                )

    manifest = {
        "book": book.book_id,
        "kind": book.kind,
        "issue": 185,
        "grant_path": "not built — leftover #22 / #150",
        "note": (
            "A green cite is not a live Connect token. Missing cites "
            "stay unset here; they are not silent empty files."
        ),
        "sections": {
            name: {"status": s.status, "note": s.note, "rows": s.rows}
            for name, s in sections.items()
        },
        "unset": list(unset),
    }

    return Pack(
        book=book,
        journal=journal,
        config=config,
        closes=tuple(closes) if closes is not None else None,
        strikes=tuple(strikes) if strikes is not None else None,
        breaks=tuple(breaks) if breaks is not None else None,
        sections=sections,
        unset=tuple(unset),
        manifest=manifest,
    )


def fetch_cites(*, token: str | None = None) -> None:
    """Refuse to pull. The grant path is not built.

    A green pack builder is not a door that opens. Connect access
    tokens are not accepted on /v1.
    """
    _ = token
    raise Refuse(
        "Connect access tokens are not accepted on /v1 — the grant path "
        "is not built (leftover #22 / #150). This app does not "
        "pretend the door opens. A green cite is not a live token"
    )


def deliver(pack: Pack, *, token: str | None = None) -> None:
    """Refuse to push a live ZIP against /v1. Same leftover as fetch_cites."""
    _ = pack
    _ = token
    raise Refuse(
        "Connect access tokens are not accepted on /v1 — the grant path "
        "is not built (leftover #22 / #150). This app does not "
        "deliver a ZIP against a door that is not open"
    )


def store_blob(*_a: Any, **_k: Any) -> None:
    """Refuse. Evidence is a ZIP of cites, not a kernel blob store."""
    raise Refuse(
        "no kernel blob store — the evidence trail is a Connect pack of "
        "cites, not a second store beside the journal. This does not "
        "close #185 by pretending a blob landed in core"
    )


def close_period(*_a: Any, **_k: Any) -> None:
    """Refuse. Period close stays a person at a terminal."""
    raise Refuse(
        "this app does not replace period close — closes:read cites "
        "ListPeriodCloses / GetPeriodClose. ratio close stays a person "
        "at a terminal"
    )


def lp_portal(*_a: Any, **_k: Any) -> None:
    raise Refuse(
        "no LP portal — that door is #161. This app cites evidence; "
        "it does not grow a client portal"
    )


def esign(*_a: Any, **_k: Any) -> None:
    raise Refuse(
        "no e-sign — e-signature stays Connect leftover, never a "
        "kernel RPC. This file does not start it"
    )


def second_journal(*_a: Any, **_k: Any) -> None:
    raise Refuse(
        "no second journal — a pack is a read of cites, not a rewrite "
        "and not a shadow book"
    )


def _csv(columns: Sequence[str], rows: Sequence[Sequence[str]]) -> str:
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(columns)
    for row in rows:
        w.writerow(row)
    return buf.getvalue()


def csv_closes(pack: Pack) -> str | None:
    if pack.closes is None or len(pack.closes) == 0:
        return None
    rows = []
    for c in pack.closes:
        rows.append(
            [
                c.name,
                c.view,
                c.closed_date.isoformat() if c.closed_date else "",
                c.actor,
                "" if c.journal_position is None else str(c.journal_position),
                c.journal_digest or "",
                c.config_digest or "",
                c.closing_entry or "",
                c.equity_destination or "",
                "" if c.surplus is None else format_minor(c.surplus),
            ]
        )
    return _csv(
        (
            "Name",
            "View",
            "Closed date",
            "Actor",
            "Journal position",
            "Journal digest",
            "Config digest",
            "Closing entry",
            "Equity destination",
            "Surplus",
        ),
        rows,
    )


def csv_strikes(pack: Pack) -> str | None:
    if pack.strikes is None or len(pack.strikes) == 0:
        return None
    rows = []
    for s in pack.strikes:
        rows.append(
            [
                s.name,
                s.view,
                s.valuation_time or "",
                s.actor,
                "" if s.journal_position is None else str(s.journal_position),
                s.journal_digest or "",
                s.config_digest or "",
                "" if s.net_asset_value is None else format_minor(s.net_asset_value),
                (
                    ""
                    if s.trial_balance_difference is None
                    else format_minor(s.trial_balance_difference)
                ),
                "; ".join(s.qualification),
                "true" if s.wash_qualified else "false",
                (
                    ""
                    if s.wash_restatement_original is None
                    else format_minor(s.wash_restatement_original)
                ),
                (
                    ""
                    if s.wash_restatement_moved is None
                    else format_minor(s.wash_restatement_moved)
                ),
            ]
        )
    return _csv(
        (
            "Name",
            "View",
            "Valuation time",
            "Actor",
            "Journal position",
            "Journal digest",
            "Config digest",
            "Net asset value",
            "Trial balance difference",
            "Qualification",
            "Wash qualified",
            "Wash restatement original",
            "Wash restatement moved",
        ),
        rows,
    )


def csv_breaks(pack: Pack) -> str | None:
    if pack.breaks is None or len(pack.breaks) == 0:
        return None
    rows = []
    for b in pack.breaks:
        exp = b.explanation
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
                exp.actor if exp is not None else "",
                exp.text if exp is not None else "",
                (exp.accept_time or "") if exp is not None else "",
                (
                    ""
                    if exp is None or exp.difference is None
                    else format_minor(exp.difference)
                ),
                (exp.config_digest or "") if exp is not None else "",
                (
                    ""
                    if exp is None or exp.journal_position is None
                    else str(exp.journal_position)
                ),
                (exp.journal_digest or "") if exp is not None else "",
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
            "Explanation actor",
            "Explanation text",
            "Explanation accept time",
            "Explanation difference",
            "Explanation config digest",
            "Explanation journal position",
            "Explanation journal digest",
        ),
        rows,
    )


def csv_config(pack: Pack) -> str | None:
    if pack.config is None or pack.config.digest is None:
        return None
    t = pack.config

    def val(v: Any) -> str:
        if v is None:
            return ""
        if isinstance(v, bool):
            return "true" if v else "false"
        return str(v)

    rows = (
        ("config_digest", t.digest or "", "empty is unset, not a pin"),
        (
            "lot_method",
            val(t.lot_method),
            "not min_tax / specific_id / average_cost / wash",
        ),
        ("long_term_days", val(t.long_term_days), "unset stays unset"),
        (
            "wash_window_days",
            val(t.wash_window_days),
            "unset stays unset, not a silent 30",
        ),
        (
            "wash_keep_holding_period",
            val(t.wash_keep_holding_period),
            "unset stays unset, not a silent keep",
        ),
        (
            "min_tax_short_weight",
            val(t.min_tax_short_weight),
            "unset stays unset, not a silent 2",
        ),
        (
            "average_cost",
            val(t.average_cost),
            "unset stays unset, not a silent true; not a lot_method",
        ),
    )
    return _csv(("Field", "Value", "Note"), rows)


def csv_journal(pack: Pack) -> str | None:
    if pack.journal is None:
        return None
    if pack.journal.digest is None and pack.journal.position is None:
        return None
    if pack.journal.digest is None:
        # A prefix without a digest is named in unset.csv. Emitting a
        # journal.csv that looks pinned would be empty-digest-as-success.
        return None
    return _csv(
        ("Field", "Value", "Note"),
        (
            (
                "journal_position",
                "" if pack.journal.position is None else str(pack.journal.position),
                "the pin — journals:read prefix",
            ),
            (
                "journal_digest",
                pack.journal.digest,
                "SHA-256 of exactly those entries. Empty is unset, not success",
            ),
        ),
    )


def csv_unset(pack: Pack) -> str:
    """Named missing cites. Silence on a sheet is the honesty, not a zero."""
    return _csv(("Unset",), [(line,) for line in pack.unset])


def manifest_json(pack: Pack) -> str:
    return json.dumps(pack.manifest, indent=2, sort_keys=True) + "\n"


def as_files(pack: Pack) -> dict[str, str]:
    """Named sheets that have a cite. Missing cites are not empty files.

    ⛔ A SILENT EMPTY closes.csv / strikes.csv / breaks.csv LOOKS
    COMPLETE. Unset and cited-empty stay in manifest.json + unset.csv
    only. deliver() still refuses — this dict is the pack shape, not
    a live ZIP against /v1.
    """
    files: dict[str, str] = {
        "manifest.json": manifest_json(pack),
        "unset.csv": csv_unset(pack),
    }
    journal = csv_journal(pack)
    if journal is not None:
        files["journal.csv"] = journal
    config = csv_config(pack)
    if config is not None:
        files["config.csv"] = config
    closes = csv_closes(pack)
    if closes is not None:
        files["closes.csv"] = closes
    strikes = csv_strikes(pack)
    if strikes is not None:
        files["strikes.csv"] = strikes
    breaks = csv_breaks(pack)
    if breaks is not None:
        files["breaks.csv"] = breaks
    return files


def as_zip(pack: Pack) -> bytes:
    """ZIP of as_files(). Not a delivery. deliver() still refuses."""
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for name, body in as_files(pack).items():
            zf.writestr(name, body)
    return buf.getvalue()
