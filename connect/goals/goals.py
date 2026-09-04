#!/usr/bin/env python3
"""Net-worth goals and what-if scenarios for BookKind PERSONAL.

A WorkOS Connect app, not a kernel RPC. Goals and scenario overlays
live here. They do not live in `ratio watch`, the operations console,
or a new kernel method. Sheet, bridge, and cash-flow stay core
(`statements:read`). This app cites those figures; it does not grow
them.

⭐ SCOPES ARE THE FROZEN CATALOG NAMES. `journal:append` is an alias
and is refused. Canonical write grant: `journals:post`. See
docs/connect-scopes.md.

⭐ `journals:post` IS AN ALLOWLIST PER client_id. An empty allowlist
refuses every post. Silence is not "all templates".

⭐ SCENARIO JOURNALS POST ONLY ON OPT-IN. Overlaying a what-if on a
cited sheet is a read. Instantiating CreateBook(Personal) templates
and proposing ApplyEvent is a write. Without opt-in the write must
not happen.

⭐ MONEY IS MINOR UNITS, PARSED BY SPLITTING ON THE POINT. A float is
how a cent disappears. A third decimal place is refused. Instantiated
legs that would wrap i64 are refused before a product is asked
anything — same reason as Ratio.Bounded.

⭐ UNSET STAYS UNSET. An empty journal is not a measured $0.00 net
worth. A goal against an unset sheet stays unset, not 0% progress.
A real zero (assets equal liabilities) is a figure.

⭐ THIS IS NOT A CASH FORECAST AND NOT A FIRE NUMBER. A scenario is
discrete hypothetical posts on already-seeded Personal templates.
Required monthly savings, a compounding path, and a retirement
number are refused — PLAN already named those as cannot-show.

⚠ THE GRANT PATH IS NOT BUILT. `fetch_statements` and `deliver`
refuse. A Connect access token is not accepted on `/v1`
(#150 / #151 / leftover #22).

⚠ CLOSED-THROUGH IS A GATE ON WRITES. A dated opt-in post on or
before the book's closed-through day refuses the batch. An overlay
is not a mutation.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from datetime import date
from typing import Any, Iterable, Mapping, Sequence

# i64 bounds. Lean's Int is unbounded; ApplyEvent runs on i64.
I64_MIN = -(2**63)
I64_MAX = 2**63 - 1

CANONICAL_SCOPES = frozenset({"statements:read", "journals:post"})
READ_SCOPES = frozenset({"statements:read"})
REFUSED_ALIASES = frozenset({"journal:append", "journal:read"})

# CreateBook(Personal) posting rules this app may instantiate.
# Legs are (account_dim, weight). Weight is ±1; the amount's magnitude
# is applied. Same ids bank-feed already holds to book.rs.
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

# chart_for(Personal) sheet accounts. Debit-positive amount on an
# asset or a liability is +ΔNW (pay down a card, buy an investment).
# Income / expense / equity are the other side of a conserved entry
# and must not be counted twice.
SHEET_ACCOUNTS = frozenset({1, 2, 40, 41, 42, 43})
CASH_ACCOUNT = 1


class Refuse(Exception):
    """The goal or scenario is not proposed. Message is the reason, not a workaround."""


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


@dataclass(frozen=True)
class Statement:
    """A statements:read cite. Unset stays unset — not a silent zero."""

    currency: str
    as_of: date | None = None
    net_worth: int | None = None
    cash: int | None = None
    beginning_net_worth: int | None = None
    ending_net_worth: int | None = None
    beginning_cash: int | None = None
    ending_cash: int | None = None
    operating: int | None = None
    investing: int | None = None
    financing: int | None = None


@dataclass(frozen=True)
class Goal:
    name: str
    target: int
    target_date: date
    currency: str


@dataclass(frozen=True)
class Progress:
    """Current sheet vs target. No percentage — that is a rounded figure."""

    name: str
    current: int | None
    target: int
    gap: int | None
    status: str
    currency: str
    as_of: date | None
    target_date: date


@dataclass(frozen=True)
class Overlay:
    """What-if on a cited sheet. Not a journal write and not a forecast."""

    projected_net_worth: int | None
    projected_cash: int | None
    net_worth_delta: int
    cash_delta: int
    currency: str
    posts: tuple[ProposedPost, ...]


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
    """Decimal string — never a float, never scientific."""
    sign = "-" if n < 0 else ""
    n = abs(n)
    if n > I64_MAX:
        raise Refuse("amount does not fit in i64 minor units")
    whole, frac = divmod(n, 100)
    return f"{sign}{whole}.{frac:02d}"


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
    legs = PERSONAL_LEGS.get(rule_id)
    if legs is None:
        raise Refuse(
            f"{rule_id!r} is not a Personal template this app instantiates — "
            "it does not invent a Method, an Order, or a lot_method variant"
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


def sheet_delta(postings: Sequence[Posting]) -> tuple[int, int]:
    """(Δ net worth, Δ cash) from instantiated legs.

    Debit-positive on a sheet account is +ΔNW. Cash is account 1.
    Income / expense / equity are the conserved other side and are
    not counted again — counting them would double a transfer into
    a fake gain.
    """
    nw = 0
    cash = 0
    currencies = {p.currency for p in postings}
    if len(currencies) > 1:
        raise Refuse(
            "a scenario move is one currency — [USD +100, EUR −100] is "
            "not a net-worth plug"
        )
    for p in postings:
        if p.account in SHEET_ACCOUNTS:
            nw = checked_add(nw, p.amount)
        if p.account == CASH_ACCOUNT:
            cash = checked_add(cash, p.amount)
    return nw, cash


def _refuse_aliases(client: Client) -> None:
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


def _require_read_scopes(client: Client) -> None:
    _refuse_aliases(client)
    if "statements:read" not in client.scopes:
        raise Refuse(
            "this app needs statements:read — sheet / bridge / cash-flow "
            "are the cites a goal is measured against. Without them the "
            "app would invent a net worth"
        )


def _require_post_scopes(client: Client) -> None:
    _require_read_scopes(client)
    if "journals:post" not in client.scopes:
        raise Refuse(
            "scenario journals need journals:post — statements:read is "
            "the cite, not a write grant"
        )


def _require_personal(book: Book) -> None:
    if book.kind != "PERSONAL":
        raise Refuse(
            f"this app is BookKind PERSONAL; {book.kind!r} keeps its own "
            "chrome and is not a household goals book"
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
            f"kind {kind!r} is not a household scenario kind this app maps — "
            "it does not invent a Method or post call_lp onto a Personal book"
        )
    return rule


def _event_id(row: Mapping[str, Any], index: int) -> str:
    raw = str(row.get("reference") or row.get("event_id") or f"scenario-{index + 1}")
    ident = raw.strip()
    if not ident or len(ident) > 64:
        raise Refuse(f"{ident!r} is not an event id")
    if not all(c.isalnum() or c in "-_." for c in ident):
        raise Refuse(
            f"{ident!r} is not an event id — letters, digits, - _ . and at most 64"
        )
    return ident


def statement_from_cite(raw: Mapping[str, Any] | None) -> Statement:
    """Read a statements:read cite. Omitted figures stay unset."""

    if raw is None:
        return Statement(currency="USD")

    def _opt_minor(key: str) -> int | None:
        v = raw.get(key)
        if v is None or v == "":
            return None
        if isinstance(v, bool):
            raise Refuse(f"{key} is an amount, not a flag")
        if isinstance(v, int):
            if v > I64_MAX or v < I64_MIN:
                raise Refuse(f"{key} does not fit in i64 minor units")
            return v
        if not isinstance(v, str):
            raise Refuse(
                f"{key} is an amount string — a number typed as a float "
                "is how a cent disappears"
            )
        t = v.strip().replace(",", "").replace("$", "")
        sign = -1 if t.startswith("-") else 1
        if t.startswith("+") or t.startswith("-"):
            t = t[1:]
        # A cited sheet may be a real zero or a real negative NW.
        # parse_minor refuses those for *posts*; cites are different.
        if t in ("", "0", "0.0", "0.00"):
            return 0
        # Reuse the splitter via a local that allows sign and zero.
        if "." in t:
            whole, _, frac = t.partition(".")
            if "." in frac:
                raise Refuse(f"{v!r} is not an amount")
        else:
            whole, frac = t, ""
        if frac and len(frac) > 2:
            raise Refuse(
                f"{v!r} has more than two decimal places; the books are kept "
                "in minor units"
            )
        if (whole and not whole.isdigit()) or (frac and not frac.isdigit()):
            raise Refuse(f"{v!r} is not an amount")
        major = int(whole) if whole else 0
        if len(frac) == 0:
            minor = 0
        elif len(frac) == 1:
            minor = int(frac) * 10
        else:
            minor = int(frac)
        if major > I64_MAX // 100:
            raise Refuse(f"{v!r} does not fit in i64 minor units")
        n = major * 100 + minor
        n = sign * n
        if n > I64_MAX or n < I64_MIN:
            raise Refuse(f"{v!r} does not fit in i64 minor units")
        return n

    currency = str(raw.get("currency") or "USD").strip().upper()
    if len(currency) != 3 or not currency.isalpha():
        raise Refuse(f"{currency!r} is not an ISO currency code")
    as_of = raw.get("as_of")
    return Statement(
        currency=currency,
        as_of=parse_day(as_of) if as_of else None,
        net_worth=_opt_minor("net_worth"),
        cash=_opt_minor("cash"),
        beginning_net_worth=_opt_minor("beginning_net_worth"),
        ending_net_worth=_opt_minor("ending_net_worth"),
        beginning_cash=_opt_minor("beginning_cash"),
        ending_cash=_opt_minor("ending_cash"),
        operating=_opt_minor("operating"),
        investing=_opt_minor("investing"),
        financing=_opt_minor("financing"),
    )


def goal_from_cite(raw: Mapping[str, Any]) -> Goal:
    name = str(raw.get("name") or "").strip()
    if not name:
        raise Refuse("a goal names itself — silence is not a household target")
    currency = str(raw.get("currency") or "").strip().upper()
    if len(currency) != 3 or not currency.isalpha():
        raise Refuse(f"{currency!r} is not an ISO currency code")
    target = parse_minor(str(raw.get("target") or ""))
    return Goal(
        name=name,
        target=target,
        target_date=parse_day(str(raw.get("target_date") or "")),
        currency=currency,
    )


def evaluate_goal(
    goal: Goal | Mapping[str, Any],
    *,
    statement: Statement | Mapping[str, Any] | None,
    book: Book,
    client: Client,
) -> Progress:
    """Cite sheet net worth against a named target.

    ⛔ NO PERCENTAGE AND NO REQUIRED-SAVINGS RATE. Those are rounded
    or forecasted figures. Current, target, and gap are the cites.
    """
    _require_read_scopes(client)
    _require_personal(book)
    resolved = goal if isinstance(goal, Goal) else goal_from_cite(goal)
    cited = (
        statement
        if isinstance(statement, Statement)
        else statement_from_cite(statement)
    )
    if cited.currency != resolved.currency:
        raise Refuse(
            f"goal is {resolved.currency} and the sheet cite is "
            f"{cited.currency} — adding them is how a NAV adds dollars to euros"
        )
    current = cited.net_worth
    if current is None:
        return Progress(
            name=resolved.name,
            current=None,
            target=resolved.target,
            gap=None,
            status="unset",
            currency=resolved.currency,
            as_of=cited.as_of,
            target_date=resolved.target_date,
        )
    gap = checked_add(resolved.target, -current)
    status = "met" if gap <= 0 else "short"
    return Progress(
        name=resolved.name,
        current=current,
        target=resolved.target,
        gap=gap,
        status=status,
        currency=resolved.currency,
        as_of=cited.as_of,
        target_date=resolved.target_date,
    )


def _instantiate_moves(
    rows: Iterable[Mapping[str, Any]],
    *,
    book: Book,
    client: Client,
    honor_closed_through: bool,
) -> list[ProposedPost]:
    proposed: list[ProposedPost] = []
    for i, row in enumerate(rows):
        rule = _template_for(row)
        if honor_closed_through:
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
                "CreateBook(Personal) never wrote it"
            )
        amount = parse_minor(row.get("amount", ""))
        currency = str(row.get("currency") or "").strip().upper()
        day = parse_day(str(row.get("dated") or ""))
        if honor_closed_through and book.closed_through is not None and day <= book.closed_through:
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


def overlay_scenario(
    rows: Iterable[Mapping[str, Any]],
    *,
    statement: Statement | Mapping[str, Any] | None,
    book: Book,
    client: Client,
) -> Overlay:
    """Project sheet / cash from discrete hypothetical posts.

    Not a write. Closed-through is not a gate here — an overlay is
    not a mutation. Opt-in posting is `propose_scenario_posts`.
    """
    _require_read_scopes(client)
    _require_personal(book)
    cited = (
        statement
        if isinstance(statement, Statement)
        else statement_from_cite(statement)
    )
    posts = _instantiate_moves(
        rows, book=book, client=client, honor_closed_through=False
    )
    nw_delta = 0
    cash_delta = 0
    for post in posts:
        if post.currency != cited.currency:
            raise Refuse(
                f"scenario move is {post.currency} and the sheet cite is "
                f"{cited.currency} — adding them is how a NAV adds dollars to euros"
            )
        dn, dc = sheet_delta(post.postings)
        nw_delta = checked_add(nw_delta, dn)
        cash_delta = checked_add(cash_delta, dc)
    projected_nw = (
        None if cited.net_worth is None else checked_add(cited.net_worth, nw_delta)
    )
    projected_cash = None if cited.cash is None else checked_add(cited.cash, cash_delta)
    return Overlay(
        projected_net_worth=projected_nw,
        projected_cash=projected_cash,
        net_worth_delta=nw_delta,
        cash_delta=cash_delta,
        currency=cited.currency,
        posts=tuple(posts),
    )


def propose_scenario_posts(
    rows: Iterable[Mapping[str, Any]],
    *,
    book: Book,
    client: Client,
    opt_in: bool,
) -> list[ProposedPost]:
    """Propose ApplyEvent posts for an opted-in scenario, or refuse.

    ⛔ NON-OPT-IN MUST NOT POST. Overlaying is the default. A missing
    opt-in is not a silent write.
    """
    _require_post_scopes(client)
    _require_personal(book)
    if not opt_in:
        raise Refuse(
            "scenario journals post only when the household administrator "
            "opts in — non-opt-in must not post"
        )
    return _instantiate_moves(
        rows, book=book, client=client, honor_closed_through=True
    )


def required_savings(
    *_args: Any,
    **_kwargs: Any,
) -> None:
    """Refuse. A required monthly rate is a cash forecast."""
    raise Refuse(
        "required savings is a cash forecast — this app cites a sheet "
        "against a target and overlays discrete Personal templates. It "
        "does not invent a FIRE number or a monthly rate"
    )


def fire_number(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. A FIRE number is not a sheet cite."""
    raise Refuse(
        "a FIRE number is refused — this app is not a retirement "
        "calculator and PLAN already named that as cannot-show"
    )


def fetch_statements(*, token: str | None = None) -> None:
    """Refuse to pull. The grant path is not built."""
    _ = token
    raise Refuse(
        "live Connect OAuth is leftover — the grant path "
        "is not built (#150 / #151 / leftover #22). This app does not "
        "pretend the door opens"
    )


def deliver(
    posts: Sequence[ProposedPost],
    *,
    token: str | None = None,
) -> None:
    """Refuse to send. The grant path is not built.

    A green overlay is not a door that opens. Connect access tokens
    until live OAuth lands. This function exists so a caller cannot
    "just" POST the proposal and believe it landed.
    """
    _ = posts
    _ = token
    raise Refuse(
        "live Connect OAuth is leftover — the grant path "
        "is not built (#150 / #151 / leftover #22). This app does not "
        "pretend the door opens"
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
