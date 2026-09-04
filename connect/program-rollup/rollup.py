#!/usr/bin/env python3
"""Project program roll-up for BookKind PROJECT.

A WorkOS Connect app, not a kernel RPC. Multi-contract / program
views live **here**. They do not live in `ratio watch`, the
operations console, or a new kernel method. `/budget` and `/billing`
stay per-book core cites. This app lists PROJECT books the subject
can see (`books:read` membership) and rolls those cites up.

⭐ SCOPES ARE THE FROZEN CATALOG NAMES. `books:read`, `budget:read`,
`billing:read`. Aliases (`projects:budget:read`, `journal:append`)
and invented strings are refused. See docs/connect-scopes.md.

⭐ THIS APP IS READ-ONLY RELATIVE TO THE JOURNAL. It does not request
`journals:post`. `journal:append` is a refused alias, not a second
write grant. Export is CSV / JSON.

⭐ AN org_id CLAIM IS NOT MEMBERSHIP. `ListBooks` is filtered to
PROJECT books the subject administers. A first-party app does not
inherit every book in an org. #151: Connect tokens must not bypass
book ACLs.

⭐ NO MEGA-BOOK. Books stay independent. This app does not merge
journals, invent a fifth BookKind, or put a program URL on
`screensFor`. Roll-up is a cite of cites.

⭐ UNSET STAYS UNSET. An unbilled job is not billed-zero. An
uncollected job is not collected-zero. A book that cannot support a
cut does not contribute 0.00 to the program total. Treating a
missing billed as 0 would print the whole contract as remaining and
invent a fake program billed. A posted `"0.00"` is a figure.

⭐ PROGRAM TOTALS SUM ONLY SET CITES. Remaining to bill, remaining
to spend, and collections are summed from the per-book cuts — never
recomputed from mixed program totals (that would treat an unset
book as billed-zero). If no book cited the figure, the program
total is blank, not 0.00.

⭐ MONEY IS MINOR UNITS, PARSED BY SPLITTING ON THE POINT. A float
is how a cent disappears. A third decimal place is refused.

⭐ NO EAC, NO G702, NO VENDOR DIRECTORY. Those doors are #169,
#184, and #172.

⚠ THE GRANT PATH IS NOT BUILT. `fetch_cites` refuses. A Connect
access token is not accepted on `/v1` (leftover #22 / #150).
"""

from __future__ import annotations

import csv
import io
import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

# i64 bounds. Lean's Int is unbounded; every money figure here is i64.
I64_MIN = -(2**63)
I64_MAX = 2**63 - 1

CANONICAL_SCOPES = frozenset({"books:read", "budget:read", "billing:read"})
REFUSED_ALIASES = frozenset(
    {
        "journal:append",
        "journal:read",
        "projects:budget:read",
        "projects:billing:read",
    }
)

# Configuration fields this roll-up cites. Names must stay the ones
# crates/ratio-rules already stores — a silent rename is how a
# roll-up invents a baseline.
PROJECT_TERM_FIELDS = (
    "budget",
    "phases",
)

REMAINING_TO_BILL_NOTE = (
    "revised − billed — a /billing cite. Treating billed as 0 would "
    "print the whole contract as remaining."
)
REMAINING_TO_SPEND_NOTE = (
    "revised − incurred − awarded — a /budget cite, not a forecast. "
    "Treating awarded as 0 would print budget − actual as headroom."
)
COLLECTED_NOTE = (
    "cash against AR: billed − outstanding receivable − retainage held. "
    "Unheld retainage is 0 for the subtraction and stays blank on the "
    "retainage line. Same door as /billing."
)


class Refuse(Exception):
    """The roll-up is not emitted. Message is the reason, not a workaround."""


@dataclass(frozen=True)
class Client:
    client_id: str
    scopes: frozenset[str]


@dataclass(frozen=True)
class BudgetCite:
    """`budget:read` original, CO equity, incurred, awarded."""

    original: int | None = None
    approved_change_orders: int | None = None
    incurred: int | None = None
    awarded: int | None = None


@dataclass(frozen=True)
class BillingCite:
    """`billing:read` billed / earned / retainage / AR."""

    billed: int | None = None
    earned: int | None = None
    retainage_receivable: int | None = None
    accounts_receivable: int | None = None


@dataclass(frozen=True)
class VisibleBook:
    """One book `books:read` can name.

    `member` is book membership — the AuthKit `sub` on the book.
    `org_id` is a claim, not membership, and does not grant a row.
    """

    book_id: str
    kind: str
    member: bool
    org_id: str | None = None
    budget: BudgetCite | None = None
    billing: BillingCite | None = None


@dataclass(frozen=True)
class BookRow:
    """One PROJECT book's cited figures. Empty amounts are unset."""

    book_id: str
    original: str
    change_orders: str
    revised: str
    incurred: str
    awarded: str
    remaining_to_spend: str
    billed: str
    earned: str
    remaining_to_bill: str
    collected: str
    accounts_receivable: str
    retainage: str


@dataclass(frozen=True)
class ProgramFigure:
    """A program total. `amount` empty means no book cited the figure."""

    figure: str
    amount: str
    cited_books: int
    books_in_program: int
    note: str


@dataclass(frozen=True)
class Rollup:
    """A program view. Not a mega-book and not a /budget field."""

    books: tuple[BookRow, ...]
    program: tuple[ProgramFigure, ...]
    unset: tuple[str, ...]
    books_in_program: int
    cited: Mapping[str, int]


def load_app(path: str) -> dict[str, Any]:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def client_from_app(app: Mapping[str, Any]) -> Client:
    connect = app.get("workos_connect") or {}
    scopes = connect.get("scopes") or []
    return Client(
        client_id=str(connect.get("client_id") or app.get("name") or "ratio-program-rollup"),
        scopes=frozenset(scopes),
    )


def parse_minor(text: str, *, allow_signed: bool = False) -> int:
    """Split on the point. Never parse a float.

    Same door as `ratio_common::parse_minor`. A third decimal place is
    refused rather than dropped. Overflow is refused rather than wrapped.
    Change-order nets and incurred may be signed; billed and original
    are magnitudes unless asked.
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


def format_optional(n: int | None) -> str:
    return "" if n is None else format_minor(n)


def _require_scopes(client: Client) -> None:
    aliases = client.scopes & REFUSED_ALIASES
    if aliases:
        raise Refuse(
            "refused alias scope "
            + ", ".join(sorted(aliases))
            + " — catalogs use books:read / budget:read / billing:read"
        )
    if "journals:post" in client.scopes:
        raise Refuse(
            "this app is read-only relative to the journal — journals:post "
            "is a write grant this roll-up does not need"
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
            "books:read is ListBooks membership (not an org_id claim); "
            "budget:read is original / incurred / awarded; billing:read "
            "is billed / earned / collections"
        )


def _checked_add(a: int, b: int) -> int:
    total = a + b
    if total > I64_MAX or total < I64_MIN:
        raise Refuse("sum does not fit in i64 minor units")
    return total


def _checked_sub(a: int, b: int) -> int:
    total = a - b
    if total > I64_MAX or total < I64_MIN:
        raise Refuse("difference does not fit in i64 minor units")
    return total


def revised_contract(original: int | None, approved: int | None) -> int | None:
    """Original + approved when the original is set.

    Same door as `revisedContract` on `/budget` / `/billing`. An
    unknown baseline cannot be priced. An unposted CO does not block
    a set original — revised equals the original, and the change-order
    *line* stays unset.
    """
    if original is None:
        return None
    return _checked_add(original, approved if approved is not None else 0)


def remaining_to_bill(revised: int | None, billed: int | None) -> int | None:
    """Revised − billed. Unset when either side cannot support it.

    Treating billed as 0 would print the whole contract as remaining.
    """
    if revised is None or billed is None:
        return None
    return _checked_sub(revised, billed)


def remaining_to_spend(
    revised: int | None,
    incurred: int | None,
    awarded: int | None,
) -> int | None:
    """Revised − incurred − awarded. Unset when the cut cannot be supported.

    Treating awarded as 0 would print budget − actual as headroom.
    Same door as `remainingToSpendOf` on `/budget`. Not an EAC.
    """
    if revised is None or incurred is None or awarded is None:
        return None
    return _checked_sub(_checked_sub(revised, incurred), awarded)


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
    return _checked_sub(_checked_sub(billed, ar), held)


def billed_minus_earned(billed: int | None, earned: int | None) -> int | None:
    """Over/under-billing. Unset until both sides have posted."""
    if billed is None or earned is None:
        return None
    return _checked_sub(billed, earned)


def sum_cited(values: Sequence[int | None]) -> tuple[int | None, int]:
    """Sum set cites only. Unset books do not contribute 0.

    Returns `(total_or_none, cited_count)`. If no book cited the
    figure, the total is unset — never a fake program 0.00.
    """
    cited = [v for v in values if v is not None]
    if not cited:
        return None, 0
    total = 0
    for v in cited:
        total = _checked_add(total, v)
    return total, len(cited)


def list_program_books(books: Sequence[VisibleBook]) -> tuple[VisibleBook, ...]:
    """`ListBooks` filtered to PROJECT kind + membership.

    ⛔ AN `org_id` CLAIM IS NOT MEMBERSHIP. A book the subject does
    not administer stays out, even when its `org_id` matches. A
    first-party app does not inherit every book in an org.
    Non-PROJECT kinds the subject can see stay out — they keep
    their own chrome and are not a job in the program.
    """
    out: list[VisibleBook] = []
    for book in books:
        if not book.member:
            continue
        if book.kind != "PROJECT":
            continue
        if not str(book.book_id or "").strip():
            raise Refuse("a books:read row names a book")
        out.append(book)
    return tuple(out)


def books_from_org_claim(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. An org_id claim is not membership."""
    raise Refuse(
        "an org_id claim is not membership — ListBooks is filtered to "
        "PROJECT books the subject administers (books:read). A "
        "first-party app does not inherit every book in an org (#151). "
        "This app does not list a program from an org claim"
    )


def mega_book(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. No mega-book in the kernel."""
    raise Refuse(
        "no mega-book — books stay independent. This app cites per-book "
        "/budget and /billing figures and rolls those cites up. Merging "
        "journals, inventing a fifth BookKind, or putting a program URL "
        "on screensFor would break BookKind independence. This does not "
        "close #179 by inventing a kernel program book"
    )


def merge_journals(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Journals are not concatenated across books."""
    raise Refuse(
        "journals are not merged across books — a program roll-up is a "
        "cite of cites, not a second journal. A mega-book that concatenated "
        "prefixes would break BookKind independence and the prefix a "
        "figure must pin"
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


def billing_from_cite(raw: Mapping[str, Any] | None) -> BillingCite:
    if raw is None:
        return BillingCite()
    return BillingCite(
        billed=parse_optional_minor(raw.get("billed")),
        earned=parse_optional_minor(raw.get("earned")),
        retainage_receivable=parse_optional_minor(raw.get("retainage_receivable")),
        accounts_receivable=parse_optional_minor(raw.get("accounts_receivable")),
    )


def _book_cuts(book: VisibleBook) -> dict[str, int | None]:
    budget = book.budget if book.budget is not None else BudgetCite()
    billing = book.billing if book.billing is not None else BillingCite()
    revised = revised_contract(budget.original, budget.approved_change_orders)
    return {
        "original": budget.original,
        "change_orders": budget.approved_change_orders,
        "revised": revised,
        "incurred": budget.incurred,
        "awarded": budget.awarded,
        "remaining_to_spend": remaining_to_spend(revised, budget.incurred, budget.awarded),
        "billed": billing.billed,
        "earned": billing.earned,
        "remaining_to_bill": remaining_to_bill(revised, billing.billed),
        "collected": collected_against_billed(
            billing.billed, billing.accounts_receivable, billing.retainage_receivable
        ),
        "accounts_receivable": billing.accounts_receivable,
        "retainage": billing.retainage_receivable,
        "billed_minus_earned": billed_minus_earned(billing.billed, billing.earned),
    }


def _book_row(book: VisibleBook, cuts: Mapping[str, int | None]) -> BookRow:
    return BookRow(
        book_id=book.book_id,
        original=format_optional(cuts["original"]),
        change_orders=format_optional(cuts["change_orders"]),
        revised=format_optional(cuts["revised"]),
        incurred=format_optional(cuts["incurred"]),
        awarded=format_optional(cuts["awarded"]),
        remaining_to_spend=format_optional(cuts["remaining_to_spend"]),
        billed=format_optional(cuts["billed"]),
        earned=format_optional(cuts["earned"]),
        remaining_to_bill=format_optional(cuts["remaining_to_bill"]),
        collected=format_optional(cuts["collected"]),
        accounts_receivable=format_optional(cuts["accounts_receivable"]),
        retainage=format_optional(cuts["retainage"]),
    )


def _program_note(figure: str, cited: int, n: int, amount: int | None) -> str:
    if n == 0:
        return "no PROJECT book the subject can see — not a silent program of 0.00"
    if cited == 0:
        return (
            f"unset — none of {n} program books cited {figure}; "
            "not a fake roll-up zero"
        )
    if cited < n:
        return (
            f"sum of {cited} of {n} books that cited {figure} — "
            "books that lack the cite stay out of the sum, not 0.00"
        )
    if amount == 0:
        return f"sum of {cited} of {n} books — a posted 0.00 is a figure"
    return f"sum of {cited} of {n} books that cited {figure}"


def _named_unset(cuts_by_book: Sequence[tuple[str, Mapping[str, int | None]]]) -> tuple[str, ...]:
    names: list[str] = []
    for book_id, cuts in cuts_by_book:
        for figure, reason in (
            ("billed", "an unbilled job is not billed-zero"),
            ("collected", "an uncollected job is not collected-zero"),
            (
                "remaining_to_bill",
                "treating billed as 0 would print the whole contract as remaining",
            ),
            (
                "remaining_to_spend",
                "treating awarded as 0 would print budget − actual as headroom",
            ),
            ("revised", "CreateBook does not invent a baseline"),
            ("earned", "billed is not a substitute for earned"),
        ):
            if cuts[figure] is None:
                names.append(f"{book_id} {figure} — {reason}")
    return tuple(names)


def build_rollup(
    *,
    client: Client,
    books: Sequence[VisibleBook],
) -> Rollup:
    """Cite membership-visible PROJECT books into a program view.

    Membership is `books:read`. An `org_id` claim does not add a row.
    Program totals sum only set per-book cites.
    """
    _require_scopes(client)
    program_books = list_program_books(books)
    n = len(program_books)
    per_book = [(book, _book_cuts(book)) for book in program_books]
    rows = tuple(_book_row(book, cuts) for book, cuts in per_book)

    figures = (
        "original",
        "change_orders",
        "revised",
        "incurred",
        "awarded",
        "remaining_to_spend",
        "billed",
        "earned",
        "remaining_to_bill",
        "collected",
        "accounts_receivable",
        "retainage",
        "billed_minus_earned",
    )
    program: list[ProgramFigure] = []
    cited_counts: dict[str, int] = {}
    for figure in figures:
        total, cited = sum_cited([cuts[figure] for _, cuts in per_book])
        cited_counts[figure] = cited
        program.append(
            ProgramFigure(
                figure=figure,
                amount=format_optional(total),
                cited_books=cited,
                books_in_program=n,
                note=_program_note(figure, cited, n, total),
            )
        )

    return Rollup(
        books=rows,
        program=tuple(program),
        unset=_named_unset([(book.book_id, cuts) for book, cuts in per_book]),
        books_in_program=n,
        cited=cited_counts,
    )


def cite_from_fixture(raw: Mapping[str, Any]) -> Rollup:
    """Build a roll-up from a fixture that looks like ListBooks + cites."""
    books_raw = raw.get("books") or ()
    if not isinstance(books_raw, (list, tuple)):
        raise Refuse("books is a list of books:read rows, not a guess")
    books: list[VisibleBook] = []
    for item in books_raw:
        if not isinstance(item, Mapping):
            raise Refuse("a books:read row is an object")
        budget_raw = item.get("budget")
        billing_raw = item.get("billing") if "billing" in item else item.get("progress")
        if not isinstance(budget_raw, Mapping):
            budget_raw = {
                "original": item.get("original", item.get("budget")),
                "approved_change_orders": item.get("approved_change_orders"),
                "incurred": item.get("incurred"),
                "awarded": item.get("awarded"),
            }
        if not isinstance(billing_raw, Mapping):
            billing_raw = {
                "billed": item.get("billed"),
                "earned": item.get("earned"),
                "retainage_receivable": item.get("retainage_receivable"),
                "accounts_receivable": item.get("accounts_receivable"),
            }
        books.append(
            VisibleBook(
                book_id=str(item.get("book_id") or item.get("id") or "").strip(),
                kind=str(item.get("kind") or "PROJECT"),
                member=bool(item.get("member", True)),
                org_id=item.get("org_id"),
                budget=budget_from_cite(budget_raw),
                billing=billing_from_cite(billing_raw),
            )
        )
    client = raw.get("client")
    return build_rollup(
        client=client if isinstance(client, Client) else client_from_app(raw.get("app") or {}),
        books=books,
    )


def fetch_cites(*, token: str | None = None) -> None:
    """Refuse to pull. The grant path is not built.

    A green roll-up builder is not a door that opens. Connect access
    tokens are not accepted on /v1.
    """
    _ = token
    raise Refuse(
        "Connect access tokens are not accepted on /v1 — the grant path "
        "is not built (leftover #22 / #150). This app does not "
        "pretend the door opens"
    )


def deliver(rollup: Rollup, *, token: str | None = None) -> None:
    """Refuse to push. Same leftover as fetch_cites."""
    _ = rollup
    _ = token
    raise Refuse(
        "Connect access tokens are not accepted on /v1 — the grant path "
        "is not built (leftover #22 / #150). This app does not "
        "deliver a roll-up against a door that is not open"
    )


def eac(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. EAC / forecast stay on #169."""
    raise Refuse(
        "EAC / forecast stay on #169 — this app cites remaining-to-spend "
        "as a /budget cut (revised − incurred − awarded) and does not "
        "emit an estimate-at-completion. /budget still does not forecast"
    )


def forecast(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Same door as eac()."""
    eac()


def render_g702(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. AIA G702 product UI is #184."""
    raise Refuse(
        "AIA G702 product UI is #184 — this app cites billed / "
        "remaining-to-bill / collections and does not pack a pay-app "
        "or render a licensed form"
    )


def vendor_directory(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. No vendor user directory in core."""
    raise Refuse(
        "no vendor user directory in core — membership is the AuthKit "
        "sub on the book. An org_id claim is not membership. Vendor / "
        "GC portal leftovers stay on #172"
    )


_BOOK_COLUMNS = (
    "Book",
    "Original",
    "Change orders",
    "Revised",
    "Incurred",
    "Awarded",
    "Remaining to spend",
    "Billed",
    "Earned",
    "Remaining to bill",
    "Collected",
    "Accounts receivable",
    "Retainage",
)

_PROGRAM_COLUMNS = ("Figure", "Amount", "Cited books", "Books in program", "Note")


def csv_books(rollup: Rollup) -> str:
    """Per-book cites. Blanks are unset — never a silent 0.00."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(_BOOK_COLUMNS)
    for r in rollup.books:
        w.writerow(
            [
                r.book_id,
                r.original,
                r.change_orders,
                r.revised,
                r.incurred,
                r.awarded,
                r.remaining_to_spend,
                r.billed,
                r.earned,
                r.remaining_to_bill,
                r.collected,
                r.accounts_receivable,
                r.retainage,
            ]
        )
    return buf.getvalue()


def csv_program(rollup: Rollup) -> str:
    """Program totals. A blank amount is no book cited the figure."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(_PROGRAM_COLUMNS)
    for r in rollup.program:
        w.writerow([r.figure, r.amount, r.cited_books, r.books_in_program, r.note])
    return buf.getvalue()


def csv_unset(rollup: Rollup) -> str:
    """Named unset lines. Silence on the roll-up is the honesty, not a zero."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(("Unset line", "Reason"))
    for name in rollup.unset:
        w.writerow([name.split(" — ", 1)[0], name])
    return buf.getvalue()


def as_json(rollup: Rollup) -> str:
    """JSON export. Empty strings stay empty — never a coerced 0."""
    return json.dumps(
        {
            "books_in_program": rollup.books_in_program,
            "books": [
                {
                    "book_id": r.book_id,
                    "original": r.original,
                    "change_orders": r.change_orders,
                    "revised": r.revised,
                    "incurred": r.incurred,
                    "awarded": r.awarded,
                    "remaining_to_spend": r.remaining_to_spend,
                    "billed": r.billed,
                    "earned": r.earned,
                    "remaining_to_bill": r.remaining_to_bill,
                    "collected": r.collected,
                    "accounts_receivable": r.accounts_receivable,
                    "retainage": r.retainage,
                }
                for r in rollup.books
            ],
            "program": [
                {
                    "figure": r.figure,
                    "amount": r.amount,
                    "cited_books": r.cited_books,
                    "books_in_program": r.books_in_program,
                    "note": r.note,
                }
                for r in rollup.program
            ],
            "unset": list(rollup.unset),
        },
        indent=2,
    ) + "\n"


def as_files(rollup: Rollup) -> dict[str, str]:
    """Named companion sheets. Not a ZIP into core and not a mega-book."""
    return {
        "books.csv": csv_books(rollup),
        "program.csv": csv_program(rollup),
        "unset.csv": csv_unset(rollup),
        "program.json": as_json(rollup),
    }
