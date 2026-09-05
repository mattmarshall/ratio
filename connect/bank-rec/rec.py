#!/usr/bin/env python3
"""Operating bank reconciliation for BookKind OPERATING.

A WorkOS Connect app, not a kernel RPC. Bank rec lives **here**.
It does not live in `ratio watch`, the operations console, or a
new kernel method. Sheet, cash-flow, aging, TB, and period close
stay core (`statements:read`). This app cites those figures; it
does not grow them.

⭐ SCOPES ARE THE FROZEN CATALOG NAMES. The issue body still says
`journal:append`. That string is an alias and is refused.
Canonical write grant: `journals:post`. See docs/connect-scopes.md.

⭐ THE RECON REPORT IS READ-ONLY BY DEFAULT. `statements:read`
cites TB / statement cash and open AR/AP. Instantiating
CreateBook(Operating) cash-moving templates is a write and
happens only on explicit opt-in plus `journals:post`.

⭐ `journals:post` IS AN ALLOWLIST PER client_id. An empty
allowlist refuses every post. Silence is not "all templates".

⭐ UNSET STAYS UNSET. A missing book-cash cite is not a cleared
$0.00. A missing bank statement is not a silent reconciled-empty.
An empty journal digest is unset, not history-intact and not
success. Open AR/AP are context, never silent reconciling items.
A posted `"0.00"` is a figure.

⭐ MONEY IS MINOR UNITS, PARSED BY SPLITTING ON THE POINT. A float
is how a cent disappears. A third decimal place is refused.
Instantiated legs that would wrap i64 are refused before a
product is asked anything — same reason as Ratio.Bounded.

⭐ NO PAYROLL, TAX FILING, INVENTORY / COGS, OR BANK OAUTH. Those
stay leftovers on #174 (payroll / tax) or stay refused
(inventory) / leftover on #165 (live bank OAuth). This app does
not invent a paycheck, a tax table, or a COGS plug.

⭐ THE GRANT PATH CALLS CONNECTAPIURL. `fetch_cites` and `deliver`
present a verified Connect access token against the Connect HTTP
API. Membership is still required. A Connect token never takes
`RATIO_DEMO_OPEN` and never matches `org:{id}`. WorkOS dashboard
registration stays leftover #22.

⚠ CLOSED-THROUGH IS A GATE ON WRITES. A dated opt-in post on or
before the book's closed-through day refuses the batch. A report
is not a mutation.
"""

from __future__ import annotations

import csv
import io
import json
from dataclasses import dataclass, field
from datetime import date
from typing import Any, Iterable, Mapping, Sequence

import grant as _grant

# i64 bounds. Lean's Int is unbounded; ApplyEvent runs on i64.
I64_MIN = -(2**63)
I64_MAX = 2**63 - 1

REQUIRED_SCOPES = frozenset({"statements:read"})
OPTIONAL_SCOPES = frozenset({"books:read", "journals:post"})
CANONICAL_SCOPES = REQUIRED_SCOPES | OPTIONAL_SCOPES
REFUSED_ALIASES = frozenset({"journal:append", "journal:read"})

# CreateBook(Operating) cash-moving posting rules this app may
# instantiate as recon adjustments. Legs are (account_dim, weight).
# Weight is ±1; the amount's magnitude is applied. Same ids
# book.rs seeds — a silent rename is how an app invents a Method.
# invoice_customer / vendor_bill do not move cash and are not here.
OPERATING_CASH_LEGS: dict[str, tuple[tuple[int, int], tuple[int, int]]] = {
    "collect_receivable": ((1, 1), (2, -1)),
    "pay_vendor": ((40, 1), (1, -1)),
    "receive_revenue": ((1, 1), (30, -1)),
    "pay_expense": ((10, 1), (1, -1)),
    "contribute_equity": ((1, 1), (20, -1)),
    "draw_equity": ((20, 1), (1, -1)),
}

KIND_TO_TEMPLATE = {
    "collect": "collect_receivable",
    "pay_vendor": "pay_vendor",
    "revenue": "receive_revenue",
    "expense": "pay_expense",
    "contribute": "contribute_equity",
    "draw": "draw_equity",
}

# What CreateBook writes on an Operating book. A client_id that
# lists `call_lp` is refused — that rule was never seeded.
OPERATING_SEEDED_RULES = frozenset(OPERATING_CASH_LEGS) | frozenset(
    {
        "invoice_customer",
        "vendor_bill",
    }
)

CASH_ACCOUNT = 1

# Wire / proto field names this app copies. A rename here that
# the proto does not share is how a pack invents a cite.
AGING_PROTO_FIELDS = (
    "current",
    "days_thirty",
    "days_sixty",
    "days_ninety",
    "days_older",
    "undated",
    "control",
)
CLOSE_PROTO_FIELDS = (
    "closed_date",
    "journal_position",
    "journal_digest",
    "config_digest",
    "surplus",
)


class Refuse(Exception):
    """The report or post is not proposed. Message is the reason, not a workaround."""


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
    """`books:read` membership. An org_id claim is not membership."""

    kind: str
    member: bool = True
    org_id: str | None = None
    approved_templates: frozenset[str] = field(
        default_factory=lambda: OPERATING_SEEDED_RULES
    )
    closed_through: date | None = None


@dataclass(frozen=True)
class Client:
    client_id: str
    allowlist: frozenset[str]
    scopes: frozenset[str]


@dataclass(frozen=True)
class AgingCite:
    """One `AgingSchedule.control`. Empty control is unset, not AR 0.00."""

    control: int | None = None
    current: int | None = None
    days_thirty: int | None = None
    days_sixty: int | None = None
    days_ninety: int | None = None
    days_older: int | None = None
    undated: int | None = None


@dataclass(frozen=True)
class Outstanding:
    """Operator-named reconciling item. Never inferred from AR/AP."""

    kind: str
    amount: int
    reference: str


@dataclass(frozen=True)
class Report:
    """Bank rec cite. Unset stays unset — not a silent cleared $0.00."""

    book_cash: int | None
    bank_ending: int | None
    difference: int | None
    outstanding: tuple[Outstanding, ...]
    outstanding_net: int | None
    remaining: int | None
    open_ar: int | None
    open_ap: int | None
    journal_digest: str | None
    journal_position: int | None
    closed_through: date | None
    as_of: date | None
    currency: str
    status: str
    unset: tuple[str, ...]


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


def parse_minor(
    text: str, *, allow_signed: bool = False, allow_zero: bool = True
) -> int:
    """Split on the point. Never parse a float.

    Same door as `ratio_common::parse_minor`. A third decimal place
    is refused rather than dropped. Overflow is refused rather than
    wrapped.
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
                f"{text!r} is signed; Kind picks the rule so a signed-amount "
                "inference cannot silently flip a collect and a pay"
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
    if isinstance(text, bool):
        raise Refuse("an amount is not a flag")
    if isinstance(text, int):
        if text > I64_MAX or text < I64_MIN:
            raise Refuse("amount does not fit in i64 minor units")
        if not allow_signed and text < 0:
            raise Refuse(
                f"{text!r} is signed; Kind picks the rule so a signed-amount "
                "inference cannot silently flip a collect and a pay"
            )
        return text
    if isinstance(text, float):
        raise Refuse(
            f"{text!r} is not an amount string — a number typed as a float "
            "is how a cent disappears"
        )
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
    if not t:
        return None
    return t


def parse_optional_int(text: Any) -> int | None:
    if text is None or text == "":
        return None
    if isinstance(text, bool):
        raise Refuse("a journal position is not a flag")
    if isinstance(text, int):
        return text
    t = str(text).strip()
    if not t:
        return None
    if not t.isdigit():
        raise Refuse(f"{text!r} is not a journal position")
    return int(t)


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
            f"weight {weight} is not ±1 — an Operating template does not "
            "scale, and this app does not invent a Method"
        )
    if weight == 1:
        return amount
    if amount == 0:
        return 0
    return -amount


def instantiate(rule_id: str, amount: int, currency: str) -> tuple[Posting, ...]:
    legs = OPERATING_CASH_LEGS.get(rule_id)
    if legs is None:
        raise Refuse(
            f"{rule_id!r} is not an Operating cash-moving template this "
            "app instantiates — it does not invent a Method, an Order, "
            "or a lot_method variant, and invoice_customer / vendor_bill "
            "do not move cash"
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
            "this app needs statements:read — TB / sheet / cash-flow / "
            "aging are the cites a recon is measured against. Without "
            "them the app would invent a cash figure"
        )


def _require_post_scopes(client: Client) -> None:
    _require_read_scopes(client)
    if "journals:post" not in client.scopes:
        raise Refuse(
            "recon adjustments need journals:post — statements:read is "
            "the cite, not a write grant"
        )


def _require_operating(book: Book) -> None:
    if book.kind != "OPERATING":
        raise Refuse(
            f"this app is BookKind OPERATING; {book.kind!r} keeps its own "
            "chrome and is not an operating bank-rec book"
        )
    if not book.member:
        raise Refuse(
            "membership is the AuthKit sub on the book — an org_id claim "
            "is not membership. Authorized-empty for a book the subject "
            "does not administer"
        )


def _template_for(row: Mapping[str, Any]) -> str:
    kind = str(row.get("kind") or "").strip().lower()
    if not kind:
        raise Refuse(
            "Kind picks the rule so a signed-amount inference cannot "
            "silently flip a collect and a pay"
        )
    rule = KIND_TO_TEMPLATE.get(kind)
    if rule is None:
        raise Refuse(
            f"kind {kind!r} is not an operating recon kind this app maps — "
            "it does not invent a Method, post call_lp onto an Operating "
            "book, or treat invoice_customer / vendor_bill as cash moves"
        )
    return rule


def _event_id(row: Mapping[str, Any], index: int) -> str:
    raw = str(row.get("reference") or row.get("event_id") or f"recon-{index + 1}")
    ident = raw.strip()
    if not ident or len(ident) > 64:
        raise Refuse(f"{ident!r} is not an event id")
    if not all(c.isalnum() or c in "-_." for c in ident):
        raise Refuse(
            f"{ident!r} is not an event id — letters, digits, - _ . and at most 64"
        )
    return ident


def aging_from_cite(raw: Mapping[str, Any] | None) -> AgingCite:
    """Read an AgingSchedule. Omitted / empty control stays unset."""
    if raw is None:
        return AgingCite()
    return AgingCite(
        control=parse_optional_minor(raw.get("control"), allow_signed=True),
        current=parse_optional_minor(raw.get("current"), allow_signed=True),
        days_thirty=parse_optional_minor(raw.get("days_thirty"), allow_signed=True),
        days_sixty=parse_optional_minor(raw.get("days_sixty"), allow_signed=True),
        days_ninety=parse_optional_minor(raw.get("days_ninety"), allow_signed=True),
        days_older=parse_optional_minor(raw.get("days_older"), allow_signed=True),
        undated=parse_optional_minor(raw.get("undated"), allow_signed=True),
    )


def outstanding_from_cite(raw: Mapping[str, Any]) -> Outstanding:
    kind = str(raw.get("kind") or "").strip().lower()
    if kind not in ("deposit", "check"):
        raise Refuse(
            f"{kind!r} is not an outstanding kind — operator-named "
            "deposit or check, never inferred from open AR/AP"
        )
    amount = parse_minor(str(raw.get("amount") or ""), allow_zero=False)
    reference = str(raw.get("reference") or raw.get("memo") or "").strip()
    if not reference:
        raise Refuse(
            "an outstanding item names itself — a blank row is how a "
            "silent reconciling item sneaks in"
        )
    return Outstanding(kind=kind, amount=amount, reference=reference)


def _named_unset(
    *,
    book_cash: int | None,
    bank_ending: int | None,
    open_ar: int | None,
    open_ap: int | None,
    journal_digest: str | None,
    closed_through: date | None,
) -> tuple[str, ...]:
    names: list[str] = []
    if book_cash is None:
        names.append(
            "book cash — a missing TB / sheet / cash-flow cite stays "
            "unset, not a silent cleared $0.00"
        )
    if bank_ending is None:
        names.append(
            "bank ending — a missing statement is unset, not a silent "
            "reconciled-empty"
        )
    if journal_digest is None:
        names.append(
            "journal digest — an empty digest is unset, not "
            "history-intact and not success"
        )
    if open_ar is None:
        names.append(
            "open AR — AgingSchedule.control stays unset when the "
            "journal cannot support the cut; not a silent AR 0.00 "
            "and not a reconciling item"
        )
    if open_ap is None:
        names.append(
            "open AP — AgingSchedule.control stays unset when the "
            "journal cannot support the cut; not a silent AP 0.00 "
            "and not a reconciling item"
        )
    if closed_through is None:
        names.append(
            "closed-through — an open period is unset, not a fake "
            "closed period. statements:read is how the close is read"
        )
    return tuple(names)


def reconcile(
    *,
    book: Book,
    client: Client,
    book_cash: Any = None,
    bank_ending: Any = None,
    receivable: Mapping[str, Any] | AgingCite | None = None,
    payable: Mapping[str, Any] | AgingCite | None = None,
    journal_digest: Any = None,
    journal_position: Any = None,
    closed_through: date | str | None = None,
    as_of: date | str | None = None,
    currency: str = "USD",
    outstanding: Iterable[Mapping[str, Any]] | None = None,
) -> Report:
    """Cite book cash against a named bank ending.

    ⛔ MISSING CITES STAY UNSET. A missing book-cash or bank-ending
    figure is not a cleared $0.00. An empty digest is not success.
    Open AR/AP are listed as context; they never fill a missing
    outstanding row.
    """
    _require_read_scopes(client)
    _require_operating(book)
    code = currency.strip().upper()
    if len(code) != 3 or not code.isalpha():
        raise Refuse(f"{currency!r} is not an ISO currency code")
    cash = parse_optional_minor(book_cash, allow_signed=True)
    bank = parse_optional_minor(bank_ending, allow_signed=True)
    ar = receivable if isinstance(receivable, AgingCite) else aging_from_cite(receivable)
    ap = payable if isinstance(payable, AgingCite) else aging_from_cite(payable)
    digest = parse_optional_digest(journal_digest)
    position = parse_optional_int(journal_position)
    close = (
        closed_through
        if isinstance(closed_through, date) or closed_through is None
        else parse_day(str(closed_through))
    )
    day = (
        as_of
        if isinstance(as_of, date) or as_of is None
        else parse_day(str(as_of))
    )
    items: list[Outstanding] = []
    for row in outstanding or ():
        items.append(outstanding_from_cite(row))
    explained: int | None = 0 if items else None
    for item in items:
        delta = item.amount if item.kind == "deposit" else -item.amount
        explained = checked_add(explained or 0, delta)
    difference = None
    remaining = None
    if cash is not None and bank is not None:
        difference = checked_add(cash, -bank)
        remaining = difference if explained is None else checked_add(difference, -explained)
    unset = _named_unset(
        book_cash=cash,
        bank_ending=bank,
        open_ar=ar.control,
        open_ap=ap.control,
        journal_digest=digest,
        closed_through=close,
    )
    # ⛔ EMPTY-DIGEST-AS-SUCCESS AND SILENT-CLEARED-ZERO ARE THE DEFECTS.
    # Status is unset until book cash, bank ending, AND a real digest
    # are all cited. A real zero cash against a real zero bank with a
    # digest can be tied.
    if cash is None or bank is None or digest is None:
        status = "unset"
    elif remaining == 0:
        status = "tied"
    else:
        status = "open"
    return Report(
        book_cash=cash,
        bank_ending=bank,
        difference=difference,
        outstanding=tuple(items),
        outstanding_net=explained,
        remaining=remaining,
        open_ar=ar.control,
        open_ap=ap.control,
        journal_digest=digest,
        journal_position=position,
        closed_through=close,
        as_of=day,
        currency=code,
        status=status,
        unset=unset,
    )


def _instantiate_moves(
    rows: Iterable[Mapping[str, Any]],
    *,
    book: Book,
    client: Client,
) -> list[ProposedPost]:
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
                "CreateBook(Operating) never wrote it"
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


def propose_recon_posts(
    rows: Iterable[Mapping[str, Any]],
    *,
    book: Book,
    client: Client,
    opt_in: bool,
) -> list[ProposedPost]:
    """Propose ApplyEvent posts for an opted-in recon adjustment, or refuse.

    ⛔ NON-OPT-IN MUST NOT POST. The recon report is the default.
    A missing opt-in is not a silent write.
    """
    _require_post_scopes(client)
    _require_operating(book)
    if not opt_in:
        raise Refuse(
            "recon adjustments post only when the operator opts in — "
            "non-opt-in must not post. The recon report is read-only"
        )
    return _instantiate_moves(rows, book=book, client=client)


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


def _csv(headers: Sequence[str], rows: Sequence[Sequence[str]]) -> str:
    buf = io.StringIO()
    writer = csv.writer(buf)
    writer.writerow(headers)
    writer.writerows(rows)
    return buf.getvalue()


def _amt(n: int | None) -> str:
    return "" if n is None else format_minor(n)


def as_files(report: Report) -> dict[str, str]:
    """CSV cite. Missing figures stay blank, named on unset.csv."""
    files = {
        "recon.csv": _csv(
            [
                "as_of",
                "currency",
                "book_cash",
                "bank_ending",
                "difference",
                "outstanding_net",
                "remaining",
                "status",
                "journal_digest",
                "journal_position",
                "closed_through",
            ],
            [
                [
                    report.as_of.isoformat() if report.as_of else "",
                    report.currency,
                    _amt(report.book_cash),
                    _amt(report.bank_ending),
                    _amt(report.difference),
                    _amt(report.outstanding_net),
                    _amt(report.remaining),
                    report.status,
                    report.journal_digest or "",
                    "" if report.journal_position is None else str(report.journal_position),
                    report.closed_through.isoformat() if report.closed_through else "",
                ]
            ],
        ),
        "aging.csv": _csv(
            ["side", "control"],
            [
                ["receivable", _amt(report.open_ar)],
                ["payable", _amt(report.open_ap)],
            ],
        ),
        "outstanding.csv": _csv(
            ["kind", "amount", "reference"],
            [[o.kind, format_minor(o.amount), o.reference] for o in report.outstanding],
        ),
        "unset.csv": _csv(
            ["cite"],
            [[name] for name in report.unset],
        ),
    }
    return files


def as_json(report: Report) -> dict[str, Any]:
    """JSON cite. Missing figures stay null, not 0.00."""
    return {
        "as_of": report.as_of.isoformat() if report.as_of else None,
        "currency": report.currency,
        "book_cash": None if report.book_cash is None else format_minor(report.book_cash),
        "bank_ending": None
        if report.bank_ending is None
        else format_minor(report.bank_ending),
        "difference": None
        if report.difference is None
        else format_minor(report.difference),
        "outstanding_net": None
        if report.outstanding_net is None
        else format_minor(report.outstanding_net),
        "remaining": None if report.remaining is None else format_minor(report.remaining),
        "open_ar": None if report.open_ar is None else format_minor(report.open_ar),
        "open_ap": None if report.open_ap is None else format_minor(report.open_ap),
        "journal_digest": report.journal_digest,
        "journal_position": report.journal_position,
        "closed_through": report.closed_through.isoformat()
        if report.closed_through
        else None,
        "status": report.status,
        "outstanding": [
            {"kind": o.kind, "amount": format_minor(o.amount), "reference": o.reference}
            for o in report.outstanding
        ],
        "unset": list(report.unset),
    }


def fetch_cites(
    *,
    token: str | None = None,
    book_id: str | None = None,
    transport: _grant.Transport | None = None,
) -> Any:
    """Pull statement / aging cites from ConnectApiUrl."""
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
    """POST opt-in ApplyEvent bodies to ConnectApiUrl."""
    return _grant.deliver_apply_events(
        posts,
        as_apply_event=as_apply_event,
        token=token,
        parent=parent,
        transport=transport,
        error=Refuse,
    )


def payroll(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Payroll stays leftover on #174."""
    raise Refuse(
        "payroll is refused — Operating books stay a thin TB + statements "
        "+ aging book. Payroll is a Connect-shaped leftover on #174, not "
        "a kernel engine and not a fake paycheck UI"
    )


def tax_filing(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Operating tax filing stays leftover on #174."""
    raise Refuse(
        "tax filing is refused — Operating tax stays a Connect-shaped "
        "leftover on #174. Household tax-pack is #166. IRS e-file stays "
        "refused. This app does not invent a return"
    )


def inventory(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Inventory / COGS stay out of Operating."""
    raise Refuse(
        "inventory / COGS is refused — chart_for(Operating) has no "
        "inventory account, and a silent COGS plug would invent a cost "
        "the chart never named"
    )


def cogs(*_args: Any, **_kwargs: Any) -> None:
    inventory()


def bank_oauth(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Live bank OAuth is leftover on #174 / #165."""
    raise Refuse(
        "live bank OAuth is leftover on #174 / #165 — this scaffold "
        "accepts a normalized bank-statement ending balance, not a "
        "provider SDK. It does not absorb #165"
    )


def kernel_recon(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. No kernel BankRec RPC."""
    raise Refuse(
        "a kernel bank-rec RPC is refused — this Connect app is the "
        "door. screensFor is not forked. /sheet /cashflow /aging / "
        "accounts stay the core cites"
    )
