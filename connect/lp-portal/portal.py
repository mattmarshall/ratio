#!/usr/bin/env python3
"""LP / investor portal for BookKind INVESTMENT.

A WorkOS Connect app, not a kernel RPC. Partner capital, statement,
and NAV reads live **here**. They do not live in `ratio watch`, the
operations console, or a new kernel method. `/capital` and `/nav`
stay core. This app cites those figures; it does not grow them.

⭐ SCOPES ARE THE FROZEN CATALOG NAMES. Required: `partners:read`,
`statements:read`, `nav:read`. Optional: `books:read`. Aliases
(`journal:append`, `journal:read`) and invented strings are refused.
See docs/connect-scopes.md.

⭐ THIS APP IS READ-ONLY RELATIVE TO THE JOURNAL. It does not
request `journals:post`. An empty `journals:post` allowlist refuses
every post. A portal is a read of cites, not a rewrite.

⭐ UNSET STAYS UNSET. A missing partner cut is not a silent 1/N of
book NAV. A book that never posted a commitment is not a callable
zero. A missing NAV strike is unset, not NAV 0.00. An empty journal
digest is unset, not history-intact. Activity-shaped beginning is
unset, not a fake zero stock. A posted `"0.00"` is a figure.

⭐ MONEY IS MINOR UNITS, PARSED BY SPLITTING ON THE POINT. A float
is how a cent disappears. A third decimal place is refused.

⭐ NO IRR, TVPI, OR WATERFALL. `partners:read` is partner master,
capital, and commitments — not a return and not preferred-return
math. PLAN already named those as cannot-show.

⭐ NO DRIP. A drip is `distribute_*` then `subscribe_*` plus an LP
election (#161 / #177). This app does not package that workflow
and does not mint an election.

⭐ NO KERNEL PORTAL. HTML routes inside `ratio watch`, LP user
tables, and a document vault stay refused. Membership is the
AuthKit `sub` on the book. An `org_id` claim is not membership.

⭐ THE GRANT PATH CALLS CONNECTAPIURL. `fetch_cites` and `deliver`
present a verified Connect access token against the Connect HTTP
API. Membership is still required. A Connect token never takes
`RATIO_DEMO_OPEN` and never matches `org:{id}`. WorkOS dashboard
registration stays leftover #22. A green cite is not a live
walk-through.
"""

from __future__ import annotations

import csv
import io
import json
from dataclasses import dataclass
from datetime import date
from typing import Any, Mapping, Sequence

import grant as _grant

# i64 bounds. Lean's Int is unbounded; every money figure here is i64.
I64_MIN = -(2**63)
I64_MAX = 2**63 - 1

REQUIRED_SCOPES = frozenset({"partners:read", "statements:read", "nav:read"})
OPTIONAL_SCOPES = frozenset({"books:read"})
CANONICAL_SCOPES = REQUIRED_SCOPES | OPTIONAL_SCOPES
REFUSED_ALIASES = frozenset({"journal:append", "journal:read"})

# Wire / proto field names this portal copies. A rename here that
# the proto does not share is how a portal invents a cite.
PARTNER_PROTO_FIELDS = (
    "partner_cut",
    "special_allocations",
    "allocation_facts",
    "notices",
)
NOTICE_PROTO_FIELDS = (
    "digest",
    "kind",
    "amount",
    "partner_cut",
    "amounts",
    "entry_id",
    "trade_date",
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
    """The cite is not proposed. Message is the reason, not a workaround."""


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
    closed_through: date | None = None


@dataclass(frozen=True)
class PartnerShare:
    """One named weight. The total is the sum, not 100 and not the count."""

    partner: str
    weight: int


@dataclass(frozen=True)
class PartnerCite:
    """One partner's `/capital` row. Empty amounts are unset."""

    grain: str
    beginning: int | None = None
    contributions: int | None = None
    distributions: int | None = None
    allocated_income: int | None = None
    allocated_expense: int | None = None
    unrealized: int | None = None
    ending: int | None = None
    units: int | None = None


@dataclass(frozen=True)
class NoticeCite:
    """One `CapitalNotice`. Empty digest is unset, not a silent notice."""

    kind: str
    amount: int | None = None
    digest: str | None = None
    amounts: tuple[tuple[str, int], ...] = ()
    entry_id: str = ""


@dataclass(frozen=True)
class NavCite:
    """`nav:read` strike + roll-forward. Missing NAV is unset, not 0.00."""

    net_asset_value: int | None = None
    journal_digest: str | None = None
    journal_position: int | None = None
    config_digest: str | None = None
    beginning: int | None = None
    contributions: int | None = None
    distributions: int | None = None
    income: int | None = None
    expense: int | None = None
    unrealized: int | None = None
    ending: int | None = None


@dataclass(frozen=True)
class Statement:
    """LP-facing capital / statement / NAV cite.

    Unset stays unset — not a silent 1/N, a callable-zero commitment,
    or a NAV 0.00 that looks like a strike.
    """

    partners: tuple[PartnerCite, ...]
    remaining_commitment: int | None
    remaining_undrawn: int | None
    nav: NavCite
    notices: tuple[NoticeCite, ...]
    closed_through: date | None
    currency: str
    unset: tuple[str, ...]


def load_app(path: str) -> dict[str, Any]:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def client_from_app(app: Mapping[str, Any]) -> Client:
    allow = app.get("journals_post_allowlist") or {}
    scopes = app.get("workos_connect", {}).get("scopes") or []
    return Client(
        client_id=str(allow.get("client_id") or ""),
        scopes=frozenset(scopes),
    )


def parse_minor(text: str, *, allow_signed: bool = False, allow_zero: bool = True) -> int:
    """Split on the point. Never parse a float.

    Same door as `ratio_common::parse_minor`. A third decimal place is
    refused rather than dropped. Overflow is refused rather than wrapped.
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
                "signed-amount inference is how a contribution and a "
                "distribution swap"
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
    if v == 0 and not allow_zero:
        raise Refuse("a zero amount is not a posting")
    return v


def parse_optional_minor(text: Any, *, allow_signed: bool = False) -> int | None:
    """Empty / omitted is unset. `"0.00"` is a set figure of nothing."""
    if text is None:
        return None
    if isinstance(text, str) and not text.strip():
        return None
    return parse_minor(str(text), allow_signed=allow_signed)


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


def format_minor(n: int) -> str:
    """Decimal string — never a float, never scientific."""
    sign = "-" if n < 0 else ""
    n = abs(n)
    if n > I64_MAX:
        raise Refuse("amount does not fit in i64 minor units")
    whole, frac = divmod(n, 100)
    return f"{sign}{whole}.{frac:02d}"


def format_optional(n: int | None) -> str:
    return "" if n is None else format_minor(n)


def checked_add(a: int, b: int) -> int:
    if a > I64_MAX or a < I64_MIN or b > I64_MAX or b < I64_MIN:
        raise Refuse("addend does not fit in i64")
    total = a + b
    if total > I64_MAX or total < I64_MIN:
        raise Refuse("sum does not fit in i64 minor units")
    return total


def checked_mul(a: int, b: int) -> int:
    """Product, refused on i64 wrap. Asked before the product."""
    if a > I64_MAX or a < I64_MIN or b > I64_MAX or b < I64_MIN:
        raise Refuse("factor does not fit in i64")
    prod = a * b
    if prod > I64_MAX or prod < I64_MIN:
        raise Refuse("product does not fit in i64 minor units")
    return prod


def _refuse_aliases(client: Client) -> None:
    aliases = client.scopes & REFUSED_ALIASES
    if aliases:
        raise Refuse(
            "refused alias scope "
            + ", ".join(sorted(aliases))
            + " — catalogs use partners:read / statements:read / nav:read; "
            "journal append is journals:post, not journal:append"
        )
    extra = client.scopes - CANONICAL_SCOPES
    if extra:
        raise Refuse(
            "unknown scope "
            + ", ".join(sorted(extra))
            + " — a string that is not in docs/connect-scopes.md is refused"
        )


def _require_read_scopes(client: Client) -> None:
    _refuse_aliases(client)
    missing = REQUIRED_SCOPES - client.scopes
    if missing:
        raise Refuse(
            "this app needs "
            + ", ".join(sorted(REQUIRED_SCOPES))
            + f"; missing {', '.join(sorted(missing))}. "
            "partners:read is partner master / capital / commitments; "
            "statements:read is how closed-through is read; "
            "nav:read is the strike and the period roll-forward. "
            "books:read is optional membership listing"
        )


def _require_investment(book: Book) -> None:
    if book.kind != "INVESTMENT":
        raise Refuse(
            f"this app is BookKind INVESTMENT; {book.kind!r} keeps its own "
            "chrome and is not an LP / investor portal book"
        )
    if not book.member:
        raise Refuse(
            "membership is the AuthKit sub on the book — an org_id claim "
            "is not membership. Authorized-empty for a book the subject "
            "does not administer"
        )


def apply_cut(figure: int | None, cut: Sequence[PartnerShare] | None) -> dict[str, int] | None:
    """Apply a named cut to a book figure.

    ⛔ EMPTY IS UNSET, NOT 1/N. `Ratio.Partners.no_cut_is_unset`.
    A figure that will not divide returns None for every partner —
    a partial fill would look exact for the ones that happened to
    land. `Ratio.Partners.a_slice_is_exactly_pro_rata`.
    The product is checked before the remainder is asked.
    """
    if figure is None:
        return None
    if not cut:
        return None
    seen: set[str] = set()
    total = 0
    for s in cut:
        if s.weight <= 0:
            raise Refuse(
                f"partner_cut {s.partner!r} weight is {s.weight}, and a "
                "non-positive weight is not a weight"
            )
        if not s.partner or s.partner in seen:
            raise Refuse(
                "two rows for one partner are two answers under one name"
            )
        seen.add(s.partner)
        total = checked_add(total, s.weight)
    if total <= 0:
        raise Refuse("partner_cut weights sum to a non-positive total")
    out: dict[str, int] = {}
    for s in cut:
        prod = checked_mul(figure, s.weight)
        if prod % total != 0:
            return None
        out[s.partner] = prod // total
    return out


def equal_split(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. A silent 1/N of book NAV is the defect #180 already named."""
    raise Refuse(
        "equal-split of book NAV is refused — allocated plugs stay unset "
        "without a named [[partner_cut]]. Ratio.Partners.no_cut_is_unset. "
        "A silent 1/N is the defect #180 already refused"
    )


def partner_from_cite(raw: Mapping[str, Any]) -> PartnerCite:
    grain = str(raw.get("grain") or raw.get("partner") or "").strip()
    if not grain:
        raise Refuse(
            "a partner row without a grain is not a cite — the grain is "
            "the suffix on Partner capital (LP, GP), not a blank"
        )
    return PartnerCite(
        grain=grain,
        beginning=parse_optional_minor(raw.get("beginning"), allow_signed=True),
        contributions=parse_optional_minor(raw.get("contributions")),
        distributions=parse_optional_minor(raw.get("distributions")),
        allocated_income=parse_optional_minor(
            raw.get("allocated_income"), allow_signed=True
        ),
        allocated_expense=parse_optional_minor(
            raw.get("allocated_expense"), allow_signed=True
        ),
        unrealized=parse_optional_minor(raw.get("unrealized"), allow_signed=True),
        ending=parse_optional_minor(raw.get("ending"), allow_signed=True),
        units=parse_optional_int(raw.get("units")),
    )


def notice_from_cite(raw: Mapping[str, Any]) -> NoticeCite:
    kind = str(raw.get("kind") or "").strip().lower()
    if kind not in ("call", "distribution"):
        raise Refuse(
            f"{kind!r} is not a capital notice kind — catalogs cite call "
            "or distribution, not preferred, catch-up, or carry"
        )
    amounts: list[tuple[str, int]] = []
    for row in raw.get("amounts") or ():
        partner = str(row.get("partner") or "").strip()
        if not partner:
            raise Refuse("a notice amount without a partner invents a slice")
        amt = parse_optional_minor(row.get("amount"), allow_signed=True)
        if amt is None:
            raise Refuse(
                "a notice amount that is empty is unset for the whole "
                "notice — not a silent 1/N of the total"
            )
        amounts.append((partner, amt))
    return NoticeCite(
        kind=kind,
        amount=parse_optional_minor(raw.get("amount"), allow_signed=True),
        digest=parse_optional_digest(raw.get("digest")),
        amounts=tuple(amounts),
        entry_id=str(raw.get("entry_id") or ""),
    )


def nav_from_cite(raw: Mapping[str, Any] | None) -> NavCite:
    if raw is None:
        return NavCite()
    return NavCite(
        net_asset_value=parse_optional_minor(
            raw.get("net_asset_value"), allow_signed=True
        ),
        journal_digest=parse_optional_digest(raw.get("journal_digest")),
        journal_position=parse_optional_int(raw.get("journal_position")),
        config_digest=parse_optional_digest(raw.get("config_digest")),
        beginning=parse_optional_minor(raw.get("beginning"), allow_signed=True),
        contributions=parse_optional_minor(raw.get("contributions")),
        distributions=parse_optional_minor(raw.get("distributions")),
        income=parse_optional_minor(raw.get("income"), allow_signed=True),
        expense=parse_optional_minor(raw.get("expense"), allow_signed=True),
        unrealized=parse_optional_minor(raw.get("unrealized"), allow_signed=True),
        ending=parse_optional_minor(raw.get("ending"), allow_signed=True),
    )


def _named_unset(
    *,
    partners: Sequence[PartnerCite],
    remaining_commitment: int | None,
    remaining_undrawn: int | None,
    nav: NavCite,
    notices: Sequence[NoticeCite],
    closed_through: date | None,
) -> tuple[str, ...]:
    names: list[str] = []
    if not partners:
        names.append(
            "partners — no Partner capital row has posted; a missing "
            "partner is unset, not a silent 0.00 share"
        )
    for p in partners:
        if p.beginning is None:
            names.append(
                f"{p.grain} beginning — activity-shaped folds leave "
                "beginning unset, not a fake zero stock"
            )
        if p.contributions is None:
            names.append(
                f"{p.grain} contributions — an unposted partner is unset, "
                "not a silent inbound zero"
            )
        if p.distributions is None:
            names.append(
                f"{p.grain} distributions — an unposted partner is unset, "
                "not a silent outbound zero"
            )
        if p.allocated_income is None:
            names.append(
                f"{p.grain} allocated income — no named [[partner_cut]] "
                "is unset, not a silent 1/N of book NAV"
            )
        if p.allocated_expense is None:
            names.append(
                f"{p.grain} allocated expense — no named [[partner_cut]] "
                "is unset, not a silent 1/N"
            )
        if p.unrealized is None:
            names.append(
                f"{p.grain} unrealized — unset until the account moved "
                "and a named cut divides; a silent 0.00 mark is the defect"
            )
        if p.ending is None:
            names.append(
                f"{p.grain} ending — an unposted partner is unset, not "
                "ending-zero"
            )
        if p.units is None:
            names.append(
                f"{p.grain} units — no unit event has posted; a PE-style "
                "contribution is not a silent 0"
            )
    if remaining_commitment is None:
        names.append(
            "remaining commitment — a book that only contributed has no "
            "posted commitment, not a callable zero"
        )
    if remaining_undrawn is None:
        names.append(
            "remaining undrawn — no commitment or undrawn posting is "
            "unset, not a callable zero"
        )
    if nav.net_asset_value is None:
        names.append("NAV strike — a missing strike is unset, not NAV 0.00")
    if nav.journal_digest is None:
        names.append(
            "NAV journal digest — empty is unset, not history-intact"
        )
    if nav.beginning is None:
        names.append(
            "NAV beginning — a window with no dated prefix is unset, "
            "not a fake zero NAV"
        )
    if nav.ending is None:
        names.append(
            "NAV ending — nothing dated on or before the window end "
            "is unset, not a fake zero NAV"
        )
    if not notices:
        names.append(
            "capital notices — empty is unset, not a silent notice and "
            "not a waterfall"
        )
    if closed_through is None:
        names.append(
            "closed-through — statements:read leaves an open period "
            "unset, not a fake closed period"
        )
    return tuple(names)


def cite_statement(
    *,
    partners: Sequence[Mapping[str, Any] | PartnerCite] | None,
    nav: NavCite | Mapping[str, Any] | None,
    book: Book,
    client: Client,
    remaining_commitment: Any = None,
    remaining_undrawn: Any = None,
    notices: Sequence[Mapping[str, Any] | NoticeCite] | None = None,
    currency: str = "USD",
    partner_cut: Sequence[PartnerShare] | None = None,
    book_income: Any = None,
    book_expense: Any = None,
    book_unrealized: Any = None,
) -> Statement:
    """Compose LP-facing capital / statement / NAV from kernel cites.

    ⛔ NO FAKE ZEROS. The kernel already leaves allocated plugs, NAV,
    and undrawn empty until those accounts post and a named cut can
    support a slice. This app cites that cut; it does not fill a
    portal with invented 0.00 or a silent 1/N of book NAV.
    """
    _require_read_scopes(client)
    _require_investment(book)
    code = currency.strip().upper()
    if len(code) != 3 or not code.isalpha():
        raise Refuse(f"{currency!r} is not an ISO currency code")

    rows: list[PartnerCite] = []
    for raw in partners or ():
        rows.append(raw if isinstance(raw, PartnerCite) else partner_from_cite(raw))

    income_shares = apply_cut(parse_optional_minor(book_income, allow_signed=True), partner_cut)
    expense_shares = apply_cut(parse_optional_minor(book_expense, allow_signed=True), partner_cut)
    unreal_shares = apply_cut(parse_optional_minor(book_unrealized, allow_signed=True), partner_cut)
    filled: list[PartnerCite] = []
    for row in rows:
        filled.append(
            PartnerCite(
                grain=row.grain,
                beginning=row.beginning,
                contributions=row.contributions,
                distributions=row.distributions,
                allocated_income=(
                    row.allocated_income
                    if row.allocated_income is not None
                    else (income_shares or {}).get(row.grain)
                ),
                allocated_expense=(
                    row.allocated_expense
                    if row.allocated_expense is not None
                    else (expense_shares or {}).get(row.grain)
                ),
                unrealized=(
                    row.unrealized
                    if row.unrealized is not None
                    else (unreal_shares or {}).get(row.grain)
                ),
                ending=row.ending,
                units=row.units,
            )
        )

    notice_rows: list[NoticeCite] = []
    for raw in notices or ():
        notice_rows.append(raw if isinstance(raw, NoticeCite) else notice_from_cite(raw))

    nav_cite = nav if isinstance(nav, NavCite) else nav_from_cite(nav)
    commit = parse_optional_minor(remaining_commitment)
    undrawn = parse_optional_minor(remaining_undrawn)
    return Statement(
        partners=tuple(filled),
        remaining_commitment=commit,
        remaining_undrawn=undrawn,
        nav=nav_cite,
        notices=tuple(notice_rows),
        closed_through=book.closed_through,
        currency=code,
        unset=_named_unset(
            partners=filled,
            remaining_commitment=commit,
            remaining_undrawn=undrawn,
            nav=nav_cite,
            notices=notice_rows,
            closed_through=book.closed_through,
        ),
    )


def fetch_cites(
    *,
    token: str | None = None,
    book_id: str | None = None,
    transport: _grant.Transport | None = None,
) -> Any:
    """Pull partner / statement / NAV cites from ConnectApiUrl."""
    return _grant.pull(
        token=token,
        book_id=book_id,
        transport=transport,
        error=Refuse,
    )


def deliver(
    statement: Statement,
    *,
    token: str | None = None,
    transport: _grant.Transport | None = None,
) -> Statement:
    """Confirm ConnectApiUrl membership, then return the local statement."""
    _grant.pull(token=token, transport=transport, error=Refuse)
    return statement


def irr(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. IRR is a return, not a journal cite."""
    raise Refuse(
        "IRR is refused — partners:read is partner master, capital, and "
        "commitments, not a return. PLAN already named IRR as cannot-show"
    )


def tvpi(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. TVPI is a return multiple."""
    raise Refuse(
        "TVPI is refused — this app cites partner capital and NAV already "
        "on the book. A multiple is not a journal cite"
    )


def waterfall(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Preferred-return math stays out of the kernel and this app."""
    raise Refuse(
        "a waterfall is refused — partners:read is not preferred-return "
        "math. Capital notices cite posted amounts, not catch-up or carry"
    )


def drip(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Drip elections stay leftover on #161 / #177."""
    raise Refuse(
        "drip elections stay leftover on #161 / #177 — a drip is "
        "distribute_* then subscribe_* plus an LP election. This app "
        "is read-only capital / statement / NAV cites"
    )


def drip_election(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Same door as drip()."""
    drip()


def kernel_portal(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. No HTML portal routes inside ratio watch / console."""
    raise Refuse(
        "an HTML LP portal inside ratio watch or the console binary is "
        "refused — client portal stays Connect. This app is the door; "
        "it does not grow kernel routes"
    )


def html_portal(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Same door as kernel_portal()."""
    kernel_portal()


def lp_directory(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. No LP user tables inside Ratio core."""
    raise Refuse(
        "an LP user directory is refused — membership is the AuthKit "
        "sub on the book, not a kernel LP table. This does not close "
        "#161 by inventing one"
    )


def document_vault(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. No document vault in core."""
    raise Refuse(
        "a document vault is refused — capital notices already on "
        "GetBook are cites (digest + pinned cut + posted amounts), "
        "not a kernel blob store. Leftover on #161 / #150"
    )


def payments_initiate(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Payment initiation is a hard non-scope."""
    raise Refuse(
        "payments:initiate is a hard non-scope — this app does not "
        "start a drip payment or a bank transfer. Bank OAuth stays refused"
    )


def csv_partners(statement: Statement) -> str:
    """Per-partner capital account. Blanks are unset."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(
        (
            "Partner",
            "Beginning",
            "Contributions",
            "Distributions",
            "Allocated income",
            "Allocated expense",
            "Unrealized",
            "Ending",
            "Units",
        )
    )
    for p in statement.partners:
        w.writerow(
            (
                p.grain,
                format_optional(p.beginning),
                format_optional(p.contributions),
                format_optional(p.distributions),
                format_optional(p.allocated_income),
                format_optional(p.allocated_expense),
                format_optional(p.unrealized),
                format_optional(p.ending),
                "" if p.units is None else str(p.units),
            )
        )
    return buf.getvalue()


def csv_capital(statement: Statement) -> str:
    """Book-level commitment / undrawn / closed-through. Blanks are unset."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(("Figure", "Amount", "Note"))
    rows = (
        (
            "remaining_commitment",
            format_optional(statement.remaining_commitment),
            "posted commitment remaining; unset when no commitment posted — not a callable zero",
        ),
        (
            "remaining_undrawn",
            format_optional(statement.remaining_undrawn),
            "posted undrawn remaining; unset when no undrawn posted — not a callable zero",
        ),
        (
            "closed_through",
            statement.closed_through.isoformat() if statement.closed_through else "",
            "statements:read; an open period is unset, not a fake closed period",
        ),
    )
    for row in rows:
        w.writerow(row)
    return buf.getvalue()


def csv_nav(statement: Statement) -> str:
    """NAV strike + roll-forward. Blanks are unset."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(("Figure", "Amount", "Note"))
    n = statement.nav
    rows = (
        (
            "net_asset_value",
            format_optional(n.net_asset_value),
            "NavStrike; a missing strike is unset, not NAV 0.00",
        ),
        (
            "journal_digest",
            n.journal_digest or "",
            "empty is unset, not history-intact",
        ),
        (
            "beginning",
            format_optional(n.beginning),
            "period roll-forward; no dated prefix is unset, not a fake zero NAV",
        ),
        (
            "contributions",
            format_optional(n.contributions),
            "same Partner capital / Capital contributions credits /capital already names",
        ),
        (
            "distributions",
            format_optional(n.distributions),
            "same Partner capital / Distributions debits /capital already names",
        ),
        (
            "income",
            format_optional(n.income),
            "period income; unset until that type moved",
        ),
        (
            "expense",
            format_optional(n.expense),
            "period expense; unset until that type moved",
        ),
        (
            "unrealized",
            format_optional(n.unrealized),
            "unset until Unrealized gain moved — a silent 0.00 mark is the defect",
        ),
        (
            "ending",
            format_optional(n.ending),
            "assets + liabilities; commitment and undrawn cancel — they are not cash that arrived",
        ),
    )
    for row in rows:
        w.writerow(row)
    return buf.getvalue()


def csv_notices(statement: Statement) -> str:
    """Citeable capital-call / distribution notices. Empty is unset."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(("Kind", "Amount", "Digest", "Amounts", "Entry"))
    for n in statement.notices:
        shown = " · ".join(f"{p} {format_minor(a)}" for p, a in n.amounts)
        w.writerow(
            (
                n.kind,
                format_optional(n.amount),
                n.digest or "",
                shown,
                n.entry_id,
            )
        )
    return buf.getvalue()


def csv_unset(statement: Statement) -> str:
    """Companion sheet: what the journal cannot support, named."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(("Unset",))
    for line in statement.unset:
        w.writerow((line,))
    return buf.getvalue()


def as_files(statement: Statement) -> dict[str, str]:
    """Named companion sheets. Not a kernel portal and not a K-1 pack."""
    return {
        "partners.csv": csv_partners(statement),
        "capital.csv": csv_capital(statement),
        "nav.csv": csv_nav(statement),
        "notices.csv": csv_notices(statement),
        "unset.csv": csv_unset(statement),
    }


def as_json(statement: Statement) -> dict[str, Any]:
    """JSON of the same cites. Missing keys stay absent, not 0."""
    def money(n: int | None) -> str | None:
        return None if n is None else format_minor(n)

    return {
        "currency": statement.currency,
        "closed_through": (
            statement.closed_through.isoformat() if statement.closed_through else None
        ),
        "remaining_commitment": money(statement.remaining_commitment),
        "remaining_undrawn": money(statement.remaining_undrawn),
        "partners": [
            {
                "grain": p.grain,
                "beginning": money(p.beginning),
                "contributions": money(p.contributions),
                "distributions": money(p.distributions),
                "allocated_income": money(p.allocated_income),
                "allocated_expense": money(p.allocated_expense),
                "unrealized": money(p.unrealized),
                "ending": money(p.ending),
                "units": p.units,
            }
            for p in statement.partners
        ],
        "nav": {
            "net_asset_value": money(statement.nav.net_asset_value),
            "journal_digest": statement.nav.journal_digest,
            "journal_position": statement.nav.journal_position,
            "config_digest": statement.nav.config_digest,
            "beginning": money(statement.nav.beginning),
            "contributions": money(statement.nav.contributions),
            "distributions": money(statement.nav.distributions),
            "income": money(statement.nav.income),
            "expense": money(statement.nav.expense),
            "unrealized": money(statement.nav.unrealized),
            "ending": money(statement.nav.ending),
        },
        "notices": [
            {
                "kind": n.kind,
                "amount": money(n.amount),
                "digest": n.digest,
                "amounts": [{"partner": p, "amount": format_minor(a)} for p, a in n.amounts],
                "entry_id": n.entry_id,
            }
            for n in statement.notices
        ],
        "unset": list(statement.unset),
    }
