#!/usr/bin/env python3
"""Bounded Plaid Transactions Sync adapter for the Personal bank feed.

Plaid is a source, never the rule chooser. A settled transaction becomes a
mapper row only when the caller supplies an explicit RuleChoice for its stable
Plaid transaction id. Pending transactions are returned as evidence and are
never posted. Modified or removed transactions refuse the cursor advance until
Ratio has an explicit correction/reversal policy.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from datetime import date
from decimal import Decimal, InvalidOperation
from typing import Any, Callable, Mapping, Sequence

import mapper

SYNC_PATH = "/transactions/sync"
REMOVE_PATH = "/item/remove"
DEFAULT_MAX_PAGES = 25


class Refuse(Exception):
    """The provider batch is not accepted and its cursor is not advanced."""


Transport = Callable[[str, Mapping[str, Any]], tuple[int, str]]


@dataclass(frozen=True)
class RuleChoice:
    kind: str
    source: str | None = None
    destination: str | None = None


@dataclass(frozen=True)
class PendingTransaction:
    transaction_id: str
    account_id: str
    dated: date
    amount: Decimal
    currency: str
    name: str


@dataclass(frozen=True)
class SyncResult:
    cursor: str
    proposed: tuple[mapper.ProposedPost, ...]
    pending: tuple[PendingTransaction, ...]
    source_ids: tuple[str, ...]


class PlaidClient:
    """Runtime-only credentials; repr deliberately reveals no secret values."""

    def __init__(
        self,
        *,
        client_id: str,
        secret: str,
        access_token: str,
        transport: Transport,
        max_pages: int = DEFAULT_MAX_PAGES,
    ) -> None:
        if not client_id or not secret or not access_token:
            raise Refuse("Plaid client_id, secret, and Item access token are required")
        if not isinstance(max_pages, int) or isinstance(max_pages, bool) or max_pages < 1:
            raise Refuse("max_pages must be a positive integer")
        self._client_id = client_id
        self._secret = secret
        self._access_token = access_token
        self._transport = transport
        self._max_pages = max_pages
        self._connected = True

    def __repr__(self) -> str:
        return "PlaidClient(credentials=<redacted>)"

    def sync(
        self,
        *,
        cursor: str = "",
        choices: Mapping[str, RuleChoice],
        book: mapper.Book,
        ratio_client: mapper.Client,
    ) -> SyncResult:
        if not self._connected:
            raise Refuse("this Plaid Item is disconnected")
        original_cursor = _cursor(cursor)
        next_cursor = original_cursor
        added: dict[str, Mapping[str, Any]] = {}
        pending: dict[str, PendingTransaction] = {}

        for page_number in range(1, self._max_pages + 1):
            payload = self._request(SYNC_PATH, {"cursor": next_cursor})
            has_more = payload.get("has_more")
            if not isinstance(has_more, bool):
                raise Refuse("Plaid response field 'has_more' is not boolean")
            modified = _records(payload, "modified")
            removed = _records(payload, "removed")
            if modified or removed:
                kinds = []
                if modified:
                    kinds.append(f"{len(modified)} modified")
                if removed:
                    kinds.append(f"{len(removed)} removed")
                raise Refuse(
                    "Plaid returned " + " and ".join(kinds)
                    + "; Ratio has no correction/reversal policy, so the cursor is unchanged"
                )
            for record in _records(payload, "added"):
                transaction_id = _identifier(record, "transaction_id")
                prior = added.get(transaction_id)
                if prior is not None and _canonical(prior) != _canonical(record):
                    raise Refuse(f"Plaid transaction {transaction_id!r} has conflicting content")
                added[transaction_id] = record

            next_cursor = _next_cursor(payload.get("next_cursor"))
            if not has_more:
                break
        else:
            raise Refuse(
                f"Plaid sync exceeded the {self._max_pages}-page bound; cursor is unchanged"
            )

        rows: list[Mapping[str, Any]] = []
        source_ids: list[str] = []
        for transaction_id, record in added.items():
            normalized = _transaction(record)
            if bool(record.get("pending")):
                pending[transaction_id] = normalized
                continue
            choice = choices.get(transaction_id)
            if choice is None:
                raise Refuse(
                    f"Plaid transaction {transaction_id!r} has no explicit user rule choice; "
                    "amount sign and provider category do not choose an accounting rule"
                )
            rows.append(_mapper_row(normalized, choice))
            source_ids.append(transaction_id)

        try:
            proposed = mapper.map_batch(rows, book=book, client=ratio_client)
        except mapper.Refuse as exc:
            raise Refuse(str(exc)) from exc
        return SyncResult(next_cursor, tuple(proposed), tuple(pending.values()), tuple(source_ids))

    def disconnect(self) -> None:
        if not self._connected:
            return
        self._request(REMOVE_PATH, {})
        self._connected = False

    def _request(self, path: str, body: Mapping[str, Any]) -> Mapping[str, Any]:
        # Provider inputs may choose endpoint parameters, never credentials.
        # Place the server-held values last so an internal caller cannot turn
        # this boundary into a confused-deputy request for another Item.
        request = {
            **body,
            "client_id": self._client_id,
            "secret": self._secret,
            "access_token": self._access_token,
        }
        try:
            status, raw = self._transport(path, request)
        except Exception as exc:
            raise Refuse(f"Plaid {path} transport failed: {type(exc).__name__}") from exc
        try:
            payload = json.loads(raw, parse_float=Decimal)
        except (json.JSONDecodeError, TypeError) as exc:
            raise Refuse(f"Plaid {path} returned malformed JSON") from exc
        if not isinstance(payload, dict):
            raise Refuse(f"Plaid {path} returned a non-object response")
        if not isinstance(status, int) or isinstance(status, bool):
            raise Refuse(f"Plaid {path} transport returned a malformed HTTP status")
        if status >= 400:
            code = str(payload.get("error_code") or "HTTP_ERROR")
            if code == "TRANSACTIONS_SYNC_MUTATION_DURING_PAGINATION":
                raise Refuse(
                    "Plaid mutated the transaction set during pagination; retry from the original cursor"
                )
            raise Refuse(f"Plaid {path} returned HTTP {status} ({code})")
        return payload


def _records(payload: Mapping[str, Any], name: str) -> Sequence[Mapping[str, Any]]:
    value = payload.get(name)
    if not isinstance(value, list) or not all(isinstance(row, dict) for row in value):
        raise Refuse(f"Plaid response field {name!r} is not a record list")
    return value


def _identifier(record: Mapping[str, Any], name: str) -> str:
    value = record.get(name)
    if not isinstance(value, str) or not value.strip() or len(value) > 256:
        raise Refuse(f"Plaid record has no stable {name}")
    return value.strip()


def _cursor(value: object) -> str:
    if value is None:
        return ""
    if not isinstance(value, str) or len(value) > 4096:
        raise Refuse("Plaid cursor is malformed")
    return value


def _next_cursor(value: object) -> str:
    cursor = _cursor(value)
    if not cursor:
        raise Refuse("Plaid response has no next_cursor; advancing would rewind the sync")
    return cursor


def _currency(record: Mapping[str, Any]) -> str:
    raw = record.get("iso_currency_code")
    if raw is None:
        raise Refuse("Plaid transaction has no ISO currency; unofficial currency is unsupported")
    code = str(raw).strip().upper()
    if len(code) != 3 or not code.isascii() or not code.isalpha():
        raise Refuse(f"Plaid currency {code!r} is not a three-letter ISO code")
    return code


def _amount(value: object) -> Decimal:
    if isinstance(value, float):
        raise Refuse("Plaid money reached the adapter as a binary float")
    try:
        amount = value if isinstance(value, Decimal) else Decimal(str(value))
    except (InvalidOperation, ValueError) as exc:
        raise Refuse("Plaid transaction amount is not decimal money") from exc
    if not amount.is_finite() or amount == 0:
        raise Refuse("Plaid transaction amount must be finite and nonzero")
    if amount.as_tuple().exponent < -2:
        raise Refuse("Plaid transaction amount has more than two decimal places")
    return amount


def _transaction(record: Mapping[str, Any]) -> PendingTransaction:
    transaction_id = _identifier(record, "transaction_id")
    account_id = _identifier(record, "account_id")
    raw_day = record.get("date")
    if not isinstance(raw_day, str):
        raise Refuse(f"Plaid transaction {transaction_id!r} has no posted date")
    try:
        posted = date.fromisoformat(raw_day)
    except ValueError as exc:
        raise Refuse(f"Plaid transaction {transaction_id!r} has an invalid posted date") from exc
    name = record.get("name")
    if not isinstance(name, str) or not name.strip():
        raise Refuse(f"Plaid transaction {transaction_id!r} has no description")
    is_pending = record.get("pending")
    if not isinstance(is_pending, bool):
        raise Refuse(f"Plaid transaction {transaction_id!r} has no boolean pending state")
    return PendingTransaction(
        transaction_id=transaction_id,
        account_id=account_id,
        dated=posted,
        amount=_amount(record.get("amount")),
        currency=_currency(record),
        name=name.strip(),
    )


def _mapper_row(transaction: PendingTransaction, choice: RuleChoice) -> Mapping[str, Any]:
    row: dict[str, Any] = {
        "dated": transaction.dated.isoformat(),
        "amount": format(abs(transaction.amount), "f"),
        "currency": transaction.currency,
        "kind": choice.kind,
        # Mapper ids are bounded to 64 safe characters. The digest is stable,
        # while SyncResult retains the actual provider id for review/citation.
        "reference": "plaid-" + hashlib.sha256(transaction.transaction_id.encode()).hexdigest()[:48],
    }
    if choice.source is not None:
        row["from"] = choice.source
    if choice.destination is not None:
        row["to"] = choice.destination
    return row


def _canonical(record: Mapping[str, Any]) -> str:
    return json.dumps(record, sort_keys=True, separators=(",", ":"), default=str)
