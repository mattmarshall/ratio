#!/usr/bin/env python3
"""Project AIA pay-app packer for BookKind PROJECT.

A WorkOS Connect app, not a kernel RPC. G702-ish / G703-ish CSV
export lives here. It does not live in `ratio watch`, the operations
console, or a new kernel method.

⭐ SCOPES ARE THE FROZEN CATALOG NAMES. `billing:read`, `budget:read`,
`statements:read`. Aliases (`projects:billing:read`, `journal:append`)
and invented strings are refused. See docs/connect-scopes.md.

⭐ THIS APP IS READ-ONLY RELATIVE TO THE JOURNAL. It does not request
`journals:post`. A pay-app pack is a read of cites, not a rewrite.

⭐ UNSET STAYS UNSET. An unbilled job is not billed-zero. An unposted
change order is not a silent net of nothing on the G702-ish change
line. An omitted prior application is not previous-certificates 0.00.
A real posted zero (`"0.00"`) is a figure. Inventing those zeros is
how a form looks complete while the journal cannot support the cut.

⭐ MONEY IS MINOR UNITS, PARSED BY SPLITTING ON THE POINT. A float
is how a cent disappears. A third decimal place is refused.

⭐ BILLED IS NOT EARNED AND COST IS NOT COMPLETED. Progress billings
credit is the G702-ish "billed to date". Project revenue is a
companion cite. Phase cost is incurred, not schedule-of-values
completed-and-stored. Substituting one for the other is a
misstatement of a pay-app.

⭐ NO PERCENTAGE. G703 column H would be a rounded figure. The pack
does not emit one.

⭐ THE GRANT PATH CALLS CONNECTAPIURL. `fetch_cites` and `deliver`
present a verified Connect access token against the Connect HTTP
API. Membership is still required. A licensed AIA form stays
refused. WorkOS dashboard registration stays leftover #22.

⚠ A LICENSED AIA FORM IS REFUSED. `render_form` refuses. No vendor
portal, no G702 product route in Console, no packing inside core.
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

CANONICAL_SCOPES = frozenset({"billing:read", "budget:read", "statements:read"})
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
class BillingCite:
    """`billing:read` / `projectProgress` cuts. Empty is unset."""

    billed: int | None = None
    earned: int | None = None
    retainage_receivable: int | None = None
    retainage_payable: int | None = None
    accounts_receivable: int | None = None


@dataclass(frozen=True)
class ContractCite:
    """`budget:read` original plus journal change-order equity."""

    original: int | None = None
    approved_change_orders: int | None = None


@dataclass(frozen=True)
class PhaseCite:
    """One work-package row. Cost `"0"` on a seeded phase is a true zero."""

    display_name: str
    budget: int | None = None
    approved_change_orders: int | None = None
    cost: int | None = None
    completed: int | None = None
    prior_completed: int | None = None


@dataclass(frozen=True)
class ApplicationCite:
    """One as-of application. Prior is a second cut, not a silent zero."""

    billed: int | None = None
    retainage_receivable: int | None = None


@dataclass(frozen=True)
class G702Line:
    """One G702-ish application line. `amount` empty means unset."""

    line: str
    amount: str
    note: str


@dataclass(frozen=True)
class G703Row:
    """One G703-ish schedule-of-values row. Blanks are unset cites."""

    item: str
    description: str
    scheduled_value: str
    change_orders: str
    revised_value: str
    previous_completed: str
    this_period: str
    completed_and_stored: str
    cost_to_date: str
    materials_stored: str
    retainage: str
    balance_to_finish: str


@dataclass(frozen=True)
class Pack:
    """A G702-ish / G703-ish pack. Not a licensed AIA submission."""

    g702: tuple[G702Line, ...]
    g703: tuple[G703Row, ...]
    companions: tuple[G702Line, ...]
    unset: tuple[str, ...]
    contract: ContractCite
    billing: BillingCite


def load_app(path: str) -> dict[str, Any]:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def client_from_app(app: Mapping[str, Any]) -> Client:
    connect = app.get("workos_connect") or {}
    scopes = connect.get("scopes") or []
    return Client(
        client_id=str(connect.get("client_id") or app.get("name") or "ratio-aia-pay-app"),
        scopes=frozenset(scopes),
    )


def parse_minor(text: str, *, allow_signed: bool = False) -> int:
    """Split on the point. Never parse a float.

    Same door as `ratio_common::parse_minor`. A third decimal place is
    refused rather than dropped. Overflow is refused rather than wrapped.
    Change-order nets and billed-minus-earned may be signed; billed,
    retainage, and original contract are magnitudes unless asked.
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
            + " — catalogs use billing:read / budget:read / statements:read"
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
            "billing:read is billed / earned / retainage; budget:read is "
            "the original contract and phase baselines; statements:read "
            "is how closed-through is read"
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

    Same door as `revisedContract` on `/billing`. An unknown baseline
    cannot be priced. An unposted CO does not block a set original —
    revised equals the original, and the change-order *line* stays unset.
    """
    if original is None:
        return None
    return _checked_add(original, approved if approved is not None else 0)


def billed_less_retainage(billed: int | None, retainage: int | None) -> int | None:
    """Billed − retainage held.

    ⛔ UNSET BILLED CANNOT SUPPORT THIS CUT. An unheld retainage is 0
    for the subtraction (no hold is not an unknown hold) — same as
    `collectedAgainstBilled` on `/billing`.
    """
    if billed is None:
        return None
    held = 0 if retainage is None else retainage
    return _checked_sub(billed, held)


def remaining_to_bill(revised: int | None, billed: int | None) -> int | None:
    """Revised − billed. Unset when either side cannot support it.

    Treating billed as 0 would print the whole contract as remaining.
    """
    if revised is None or billed is None:
        return None
    return _checked_sub(revised, billed)


def collected_against_billed(
    billed: int | None,
    ar: int | None,
    retainage: int | None,
) -> int | None:
    """Cash against AR: billed − AR − retainage held.

    Unset billed or unset AR cannot support the cut. Unheld retainage
    is 0 for the subtraction.
    """
    if billed is None or ar is None:
        return None
    held = 0 if retainage is None else retainage
    return _checked_sub(_checked_sub(billed, ar), held)


def current_payment_due(
    this_less_retainage: int | None,
    previous_less_retainage: int | None,
) -> int | None:
    """This application minus previous certificates.

    ⛔ AN OMITTED PRIOR IS NOT PREVIOUS 0.00. That invention is how a
    first pay-app looks current when the journal has no prior cut.
    """
    if this_less_retainage is None or previous_less_retainage is None:
        return None
    return _checked_sub(this_less_retainage, previous_less_retainage)


def this_period_completed(completed: int | None, prior: int | None) -> int | None:
    """Current completed minus prior completed. Both cites required."""
    if completed is None or prior is None:
        return None
    return _checked_sub(completed, prior)


def _line(name: str, amount: int | None, note: str) -> G702Line:
    return G702Line(line=name, amount=format_optional(amount), note=note)


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


def contract_from_cite(raw: Mapping[str, Any] | None) -> ContractCite:
    if raw is None:
        return ContractCite()
    return ContractCite(
        original=parse_optional_minor(raw.get("original") if "original" in raw else raw.get("budget")),
        approved_change_orders=parse_optional_minor(
            raw.get("approved_change_orders"), allow_signed=True
        ),
    )


def prior_from_cite(raw: Mapping[str, Any] | None) -> ApplicationCite | None:
    """None when the prior cut is omitted. A present object may still have unset fields."""
    if raw is None:
        return None
    if not isinstance(raw, Mapping):
        raise Refuse("a prior application is a cite record, not a silent zero")
    return ApplicationCite(
        billed=parse_optional_minor(raw.get("billed")),
        retainage_receivable=parse_optional_minor(raw.get("retainage_receivable")),
    )


def phase_from_cite(raw: Mapping[str, Any]) -> PhaseCite:
    name = str(raw.get("display_name") or raw.get("description") or "").strip()
    if not name:
        raise Refuse("a schedule-of-values row names a work package")
    return PhaseCite(
        display_name=name,
        budget=parse_optional_minor(raw.get("budget")),
        approved_change_orders=parse_optional_minor(
            raw.get("approved_change_orders"), allow_signed=True
        ),
        cost=parse_optional_minor(raw.get("cost"), allow_signed=True),
        completed=parse_optional_minor(raw.get("completed")),
        prior_completed=parse_optional_minor(raw.get("prior_completed")),
    )


def _g703_row(phase: PhaseCite, item: str) -> G703Row:
    revised = revised_contract(phase.budget, phase.approved_change_orders)
    previous = phase.prior_completed
    this_period = this_period_completed(phase.completed, previous)
    # ⛔ COST IS NOT COMPLETED-AND-STORED. Only an explicit completed cite
    # fills that column. Phase cost is a companion, labelled as cost.
    completed = phase.completed
    balance = remaining_to_bill(revised, completed)
    return G703Row(
        item=item,
        description=phase.display_name,
        scheduled_value=format_optional(phase.budget),
        change_orders=format_optional(phase.approved_change_orders),
        revised_value=format_optional(revised),
        previous_completed=format_optional(previous),
        this_period=format_optional(this_period),
        completed_and_stored=format_optional(completed),
        cost_to_date=format_optional(phase.cost),
        materials_stored="",
        retainage="",
        balance_to_finish=format_optional(balance),
    )


def build_pack(
    *,
    book: Book,
    client: Client,
    contract: ContractCite | None = None,
    billing: BillingCite | None = None,
    prior: ApplicationCite | None | object = ...,
    phases: Sequence[PhaseCite] | None = None,
) -> Pack:
    """Read billing / budget / retainage / CO cites into a pay-app pack.

    `prior is ...` (omitted) means no previous-application cut — those
    lines stay unset. Pass `ApplicationCite(...)` to cite one.
    """
    _require_scopes(client)
    if book.kind != "PROJECT":
        raise Refuse(
            f"this app is BookKind PROJECT; {book.kind!r} keeps its own "
            "chrome and is not a job pay-app pack"
        )
    resolved = contract if contract is not None else ContractCite()
    progress = billing if billing is not None else BillingCite()
    prior_cite: ApplicationCite | None
    if prior is ...:
        prior_cite = None
    else:
        prior_cite = prior  # type: ignore[assignment]

    revised = revised_contract(resolved.original, resolved.approved_change_orders)
    this_net = billed_less_retainage(progress.billed, progress.retainage_receivable)
    if prior_cite is None:
        previous_net = None
        previous_note = (
            "unset — no previous-application cut was cited, not a silent "
            "first pay-app of previous 0.00"
        )
    else:
        previous_net = billed_less_retainage(
            prior_cite.billed, prior_cite.retainage_receivable
        )
        previous_note = (
            "previous billed less previous retainage"
            if previous_net is not None
            else "unset — previous application cited without billed"
        )
    current = current_payment_due(this_net, previous_net)
    leftover = remaining_to_bill(revised, progress.billed)
    collected = collected_against_billed(
        progress.billed, progress.accounts_receivable, progress.retainage_receivable
    )
    billed_minus_earned = None
    if progress.billed is not None and progress.earned is not None:
        billed_minus_earned = _checked_sub(progress.billed, progress.earned)

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

    if progress.billed is None:
        billed_note = "unset until a progress bill posts — not a fake zero billed"
    else:
        billed_note = "Progress billings credit — not earned, not phase cost"

    if progress.retainage_receivable is None:
        retainage_note = "unset until a retainage hold posts — not a silent hold of 0.00"
    else:
        retainage_note = "Retainage receivable — held from a progress bill"

    if this_net is None:
        net_note = "unset until billed can support billed minus retainage"
    elif progress.retainage_receivable is None:
        net_note = "billed minus retainage; unheld retainage is 0 for the subtraction"
    else:
        net_note = "billed minus retainage held"

    if current is None:
        current_note = (
            "unset until this application and a previous-application cut "
            "can both be cited — not a silent current equal to billed-to-date"
        )
    else:
        current_note = "this billed-less-retainage minus previous certificates"

    if leftover is None:
        leftover_note = (
            "unset until revised and billed can support the cut — not the "
            "whole contract as a fake remainder"
        )
    else:
        leftover_note = "revised minus billed — remaining to bill"

    g702 = (
        _line("original_contract", resolved.original, original_note),
        _line("net_change_orders", resolved.approved_change_orders, co_note),
        _line("contract_sum_to_date", revised, revised_note),
        _line("total_billed_to_date", progress.billed, billed_note),
        _line("retainage_held", progress.retainage_receivable, retainage_note),
        _line("billed_less_retainage", this_net, net_note),
        _line("previous_certificates", previous_net, previous_note),
        _line("current_payment_due", current, current_note),
        _line("balance_to_finish", leftover, leftover_note),
    )

    companions: list[G702Line] = [
        _line(
            "earned_to_date",
            progress.earned,
            "Project revenue credit — independent of billings; not a substitute for billed",
        ),
        _line(
            "billed_minus_earned",
            billed_minus_earned,
            (
                "unset until both billed and earned have posted — not a fake caught-up zero"
                if billed_minus_earned is None
                else "overbilling when positive; underbilling when negative"
            ),
        ),
        _line(
            "collected",
            collected,
            (
                "unset until billed and accounts receivable can support cash against AR"
                if collected is None
                else "cash against AR — billed minus outstanding receivable and retainage held"
            ),
        ),
        _line(
            "retainage_payable",
            progress.retainage_payable,
            "held from a vendor invoice — a different account from retainage receivable",
        ),
    ]
    if book.closed_through is not None:
        companions.append(
            G702Line(
                line="closed_through",
                amount=book.closed_through.isoformat(),
                note="cited from statements:read — this pack does not post",
            )
        )

    sov = tuple(
        _g703_row(phase, str(i + 1)) for i, phase in enumerate(phases or ())
    )

    unset = tuple(line.line for line in g702 if line.amount == "")
    return Pack(
        g702=g702,
        g703=sov,
        companions=tuple(companions),
        unset=unset,
        contract=resolved,
        billing=progress,
    )


def cite_from_fixture(raw: Mapping[str, Any]) -> Pack:
    """Build a pack from a fixture that looks like `/billing` + `/budget`."""
    kind = str(raw.get("kind") or "PROJECT")
    book = Book(
        kind=kind,
        closed_through=parse_optional_day(raw.get("closed_through")),
    )
    contract = contract_from_cite(
        {
            "original": raw.get("original", raw.get("budget")),
            "approved_change_orders": raw.get("approved_change_orders"),
        }
    )
    progress_raw = raw.get("progress") if isinstance(raw.get("progress"), Mapping) else raw
    billing = billing_from_cite(progress_raw if isinstance(progress_raw, Mapping) else None)
    prior = prior_from_cite(raw.get("prior") if "prior" in raw else None)
    phases_raw = raw.get("phases") or ()
    if not isinstance(phases_raw, (list, tuple)):
        raise Refuse("phases is a list of work-package cites, not a guess")
    phases = tuple(phase_from_cite(p) for p in phases_raw)
    client = raw.get("client")
    return build_pack(
        book=book,
        client=client if isinstance(client, Client) else client_from_app(raw.get("app") or {}),
        contract=contract,
        billing=billing,
        prior=prior if "prior" in raw else ...,
        phases=phases,
    )


def fetch_cites(
    *,
    token: str | None = None,
    book_id: str | None = None,
    transport: _grant.Transport | None = None,
) -> Any:
    """Pull billing / budget cites from ConnectApiUrl."""
    return _grant.pull(
        token=token,
        book_id=book_id,
        transport=transport,
        error=Refuse,
    )


def render_form(pack: Pack, *, token: str | None = None) -> None:
    """Refuse a licensed AIA PDF. Core refuses G702 product UI; so does this app."""
    _ = pack
    _ = token
    raise Refuse(
        "a licensed AIA G702/G703 form is refused — this app emits "
        "G702-ish / G703-ish CSV of cites, not a copyrighted form and "
        "not a vendor portal. This does not close #184 by pretending "
        "a pay-app was filed"
    )


def deliver(
    pack: Pack,
    *,
    token: str | None = None,
    transport: _grant.Transport | None = None,
) -> Pack:
    """Confirm ConnectApiUrl membership, then return the local pack.

    Does not render a licensed AIA form — `render_form` stays refused.
    """
    _grant.pull(token=token, transport=transport, error=Refuse)
    return pack


_G702_COLUMNS = ("Line", "Amount", "Note")
_G703_COLUMNS = (
    "Item",
    "Description of work",
    "Scheduled value",
    "Change orders",
    "Revised value",
    "Previous completed",
    "This period",
    "Completed and stored",
    "Cost to date",
    "Materials stored",
    "Retainage",
    "Balance to finish",
)


def csv_g702(pack: Pack) -> str:
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(_G702_COLUMNS)
    for r in pack.g702:
        w.writerow([r.line, r.amount, r.note])
    return buf.getvalue()


def csv_g703(pack: Pack) -> str:
    """Schedule of values. No percent column — that would be a rounded figure."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(_G703_COLUMNS)
    for r in pack.g703:
        w.writerow(
            [
                r.item,
                r.description,
                r.scheduled_value,
                r.change_orders,
                r.revised_value,
                r.previous_completed,
                r.this_period,
                r.completed_and_stored,
                r.cost_to_date,
                r.materials_stored,
                r.retainage,
                r.balance_to_finish,
            ]
        )
    return buf.getvalue()


def csv_companions(pack: Pack) -> str:
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(_G702_COLUMNS)
    for r in pack.companions:
        w.writerow([r.line, r.amount, r.note])
    return buf.getvalue()


def csv_unset(pack: Pack) -> str:
    """Named unset lines. Silence on the form is the honesty, not a zero."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(("Unset line", "Reason"))
    by_name = {r.line: r.note for r in pack.g702}
    for name in pack.unset:
        w.writerow([name, by_name.get(name, "unset")])
    return buf.getvalue()


def as_files(pack: Pack) -> dict[str, str]:
    """Named companion sheets. Not a ZIP to an AIA portal."""
    return {
        "g702.csv": csv_g702(pack),
        "g703.csv": csv_g703(pack),
        "companions.csv": csv_companions(pack),
        "unset.csv": csv_unset(pack),
    }
