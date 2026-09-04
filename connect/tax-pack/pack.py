#!/usr/bin/env python3
"""Household tax-pack builder for BookKind PERSONAL.

A WorkOS Connect app, not a kernel RPC. Tax packing and 8949-ish /
CSV export live here. They do not live in `ratio watch`, the
operations console, or a new kernel method.

⭐ SCOPES ARE THE FROZEN CATALOG NAMES. `lots:read`, `statements:read`,
`config:read`. Aliases and invented strings are refused. See
docs/connect-scopes.md.

⭐ THIS APP IS READ-ONLY RELATIVE TO THE JOURNAL. It does not request
`journals:post`. A tax pack is a read of cites, not a rewrite.

⭐ A HOLDING-PERIOD CATEGORY IS A DATE AGREEMENT, NOT A GUESS.
When the acquired dates on a disposal disagree — the average-cost
pool leftover on #9 — the pack leaves the category unset. Inventing
FIFO's oldest date makes a mixed pool long-term. Inventing two boxes
makes it both. Conservation holds; the figure that goes wrong is the
rate. `Ratio.Lots.Methods.the_threshold_day_is_long_term` is the
boundary when the dates agree.

⭐ MONEY IS MINOR UNITS, PARSED BY SPLITTING ON THE POINT. A float
is how a cent disappears. A third decimal place is refused.

⭐ WASH IS A CITE, NOT A REWRITE. `Ratio.Lots.WashRestatement` — a
restatement cites the strike. The pack copies the code and the
adjustment; it does not re-run the wash engine and it does not
invent `lot_method = "wash"`.

⚠ THE GRANT PATH IS NOT BUILT. `fetch_cites` refuses. A Connect
access token is not accepted on `/v1` (#150 / #151 / leftover #22).

⚠ IRS E-FILE IS REFUSED. `submit` refuses. No CPA portal, no
MeF transmission, no packing inside core.
"""

from __future__ import annotations

import csv
import io
import json
from dataclasses import dataclass
from datetime import date
from typing import Any, Iterable, Mapping, Sequence

# i64 bounds. Lean's Int is unbounded; every money figure here is i64.
I64_MIN = -(2**63)
I64_MAX = 2**63 - 1

CANONICAL_SCOPES = frozenset({"lots:read", "statements:read", "config:read"})
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

# RuleSet fields this pack cites. Names must stay the ones
# crates/ratio-rules already stores — a silent rename is how a
# pack invents an election.
LOT_TERM_FIELDS = (
    "long_term_days",
    "wash_window_days",
    "wash_keep_holding_period",
    "lot_method",
    "min_tax_short_weight",
    "average_cost",
)

# Kernel default when the cite omits the threshold. Same number
# `RuleSet` writes. Wash window has no such default — silence is
# unset, not 30.
DEFAULT_LONG_TERM_DAYS = 365


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
class LotTerms:
    """Lot-terms cite from `config:read`. Unset stays unset."""

    long_term_days: int = DEFAULT_LONG_TERM_DAYS
    wash_window_days: int | None = None
    wash_keep_holding_period: bool | None = None
    lot_method: str | None = None
    min_tax_short_weight: int | None = None
    average_cost: bool | None = None


@dataclass(frozen=True)
class WashCite:
    """A WashRestatement cite. Copied, not recomputed."""

    disallowed_loss: int
    code: str = "W"
    sold_on: date | None = None
    window_days: int | None = None


@dataclass(frozen=True)
class Disposal:
    """One lots:read disposal the pack is asked to report."""

    instrument: str
    disposed: date
    proceeds: int
    basis: int
    currency: str
    description: str = ""
    acquired: date | None = None
    acquired_dates: tuple[date | None, ...] = ()
    units: int | None = None
    wash: WashCite | None = None


@dataclass(frozen=True)
class Form8949Row:
    """One 8949-ish line. `category` is SHORT, LONG, or empty."""

    description: str
    acquired: str
    disposed: str
    proceeds: str
    basis: str
    adjustment_code: str
    adjustment: str
    gain: str
    category: str
    ambiguity: str
    currency: str
    instrument: str


@dataclass(frozen=True)
class Pack:
    """An 8949-ish pack plus companion sheets. Not an IRS submission."""

    form_8949: tuple[Form8949Row, ...]
    unclassified: tuple[Form8949Row, ...]
    wash_cites: tuple[Form8949Row, ...]
    lot_terms: LotTerms
    ambiguities: tuple[str, ...]


def load_app(path: str) -> dict[str, Any]:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def client_from_app(app: Mapping[str, Any]) -> Client:
    connect = app.get("workos_connect") or {}
    scopes = connect.get("scopes") or []
    return Client(
        client_id=str(connect.get("client_id") or app.get("name") or "ratio-tax-pack"),
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
            f"{text!r} is signed; proceeds and basis are magnitudes, and a "
            "signed-amount inference is how a loss and a gain swap"
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
    return v


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
        raise Refuse("an undated disposal is refused — a holding period needs two days")
    t = text.strip()
    try:
        y, m, d = t.split("-")
        return date(int(y), int(m), int(d))
    except ValueError as e:
        raise Refuse(f"{text!r} is not a calendar day YYYY-MM-DD") from e


def parse_optional_day(text: Any) -> date | None:
    if text is None:
        return None
    if not isinstance(text, str) or not text.strip():
        return None
    return parse_day(text)


def _require_scopes(client: Client) -> None:
    aliases = client.scopes & REFUSED_ALIASES
    if aliases:
        raise Refuse(
            "refused alias scope "
            + ", ".join(sorted(aliases))
            + " — catalogs use lots:read / statements:read / config:read"
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
            "is a write grant this pack does not need"
        )
    missing = CANONICAL_SCOPES - client.scopes
    if missing:
        raise Refuse(
            "this app needs "
            + ", ".join(sorted(CANONICAL_SCOPES))
            + f"; missing {', '.join(sorted(missing))}. "
            "lots:read is the disposals; config:read is lot-terms; "
            "statements:read is how closed-through is read"
        )


def _require_terms(terms: LotTerms) -> None:
    if terms.lot_method is not None:
        method = terms.lot_method.strip().lower()
        if method in REFUSED_LOT_METHODS:
            raise Refuse(
                f'lot_method = "{terms.lot_method}" stays refused — '
                "MinTax, SpecID, average cost, and wash are elections, "
                "not a Method, an Order, or a lot_method variant"
            )
    if terms.average_cost is False:
        raise Refuse(
            "average_cost = false is not an election — omit the field. "
            "None is unset, not a silent true"
        )
    if terms.wash_keep_holding_period is False:
        raise Refuse(
            "wash_keep_holding_period = false is not an election — omit "
            "the field. None is unset, not a silent keep"
        )
    if terms.wash_keep_holding_period is True and terms.wash_window_days is None:
        raise Refuse(
            "this configuration elects wash_keep_holding_period without "
            "wash_window_days. A holding-period variant of a wash nobody "
            "wrote is not a cite"
        )
    if terms.long_term_days <= 0:
        raise Refuse(
            f"long_term_days is {terms.long_term_days}, and a non-positive "
            "threshold is not a holding period"
        )
    if terms.wash_window_days is not None and terms.wash_window_days < 0:
        raise Refuse(
            f"wash_window_days is {terms.wash_window_days}, and a negative "
            "window is not a window"
        )


def pool_dates(disposal: Disposal) -> tuple[date | None, ...]:
    """The acquired dates the pack is allowed to see.

    ⛔ A SINGLE `acquired` NEXT TO A DISAGREEING LIST IS NOT FIFO'S
    OLDEST. If both are present they must agree; otherwise the pack
    would silently prefer one shape and invent a category.
    """
    listed = disposal.acquired_dates
    if listed:
        if disposal.acquired is not None and disposal.acquired not in listed:
            raise Refuse(
                f"{disposal.instrument}: acquired {disposal.acquired.isoformat()} "
                "is not among acquired_dates — preferring the single date "
                "would invent FIFO's oldest on a mixed pool"
            )
        return listed
    return (disposal.acquired,)


def holding_period_category(
    acquired_dates: Sequence[date | None],
    disposed: date,
    long_term_days: int,
) -> str:
    """SHORT or LONG when every date agrees. Refuse when they do not.

    ⛔ MIXED DATES STAY UNSET. US single-category would invent FIFO's
    oldest date and classify the sale long-term. Double-category would
    invent two boxes. Both invent a short-vs-long answer the lots do
    not support. #9 leftover; the tax pack must not close it by guessing.

    ⭐ `the_threshold_day_is_long_term`. Held exactly `long_term_days`
    is LONG. Off by one moves a disposal between tax rates and nothing
    about the figure looks unusual.
    """
    if not acquired_dates:
        raise Refuse(
            "a disposal with no acquisition date cannot be classified — "
            "the epoch would make it long and today would make it short"
        )
    if any(d is None for d in acquired_dates):
        raise Refuse(
            "a missing acquisition date cannot be classified — "
            "silence is not a silent long and not a silent short"
        )
    distinct = set(acquired_dates)
    if len(distinct) > 1:
        shown = ", ".join(sorted(d.isoformat() for d in distinct if d is not None))
        raise Refuse(
            "acquired dates disagree ("
            + shown
            + ") — a pooled holding-period category would invent "
            "FIFO's oldest date or two Form 8949 boxes. Mixed dates "
            "stay unset (#9 leftover)"
        )
    acquired = next(iter(distinct))
    assert acquired is not None
    held = (disposed - acquired).days
    if held < 0:
        raise Refuse(
            f"acquired {acquired.isoformat()} is after disposed "
            f"{disposed.isoformat()} — a negative holding period is not a category"
        )
    if held >= long_term_days:
        return "LONG"
    return "SHORT"


def _gain(proceeds: int, basis: int, adjustment: int) -> int:
    """Form 8949 column (h) ≈ proceeds − basis + adjustment.

    Asked before the product. A wrap is a refuse, not a gain.
    Python's int is unbounded; the refuse is the i64 door.
    """
    if not (0 <= proceeds <= I64_MAX) or not (0 <= basis <= I64_MAX):
        raise Refuse("proceeds and basis must fit in i64 magnitudes")
    if adjustment > I64_MAX or adjustment < I64_MIN:
        raise Refuse("adjustment does not fit in i64")
    total = (proceeds - basis) + adjustment
    if total > I64_MAX or total < I64_MIN:
        raise Refuse("gain does not fit in i64 minor units")
    return total


def _row_from(
    disposal: Disposal,
    *,
    category: str,
    ambiguity: str,
) -> Form8949Row:
    wash = disposal.wash
    adj = wash.disallowed_loss if wash is not None else 0
    code = wash.code if wash is not None and adj else ""
    if wash is not None and wash.code and wash.code != "W":
        raise Refuse(
            f"wash code {wash.code!r} is not a cite this pack copies — "
            "it does not invent an adjustment code"
        )
    dates = pool_dates(disposal)
    agreed = None
    present = [d for d in dates if d is not None]
    if len(set(present)) == 1 and all(d is not None for d in dates):
        agreed = present[0]
    acquired_text = agreed.isoformat() if agreed is not None else ""
    desc = disposal.description or disposal.instrument
    return Form8949Row(
        description=desc,
        acquired=acquired_text,
        disposed=disposal.disposed.isoformat(),
        proceeds=format_minor(disposal.proceeds),
        basis=format_minor(disposal.basis),
        adjustment_code=code,
        adjustment=format_minor(adj) if adj else "",
        gain=format_minor(_gain(disposal.proceeds, disposal.basis, adj)),
        category=category,
        ambiguity=ambiguity,
        currency=disposal.currency,
        instrument=disposal.instrument,
    )


def disposal_from_cite(row: Mapping[str, Any]) -> Disposal:
    instrument = str(row.get("instrument") or "").strip()
    if not instrument:
        raise Refuse("a disposal names an instrument")
    currency = str(row.get("currency") or "").strip().upper()
    if len(currency) != 3 or not currency.isalpha():
        raise Refuse(f"{currency!r} is not an ISO currency code")
    disposed = parse_day(str(row.get("disposed") or ""))
    acquired = parse_optional_day(row.get("acquired"))
    raw_dates = row.get("acquired_dates")
    dates: tuple[date | None, ...] = ()
    if raw_dates is not None:
        if not isinstance(raw_dates, (list, tuple)):
            raise Refuse("acquired_dates is a list of calendar days, not a guess")
        dates = tuple(parse_optional_day(d) for d in raw_dates)
    wash = None
    raw_wash = row.get("wash")
    if raw_wash:
        if not isinstance(raw_wash, Mapping):
            raise Refuse("a wash cite is a record, not a Method")
        loss = parse_minor(str(raw_wash.get("disallowed_loss") or "0"))
        code = str(raw_wash.get("code") or "W").strip() or "W"
        window = raw_wash.get("window_days")
        wash = WashCite(
            disallowed_loss=loss,
            code=code,
            sold_on=parse_optional_day(raw_wash.get("sold_on")),
            window_days=int(window) if window is not None and window != "" else None,
        )
    units = row.get("units")
    return Disposal(
        instrument=instrument,
        description=str(row.get("description") or "").strip(),
        acquired=acquired,
        acquired_dates=dates,
        disposed=disposed,
        proceeds=parse_minor(str(row.get("proceeds") or "")),
        basis=parse_minor(str(row.get("basis") or "")),
        currency=currency,
        units=int(units) if units not in (None, "") else None,
        wash=wash,
    )


def terms_from_cite(raw: Mapping[str, Any] | None) -> LotTerms:
    if raw is None:
        return LotTerms()
    method = raw.get("lot_method")
    method_s = str(method).strip() if method not in (None, "") else None

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
            raise Refuse(f"{key} is a flag; omit the field rather than inventing a third meaning")
        return v

    long_term = raw.get("long_term_days")
    return LotTerms(
        long_term_days=int(long_term) if long_term not in (None, "") else DEFAULT_LONG_TERM_DAYS,
        wash_window_days=_opt_int("wash_window_days"),
        wash_keep_holding_period=_opt_bool("wash_keep_holding_period"),
        lot_method=method_s,
        min_tax_short_weight=_opt_int("min_tax_short_weight"),
        average_cost=_opt_bool("average_cost"),
    )


def build_pack(
    rows: Iterable[Mapping[str, Any]],
    *,
    book: Book,
    client: Client,
    terms: LotTerms | None = None,
) -> Pack:
    """Read lot / wash / lot-terms cites into an 8949-ish pack.

    One mixed-date row does not invent a category for the others.
    Classified rows go on Form 8949; unclassified rows go on the
    companion sheet with the ambiguity named.
    """
    _require_scopes(client)
    if book.kind != "PERSONAL":
        raise Refuse(
            f"this app is BookKind PERSONAL; {book.kind!r} keeps its own "
            "chrome and is not a household tax pack"
        )
    resolved = terms if terms is not None else LotTerms()
    _require_terms(resolved)

    classified: list[Form8949Row] = []
    unclassified: list[Form8949Row] = []
    washes: list[Form8949Row] = []
    ambiguities: list[str] = []

    for raw in rows:
        disposal = disposal_from_cite(raw)
        if book.closed_through is not None and disposal.disposed <= book.closed_through:
            raise Refuse(
                f"disposal dated {disposal.disposed.isoformat()} is on or before "
                f"closed-through {book.closed_through.isoformat()}"
            )
        dates = pool_dates(disposal)
        try:
            category = holding_period_category(
                dates, disposal.disposed, resolved.long_term_days
            )
            ambiguity = ""
        except Refuse as e:
            msg = str(e)
            # ⛔ ONLY DATE-AGREEMENT / MISSING-DATE REFUSALS BECOME A
            # COMPANION ROW. A wrapped amount or a forged wash code still
            # fails the pack — those are not #9 leftovers.
            date_reasons = (
                "acquired dates disagree",
                "no acquisition date",
                "missing acquisition date",
                "after disposed",
            )
            if not any(s in msg for s in date_reasons):
                raise
            category = ""
            ambiguity = msg
        row = _row_from(disposal, category=category, ambiguity=ambiguity)
        if category:
            classified.append(row)
        else:
            unclassified.append(row)
            ambiguities.append(f"{disposal.instrument}: {ambiguity}")
        if disposal.wash is not None:
            washes.append(row)

    return Pack(
        form_8949=tuple(classified),
        unclassified=tuple(unclassified),
        wash_cites=tuple(washes),
        lot_terms=resolved,
        ambiguities=tuple(ambiguities),
    )


def fetch_cites(*, token: str | None = None) -> None:
    """Refuse to pull. The grant path is not built.

    A green pack builder is not a door that opens. Connect access
    tokens are not accepted on /v1.
    """
    _ = token
    raise Refuse(
        "Connect access tokens are not accepted on /v1 — the grant path "
        "is not built (#150 / #151 / leftover #22). This app does not "
        "pretend the door opens"
    )


def submit(pack: Pack, *, token: str | None = None) -> None:
    """Refuse to e-file. Core refuses tax packing; so does this app."""
    _ = pack
    _ = token
    raise Refuse(
        "IRS e-file and a CPA portal are refused — tax packing is a "
        "Connect-app CSV of cites, not a submission. This does not "
        "close #166 by pretending a return was filed"
    )


_8949_COLUMNS = (
    "Description of property",
    "Date acquired",
    "Date sold or disposed of",
    "Proceeds",
    "Cost or other basis",
    "Code(s) from instructions",
    "Amount of adjustment",
    "Gain or (loss)",
    "Holding-period category",
    "Ambiguity",
    "Currency",
    "Instrument",
)


def _write_8949(rows: Sequence[Form8949Row]) -> str:
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(_8949_COLUMNS)
    for r in rows:
        w.writerow(
            [
                r.description,
                r.acquired,
                r.disposed,
                r.proceeds,
                r.basis,
                r.adjustment_code,
                r.adjustment,
                r.gain,
                r.category,
                r.ambiguity,
                r.currency,
                r.instrument,
            ]
        )
    return buf.getvalue()


def csv_form_8949(pack: Pack) -> str:
    """Classified rows only. Empty category never appears here."""
    for r in pack.form_8949:
        if r.category not in ("SHORT", "LONG"):
            raise Refuse(
                "form_8949.csv would carry an invented or blank category — "
                "unclassified rows belong on the companion sheet"
            )
    return _write_8949(pack.form_8949)


def csv_unclassified(pack: Pack) -> str:
    """Companion sheet: date disagreement and missing dates, named."""
    return _write_8949(pack.unclassified)


def csv_wash(pack: Pack) -> str:
    """WashRestatement cites. Copied, not recomputed."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(
        (
            "Instrument",
            "Date sold",
            "Disallowed loss",
            "Code",
            "Gain or (loss) after cite",
            "Note",
        )
    )
    for r in pack.wash_cites:
        w.writerow(
            [
                r.instrument,
                r.disposed,
                r.adjustment,
                r.adjustment_code,
                r.gain,
                "WashRestatement cite — the strike is not rewritten",
            ]
        )
    return buf.getvalue()


def csv_lot_terms(pack: Pack) -> str:
    """Lot-terms cite. Unset stays a blank, not a silent default."""
    t = pack.lot_terms
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(("Field", "Value", "Note"))

    def val(v: Any) -> str:
        if v is None:
            return ""
        if isinstance(v, bool):
            return "true" if v else "false"
        return str(v)

    rows = (
        (
            "long_term_days",
            val(t.long_term_days),
            "threshold day is long-term; kernel default is 365 when the cite omits it",
        ),
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
            "lot_method",
            val(t.lot_method),
            "not min_tax / specific_id / average_cost / wash",
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
    for row in rows:
        w.writerow(row)
    return buf.getvalue()


def as_files(pack: Pack) -> dict[str, str]:
    """Named companion sheets. Not a ZIP to MeF."""
    return {
        "form_8949.csv": csv_form_8949(pack),
        "unclassified.csv": csv_unclassified(pack),
        "wash_cites.csv": csv_wash(pack),
        "lot_terms.csv": csv_lot_terms(pack),
    }
