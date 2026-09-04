#!/usr/bin/env python3
"""Vendor / GC portal for BookKind PROJECT.

A WorkOS Connect app, not a kernel RPC. Billing and retainage reads
live here. They do not live in `ratio watch`, the operations console,
or a new kernel method. `/billing` and `/budget` stay core. This app
cites those figures; it does not grow them.

⭐ SCOPES ARE THE FROZEN CATALOG NAMES. `journal:append` is an alias
and is refused. Canonical write grant: `journals:post`. Catalog
near-misses `projects:billing:read` / `projects:budget:read` are
refused. See docs/connect-scopes.md.

⭐ `journals:post` IS AN ALLOWLIST PER client_id. An empty allowlist
refuses every post. Silence is not "all templates". The issue body
still says `journal:append` for vendor invoices; that string is not
grantable. The named templates are CreateBook(Project)
`vendor_invoice*`.

⭐ UNSET STAYS UNSET. An unbilled job is not billed-zero. An unheld
retainage is not a silent 0% holdback on the retainage *line*.
Treating billed as 0 would print the whole contract as remaining.
A posted `"0.00"` is a figure.

⭐ MONEY IS MINOR UNITS, PARSED BY SPLITTING ON THE POINT. A float
is how a cent disappears. A third decimal place is refused.

⭐ BILLED IS NOT EARNED. Progress billings credit is billed.
Project revenue is earned. They can diverge while every entry
conserves. Substituting one for the other is a misstatement.

⭐ NO PERCENTAGE AND NO EAC. A % complete or a cost-to-complete is
a rounded or forecasted figure. `/budget` still does not forecast.

⚠ THE GRANT PATH IS NOT BUILT. `fetch_cites` and `deliver` refuse.
A Connect access token is not accepted on `/v1` (leftover #22 /
#150). Write-route actor binding landed (#151).

⚠ AIA G702 PRODUCT UI IS REFUSED. That door is #184. This app does
not pack a pay-app and does not render a licensed form.

⚠ NO VENDOR USER DIRECTORY IN CORE. Membership is the AuthKit `sub`
on the book. An `org_id` claim is not membership.
"""

from __future__ import annotations

import csv
import io
import json
from dataclasses import dataclass, field
from datetime import date
from typing import Any, Iterable, Mapping, Sequence

# i64 bounds. Lean's Int is unbounded; every money figure here is i64.
I64_MIN = -(2**63)
I64_MAX = 2**63 - 1

READ_SCOPES = frozenset({"billing:read", "budget:read", "statements:read"})
CANONICAL_SCOPES = READ_SCOPES | frozenset({"journals:post"})
REFUSED_ALIASES = frozenset(
    {
        "journal:append",
        "journal:read",
        "projects:budget:read",
        "projects:billing:read",
    }
)

# CreateBook(Project) posting rules this app may instantiate for a
# vendor invoice. Legs are (account_dim, weight). Weight is ±1.
# Same ids book.rs seeds — a silent rename is how a portal invents
# a Method.
VENDOR_INVOICE_LEGS: dict[str, tuple[tuple[int, int], tuple[int, int]]] = {
    "vendor_invoice": ((10, 1), (40, -1)),
    "vendor_invoice_site": ((11, 1), (40, -1)),
    "vendor_invoice_structure": ((12, 1), (40, -1)),
    "vendor_invoice_finishes": ((13, 1), (40, -1)),
}

KIND_TO_TEMPLATE = {
    "invoice": "vendor_invoice",
    "vendor_invoice": "vendor_invoice",
    "site": "vendor_invoice_site",
    "structure": "vendor_invoice_structure",
    "finishes": "vendor_invoice_finishes",
}

# What CreateBook writes on a Project book. A client_id that lists
# `call_lp` is refused — that rule was never seeded.
PROJECT_SEEDED_RULES = frozenset(VENDOR_INVOICE_LEGS) | frozenset(
    {
        "project_cost",
        "project_cost_site",
        "project_cost_structure",
        "project_cost_finishes",
        "pay_vendor",
        "progress_bill",
        "hold_retainage",
        "release_retainage",
        "collect_receivable",
        "earn_progress",
        "hold_vendor_retainage",
        "release_vendor_retainage",
    }
)

# Configuration fields this portal cites. Names must stay the ones
# crates/ratio-rules already stores — a silent rename is how a
# portal invents a baseline.
PROJECT_TERM_FIELDS = (
    "budget",
    "phases",
)


class Refuse(Exception):
    """The cite or post is not proposed. Message is the reason, not a workaround."""


@dataclass(frozen=True)
class Client:
    client_id: str
    allowlist: frozenset[str]
    scopes: frozenset[str]


@dataclass(frozen=True)
class Book:
    kind: str
    approved_templates: frozenset[str] = field(default_factory=lambda: PROJECT_SEEDED_RULES)
    closed_through: date | None = None


@dataclass(frozen=True)
class BillingCite:
    """`billing:read` / `projectProgress` cuts. Empty is unset."""

    billed: int | None = None
    earned: int | None = None
    retainage_receivable: int | None = None
    retainage_payable: int | None = None
    accounts_receivable: int | None = None


@dataclass(frozen=True)
class BudgetCite:
    """`budget:read` original plus journal change-order / award equity."""

    original: int | None = None
    approved_change_orders: int | None = None
    incurred: int | None = None
    awarded: int | None = None


@dataclass(frozen=True)
class Statement:
    """Vendor-facing billing / retainage / collections cite.

    Unset stays unset — not a silent billed-zero or a fake remaining
    equal to the whole contract.
    """

    billed: int | None
    earned: int | None
    billed_minus_earned: int | None
    retainage_receivable: int | None
    retainage_payable: int | None
    remaining_to_bill: int | None
    collections: int | None
    original: int | None
    approved_change_orders: int | None
    revised: int | None
    remaining_to_spend: int | None
    currency: str
    unset: tuple[str, ...]


@dataclass(frozen=True)
class Posting:
    account: int
    amount: int
    currency: str


@dataclass(frozen=True)
class ProposedPost:
    """An ApplyEvent-shaped proposal. Not a write."""

    rule_id: str
    amount: str
    currency: str
    trade_date: date
    event_id: str
    postings: tuple[Posting, ...]


def load_app(path: str) -> dict[str, Any]:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def client_from_app(app: Mapping[str, Any]) -> Client:
    allow = app.get("journals_post_allowlist") or {}
    scopes = app.get("workos_connect", {}).get("scopes") or []
    return Client(
        client_id=str(allow.get("client_id") or ""),
        allowlist=frozenset(allow.get("templates") or []),
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


def parse_day(text: str) -> date:
    if not isinstance(text, str) or not text.strip():
        raise Refuse("an undated row is refused — it cannot honor closed-through")
    t = text.strip()
    try:
        y, m, d = t.split("-")
        return date(int(y), int(m), int(d))
    except ValueError as e:
        raise Refuse(f"{text!r} is not a calendar day YYYY-MM-DD") from e


def checked_add(a: int, b: int) -> int:
    if a > I64_MAX or a < I64_MIN or b > I64_MAX or b < I64_MIN:
        raise Refuse("addend does not fit in i64")
    total = a + b
    if total > I64_MAX or total < I64_MIN:
        raise Refuse("sum does not fit in i64 minor units")
    return total


def checked_sub(a: int, b: int) -> int:
    return checked_add(a, -b)


def checked_scale(amount: int, weight: int) -> int:
    """amount × weight, refused on i64 wrap. Asked before the product."""
    if amount < 0 or amount > I64_MAX:
        raise Refuse("amount does not fit in i64")
    if weight not in (-1, 1):
        raise Refuse(
            f"weight {weight} is not ±1 — a Project template does not "
            "scale, and this app does not invent a Method"
        )
    if weight == 1:
        return amount
    if amount == 0:
        return 0
    return -amount


def _refuse_aliases(client: Client) -> None:
    aliases = client.scopes & REFUSED_ALIASES
    if aliases:
        raise Refuse(
            "refused alias scope "
            + ", ".join(sorted(aliases))
            + " — catalogs use billing:read / budget:read / statements:read; "
            "invoice append is journals:post, not journal:append"
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
    missing = READ_SCOPES - client.scopes
    if missing:
        raise Refuse(
            "this app needs "
            + ", ".join(sorted(READ_SCOPES))
            + f"; missing {', '.join(sorted(missing))}. "
            "billing:read is billed / earned / retainage / collections; "
            "budget:read is the original contract; statements:read is "
            "how closed-through is read"
        )


def _require_post_scopes(client: Client) -> None:
    _require_read_scopes(client)
    if "journals:post" not in client.scopes:
        raise Refuse(
            "vendor invoices need journals:post — the issue body's "
            "journal:append is an alias and is refused. billing:read "
            "is the cite, not a write grant"
        )


def _require_project(book: Book) -> None:
    if book.kind != "PROJECT":
        raise Refuse(
            f"this app is BookKind PROJECT; {book.kind!r} keeps its own "
            "chrome and is not a vendor / GC portal book"
        )


def revised_contract(original: int | None, approved: int | None) -> int | None:
    """Original + approved when the original is set.

    Same door as `revisedContract` on `/billing`. An unknown baseline
    cannot be priced. An unposted CO does not block a set original —
    revised equals the original, and the change-order *line* stays unset.
    """
    if original is None:
        return None
    return checked_add(original, approved if approved is not None else 0)


def remaining_to_bill(revised: int | None, billed: int | None) -> int | None:
    """Revised − billed. Unset when either side cannot support it.

    Treating billed as 0 would print the whole contract as remaining.
    """
    if revised is None or billed is None:
        return None
    return checked_sub(revised, billed)


def collected_against_billed(
    billed: int | None,
    ar: int | None,
    retainage: int | None,
) -> int | None:
    """Cash against AR: billed − AR − retainage held.

    Unset billed or unset AR cannot support the cut. Unheld retainage
    is 0 for the subtraction — same as `collectedAgainstBilled` on
    `/billing`. No hold is not an unknown hold.
    """
    if billed is None or ar is None:
        return None
    held = 0 if retainage is None else retainage
    return checked_sub(checked_sub(billed, ar), held)


def billed_minus_earned(billed: int | None, earned: int | None) -> int | None:
    """Over/under-billing. Unset until both sides have posted."""
    if billed is None or earned is None:
        return None
    return checked_sub(billed, earned)


def remaining_to_spend(
    revised: int | None,
    incurred: int | None,
    awarded: int | None,
) -> int | None:
    """Revised − incurred − awarded. Unset when the cut cannot be supported.

    Treating awarded as 0 would print budget − actual as headroom.
    """
    if revised is None or incurred is None or awarded is None:
        return None
    return checked_sub(checked_sub(revised, incurred), awarded)


def billing_from_cite(raw: Mapping[str, Any] | None) -> BillingCite:
    if raw is None:
        return BillingCite()
    return BillingCite(
        billed=parse_optional_minor(raw.get("billed")),
        earned=parse_optional_minor(raw.get("earned")),
        retainage_receivable=parse_optional_minor(raw.get("retainage_receivable")),
        retainage_payable=parse_optional_minor(raw.get("retainage_payable")),
        accounts_receivable=parse_optional_minor(raw.get("accounts_receivable")),
    )


def budget_from_cite(raw: Mapping[str, Any] | None) -> BudgetCite:
    if raw is None:
        return BudgetCite()
    original = raw.get("original") if "original" in raw else raw.get("budget")
    return BudgetCite(
        original=parse_optional_minor(original),
        approved_change_orders=parse_optional_minor(
            raw.get("approved_change_orders"), allow_signed=True
        ),
        incurred=parse_optional_minor(raw.get("incurred"), allow_signed=True),
        awarded=parse_optional_minor(raw.get("awarded"), allow_signed=True),
    )


def _named_unset(
    *,
    billed: int | None,
    earned: int | None,
    retainage_receivable: int | None,
    retainage_payable: int | None,
    remaining: int | None,
    collections: int | None,
    original: int | None,
    approved: int | None,
    remaining_spend: int | None,
) -> tuple[str, ...]:
    names: list[str] = []
    if billed is None:
        names.append("billed — an unbilled job is not billed-zero")
    if earned is None:
        names.append("earned — billed is not a substitute")
    if retainage_receivable is None:
        names.append("retainage receivable — no hold is unset on the line, not 0%")
    if retainage_payable is None:
        names.append("retainage payable — a vendor hold that never posted stays unset")
    if remaining is None:
        names.append(
            "remaining to bill — treating billed as 0 would print the whole contract"
        )
    if collections is None:
        names.append(
            "collections — unset billed or unset AR cannot support cash-against-AR"
        )
    if original is None:
        names.append("original contract — CreateBook does not invent a baseline")
    if approved is None:
        names.append("approved change orders — unposted is unset, not a silent net of nothing")
    if remaining_spend is None:
        names.append(
            "remaining to spend — treating awarded as 0 would print budget − actual as headroom"
        )
    return tuple(names)


def cite_statement(
    *,
    billing: BillingCite | Mapping[str, Any] | None,
    budget: BudgetCite | Mapping[str, Any] | None,
    book: Book,
    client: Client,
    currency: str = "USD",
) -> Statement:
    """Compose vendor-facing billing / retainage / collections from kernel cites.

    ⛔ NO FAKE ZEROS. The kernel already leaves billed / earned / retainage
    empty until those accounts post. This app cites that cut; it does not
    fill a portal with invented 0.00.
    """
    _require_read_scopes(client)
    _require_project(book)
    billed_cite = billing if isinstance(billing, BillingCite) else billing_from_cite(billing)
    budget_cite = budget if isinstance(budget, BudgetCite) else budget_from_cite(budget)
    code = currency.strip().upper()
    if len(code) != 3 or not code.isalpha():
        raise Refuse(f"{currency!r} is not an ISO currency code")
    revised = revised_contract(budget_cite.original, budget_cite.approved_change_orders)
    remaining = remaining_to_bill(revised, billed_cite.billed)
    collections = collected_against_billed(
        billed_cite.billed,
        billed_cite.accounts_receivable,
        billed_cite.retainage_receivable,
    )
    over_under = billed_minus_earned(billed_cite.billed, billed_cite.earned)
    spend = remaining_to_spend(revised, budget_cite.incurred, budget_cite.awarded)
    return Statement(
        billed=billed_cite.billed,
        earned=billed_cite.earned,
        billed_minus_earned=over_under,
        retainage_receivable=billed_cite.retainage_receivable,
        retainage_payable=billed_cite.retainage_payable,
        remaining_to_bill=remaining,
        collections=collections,
        original=budget_cite.original,
        approved_change_orders=budget_cite.approved_change_orders,
        revised=revised,
        remaining_to_spend=spend,
        currency=code,
        unset=_named_unset(
            billed=billed_cite.billed,
            earned=billed_cite.earned,
            retainage_receivable=billed_cite.retainage_receivable,
            retainage_payable=billed_cite.retainage_payable,
            remaining=remaining,
            collections=collections,
            original=budget_cite.original,
            approved=budget_cite.approved_change_orders,
            remaining_spend=spend,
        ),
    )


def instantiate(rule_id: str, amount: int, currency: str) -> tuple[Posting, ...]:
    legs = VENDOR_INVOICE_LEGS.get(rule_id)
    if legs is None:
        raise Refuse(
            f"{rule_id!r} is not a Project vendor_invoice template this app "
            "instantiates — it does not invent a Method, an Order, or a "
            "lot_method variant"
        )
    if not currency or not isinstance(currency, str):
        raise Refuse(
            "a posting names a currency; guessing the base is how a NAV "
            "adds dollars to euros"
        )
    currency = currency.strip().upper()
    if len(currency) != 3 or not currency.isalpha():
        raise Refuse(f"{currency!r} is not an ISO currency code")
    posts = []
    for account, weight in legs:
        posts.append(
            Posting(account=account, amount=checked_scale(amount, weight), currency=currency)
        )
    return tuple(posts)


def conserves(postings: Sequence[Posting]) -> bool:
    """Zero in every currency. A flat total is not a figure."""
    nets: dict[str, int] = {}
    for p in postings:
        nets[p.currency] = nets.get(p.currency, 0) + p.amount
    return bool(postings) and all(n == 0 for n in nets.values())


def _template_for(row: Mapping[str, Any]) -> str:
    kind = str(row.get("kind") or row.get("template") or "").strip().lower()
    if not kind:
        raise Refuse(
            "Kind picks the vendor_invoice* rule so a signed-amount "
            "inference cannot silently flip a site invoice and a finishes one"
        )
    rule = KIND_TO_TEMPLATE.get(kind)
    if rule is None:
        raise Refuse(
            f"kind {kind!r} is not a vendor invoice this app maps — "
            "it does not invent a Method or post call_lp onto a Project book"
        )
    return rule


def _event_id(row: Mapping[str, Any], index: int) -> str:
    raw = str(row.get("reference") or row.get("event_id") or f"vendor-invoice-{index + 1}")
    ident = raw.strip()
    if not ident or len(ident) > 64:
        raise Refuse(f"{ident!r} is not an event id")
    if not all(c.isalnum() or c in "-_." for c in ident):
        raise Refuse(
            f"{ident!r} is not an event id — letters, digits, - _ . and at most 64"
        )
    return ident


def propose_vendor_invoices(
    rows: Iterable[Mapping[str, Any]],
    *,
    book: Book,
    client: Client,
) -> list[ProposedPost]:
    """Propose ApplyEvent posts for allowlisted vendor_invoice* templates.

    ⛔ EMPTY ALLOWLIST REFUSES EVERY POST. Silence is not all templates.
    Closed-through refuses the batch. An undated row is refused so it
    cannot sneak past the gate.
    """
    _require_post_scopes(client)
    _require_project(book)
    proposed: list[ProposedPost] = []
    for i, row in enumerate(rows):
        rule = _template_for(row)
        if not client.allowlist:
            raise Refuse(
                "empty journals:post allowlist refuses every post — silence "
                "is not all templates"
            )
        if rule not in client.allowlist:
            raise Refuse(
                f"{rule} is not on client {client.client_id!r}'s allowlist"
            )
        if rule not in book.approved_templates:
            raise Refuse(
                f"{rule} is not in this book's approved RuleSet — "
                "CreateBook(Project) never wrote it"
            )
        amount = parse_minor(str(row.get("amount") or ""), allow_zero=False)
        currency = str(row.get("currency") or "").strip().upper()
        day = parse_day(str(row.get("dated") or ""))
        if book.closed_through is not None and day <= book.closed_through:
            raise Refuse(
                f"entry dated {day.isoformat()} is on or before closed-through "
                f"{book.closed_through.isoformat()}"
            )
        posts = instantiate(rule, amount, currency)
        if not conserves(posts):
            raise Refuse(
                f"{rule} instantiated at {format_minor(amount)} {currency} "
                "does not conserve in every currency"
            )
        proposed.append(
            ProposedPost(
                rule_id=rule,
                amount=format_minor(amount),
                currency=currency,
                trade_date=day,
                event_id=_event_id(row, i),
                postings=posts,
            )
        )
    return proposed


def fetch_cites(*, token: str | None = None) -> None:
    """Refuse to pull. The grant path is not built."""
    _ = token
    raise Refuse(
        "live Connect OAuth is leftover — the grant path "
        "is not built (#150 / leftover #22). This app does not pretend "
        "the door opens"
    )


def deliver(
    posts: Sequence[ProposedPost] | Statement,
    *,
    token: str | None = None,
) -> None:
    """Refuse to send. The grant path is not built."""
    _ = posts
    _ = token
    raise Refuse(
        "live Connect OAuth is leftover — the grant path "
        "is not built (#150 / leftover #22). This app does not pretend "
        "the door opens"
    )


def render_g702(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. AIA G702 product UI is #184, not this portal."""
    raise Refuse(
        "AIA G702/G703 product UI is refused — that door is #184. This "
        "app cites billed / earned / retainage / collections; it does "
        "not pack a pay-app or render a licensed form"
    )


def eac(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Estimate-at-completion is a forecast."""
    raise Refuse(
        "EAC / cost-to-complete is a forecast — this app cites billed / "
        "earned / retainage from the journal. /budget still does not "
        "forecast. Leftover on #169"
    )


def forecast(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. A cash or cost forecast is not a journal cite."""
    raise Refuse(
        "a forecast is refused — this app is not EAC and PLAN already "
        "named that as cannot-show (#169)"
    )


def vendor_directory(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. No vendor user directory inside Ratio core."""
    raise Refuse(
        "a vendor user directory is refused — membership is the AuthKit "
        "sub on the book, not a kernel vendor table. This does not close "
        "#172 by inventing one"
    )


def as_apply_event(post: ProposedPost, *, parent: str) -> dict[str, Any]:
    """Wire shape for ApplyEvent. Not submitted."""
    return {
        "parent": parent,
        "rule_id": post.rule_id,
        "event_id": post.event_id,
        "amount": post.amount,
        "trade_date": {
            "year": post.trade_date.year,
            "month": post.trade_date.month,
            "day": post.trade_date.day,
        },
        "validate_only": True,
    }


def csv_billing(statement: Statement) -> str:
    """Vendor-facing billing / retainage / collections. Blanks are unset."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(("Figure", "Amount", "Note"))
    rows = (
        (
            "billed",
            format_optional(statement.billed),
            "Progress billings credit; unset until a bill posts — not billed-zero",
        ),
        (
            "earned",
            format_optional(statement.earned),
            "Project revenue; billed is not a substitute",
        ),
        (
            "billed_minus_earned",
            format_optional(statement.billed_minus_earned),
            "unset until both billed and earned have posted",
        ),
        (
            "retainage_receivable",
            format_optional(statement.retainage_receivable),
            "unset on the line until a hold posts; no hold is 0 for collections subtraction",
        ),
        (
            "retainage_payable",
            format_optional(statement.retainage_payable),
            "vendor holdback; unset until hold_vendor_retainage posts",
        ),
        (
            "remaining_to_bill",
            format_optional(statement.remaining_to_bill),
            "revised − billed; treating billed as 0 would print the whole contract",
        ),
        (
            "collections",
            format_optional(statement.collections),
            "billed − AR − retainage held; unset billed or unset AR cannot support the cut",
        ),
    )
    for row in rows:
        w.writerow(row)
    return buf.getvalue()


def csv_budget(statement: Statement) -> str:
    """Contract / remaining-to-spend cite. Blanks are unset."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(("Figure", "Amount", "Note"))
    rows = (
        (
            "original",
            format_optional(statement.original),
            "[project] budget; CreateBook does not invent a baseline",
        ),
        (
            "approved_change_orders",
            format_optional(statement.approved_change_orders),
            "unposted is unset, not a silent net of nothing",
        ),
        (
            "revised",
            format_optional(statement.revised),
            "original + approved when the original is set",
        ),
        (
            "remaining_to_spend",
            format_optional(statement.remaining_to_spend),
            "revised − incurred − awarded; treating awarded as 0 invents headroom",
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
    """Named companion sheets. Not a vendor directory and not a G702 pack."""
    return {
        "billing.csv": csv_billing(statement),
        "budget.csv": csv_budget(statement),
        "unset.csv": csv_unset(statement),
    }
