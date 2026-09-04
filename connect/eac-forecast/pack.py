#!/usr/bin/env python3
"""Project EAC / forecast packer for BookKind PROJECT.

A WorkOS Connect app, not a kernel RPC. Estimate-at-completion and
cost-to-complete live **here**. They do not live in `ratio watch`, the
operations console, or a new kernel method. `/budget` remaining-to-spend
stays core: revised − incurred − awarded.

⭐ SCOPES ARE THE FROZEN CATALOG NAMES. `budget:read`, `billing:read`,
`statements:read`. Aliases (`projects:budget:read`, `journal:append`)
and invented strings are refused. See docs/connect-scopes.md.

⭐ THIS APP IS READ-ONLY RELATIVE TO THE JOURNAL. It does not request
`journals:post`. The catalog has no forecast template. Posting
`project_cost*` as a what-if would mix a forecast into the book of
record. `journal:append` is a refused alias, not a second write
grant. Export is CSV / JSON.

⭐ UNSET STAYS UNSET. Remaining to spend is revised − incurred −
awarded. Treating awarded as 0 would print budget − actual as
headroom. An unawarded job is not awarded-zero. A missing remaining
is not an EAC of 0.00. A posted `"0.00"` is a figure.

⭐ EAC IS OUTSIDE THE JOURNAL. When remaining-to-spend can be cited:
EAC = incurred + remaining + awarded (= revised). ETC = remaining +
awarded (= revised − incurred). The assumption is written on the
row. This is not CPI / SPI and not a percent-complete forecast.

⭐ MONEY IS MINOR UNITS, PARSED BY SPLITTING ON THE POINT. A float
is how a cent disappears. A third decimal place is refused.

⭐ NO PERCENTAGE. A CPI / percent-complete EAC is a rounded figure.
`cpi_eac` refuses.

⚠ THE GRANT PATH IS NOT BUILT. `fetch_cites` refuses. A Connect
access token is not accepted on `/v1` (leftover #22 / #150).
"""

from __future__ import annotations

import csv
import io
import json
from dataclasses import dataclass
from datetime import date
from typing import Any, Mapping, Sequence

# i64 bounds. Lean's Int is unbounded; every money figure here is i64.
I64_MIN = -(2**63)
I64_MAX = 2**63 - 1

CANONICAL_SCOPES = frozenset({"budget:read", "billing:read", "statements:read"})
REFUSED_ALIASES = frozenset(
    {
        "journal:append",
        "journal:read",
        "projects:budget:read",
        "projects:billing:read",
    }
)

# Configuration fields this pack cites. Names must stay the ones
# crates/ratio-rules already stores — a silent rename is how a
# pack invents a baseline.
PROJECT_TERM_FIELDS = (
    "budget",
    "phases",
)

EAC_ASSUMPTION = (
    "EAC = incurred + remaining-to-spend + awarded (= revised). "
    "Assumption: the job finishes at the revised contract — remaining-to-spend "
    "and awarded convert to incurred. Not CPI / SPI, not a percent-complete "
    "forecast, and not a silent zero."
)

ETC_ASSUMPTION = (
    "ETC = remaining-to-spend + awarded (= revised − incurred). "
    "Assumption: cost to complete is uncommitted remaining plus awarded "
    "commitments. Unset when remaining-to-spend cannot be cited."
)

REMAINING_NOTE = (
    "revised − incurred − awarded — a /budget cite, not a forecast. "
    "Treating awarded as 0 would print budget − actual as headroom."
)


class Refuse(Exception):
    """The pack is not emitted. Message is the reason, not a workaround."""


@dataclass(frozen=True)
class Client:
    client_id: str
    scopes: frozenset[str]


@dataclass(frozen=True)
class Book:
    kind: str
    closed_through: date | None = None


@dataclass(frozen=True)
class BudgetCite:
    """`budget:read` original, CO equity, incurred, awarded."""

    original: int | None = None
    approved_change_orders: int | None = None
    incurred: int | None = None
    awarded: int | None = None


@dataclass(frozen=True)
class BillingCite:
    """`billing:read` companions. Never a substitute for incurred."""

    billed: int | None = None
    earned: int | None = None
    accounts_receivable: int | None = None


@dataclass(frozen=True)
class PhaseCite:
    """One work-package row. Cost `"0"` on a seeded phase is a true zero."""

    display_name: str
    budget: int | None = None
    approved_change_orders: int | None = None
    incurred: int | None = None
    awarded: int | None = None


@dataclass(frozen=True)
class Line:
    """One named figure. `amount` empty means unset."""

    figure: str
    amount: str
    note: str


@dataclass(frozen=True)
class PhaseRow:
    """One work-package EAC row. Blanks are unset cites."""

    description: str
    original: str
    change_orders: str
    revised: str
    incurred: str
    awarded: str
    remaining_to_spend: str
    etc: str
    eac: str
    assumption: str


@dataclass(frozen=True)
class Pack:
    """An EAC / forecast pack. Not a journal rewrite and not a /budget field."""

    cites: tuple[Line, ...]
    forecast: tuple[Line, ...]
    companions: tuple[Line, ...]
    phases: tuple[PhaseRow, ...]
    unset: tuple[str, ...]
    budget: BudgetCite
    remaining_to_spend: int | None
    etc: int | None
    eac: int | None


def load_app(path: str) -> dict[str, Any]:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def client_from_app(app: Mapping[str, Any]) -> Client:
    connect = app.get("workos_connect") or {}
    scopes = connect.get("scopes") or []
    return Client(
        client_id=str(connect.get("client_id") or app.get("name") or "ratio-eac-forecast"),
        scopes=frozenset(scopes),
    )


def parse_minor(text: str, *, allow_signed: bool = False) -> int:
    """Split on the point. Never parse a float.

    Same door as `ratio_common::parse_minor`. A third decimal place is
    refused rather than dropped. Overflow is refused rather than wrapped.
    Change-order nets and incurred may be signed; original contract and
    awarded are magnitudes unless asked.
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


def parse_optional_day(text: Any) -> date | None:
    if text is None:
        return None
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
            + " — catalogs use budget:read / billing:read / statements:read"
        )
    if "journals:post" in client.scopes:
        raise Refuse(
            "this app is read-only relative to the journal — journals:post "
            "is a write grant this pack does not need. The catalog has no "
            "forecast template; posting project_cost* as a forecast would "
            "make the journal a second ledger"
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
            "budget:read is original / incurred / awarded; billing:read is "
            "companion billed / earned; statements:read is how closed-through "
            "is read"
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

    Same door as `revisedContract` on `/budget`. An unknown baseline
    cannot be priced. An unposted CO does not block a set original —
    revised equals the original, and the change-order *line* stays unset.
    """
    if original is None:
        return None
    return _checked_add(original, approved if approved is not None else 0)


def remaining_to_spend(
    revised: int | None,
    incurred: int | None,
    awarded: int | None,
) -> int | None:
    """Revised − incurred − awarded. Unset when the cut cannot be supported.

    Treating awarded as 0 would print budget − actual as headroom.
    Same door as `remainingToSpendOf` on `/budget`.
    """
    if revised is None or incurred is None or awarded is None:
        return None
    return _checked_sub(_checked_sub(revised, incurred), awarded)


def estimate_to_complete(remaining: int | None, awarded: int | None) -> int | None:
    """Remaining-to-spend + awarded. Unset when remaining cannot be cited.

    ⛔ AN UNSET REMAINING IS NOT ETC = AWARDED. That invention is how a
    job without an award looks like it has no cost left.
    """
    if remaining is None or awarded is None:
        return None
    return _checked_add(remaining, awarded)


def estimate_at_completion(incurred: int | None, etc: int | None) -> int | None:
    """Incurred + ETC. Unset when either side cannot support it.

    ⛔ AN UNSET ETC IS NOT EAC = 0. That invention is the silent forecast
    `/budget` already refuses. When the cut is supported this equals
    revised (incurred + remaining + awarded).
    """
    if incurred is None or etc is None:
        return None
    return _checked_add(incurred, etc)


def remaining_to_bill(revised: int | None, billed: int | None) -> int | None:
    """Revised − billed. Companion only — not an EAC input."""
    if revised is None or billed is None:
        return None
    return _checked_sub(revised, billed)


def _line(name: str, amount: int | None, note: str) -> Line:
    return Line(figure=name, amount=format_optional(amount), note=note)


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
        accounts_receivable=parse_optional_minor(raw.get("accounts_receivable")),
    )


def phase_from_cite(raw: Mapping[str, Any]) -> PhaseCite:
    name = str(raw.get("display_name") or raw.get("description") or "").strip()
    if not name:
        raise Refuse("a work-package row names a phase")
    incurred = raw.get("incurred") if "incurred" in raw else raw.get("cost")
    return PhaseCite(
        display_name=name,
        budget=parse_optional_minor(raw.get("budget")),
        approved_change_orders=parse_optional_minor(
            raw.get("approved_change_orders"), allow_signed=True
        ),
        incurred=parse_optional_minor(incurred, allow_signed=True),
        awarded=parse_optional_minor(raw.get("awarded"), allow_signed=True),
    )


def _phase_row(phase: PhaseCite) -> PhaseRow:
    revised = revised_contract(phase.budget, phase.approved_change_orders)
    remaining = remaining_to_spend(revised, phase.incurred, phase.awarded)
    etc = estimate_to_complete(remaining, phase.awarded)
    eac = estimate_at_completion(phase.incurred, etc)
    if remaining is None:
        assumption = (
            "unset — revised, incurred, and awarded cannot support "
            "remaining-to-spend; EAC is not a silent 0.00"
        )
    else:
        assumption = EAC_ASSUMPTION
    return PhaseRow(
        description=phase.display_name,
        original=format_optional(phase.budget),
        change_orders=format_optional(phase.approved_change_orders),
        revised=format_optional(revised),
        incurred=format_optional(phase.incurred),
        awarded=format_optional(phase.awarded),
        remaining_to_spend=format_optional(remaining),
        etc=format_optional(etc),
        eac=format_optional(eac),
        assumption=assumption,
    )


def _named_unset(
    *,
    original: int | None,
    approved: int | None,
    incurred: int | None,
    awarded: int | None,
    remaining: int | None,
    eac: int | None,
    etc: int | None,
) -> tuple[str, ...]:
    names: list[str] = []
    if original is None:
        names.append("original contract — CreateBook does not invent a baseline")
    if approved is None:
        names.append(
            "approved change orders — unposted is unset, not a silent net of nothing"
        )
    if incurred is None:
        names.append("incurred — billed / earned are not a substitute")
    if awarded is None:
        names.append(
            "awarded — treating awarded as 0 would print budget − actual as headroom"
        )
    if remaining is None:
        names.append(
            "remaining to spend — unset until revised, incurred, and awarded "
            "can support the cut; not a silent forecast of 0"
        )
    if etc is None:
        names.append("ETC — unset when remaining-to-spend cannot be cited")
    if eac is None:
        names.append("EAC — unset when remaining-to-spend cannot be cited, not 0.00")
    return tuple(names)


def build_pack(
    *,
    book: Book,
    client: Client,
    budget: BudgetCite | None = None,
    billing: BillingCite | None = None,
    phases: Sequence[PhaseCite] | None = None,
) -> Pack:
    """Read budget / billing / statements cites into an EAC pack.

    Remaining-to-spend is the core formula. EAC / ETC are computed
    outside the journal and stay unset when that formula cannot run.
    """
    _require_scopes(client)
    if book.kind != "PROJECT":
        raise Refuse(
            f"this app is BookKind PROJECT; {book.kind!r} keeps its own "
            "chrome and is not a job EAC pack. Personal cash forecast is #163"
        )
    resolved = budget if budget is not None else BudgetCite()
    progress = billing if billing is not None else BillingCite()

    revised = revised_contract(resolved.original, resolved.approved_change_orders)
    remaining = remaining_to_spend(revised, resolved.incurred, resolved.awarded)
    etc = estimate_to_complete(remaining, resolved.awarded)
    eac = estimate_at_completion(resolved.incurred, etc)
    leftover_bill = remaining_to_bill(revised, progress.billed)

    if resolved.original is None:
        original_note = "unset until [project] budget is set — not a priced zero"
    else:
        original_note = "[project] budget — the baseline a change order must not rewrite"

    if resolved.approved_change_orders is None:
        co_note = "unset — no approved change order has posted, not a silent zero"
    else:
        co_note = "work-package grain equity pair; does not rewrite [project] budget"

    if revised is None:
        revised_note = "unset until [project] budget is set — not a priced zero"
    elif resolved.approved_change_orders is None:
        revised_note = "equals the original — no approved change order has posted"
    else:
        revised_note = "revised contract when priced — original plus approved changes"

    if resolved.incurred is None:
        incurred_note = (
            "unset — billed / earned are not a substitute for incurred cost"
        )
    else:
        incurred_note = "costs + WIP — incurred, not billed, not earned"

    if resolved.awarded is None:
        awarded_note = (
            "unset until an award posts — treating awarded as 0 would print "
            "budget − actual as headroom"
        )
    else:
        awarded_note = "Awarded commitments — credit-normal; a posted 0 is a real zero"

    if remaining is None:
        remaining_note = (
            "unset until revised, incurred, and awarded can support the cut — "
            "not a silent forecast of 0"
        )
    else:
        remaining_note = REMAINING_NOTE

    if eac is None:
        eac_note = (
            "unset — remaining-to-spend cannot support a figure; not a silent "
            "EAC of 0.00"
        )
    else:
        eac_note = EAC_ASSUMPTION

    if etc is None:
        etc_note = (
            "unset — remaining-to-spend cannot support a figure; not a silent "
            "cost-to-complete of 0.00"
        )
    else:
        etc_note = ETC_ASSUMPTION

    cites = (
        _line("original_contract", resolved.original, original_note),
        _line("approved_change_orders", resolved.approved_change_orders, co_note),
        _line("revised_contract", revised, revised_note),
        _line("incurred", resolved.incurred, incurred_note),
        _line("awarded", resolved.awarded, awarded_note),
        _line("remaining_to_spend", remaining, remaining_note),
    )
    forecast = (
        _line("etc", etc, etc_note),
        _line("eac", eac, eac_note),
    )
    companions: list[Line] = [
        _line(
            "billed",
            progress.billed,
            "Progress billings — a /billing companion, not incurred and not an EAC input",
        ),
        _line(
            "earned",
            progress.earned,
            "Project revenue — independent of billings; not a substitute for incurred",
        ),
        _line(
            "remaining_to_bill",
            leftover_bill,
            (
                "unset until revised and billed can support the cut — not the "
                "whole contract as a fake remainder"
                if leftover_bill is None
                else "revised minus billed — a /billing cite, not EAC"
            ),
        ),
    ]
    if book.closed_through is not None:
        companions.append(
            Line(
                figure="closed_through",
                amount=book.closed_through.isoformat(),
                note="cited from statements:read — this pack does not post",
            )
        )

    phase_rows = tuple(_phase_row(phase) for phase in (phases or ()))
    unset = _named_unset(
        original=resolved.original,
        approved=resolved.approved_change_orders,
        incurred=resolved.incurred,
        awarded=resolved.awarded,
        remaining=remaining,
        eac=eac,
        etc=etc,
    )
    return Pack(
        cites=cites,
        forecast=forecast,
        companions=tuple(companions),
        phases=phase_rows,
        unset=unset,
        budget=resolved,
        remaining_to_spend=remaining,
        etc=etc,
        eac=eac,
    )


def cite_from_fixture(raw: Mapping[str, Any]) -> Pack:
    """Build a pack from a fixture that looks like `/budget` + `/billing`."""
    kind = str(raw.get("kind") or "PROJECT")
    book = Book(
        kind=kind,
        closed_through=parse_optional_day(raw.get("closed_through")),
    )
    budget = budget_from_cite(
        {
            "original": raw.get("original", raw.get("budget")),
            "approved_change_orders": raw.get("approved_change_orders"),
            "incurred": raw.get("incurred"),
            "awarded": raw.get("awarded"),
        }
    )
    progress_raw = raw.get("progress") if isinstance(raw.get("progress"), Mapping) else raw
    billing = billing_from_cite(progress_raw if isinstance(progress_raw, Mapping) else None)
    phases_raw = raw.get("phases") or ()
    if not isinstance(phases_raw, (list, tuple)):
        raise Refuse("phases is a list of work-package cites, not a guess")
    phases = tuple(phase_from_cite(p) for p in phases_raw)
    client = raw.get("client")
    return build_pack(
        book=book,
        client=client if isinstance(client, Client) else client_from_app(raw.get("app") or {}),
        budget=budget,
        billing=billing,
        phases=phases,
    )


def fetch_cites(*, token: str | None = None) -> None:
    """Refuse to pull. The grant path is not built.

    A green pack builder is not a door that opens. Live Connect
    OAuth is leftover; this app does not call /v1.
    """
    _ = token
    raise Refuse(
        "live Connect OAuth is leftover — the grant path "
        "is not built (leftover #22 / #150). This app does not "
        "pretend the door opens"
    )


def deliver(pack: Pack, *, token: str | None = None) -> None:
    """Refuse to push. Same leftover as fetch_cites."""
    _ = pack
    _ = token
    raise Refuse(
        "live Connect OAuth is leftover — the grant path "
        "is not built (leftover #22 / #150). This app does not "
        "deliver a pack against a door that is not open"
    )


def post_forecast(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. Forecast lines are not posted into the journal."""
    raise Refuse(
        "forecast lines are not posted — the catalog has no forecast "
        "template, journals:post is not requested, and journal:append is "
        "a refused alias. This app exports CSV/JSON. Mixing a forecast "
        "into project_cost* would make the journal a second ledger. "
        "This does not close #169 by inventing a write"
    )


def cpi_eac(*_args: Any, **_kwargs: Any) -> None:
    """Refuse. A CPI / percent-complete EAC is a rounded forecast."""
    raise Refuse(
        "a CPI / percent-complete EAC is a rounded figure and a silent "
        "forecast — this pack cites remaining-to-spend and emits EAC "
        "only when that cut is supported. No percentage"
    )


_CITE_COLUMNS = ("Figure", "Amount", "Note")
_FORECAST_COLUMNS = ("Figure", "Amount", "Assumption")
_PHASE_COLUMNS = (
    "Description",
    "Original",
    "Change orders",
    "Revised",
    "Incurred",
    "Awarded",
    "Remaining to spend",
    "ETC",
    "EAC",
    "Assumption",
)


def csv_cites(pack: Pack) -> str:
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(_CITE_COLUMNS)
    for r in pack.cites:
        w.writerow([r.figure, r.amount, r.note])
    return buf.getvalue()


def csv_forecast(pack: Pack) -> str:
    """EAC / ETC with the assumption written on the row. Blanks are unset."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(_FORECAST_COLUMNS)
    for r in pack.forecast:
        w.writerow([r.figure, r.amount, r.note])
    return buf.getvalue()


def csv_companions(pack: Pack) -> str:
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(_CITE_COLUMNS)
    for r in pack.companions:
        w.writerow([r.figure, r.amount, r.note])
    return buf.getvalue()


def csv_phases(pack: Pack) -> str:
    """Work-package EAC. No percent column — that would be a rounded figure."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(_PHASE_COLUMNS)
    for r in pack.phases:
        w.writerow(
            [
                r.description,
                r.original,
                r.change_orders,
                r.revised,
                r.incurred,
                r.awarded,
                r.remaining_to_spend,
                r.etc,
                r.eac,
                r.assumption,
            ]
        )
    return buf.getvalue()


def csv_unset(pack: Pack) -> str:
    """Named unset lines. Silence on the pack is the honesty, not a zero."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(("Unset line", "Reason"))
    for name in pack.unset:
        w.writerow([name.split(" — ", 1)[0], name])
    return buf.getvalue()


def as_json(pack: Pack) -> str:
    """JSON export. Empty strings stay empty — never a coerced 0."""
    return json.dumps(
        {
            "remaining_to_spend": format_optional(pack.remaining_to_spend),
            "etc": format_optional(pack.etc),
            "eac": format_optional(pack.eac),
            "cites": [
                {"figure": r.figure, "amount": r.amount, "note": r.note}
                for r in pack.cites
            ],
            "forecast": [
                {"figure": r.figure, "amount": r.amount, "assumption": r.note}
                for r in pack.forecast
            ],
            "unset": list(pack.unset),
        },
        indent=2,
    ) + "\n"


def as_files(pack: Pack) -> dict[str, str]:
    """Named companion sheets. Not a ZIP into core and not a /budget field."""
    return {
        "cites.csv": csv_cites(pack),
        "eac.csv": csv_forecast(pack),
        "companions.csv": csv_companions(pack),
        "phases.csv": csv_phases(pack),
        "unset.csv": csv_unset(pack),
        "eac.json": as_json(pack),
    }
