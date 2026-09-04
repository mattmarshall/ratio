#!/usr/bin/env python3
"""Bank-feed → journal mapper for BookKind PERSONAL.

A WorkOS Connect app, not a kernel RPC. The feed lives here; Ratio's
core stays the journal append ACL and the closed-through gate already
in the store.

⭐ SCOPES ARE THE FROZEN CATALOG NAMES. `journal:append` is an alias
and is refused. See docs/connect-scopes.md.

⭐ `journals:post` IS AN ALLOWLIST PER client_id. An empty allowlist
refuses every post. Silence is not "all templates".

⭐ MONEY IS MINOR UNITS, PARSED BY SPLITTING ON THE POINT. A float is
how a cent disappears (`1.005 * 100` is 100.4999…). A third decimal
place is refused. Instantiated legs that would wrap i64 are refused
before a product is asked anything — same reason as Ratio.Bounded.

⭐ CONSERVATION IS PER CURRENCY. `[USD +100, EUR −100]` is not a
balanced entry. A Personal template is two legs of opposite weight;
the mapper instantiates those legs and checks them. It does not send
a posting list the kernel would have to trust.

⭐ THE GRANT PATH CALLS CONNECTAPIURL. `deliver` presents a verified
Connect access token and POSTs allowlisted ApplyEvent bodies to the
Connect HTTP API. Membership is still required. Live bank OAuth
stays leftover on #165. WorkOS dashboard registration stays leftover #22.

⚠ LIVE BANK OAUTH IS NOT WIRED. A normalized row is the input. Plaid /
MX / TrueLayer stay leftover on #165.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from datetime import date
from typing import Any, Iterable, Mapping, Sequence

import grant as _grant

# i64 bounds. Lean's Int is unbounded; ApplyEvent runs on i64.
I64_MIN = -(2**63)
I64_MAX = 2**63 - 1

CANONICAL_SCOPES = frozenset({"books:read", "statements:read", "journals:post"})
REFUSED_ALIASES = frozenset({"journal:append", "journal:read"})

# CreateBook(Personal) posting rules this app may instantiate.
# Legs are (account_dim, weight). Weight is ±1; the amount's magnitude
# is applied, never a signed-amount inference — same reason the
# bank-statement ingest template has a Kind column.
PERSONAL_LEGS: dict[str, tuple[tuple[int, int], tuple[int, int]]] = {
    "living_expense": ((10, 1), (1, -1)),
    "household_income": ((1, 1), (30, -1)),
    "card_charge": ((10, 1), (40, -1)),
    "xfer_cash_investments": ((2, 1), (1, -1)),
    "xfer_investments_cash": ((1, 1), (2, -1)),
    "xfer_cash_cards": ((40, 1), (1, -1)),
    "xfer_cards_cash": ((1, 1), (40, -1)),
    "xfer_investments_cards": ((40, 1), (2, -1)),
    "xfer_cards_investments": ((2, 1), (40, -1)),
    "spend_cash": ((10, 1), (1, -1)),
    "spend_card": ((10, 1), (40, -1)),
    "receive_income": ((1, 1), (30, -1)),
}

KIND_TO_TEMPLATE = {
    "expense": "living_expense",
    "income": "household_income",
    "card": "card_charge",
    "spend_cash": "spend_cash",
    "spend_card": "spend_card",
    "receive_income": "receive_income",
}

TRANSFER_POCKETS = ("cash", "investments", "cards")

# What CreateBook writes on a Personal book. A client_id that lists
# `call_lp` is refused — that rule was never seeded.
PERSONAL_SEEDED_RULES = frozenset(PERSONAL_LEGS) | frozenset(
    {
        "pay_tax",
        "mortgage_interest",
        "mortgage_principal",
        "auto_interest",
        "auto_principal",
        "student_interest",
        "student_principal",
    }
)


class Refuse(Exception):
    """The batch is not proposed. Message is the reason, not a workaround."""


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


@dataclass(frozen=True)
class Book:
    kind: str
    approved_templates: frozenset[str] = field(default_factory=lambda: PERSONAL_SEEDED_RULES)
    closed_through: date | None = None


@dataclass(frozen=True)
class Client:
    client_id: str
    allowlist: frozenset[str]
    scopes: frozenset[str]


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


def parse_minor(text: str) -> int:
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
    if t.startswith("-"):
        raise Refuse(
            f"{text!r} is signed; Kind picks the rule so a signed-amount "
            "inference cannot silently flip income and a card charge"
        )
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
    if v > I64_MAX or v < 0:
        raise Refuse(f"{text!r} does not fit in i64 minor units")
    if v == 0:
        raise Refuse("a zero amount is not a posting")
    return v


def format_minor(n: int) -> str:
    """Decimal string ApplyEvent wants — never a float, never scientific."""
    if n < 0:
        raise Refuse("ApplyEvent amount is a magnitude; the rule sets direction")
    whole, frac = divmod(n, 100)
    return f"{whole}.{frac:02d}"


def parse_day(text: str) -> date:
    if not isinstance(text, str) or not text.strip():
        raise Refuse("an undated feed row is refused — it cannot honor closed-through")
    t = text.strip()
    try:
        y, m, d = t.split("-")
        return date(int(y), int(m), int(d))
    except ValueError as e:
        raise Refuse(f"{text!r} is not a calendar day YYYY-MM-DD") from e


def checked_scale(amount: int, weight: int) -> int:
    """amount × weight, refused on i64 wrap. Asked before the product."""
    if amount < 0 or amount > I64_MAX:
        raise Refuse("amount does not fit in i64")
    if weight not in (-1, 1):
        raise Refuse(
            f"weight {weight} is not ±1 — a Personal template does not "
            "scale, and this app does not invent a Method"
        )
    if weight == 1:
        return amount
    # weight == -1. i64::MIN has no representable negation; amounts here
    # are positive, so -amount is in range when amount <= I64_MAX.
    if amount == 0:
        return 0
    return -amount


def instantiate(rule_id: str, amount: int, currency: str) -> tuple[Posting, ...]:
    legs = PERSONAL_LEGS.get(rule_id)
    if legs is None:
        raise Refuse(
            f"{rule_id!r} is not a Personal template this app instantiates — "
            "it does not invent a Method, an Order, or a lot_method variant"
        )
    if not currency or not isinstance(currency, str):
        raise Refuse("a posting names a currency; guessing the base is how a NAV adds dollars to euros")
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
        # Use a wider accumulator so a pair of i64 values cannot wrap
        # the test itself.
        nets[p.currency] = nets.get(p.currency, 0) + p.amount
    return bool(postings) and all(n == 0 for n in nets.values())


def _require_scopes(client: Client) -> None:
    aliases = client.scopes & REFUSED_ALIASES
    if aliases:
        raise Refuse(
            "refused alias scope "
            + ", ".join(sorted(aliases))
            + " — catalogs use journals:post, not journal:append"
        )
    extra = client.scopes - CANONICAL_SCOPES
    if extra:
        raise Refuse(
            "unknown scope "
            + ", ".join(sorted(extra))
            + " — a string that is not in docs/connect-scopes.md is refused"
        )
    missing = CANONICAL_SCOPES - client.scopes
    if missing:
        raise Refuse(
            "this app needs "
            + ", ".join(sorted(CANONICAL_SCOPES))
            + f"; missing {', '.join(sorted(missing))}. "
            "statements:read is how closed-through is read; without it the "
            "app cannot honor a close"
        )


def _template_for(row: Mapping[str, Any]) -> str:
    kind = str(row.get("kind") or "").strip().lower()
    if not kind:
        raise Refuse(
            "Kind picks the rule so a signed-amount inference cannot "
            "silently flip income and a card charge"
        )
    if kind == "transfer":
        src = str(row.get("from") or "").strip().lower()
        dst = str(row.get("to") or "").strip().lower()
        if src not in TRANSFER_POCKETS or dst not in TRANSFER_POCKETS:
            raise Refuse(
                f"a transfer names from/to in {TRANSFER_POCKETS}, not {src!r} → {dst!r}"
            )
        if src == dst:
            raise Refuse("a transfer to the same pocket is not a posting")
        return f"xfer_{src}_{dst}"
    rule = KIND_TO_TEMPLATE.get(kind)
    if rule is None:
        raise Refuse(
            f"kind {kind!r} is not a household feed kind this app maps — "
            "it does not invent a Method or post call_lp onto a Personal book"
        )
    return rule


def _event_id(row: Mapping[str, Any], index: int) -> str:
    raw = str(row.get("reference") or row.get("event_id") or f"feed-{index + 1}")
    ident = raw.strip()
    if not ident or len(ident) > 64:
        raise Refuse(f"{ident!r} is not an event id")
    if not all(c.isalnum() or c in "-_." for c in ident):
        raise Refuse(
            f"{ident!r} is not an event id — letters, digits, - _ . and at most 64"
        )
    return ident


def map_batch(
    rows: Iterable[Mapping[str, Any]],
    *,
    book: Book,
    client: Client,
) -> list[ProposedPost]:
    """Map a feed batch to proposed ApplyEvent posts, or refuse the lot.

    One refusal fails the batch. A closed row next to an open one must
    not partial-post into the closed period.
    """
    _require_scopes(client)
    if book.kind != "PERSONAL":
        raise Refuse(
            f"this app is BookKind PERSONAL; {book.kind!r} keeps its own "
            "chrome and is not a household feed"
        )
    if not client.allowlist:
        raise Refuse(
            "empty journals:post allowlist refuses every post — silence "
            "is not all templates"
        )

    proposed: list[ProposedPost] = []
    for i, row in enumerate(rows):
        rule = _template_for(row)
        if rule not in client.allowlist:
            raise Refuse(
                f"{rule} is not on client {client.client_id!r}'s allowlist"
            )
        if rule not in book.approved_templates:
            raise Refuse(
                f"{rule} is not in this book's approved RuleSet — "
                "CreateBook(Personal) never wrote it"
            )
        amount = parse_minor(row.get("amount", ""))
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


def deliver(
    posts: Sequence[ProposedPost],
    *,
    token: str | None = None,
    parent: str | None = None,
    transport: _grant.Transport | None = None,
) -> list[Any]:
    """POST allowlisted ApplyEvent bodies to ConnectApiUrl."""
    return _grant.deliver_apply_events(
        posts,
        as_apply_event=as_apply_event,
        token=token,
        parent=parent,
        transport=transport,
        error=Refuse,
    )


def as_apply_event(post: ProposedPost, *, parent: str) -> dict[str, Any]:
    """Wire shape for ApplyEvent. deliver() submits this to ConnectApiUrl."""
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
