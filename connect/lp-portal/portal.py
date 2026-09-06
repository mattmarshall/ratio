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
tables, and a document vault stay refused. `as_html` is a
read-only cite-backed page in this tree — not a kernel route
and not a hosted product walk-through. Membership is the
AuthKit `sub` on the book. An `org_id` claim is not membership.

⭐ THE GRANT PATH CALLS CONNECTAPIURL. `fetch_cites` and `deliver`
present a verified Connect access token against the Connect HTTP
API. Membership is still required. A Connect token never takes
`RATIO_DEMO_OPEN` and never matches `org:{id}`. This app's
Connect grant is proven. WorkOS dashboard registration of
*other* Connect apps stays leftover #22. A green cite is not a
hosted live walk-through.
"""

from __future__ import annotations

import csv
import html
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
    "fee_receivable",
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
    "qualification",
)
BOOK_PROTO_FIELDS = (
    "partner_cut",
    "special_allocations",
    "allocation_facts",
    "notices",
    "fee_receivable",
    "trial_balance_difference",
    "currency_code",
    "config_digest",
)
ALLOCATION_KINDS = frozenset({"income", "expense", "unrealized"})


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
class SpecialCite:
    """One standing special. Empty list is silence — that kind uses the cut."""

    partner: str
    kind: str
    weight: int


@dataclass(frozen=True)
class FactCite:
    """One journal special. Exact amount, not a weight."""

    partner: str
    kind: str
    amount: int
    trade_date: date | None = None


@dataclass(frozen=True)
class NoticeCite:
    """One `CapitalNotice`. Empty digest is unset, not a silent notice."""

    kind: str
    amount: int | None = None
    digest: str | None = None
    amounts: tuple[tuple[str, int], ...] = ()
    partner_cut: tuple[PartnerShare, ...] = ()
    entry_id: str = ""
    trade_date: date | None = None


@dataclass(frozen=True)
class NavCite:
    """`nav:read` strike + roll-forward. Missing NAV is unset, not 0.00."""

    net_asset_value: int | None = None
    journal_digest: str | None = None
    journal_position: int | None = None
    config_digest: str | None = None
    valuation_time: str | None = None
    trial_balance_difference: int | None = None
    qualification: tuple[str, ...] = ()
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
    partner_cut: tuple[PartnerShare, ...] = ()
    special_allocations: tuple[SpecialCite, ...] = ()
    allocation_facts: tuple[FactCite, ...] = ()
    fee_receivable: int | None = None
    trial_balance_difference: int | None = None
    config_digest: str | None = None
    book_id: str = ""


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


def pick(raw: Mapping[str, Any], *names: str) -> Any:
    """First present key. GetBook wire is camelCase; fixtures may be snake."""
    for name in names:
        if name in raw and raw[name] is not None:
            return raw[name]
    return None


def parse_wire_minor(text: Any, *, allow_signed: bool = False) -> int | None:
    """GetBook / NavStrike Int64 money: digits are already minor units.

    Empty is unset. `"0"` is a real zero. A decimal point is the
    display form (`"75.00"`) and goes through `parse_minor` so a
    fixture and a live GetBook can share this door. A float object
    is refused — that is how a cent disappears.
    """
    if text is None:
        return None
    if isinstance(text, bool):
        raise Refuse("a money figure is not a flag")
    if isinstance(text, int):
        if text > I64_MAX or text < I64_MIN:
            raise Refuse("amount does not fit in i64 minor units")
        if text < 0 and not allow_signed:
            raise Refuse(
                f"{text!r} is signed; this figure is a magnitude, and a "
                "signed-amount inference is how a contribution and a "
                "distribution swap"
            )
        return text
    if not isinstance(text, str):
        raise Refuse(
            f"{text!r} is not an amount string — a number typed as a float "
            "is how a cent disappears"
        )
    t = text.strip()
    if not t:
        return None
    if "." in t or "," in t or "$" in t:
        return parse_minor(t, allow_signed=allow_signed)
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
    if not t.isdigit():
        raise Refuse(f"{text!r} is not an amount")
    v = int(t)
    if sign < 0:
        v = -v
    if v > I64_MAX or v < I64_MIN:
        raise Refuse(f"{text!r} does not fit in i64 minor units")
    return v


def parse_trade_date(raw: Any) -> date | None:
    """GetBook `tradeDate` is `{year, month, day}` or a YYYY-MM-DD string."""
    if raw is None:
        return None
    if isinstance(raw, date):
        return raw
    if isinstance(raw, str):
        t = raw.strip()
        if not t:
            return None
        try:
            return date.fromisoformat(t[:10])
        except ValueError as e:
            raise Refuse(f"{raw!r} is not a trade date") from e
    if isinstance(raw, Mapping):
        y = raw.get("year")
        m = raw.get("month")
        d = raw.get("day")
        if y in (None, 0) or m in (None, 0) or d in (None, 0):
            return None
        try:
            return date(int(y), int(m), int(d))
        except (TypeError, ValueError) as e:
            raise Refuse(f"{raw!r} is not a trade date") from e
    raise Refuse(f"{raw!r} is not a trade date")


def parse_kind(raw: Any) -> str:
    """`INVESTMENT`, not `KIND_INVESTMENT`. UNSPECIFIED is not a fifth kind."""
    text = str(raw or "").strip()
    if text.startswith("KIND_"):
        text = text[5:]
    return text


def shares_from_cite(
    raw: Sequence[Mapping[str, Any] | PartnerShare] | None,
) -> tuple[PartnerShare, ...]:
    """Named weights. Empty is unset — not a silent 1/N."""
    out: list[PartnerShare] = []
    seen: set[str] = set()
    for row in raw or ():
        if isinstance(row, PartnerShare):
            share = row
        else:
            partner = str(row.get("partner") or "").strip()
            weight_raw = pick(row, "weight")
            if not partner:
                raise Refuse(
                    "a partner_cut row without a partner is not a cut — "
                    "the grain is the suffix on Partner capital (LP, GP)"
                )
            if weight_raw is None or (
                isinstance(weight_raw, str) and not str(weight_raw).strip()
            ):
                raise Refuse(
                    f"partner_cut {partner!r} is missing a weight — "
                    "inventing one is how a silent 1/N ships"
                )
            try:
                weight = int(weight_raw)
            except (TypeError, ValueError) as e:
                raise Refuse(
                    f"partner_cut {partner!r} weight {weight_raw!r} is not "
                    "an integer share"
                ) from e
            share = PartnerShare(partner, weight)
        if share.weight <= 0:
            raise Refuse(
                f"partner_cut {share.partner!r} weight is {share.weight}, "
                "and a non-positive weight is not a weight"
            )
        if not share.partner or share.partner in seen:
            raise Refuse(
                "two rows for one partner are two answers under one name"
            )
        seen.add(share.partner)
        out.append(share)
    return tuple(out)


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


def cut_for_kind(
    kind: str,
    cut: Sequence[PartnerShare] | None,
    specials: Sequence[SpecialCite] | None,
) -> tuple[PartnerShare, ...] | None:
    """Standing specials for this kind, else the default. `Ratio.Partners.cutFor`."""
    named = tuple(s for s in (specials or ()) if s.kind == kind)
    if named:
        return tuple(PartnerShare(s.partner, s.weight) for s in named)
    if cut:
        return tuple(cut)
    return None


def apply_facts(
    figure: int | None,
    facts: Sequence[FactCite] | None,
    kind: str,
    remainder: Sequence[PartnerShare] | None,
) -> dict[str, int] | None:
    """Journal specials of one kind, then the remainder cut.

    `None` facts fall through to the cut. `[]` is elected and unnamed
    and stays unset — the SpecID shape, not a silent 1/N.
    Facts that cover the figure are the allocation. A remainder
    needs a cut that divides. An overshoot stays unset.
    `Ratio.Partners.applyFacts`.
    """
    if figure is None:
        return None
    if facts is None:
        return apply_cut(figure, remainder)
    if len(facts) == 0:
        return None
    taken = 0
    out: dict[str, int] = {}
    any_of_kind = False
    for fact in facts:
        if fact.kind != kind:
            continue
        if not fact.partner:
            return None
        any_of_kind = True
        taken = checked_add(taken, fact.amount)
        prev = out.get(fact.partner, 0)
        out[fact.partner] = checked_add(prev, fact.amount)
    if taken == figure:
        return out
    if taken > figure:
        return None
    if not any_of_kind:
        return apply_cut(figure, remainder)
    left = figure - taken
    rest = apply_cut(left, remainder)
    if rest is None:
        return None
    for partner, amount in rest.items():
        prev = out.get(partner, 0)
        out[partner] = checked_add(prev, amount)
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


def notice_from_cite(raw: Mapping[str, Any], *, wire: bool = False) -> NoticeCite:
    kind = str(raw.get("kind") or "").strip().lower()
    if kind not in ("call", "distribution"):
        raise Refuse(
            f"{kind!r} is not a capital notice kind — catalogs cite call "
            "or distribution, not preferred, catch-up, or carry"
        )
    money = parse_wire_minor if wire else parse_optional_minor
    amounts: list[tuple[str, int]] = []
    for row in raw.get("amounts") or ():
        partner = str(row.get("partner") or "").strip()
        if not partner:
            raise Refuse("a notice amount without a partner invents a slice")
        amt = money(row.get("amount"), allow_signed=True)
        if amt is None:
            raise Refuse(
                "a notice amount that is empty is unset for the whole "
                "notice — not a silent 1/N of the total"
            )
        amounts.append((partner, amt))
    return NoticeCite(
        kind=kind,
        amount=money(raw.get("amount"), allow_signed=True),
        digest=parse_optional_digest(raw.get("digest")),
        amounts=tuple(amounts),
        partner_cut=shares_from_cite(pick(raw, "partner_cut", "partnerCut") or ()),
        entry_id=str(pick(raw, "entry_id", "entryId") or ""),
        trade_date=parse_trade_date(pick(raw, "trade_date", "tradeDate")),
    )


def nav_from_cite(raw: Mapping[str, Any] | None, *, wire: bool = False) -> NavCite:
    if raw is None:
        return NavCite()
    money = parse_wire_minor if wire else parse_optional_minor
    quals = pick(raw, "qualification") or ()
    if isinstance(quals, str):
        quals = (quals,) if quals.strip() else ()
    return NavCite(
        net_asset_value=money(
            pick(raw, "net_asset_value", "netAssetValue"), allow_signed=True
        ),
        journal_digest=parse_optional_digest(
            pick(raw, "journal_digest", "journalDigest")
        ),
        journal_position=parse_optional_int(
            pick(raw, "journal_position", "journalPosition")
        ),
        config_digest=parse_optional_digest(
            pick(raw, "config_digest", "configDigest")
        ),
        valuation_time=(
            str(pick(raw, "valuation_time", "valuationTime") or "").strip() or None
        ),
        trial_balance_difference=money(
            pick(raw, "trial_balance_difference", "trialBalanceDifference"),
            allow_signed=True,
        ),
        qualification=tuple(str(q) for q in quals),
        beginning=money(raw.get("beginning"), allow_signed=True),
        contributions=parse_optional_minor(raw.get("contributions"))
        if not wire
        else parse_wire_minor(raw.get("contributions")),
        distributions=parse_optional_minor(raw.get("distributions"))
        if not wire
        else parse_wire_minor(raw.get("distributions")),
        income=money(raw.get("income"), allow_signed=True),
        expense=money(raw.get("expense"), allow_signed=True),
        unrealized=money(raw.get("unrealized"), allow_signed=True),
        ending=money(raw.get("ending"), allow_signed=True),
    )


def _named_unset(
    *,
    partners: Sequence[PartnerCite],
    remaining_commitment: int | None,
    remaining_undrawn: int | None,
    nav: NavCite,
    notices: Sequence[NoticeCite],
    closed_through: date | None,
    partner_cut: Sequence[PartnerShare] | None = None,
    special_allocations: Sequence[SpecialCite] | None = None,
    allocation_facts: Sequence[FactCite] | None = None,
    fee_receivable: int | None = None,
    trial_balance_difference: int | None = None,
    config_digest: str | None = None,
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
    if not partner_cut:
        names.append(
            "partner_cut — empty is unset, not a silent 1/N of book NAV"
        )
    if not special_allocations:
        names.append(
            "special_allocations — empty is silence; that kind uses "
            "partner_cut, or stays unset"
        )
    if not allocation_facts:
        names.append(
            "allocation_facts — empty is silence, not an unnamed [] "
            "and not a silent 1/N"
        )
    if fee_receivable is None:
        names.append(
            "fee receivable — no elected fee terms is unset, not a "
            "silent zero receivable"
        )
    if trial_balance_difference is None:
        names.append(
            "trial-balance difference — empty is unset, not a silent "
            "tied 0.00"
        )
    if config_digest is None:
        names.append(
            "book config digest — empty is unset, not a silent pin"
        )
    if nav.net_asset_value is None:
        names.append("NAV strike — a missing strike is unset, not NAV 0.00")
    if nav.journal_digest is None:
        names.append(
            "NAV journal digest — empty is unset, not history-intact"
        )
    if nav.valuation_time is None:
        names.append(
            "NAV valuation time — a missing strike time is unset, not "
            "invented as now"
        )
    if nav.trial_balance_difference is None:
        names.append(
            "NAV trial-balance difference — empty is unset, not a "
            "silent tied 0.00"
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
    partner_cut: Sequence[PartnerShare | Mapping[str, Any]] | None = None,
    special_allocations: Sequence[SpecialCite | Mapping[str, Any]] | None = None,
    allocation_facts: Sequence[FactCite | Mapping[str, Any]] | None = None,
    book_income: Any = None,
    book_expense: Any = None,
    book_unrealized: Any = None,
    fee_receivable: Any = None,
    trial_balance_difference: Any = None,
    config_digest: Any = None,
    wire: bool = False,
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

    money = parse_wire_minor if wire else parse_optional_minor
    cut = shares_from_cite(partner_cut)
    specials = _specials_from_cite(special_allocations)
    facts = _facts_from_cite(allocation_facts, wire=wire)

    rows: list[PartnerCite] = []
    for raw in partners or ():
        rows.append(raw if isinstance(raw, PartnerCite) else partner_from_cite(raw))

    income_shares = apply_facts(
        money(book_income, allow_signed=True),
        facts if facts else None,
        "income",
        cut_for_kind("income", cut, specials),
    )
    expense_shares = apply_facts(
        money(book_expense, allow_signed=True),
        facts if facts else None,
        "expense",
        cut_for_kind("expense", cut, specials),
    )
    unreal_shares = apply_facts(
        money(book_unrealized, allow_signed=True),
        facts if facts else None,
        "unrealized",
        cut_for_kind("unrealized", cut, specials),
    )
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
        notice_rows.append(
            raw if isinstance(raw, NoticeCite) else notice_from_cite(raw, wire=wire)
        )

    nav_cite = nav if isinstance(nav, NavCite) else nav_from_cite(nav, wire=wire)
    commit = money(remaining_commitment)
    undrawn = money(remaining_undrawn)
    fee = money(fee_receivable, allow_signed=True)
    tb = money(trial_balance_difference, allow_signed=True)
    digest = parse_optional_digest(config_digest)
    return Statement(
        partners=tuple(filled),
        remaining_commitment=commit,
        remaining_undrawn=undrawn,
        nav=nav_cite,
        notices=tuple(notice_rows),
        closed_through=book.closed_through,
        currency=code,
        partner_cut=cut,
        special_allocations=specials,
        allocation_facts=facts,
        fee_receivable=fee,
        trial_balance_difference=tb,
        config_digest=digest,
        book_id=book.book_id,
        unset=_named_unset(
            partners=filled,
            remaining_commitment=commit,
            remaining_undrawn=undrawn,
            nav=nav_cite,
            notices=notice_rows,
            closed_through=book.closed_through,
            partner_cut=cut,
            special_allocations=specials,
            allocation_facts=facts,
            fee_receivable=fee,
            trial_balance_difference=tb,
            config_digest=digest,
        ),
    )


def _specials_from_cite(
    raw: Sequence[SpecialCite | Mapping[str, Any]] | None,
) -> tuple[SpecialCite, ...]:
    out: list[SpecialCite] = []
    for row in raw or ():
        if isinstance(row, SpecialCite):
            out.append(row)
            continue
        partner = str(row.get("partner") or "").strip()
        kind = str(row.get("kind") or "").strip().lower()
        if not partner:
            raise Refuse("a special allocation without a partner invents a slice")
        if kind not in ALLOCATION_KINDS:
            raise Refuse(
                f"{kind!r} is not a special-allocation kind — catalogs "
                "cite income, expense, or unrealized"
            )
        try:
            weight = int(row.get("weight"))
        except (TypeError, ValueError) as e:
            raise Refuse(
                f"special_allocation {partner!r} weight is not an integer share"
            ) from e
        if weight <= 0:
            raise Refuse(
                f"special_allocation {partner!r} weight is {weight}, and a "
                "non-positive weight is not a weight"
            )
        out.append(SpecialCite(partner=partner, kind=kind, weight=weight))
    return tuple(out)


def _facts_from_cite(
    raw: Sequence[FactCite | Mapping[str, Any]] | None,
    *,
    wire: bool = False,
) -> tuple[FactCite, ...]:
    money = parse_wire_minor if wire else parse_optional_minor
    out: list[FactCite] = []
    for row in raw or ():
        if isinstance(row, FactCite):
            out.append(row)
            continue
        partner = str(row.get("partner") or "").strip()
        kind = str(row.get("kind") or "").strip().lower()
        if not partner:
            raise Refuse("an allocation fact without a partner invents a slice")
        if kind not in ALLOCATION_KINDS:
            raise Refuse(
                f"{kind!r} is not an allocation-fact kind — catalogs cite "
                "income, expense, or unrealized"
            )
        amount = money(row.get("amount"), allow_signed=True)
        if amount is None:
            raise Refuse(
                "an allocation fact without an amount is unset for the "
                "whole fact — not a silent 1/N of the remainder"
            )
        out.append(
            FactCite(
                partner=partner,
                kind=kind,
                amount=amount,
                trade_date=parse_trade_date(pick(row, "trade_date", "tradeDate")),
            )
        )
    return tuple(out)


def book_from_getbook(
    raw: Mapping[str, Any],
    *,
    member: bool = True,
    closed_through: date | None = None,
) -> Book:
    """`books:read` membership from a GetBook payload. org_id is not membership."""
    name = str(pick(raw, "name", "book_id", "bookId") or "").strip()
    book_id = name[6:] if name.startswith("books/") else name
    kind = parse_kind(pick(raw, "kind"))
    org = str(pick(raw, "organization", "org_id", "orgId") or "").strip() or None
    return Book(
        book_id=book_id or "",
        kind=kind or "UNSPECIFIED",
        member=member,
        org_id=org,
        closed_through=closed_through,
    )


def statement_from_getbook(
    raw: Mapping[str, Any],
    *,
    client: Client,
    book: Book | None = None,
    partners: Sequence[Mapping[str, Any] | PartnerCite] | None = None,
    nav: NavCite | Mapping[str, Any] | None = None,
    remaining_commitment: Any = None,
    remaining_undrawn: Any = None,
    book_income: Any = None,
    book_expense: Any = None,
    book_unrealized: Any = None,
    closed_through: date | None = None,
    member: bool = True,
) -> Statement:
    """Cite GetBook fields already on the book. Missing stays unset.

    GetBook carries partner_cut, special_allocations, allocation_facts,
    fee_receivable, trial_balance_difference, notices, and currency.
    Partner capital rows, commitments / undrawn, and NavStrike /
    period roll-forward are separate cites — pass them when the
    journal has them. Do not invent a 1/N, a callable-zero, or NAV 0.00.
    """
    if "books" in raw and isinstance(raw.get("books"), list):
        raise Refuse(
            "ListBooks is a listing — name a book; inventing one is how "
            "a portal picks the wrong NAV"
        )
    derived = book or book_from_getbook(
        raw, member=member, closed_through=closed_through
    )
    if closed_through is not None and book is not None:
        derived = Book(
            book_id=book.book_id,
            kind=book.kind,
            member=book.member,
            org_id=book.org_id,
            closed_through=closed_through,
        )
    currency = str(pick(raw, "currency_code", "currencyCode", "currency") or "USD")
    notices = pick(raw, "notices") or ()
    return cite_statement(
        partners=partners,
        nav=nav,
        book=derived,
        client=client,
        remaining_commitment=remaining_commitment,
        remaining_undrawn=remaining_undrawn,
        notices=notices,
        currency=currency,
        partner_cut=pick(raw, "partner_cut", "partnerCut") or (),
        special_allocations=pick(raw, "special_allocations", "specialAllocations") or (),
        allocation_facts=pick(raw, "allocation_facts", "allocationFacts") or (),
        book_income=book_income,
        book_expense=book_expense,
        book_unrealized=book_unrealized,
        fee_receivable=pick(raw, "fee_receivable", "feeReceivable"),
        trial_balance_difference=pick(
            raw, "trial_balance_difference", "trialBalanceDifference"
        ),
        config_digest=pick(raw, "config_digest", "configDigest"),
        wire=True,
    )


def cite_from_fetch(
    *,
    token: str | None = None,
    book_id: str | None = None,
    transport: _grant.Transport | None = None,
    client: Client,
    partners: Sequence[Mapping[str, Any] | PartnerCite] | None = None,
    nav: NavCite | Mapping[str, Any] | None = None,
    remaining_commitment: Any = None,
    remaining_undrawn: Any = None,
    book_income: Any = None,
    book_expense: Any = None,
    book_unrealized: Any = None,
    closed_through: date | None = None,
    member: bool = True,
) -> Statement:
    """Pull GetBook from ConnectApiUrl and cite it. Membership still required."""
    payload = fetch_cites(token=token, book_id=book_id, transport=transport)
    if not isinstance(payload, Mapping):
        raise Refuse("ConnectApiUrl GetBook returned a non-object cite")
    return statement_from_getbook(
        payload,
        client=client,
        partners=partners,
        nav=nav,
        remaining_commitment=remaining_commitment,
        remaining_undrawn=remaining_undrawn,
        book_income=book_income,
        book_expense=book_expense,
        book_unrealized=book_unrealized,
        closed_through=closed_through,
        member=member,
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
        "is read-only capital / statement / NAV cites. Equalization "
        "and side-pocket stay Connect/#177; they are not kernel primitives"
    )


def drip_election(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Same door as drip()."""
    drip()


def kernel_portal(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. No HTML portal routes inside ratio watch / console."""
    raise Refuse(
        "an HTML LP portal inside ratio watch or the console binary is "
        "refused — client portal stays Connect. as_html is the "
        "first-party cite-backed page in this tree; it does not grow "
        "kernel routes"
    )


def html_portal(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Same door as kernel_portal(). as_html is the Connect-side page."""
    kernel_portal()


def lp_directory(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. No LP user tables inside Ratio core."""
    raise Refuse(
        "an LP user directory is refused — membership is the AuthKit "
        "sub on the book, not a kernel LP table. Partner grains on "
        "GetBook are capital cites, not a user directory. This does "
        "not close #161 by inventing one"
    )


def document_vault(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. No document vault in core."""
    raise Refuse(
        "a document vault is refused — capital notices already on "
        "GetBook are cites (digest + pinned cut + posted amounts + "
        "trade date), not a kernel blob store. Leftover on #161 / #150"
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
            "valuation_time",
            n.valuation_time or "",
            "the valuation point — empty is unset, not invented as now",
        ),
        (
            "trial_balance_difference",
            format_optional(n.trial_balance_difference),
            "on the strike; empty is unset, not a silent tied 0.00",
        ),
        (
            "qualification",
            " · ".join(n.qualification),
            "why the figure cannot be read at face value; empty means it stands",
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
    w.writerow(("Kind", "Amount", "Digest", "Cut", "Amounts", "Entry", "Trade date"))
    for n in statement.notices:
        shown = " · ".join(f"{p} {format_minor(a)}" for p, a in n.amounts)
        cut = " / ".join(f"{s.partner} {s.weight}" for s in n.partner_cut)
        w.writerow(
            (
                n.kind,
                format_optional(n.amount),
                n.digest or "",
                cut,
                shown,
                n.entry_id,
                n.trade_date.isoformat() if n.trade_date else "",
            )
        )
    return buf.getvalue()


def csv_cut(statement: Statement) -> str:
    """Named [[partner_cut]]. Empty is unset, not 1/N."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(("Partner", "Weight"))
    for s in statement.partner_cut:
        w.writerow((s.partner, str(s.weight)))
    return buf.getvalue()


def csv_specials(statement: Statement) -> str:
    """Standing specials by kind. Empty is silence."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(("Partner", "Kind", "Weight"))
    for s in statement.special_allocations:
        w.writerow((s.partner, s.kind, str(s.weight)))
    return buf.getvalue()


def csv_facts(statement: Statement) -> str:
    """Journal specials GetBook walked. Empty is silence."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(("Partner", "Kind", "Amount", "Trade date"))
    for f in statement.allocation_facts:
        w.writerow(
            (
                f.partner,
                f.kind,
                format_minor(f.amount),
                f.trade_date.isoformat() if f.trade_date else "",
            )
        )
    return buf.getvalue()


def csv_book(statement: Statement) -> str:
    """GetBook book-level cites. Blanks are unset."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(("Figure", "Amount", "Note"))
    rows = (
        (
            "fee_receivable",
            format_optional(statement.fee_receivable),
            "no elected fee terms is unset, not a silent zero receivable",
        ),
        (
            "trial_balance_difference",
            format_optional(statement.trial_balance_difference),
            "GetBook TB; empty is unset, not a silent tied 0.00",
        ),
        (
            "config_digest",
            statement.config_digest or "",
            "empty is unset, not a silent pin",
        ),
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
    )
    for row in rows:
        w.writerow(row)
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
        "book.csv": csv_book(statement),
        "cut.csv": csv_cut(statement),
        "specials.csv": csv_specials(statement),
        "facts.csv": csv_facts(statement),
        "nav.csv": csv_nav(statement),
        "notices.csv": csv_notices(statement),
        "unset.csv": csv_unset(statement),
        "statement.html": as_html(statement),
    }


def _html_money(n: int | None) -> str:
    return html.escape(format_minor(n)) if n is not None else "—"


def _html_text(value: str | None) -> str:
    return html.escape(value) if value else "—"


def as_html(statement: Statement) -> str:
    """Read-only cite-backed page. Not a kernel portal and not a walk-through.

    Blanks stay em-dash. Missing NAV is not 0.00. Empty digest is not
    history-intact. A missing cut is not 1/N. This file lives in
    `connect/lp-portal/`; `html_portal()` still refuses a route inside
    `ratio watch`.
    """
    partners_rows = []
    for p in statement.partners:
        partners_rows.append(
            "<tr>"
            f"<td>{html.escape(p.grain)}</td>"
            f"<td>{_html_money(p.beginning)}</td>"
            f"<td>{_html_money(p.contributions)}</td>"
            f"<td>{_html_money(p.distributions)}</td>"
            f"<td>{_html_money(p.allocated_income)}</td>"
            f"<td>{_html_money(p.allocated_expense)}</td>"
            f"<td>{_html_money(p.unrealized)}</td>"
            f"<td>{_html_money(p.ending)}</td>"
            f"<td>{html.escape(str(p.units)) if p.units is not None else '—'}</td>"
            "</tr>"
        )
    if not partners_rows:
        partners_rows.append(
            '<tr><td colspan="9">— no Partner capital row has posted</td></tr>'
        )
    cut_rows = [
        f"<tr><td>{html.escape(s.partner)}</td><td>{s.weight}</td></tr>"
        for s in statement.partner_cut
    ] or ['<tr><td colspan="2">— empty cut is unset, not a silent 1/N</td></tr>']
    special_rows = [
        f"<tr><td>{html.escape(s.partner)}</td><td>{html.escape(s.kind)}</td><td>{s.weight}</td></tr>"
        for s in statement.special_allocations
    ] or ['<tr><td colspan="3">— empty is silence; that kind uses the cut</td></tr>']
    fact_rows = [
        "<tr>"
        f"<td>{html.escape(f.partner)}</td>"
        f"<td>{html.escape(f.kind)}</td>"
        f"<td>{_html_money(f.amount)}</td>"
        f"<td>{f.trade_date.isoformat() if f.trade_date else '—'}</td>"
        "</tr>"
        for f in statement.allocation_facts
    ] or ['<tr><td colspan="4">— empty is silence, not an unnamed []</td></tr>']
    notice_rows = []
    for n in statement.notices:
        cut = " / ".join(f"{s.partner} {s.weight}" for s in n.partner_cut) or "—"
        shown = " · ".join(f"{p} {format_minor(a)}" for p, a in n.amounts) or "—"
        notice_rows.append(
            "<tr>"
            f"<td>{html.escape(n.kind)}</td>"
            f"<td>{_html_money(n.amount)}</td>"
            f"<td><code>{_html_text(n.digest)}</code></td>"
            f"<td>{html.escape(cut)}</td>"
            f"<td>{html.escape(shown)}</td>"
            f"<td>{html.escape(n.entry_id) if n.entry_id else '—'}</td>"
            f"<td>{n.trade_date.isoformat() if n.trade_date else '—'}</td>"
            "</tr>"
        )
    if not notice_rows:
        notice_rows.append(
            '<tr><td colspan="7">— empty is unset, not a silent waterfall</td></tr>'
        )
    unset_items = "".join(f"<li>{html.escape(line)}</li>" for line in statement.unset)
    n = statement.nav
    title = html.escape(statement.book_id or "LP statement")
    return (
        "<!DOCTYPE html>\n"
        '<html lang="en"><head><meta charset="utf-8">'
        f"<title>{title}</title>"
        "<style>body{font:15px/1.4 sans-serif;margin:2rem;color:#111}"
        "table{border-collapse:collapse;margin:1rem 0;width:100%}"
        "th,td{border:1px solid #ccc;padding:.4rem .6rem;text-align:left}"
        "th{background:#f4f4f4}td.num,th.num{text-align:right}"
        "code{font:13px ui-monospace,monospace}.unset{color:#555}"
        ".note{max-width:44rem}</style></head><body>\n"
        f"<h1>{title}</h1>\n"
        '<p class="note">Read-only cites from the book. Missing figures '
        "stay unset — never a silent 1/N of book NAV, a callable-zero "
        "commitment, a NAV 0.00, or an empty digest that looks like "
        "success. This page is not a kernel portal, not an LP directory, "
        "not a document vault, and not a payment or drip election.</p>\n"
        f"<p>Currency {_html_text(statement.currency)} · closed-through "
        f"{statement.closed_through.isoformat() if statement.closed_through else '—'}</p>\n"
        "<h2>Partner capital</h2>\n"
        "<table><thead><tr><th>Partner</th><th class='num'>Beginning</th>"
        "<th class='num'>Contributions</th><th class='num'>Distributions</th>"
        "<th class='num'>Allocated income</th><th class='num'>Allocated expense</th>"
        "<th class='num'>Unrealized</th><th class='num'>Ending</th>"
        "<th class='num'>Units</th></tr></thead><tbody>"
        + "".join(partners_rows)
        + "</tbody></table>\n"
        "<h2>Commitments / fee / trial balance</h2>\n"
        "<table><tbody>"
        f"<tr><th>Remaining commitment</th><td class='num'>{_html_money(statement.remaining_commitment)}</td></tr>"
        f"<tr><th>Remaining undrawn</th><td class='num'>{_html_money(statement.remaining_undrawn)}</td></tr>"
        f"<tr><th>Fee receivable</th><td class='num'>{_html_money(statement.fee_receivable)}</td></tr>"
        f"<tr><th>Trial-balance difference</th><td class='num'>{_html_money(statement.trial_balance_difference)}</td></tr>"
        f"<tr><th>Config digest</th><td><code>{_html_text(statement.config_digest)}</code></td></tr>"
        "</tbody></table>\n"
        "<h2>Partner cut</h2>\n"
        "<table><thead><tr><th>Partner</th><th>Weight</th></tr></thead><tbody>"
        + "".join(cut_rows)
        + "</tbody></table>\n"
        "<h2>Special allocations</h2>\n"
        "<table><thead><tr><th>Partner</th><th>Kind</th><th>Weight</th></tr></thead><tbody>"
        + "".join(special_rows)
        + "</tbody></table>\n"
        "<h2>Allocation facts</h2>\n"
        "<table><thead><tr><th>Partner</th><th>Kind</th><th class='num'>Amount</th><th>Trade date</th></tr></thead><tbody>"
        + "".join(fact_rows)
        + "</tbody></table>\n"
        "<h2>Capital notices</h2>\n"
        "<table><thead><tr><th>Kind</th><th class='num'>Amount</th><th>Digest</th>"
        "<th>Cut</th><th>Amounts</th><th>Entry</th><th>Trade date</th></tr></thead><tbody>"
        + "".join(notice_rows)
        + "</tbody></table>\n"
        "<h2>NAV strike / period roll-forward</h2>\n"
        "<table><tbody>"
        f"<tr><th>Net asset value</th><td class='num'>{_html_money(n.net_asset_value)}</td></tr>"
        f"<tr><th>Journal digest</th><td><code>{_html_text(n.journal_digest)}</code></td></tr>"
        f"<tr><th>Valuation time</th><td>{_html_text(n.valuation_time)}</td></tr>"
        f"<tr><th>Trial-balance difference</th><td class='num'>{_html_money(n.trial_balance_difference)}</td></tr>"
        f"<tr><th>Qualification</th><td>{_html_text(' · '.join(n.qualification) or None)}</td></tr>"
        f"<tr><th>Beginning</th><td class='num'>{_html_money(n.beginning)}</td></tr>"
        f"<tr><th>Contributions</th><td class='num'>{_html_money(n.contributions)}</td></tr>"
        f"<tr><th>Distributions</th><td class='num'>{_html_money(n.distributions)}</td></tr>"
        f"<tr><th>Income</th><td class='num'>{_html_money(n.income)}</td></tr>"
        f"<tr><th>Expense</th><td class='num'>{_html_money(n.expense)}</td></tr>"
        f"<tr><th>Unrealized</th><td class='num'>{_html_money(n.unrealized)}</td></tr>"
        f"<tr><th>Ending</th><td class='num'>{_html_money(n.ending)}</td></tr>"
        "</tbody></table>\n"
        "<h2>Unset</h2>\n"
        f'<ul class="unset">{unset_items}</ul>\n'
        "</body></html>\n"
    )


def as_json(statement: Statement) -> dict[str, Any]:
    """JSON of the same cites. Missing keys stay absent, not 0."""
    def money(n: int | None) -> str | None:
        return None if n is None else format_minor(n)

    return {
        "book_id": statement.book_id or None,
        "currency": statement.currency,
        "closed_through": (
            statement.closed_through.isoformat() if statement.closed_through else None
        ),
        "remaining_commitment": money(statement.remaining_commitment),
        "remaining_undrawn": money(statement.remaining_undrawn),
        "fee_receivable": money(statement.fee_receivable),
        "trial_balance_difference": money(statement.trial_balance_difference),
        "config_digest": statement.config_digest,
        "partner_cut": [
            {"partner": s.partner, "weight": s.weight} for s in statement.partner_cut
        ],
        "special_allocations": [
            {"partner": s.partner, "kind": s.kind, "weight": s.weight}
            for s in statement.special_allocations
        ],
        "allocation_facts": [
            {
                "partner": f.partner,
                "kind": f.kind,
                "amount": format_minor(f.amount),
                "trade_date": f.trade_date.isoformat() if f.trade_date else None,
            }
            for f in statement.allocation_facts
        ],
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
            "valuation_time": statement.nav.valuation_time,
            "trial_balance_difference": money(statement.nav.trial_balance_difference),
            "qualification": list(statement.nav.qualification),
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
                "partner_cut": [
                    {"partner": s.partner, "weight": s.weight} for s in n.partner_cut
                ],
                "amounts": [{"partner": p, "amount": format_minor(a)} for p, a in n.amounts],
                "entry_id": n.entry_id,
                "trade_date": n.trade_date.isoformat() if n.trade_date else None,
            }
            for n in statement.notices
        ],
        "unset": list(statement.unset),
    }
