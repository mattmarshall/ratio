#!/usr/bin/env python3
"""ECB reference rates -> Ratio's Personal fact plane.

The provider reports how many units of a currency equal one euro. Ratio needs
hundredths of the declared reporting base per unit of each foreign currency.
Cross rates are computed with Decimal and quantized once with ROUND_HALF_EVEN;
the normalized delivery retains the raw ECB observations it cites.

This app ingests and admits reference facts through ConnectApiUrl. It has no
journal scope and never calls ApplyEvent.
"""

from __future__ import annotations

import csv
import io
import json
from dataclasses import dataclass
from datetime import date
from decimal import Decimal, InvalidOperation, ROUND_HALF_EVEN, localcontext
from typing import Callable, Iterable, Mapping
from urllib.parse import urlencode
from urllib.request import Request, urlopen

import grant

ECB_API = "https://data-api.ecb.europa.eu/service/data/EXR"
SCOPES = frozenset({"books:read", "books:ingest", "facts:admit"})
TEMPLATE_ID = "ecb-reference-rates"


class Refuse(Exception):
    """No facts are delivered. The message names the unsupported claim."""


ProviderTransport = Callable[[str], tuple[int, str]]


@dataclass(frozen=True)
class Observation:
    currency: str
    on_day: date
    per_eur: Decimal
    raw: str


def iso_currency(raw: object) -> str:
    code = str(raw or "").strip().upper()
    if len(code) != 3 or not code.isascii() or not code.isalpha():
        raise Refuse(f"{code!r} is not a three-letter ISO currency code")
    return code


def calendar_day(raw: object) -> date:
    try:
        return date.fromisoformat(str(raw or "").strip())
    except ValueError as exc:
        raise Refuse(f"{raw!r} is not a calendar day YYYY-MM-DD") from exc


def series_url(currency: str, on_day: date) -> str:
    code = iso_currency(currency)
    query = urlencode(
        {
            "startPeriod": on_day.isoformat(),
            "endPeriod": on_day.isoformat(),
            "format": "csvdata",
        }
    )
    return f"{ECB_API}/D.{code}.EUR.SP00.A?{query}"


def parse_observation(content: str, currency: str, on_day: date) -> Observation:
    code = iso_currency(currency)
    try:
        rows = list(csv.DictReader(io.StringIO(content)))
    except csv.Error as exc:
        raise Refuse("ECB returned malformed CSV") from exc
    matching = [
        row
        for row in rows
        if str(row.get("TIME_PERIOD") or "").strip() == on_day.isoformat()
        and str(row.get("CURRENCY") or "").strip().upper() == code
        and str(row.get("CURRENCY_DENOM") or "").strip().upper() == "EUR"
    ]
    if len(matching) != 1:
        raise Refuse(
            f"ECB returned {len(matching)} {code}/EUR observations for "
            f"{on_day.isoformat()}; exactly one is required"
        )
    raw = str(matching[0].get("OBS_VALUE") or "").strip()
    try:
        value = Decimal(raw)
    except InvalidOperation as exc:
        raise Refuse(f"ECB returned a non-decimal {code} observation {raw!r}") from exc
    if not value.is_finite() or value <= 0:
        raise Refuse(f"ECB returned a nonpositive {code} observation {raw!r}")
    return Observation(code, on_day, value, raw)


def fetch_observation(
    currency: str,
    on_day: date,
    *,
    transport: ProviderTransport | None = None,
) -> Observation:
    code = iso_currency(currency)
    if code == "EUR":
        return Observation("EUR", on_day, Decimal("1"), "1")
    status, body = (transport or _provider_transport)(series_url(code, on_day))
    if status >= 400:
        raise Refuse(f"ECB {code}/EUR request returned HTTP {status}")
    return parse_observation(body, code, on_day)


def normalized_delivery(
    *,
    base: str,
    currencies: Iterable[str],
    on_day: date,
    observations: Mapping[str, Observation],
    today: date | None = None,
) -> str:
    if on_day > (today or date.today()):
        raise Refuse("a future ECB observation cannot support today's figure")
    target = iso_currency(base)
    declared = tuple(dict.fromkeys(iso_currency(c) for c in currencies))
    if not declared:
        raise Refuse("the Personal book declares no currencies")
    if target not in declared:
        raise Refuse(f"reporting base {target} is not a declared currency")
    needed = set(declared) | {target}
    missing = sorted(code for code in needed if code != "EUR" and code not in observations)
    if missing:
        raise Refuse("missing ECB observations for " + ", ".join(missing))
    obs = dict(observations)
    obs.setdefault("EUR", Observation("EUR", on_day, Decimal("1"), "1"))
    for code in needed:
        if obs[code].on_day != on_day:
            raise Refuse(
                f"{code} observation is dated {obs[code].on_day.isoformat()}, "
                f"not requested day {on_day.isoformat()}"
            )

    out = io.StringIO(newline="")
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow(["Reference", "AsOf", "Currency", "Rate", "Base", "SourceRate", "SourceBase"])
    with localcontext() as ctx:
        ctx.prec = 40
        for code in declared:
            if code == target:
                continue
            cross = obs[target].per_eur / obs[code].per_eur
            rounded = cross.quantize(Decimal("0.01"), rounding=ROUND_HALF_EVEN)
            if rounded <= 0:
                raise Refuse(f"{code}/{target} rounds to a nonpositive hundredths factor")
            writer.writerow(
                [
                    f"ECB-EXR-{on_day.isoformat()}-{code}-{target}",
                    on_day.isoformat(),
                    code,
                    format(rounded, ".2f"),
                    target,
                    obs[code].raw,
                    obs[target].raw,
                ]
            )
    return out.getvalue()


def build_delivery(
    *,
    base: str,
    currencies: Iterable[str],
    on_day: date,
    transport: ProviderTransport | None = None,
    today: date | None = None,
) -> str:
    declared = tuple(dict.fromkeys(iso_currency(c) for c in currencies))
    needed = set(declared) | {iso_currency(base)}
    observations = {
        code: fetch_observation(code, on_day, transport=transport) for code in sorted(needed)
    }
    return normalized_delivery(
        base=base,
        currencies=declared,
        on_day=on_day,
        observations=observations,
        today=today,
    )


def sync(
    book_id: str,
    on_day: date,
    *,
    token: str | None = None,
    provider_transport: ProviderTransport | None = None,
    ratio_transport: grant.Transport | None = None,
    today: date | None = None,
) -> tuple[object, object]:
    book = grant.pull(
        token=token,
        book_id=book_id,
        transport=ratio_transport,
        error=Refuse,
    )
    kind = str(book.get("kind") or "").upper()
    if kind not in {"PERSONAL", "KIND_PERSONAL"}:
        raise Refuse(f"ECB rates are for a Personal book; got {kind or 'unspecified'}")
    base = str(book.get("currencyCode") or book.get("currency_code") or "").strip()
    currencies = book.get("currencies") or []
    content = build_delivery(
        base=base,
        currencies=currencies,
        on_day=on_day,
        transport=provider_transport,
        today=today,
    )
    parent = f"funds/{book_id.strip().strip('/').removeprefix('books/').removeprefix('funds/')}"
    ingest = grant.push(
        token=token,
        path=f"/v1/{parent}:ingest",
        body={
            "parent": parent,
            "templateId": TEMPLATE_ID,
            "content": content,
            "origin": f"ecb:EXR:{on_day.isoformat()}",
            "validateOnly": False,
        },
        transport=ratio_transport,
        error=Refuse,
    )
    admit = grant.push(
        token=token,
        path=f"/v1/{parent}:admit",
        body={"parent": parent, "validateOnly": False},
        transport=ratio_transport,
        error=Refuse,
    )
    return ingest, admit


def _provider_transport(url: str) -> tuple[int, str]:
    req = Request(url, headers={"accept": "text/csv", "user-agent": "ratio-ecb-rates/1"})
    try:
        with urlopen(req, timeout=30) as response:
            return response.status, response.read().decode("utf-8-sig")
    except Exception as exc:
        raise Refuse(f"ECB request failed: {exc}") from exc


if __name__ == "__main__":
    raise SystemExit("import ecb_rates and call sync; no credential-bearing CLI is provided")
