#!/usr/bin/env python3
"""Bank-balance predictor → forecast journal material for BookKind PERSONAL.

A WorkOS Connect app, not a kernel RPC. Predicted movements live here.
They do not live in `ratio watch`, the operations console, or a new
kernel method. `/cashflow` already cites posted `forecast_*` /
`scheduled_*` kinds (`filter=forecast-YYYY[-MM]`); unset when none.

⭐ SCOPES ARE THE FROZEN CATALOG NAMES. The issue body still says
`journal:append`. That string is an alias and is refused. Canonical
write grant: `journals:post`. See docs/connect-scopes.md.

⭐ `journals:post` IS AN ALLOWLIST PER client_id. An empty allowlist
refuses every post. Silence is not "all templates".

⭐ THIS APP POSTS `forecast_*` ONLY. CreateBook(Personal) seeded
`forecast_income` / `forecast_spend`. ApplyEvent marks
`JournalEntry.kind` from the rule-id prefix. A future-dated
`spend_cash` is still an actual and is refused here. `scheduled_*`
is the calendar-bills sibling.

⭐ MONEY IS MINOR UNITS, PARSED BY SPLITTING ON THE POINT. A float is
how a cent disappears. A third decimal place is refused. Instantiated
legs that would wrap i64 are refused before a product is asked
anything — same reason as Ratio.Bounded.

⭐ UNSET STAYS UNSET. An empty predicted batch is not a measured $0.00
forecast. A posted income and spend that net to zero is a real zero.
Cited cash that is unset is not a silent 0.00 baseline for a
predicted ending balance.

⛔ PAYROLL AND ENVELOPE ARE NOT INVENTED. No `forecast_payroll`,
no `forecast_envelope`, no envelope chrome. #164 stays refused.

⭐ THE GRANT PATH CALLS CONNECTAPIURL. `fetch_statements` and
`deliver` present a verified Connect access token against the
Connect HTTP API. Membership is still required. Live bank OAuth
stays leftover on #163. WorkOS dashboard registration stays leftover #22.

⚠ LIVE BANK OAUTH IS NOT WIRED. A normalized predicted movement
(or a predicted ending balance) is the input. Plaid / MX / TrueLayer
stay leftover on #163.
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

CANONICAL_SCOPES = frozenset({"statements:read", "journals:post"})
REFUSED_ALIASES = frozenset({"journal:append", "journal:read"})

# CreateBook(Personal) forecast templates this app may instantiate.
# Legs are (account_dim, weight). Same ids book.rs seeds; same
# cash/expense and cash/income pairs as spend_cash / receive_income.
FORECAST_LEGS: dict[str, tuple[tuple[int, int], tuple[int, int]]] = {
    "forecast_spend": ((10, 1), (1, -1)),
    "forecast_income": ((1, 1), (30, -1)),
}

KIND_TO_TEMPLATE = {
    "spend": "forecast_spend",
    "expense": "forecast_spend",
    "income": "forecast_income",
}

# What CreateBook writes on a Personal book. A client_id that lists
# `call_lp` or `forecast_payroll` is refused — those rules were never
# seeded. The allowlist is the two forecast templates; the rest is
# here so a missing RuleSet check can name the invention.
PERSONAL_SEEDED_RULES = frozenset(FORECAST_LEGS) | frozenset(
    {
        "living_expense",
        "household_income",
        "card_charge",
        "spend_cash",
        "spend_card",
        "receive_income",
        "pay_tax",
        "scheduled_spend",
        "scheduled_income",
        "mortgage_interest",
        "mortgage_principal",
        "auto_interest",
        "auto_principal",
        "student_interest",
        "student_principal",
    }
)

REFUSED_KINDS = frozenset(
    {
        "payroll",
        "paycheck",
        "salary",
        "wage",
        "envelope",
        "forecast_payroll",
        "scheduled_payroll",
        "forecast_envelope",
        "scheduled_envelope",
        "card",
        "transfer",
        "living_expense",
        "household_income",
        "spend_cash",
        "receive_income",
        "call_lp",
        "scheduled_spend",
        "scheduled_income",
    }
)

CASH_ACCOUNT = 1


class Refuse(Exception):
    """The prediction is not proposed. Message is the reason, not a workaround."""


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
    approved_templates: frozenset[str] = field(
        default_factory=lambda: PERSONAL_SEEDED_RULES
    )
    closed_through: date | None = None


@dataclass(frozen=True)
class Client:
    client_id: str
    allowlist: frozenset[str]
    scopes: frozenset[str]


@dataclass(frozen=True)
class Statement:
    """A statements:read cite. Unset stays unset — not a silent zero."""

    currency: str
    as_of: date | None = None
    cash: int | None = None


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
            "inference cannot silently flip income and a spend"
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


def parse_signed_minor(text: str) -> int:
    """A cited or predicted sheet figure. Sign is allowed; zero is a figure."""
    if not isinstance(text, str):
        raise Refuse(
            f"{text!r} is not an amount string — a number typed as a float "
            "is how a cent disappears"
        )
    t = text.strip().replace(",", "").replace("$", "")
    sign = -1 if t.startswith("-") else 1
    if t.startswith("+") or t.startswith("-"):
        t = t[1:]
    if t in ("", "0", "0.0", "0.00"):
        return 0
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
    major = int(whole) if whole else 0
    if len(frac) == 0:
        minor = 0
    elif len(frac) == 1:
        minor = int(frac) * 10
    else:
        minor = int(frac)
    if major > I64_MAX // 100:
        raise Refuse(f"{text!r} does not fit in i64 minor units")
    n = sign * (major * 100 + minor)
    if n > I64_MAX or n < I64_MIN:
        raise Refuse(f"{text!r} does not fit in i64 minor units")
    return n


def format_minor(n: int) -> str:
    """Decimal string ApplyEvent wants — never a float, never scientific."""
    if n < 0:
        raise Refuse("ApplyEvent amount is a magnitude; the rule sets direction")
    whole, frac = divmod(n, 100)
    return f"{whole}.{frac:02d}"


def parse_day(text: str) -> date:
    if not isinstance(text, str) or not text.strip():
        raise Refuse(
            "an undated predicted row is refused — it cannot honor closed-through"
        )
    t = text.strip()
    try:
        y, m, d = t.split("-")
        return date(int(y), int(m), int(d))
    except ValueError as e:
        raise Refuse(f"{text!r} is not a calendar day YYYY-MM-DD") from e


def checked_add(a: int, b: int) -> int:
    """a + b, refused on i64 wrap. Asked before the sum is treated as a figure."""
    if a > I64_MAX or a < I64_MIN or b > I64_MAX or b < I64_MIN:
        raise Refuse("addend does not fit in i64")
    total = a + b
    if total > I64_MAX or total < I64_MIN:
        raise Refuse("sum does not fit in i64 minor units")
    return total


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
    if amount == 0:
        return 0
    return -amount


def instantiate(rule_id: str, amount: int, currency: str) -> tuple[Posting, ...]:
    legs = FORECAST_LEGS.get(rule_id)
    if legs is None:
        raise Refuse(
            f"{rule_id!r} is not a forecast template this app instantiates — "
            "it does not invent a Method, an Order, a lot_method variant, "
            "payroll, or an envelope"
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


def cash_delta(postings: Sequence[Posting]) -> int:
    """Δ cash from instantiated legs. Cash is account 1."""
    delta = 0
    currencies = {p.currency for p in postings}
    if len(currencies) > 1:
        raise Refuse(
            "a forecast move is one currency — [USD +100, EUR −100] is "
            "not a cash-forecast plug"
        )
    for p in postings:
        if p.account == CASH_ACCOUNT:
            delta = checked_add(delta, p.amount)
    return delta


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
            "statements:read is how closed-through and cited cash are read; "
            "without it the app cannot honor a close or an unset sheet"
        )


def _require_personal(book: Book) -> None:
    if book.kind != "PERSONAL":
        raise Refuse(
            f"this app is BookKind PERSONAL; {book.kind!r} keeps its own "
            "chrome and is not a household forecast book. Project EAC is "
            "connect/eac-forecast/"
        )


def _template_for(row: Mapping[str, Any]) -> str:
    kind = str(row.get("kind") or "").strip().lower()
    if not kind:
        raise Refuse(
            "Kind picks the rule so a signed-amount inference cannot "
            "silently flip income and a spend"
        )
    if kind in REFUSED_KINDS:
        raise Refuse(
            f"kind {kind!r} is not a forecast movement this app maps — "
            "payroll and envelope kinds are not invented, scheduled_* "
            "is the calendar-bills sibling, and actuals stay actuals"
        )
    rule = KIND_TO_TEMPLATE.get(kind)
    if rule is None:
        raise Refuse(
            f"kind {kind!r} is not a household forecast kind this app maps — "
            "it does not invent a Method or post call_lp onto a Personal book"
        )
    return rule


def _event_id(row: Mapping[str, Any], index: int) -> str:
    raw = str(row.get("reference") or row.get("event_id") or f"forecast-{index + 1}")
    ident = raw.strip()
    if not ident or len(ident) > 64:
        raise Refuse(f"{ident!r} is not an event id")
    if not all(c.isalnum() or c in "-_." for c in ident):
        raise Refuse(
            f"{ident!r} is not an event id — letters, digits, - _ . and at most 64"
        )
    return ident


def statement_from_cite(raw: Mapping[str, Any] | None) -> Statement:
    """Read a statements:read cash cite. Omitted cash stays unset."""
    if raw is None:
        return Statement(currency="USD")
    currency = str(raw.get("currency") or "USD").strip().upper()
    if len(currency) != 3 or not currency.isalpha():
        raise Refuse(f"{currency!r} is not an ISO currency code")
    as_of = raw.get("as_of")
    cash_raw = raw.get("cash")
    cash: int | None
    if cash_raw is None or cash_raw == "":
        cash = None
    elif isinstance(cash_raw, bool):
        raise Refuse("cash is an amount, not a flag")
    elif isinstance(cash_raw, int):
        if cash_raw > I64_MAX or cash_raw < I64_MIN:
            raise Refuse("cash does not fit in i64 minor units")
        cash = cash_raw
    elif isinstance(cash_raw, str):
        cash = parse_signed_minor(cash_raw)
    else:
        raise Refuse(
            "cash is an amount string — a number typed as a float "
            "is how a cent disappears"
        )
    return Statement(
        currency=currency,
        as_of=parse_day(as_of) if as_of else None,
        cash=cash,
    )


def _propose(
    rule: str,
    amount: int,
    currency: str,
    day: date,
    event_id: str,
    *,
    book: Book,
    client: Client,
) -> ProposedPost:
    if not client.allowlist:
        raise Refuse(
            "empty journals:post allowlist refuses every post — silence "
            "is not all templates"
        )
    if rule not in client.allowlist:
        raise Refuse(f"{rule} is not on client {client.client_id!r}'s allowlist")
    if rule not in book.approved_templates:
        raise Refuse(
            f"{rule} is not in this book's approved RuleSet — "
            "CreateBook(Personal) never wrote it"
        )
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
    return ProposedPost(
        rule_id=rule,
        amount=format_minor(amount),
        currency=currency,
        trade_date=day,
        event_id=event_id,
        postings=posts,
    )


def map_batch(
    rows: Iterable[Mapping[str, Any]],
    *,
    book: Book,
    client: Client,
) -> list[ProposedPost]:
    """Map predicted movements to proposed forecast_* posts, or refuse the lot.

    One refusal fails the batch. A closed row next to an open one must
    not partial-post into the closed period.
    """
    _require_scopes(client)
    _require_personal(book)

    proposed: list[ProposedPost] = []
    for i, row in enumerate(rows):
        rule = _template_for(row)
        amount = parse_minor(row.get("amount", ""))
        currency = str(row.get("currency") or "").strip().upper()
        day = parse_day(str(row.get("dated") or ""))
        proposed.append(
            _propose(
                rule,
                amount,
                currency,
                day,
                _event_id(row, i),
                book=book,
                client=client,
            )
        )
    return proposed


def from_predicted_balance(
    *,
    predicted: str,
    dated: str,
    statement: Statement | Mapping[str, Any] | None,
    book: Book,
    client: Client,
    reference: str = "predicted-balance",
) -> ProposedPost:
    """One forecast post from (cited cash, predicted ending balance).

    ⛔ UNSET CASH IS NOT A SILENT 0.00 BASELINE. A predictor that
    treated an empty sheet as zero would invent the movement the
    core cite already refused to fake.
    """
    _require_scopes(client)
    _require_personal(book)
    cited = (
        statement
        if isinstance(statement, Statement)
        else statement_from_cite(statement)
    )
    if cited.cash is None:
        raise Refuse(
            "cited cash is unset — a predicted ending balance cannot "
            "invent a 0.00 baseline. Unset stays unset"
        )
    predicted_cash = parse_signed_minor(predicted)
    currency = cited.currency
    delta = checked_add(predicted_cash, -cited.cash)
    if delta == 0:
        raise Refuse(
            "predicted ending balance equals cited cash — a zero "
            "amount is not a posting"
        )
    if delta > 0:
        rule = "forecast_income"
        amount = delta
    else:
        rule = "forecast_spend"
        amount = -delta
    day = parse_day(dated)
    return _propose(
        rule,
        amount,
        currency,
        day,
        _event_id({"reference": reference}, 0),
        book=book,
        client=client,
    )


def cite_forecast_net(posts: Sequence[ProposedPost]) -> int | None:
    """What `/cashflow` would fold from these proposals.

    Empty is unset — not a measured $0.00. A net-zero pair of posts
    is a real zero.
    """
    if not posts:
        return None
    net = 0
    for post in posts:
        net = checked_add(net, cash_delta(post.postings))
    return net


def envelope_budget(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Envelope invention stays on the #164 refusal boundary."""
    raise Refuse(
        "envelope invention stays refused — #164 is not this door and "
        "this app does not rebuild envelope chrome"
    )


def payroll(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Payroll kinds are not invented."""
    raise Refuse(
        "payroll kinds are not invented — CreateBook(Personal) never "
        "wrote forecast_payroll, and this app does not mint it"
    )


def fetch_statements(
    *,
    token: str | None = None,
    book_id: str | None = None,
    transport: _grant.Transport | None = None,
) -> Any:
    """Pull statement cites from ConnectApiUrl."""
    return _grant.pull(
        token=token,
        book_id=book_id,
        transport=transport,
        error=Refuse,
    )


def deliver(
    posts: Sequence[ProposedPost],
    *,
    token: str | None = None,
    parent: str | None = None,
    transport: _grant.Transport | None = None,
) -> list[Any]:
    """POST allowlisted forecast_* ApplyEvent bodies to ConnectApiUrl."""
    return _grant.deliver_apply_events(
        posts,
        as_apply_event=as_apply_event,
        token=token,
        parent=parent,
        transport=transport,
        error=Refuse,
    )


def as_apply_event(post: ProposedPost, *, parent: str) -> dict[str, Any]:
    """Wire shape for ApplyEvent. Not submitted.

    Kind is not a proto field — ApplyEvent marks it from the rule-id
    prefix (`forecast_*` → forecast). validate_only stays true.
    """
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
