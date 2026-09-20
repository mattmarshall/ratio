"""The ``ratio.v1`` messages as dataclasses, and their proto3 JSON.

Every ``int64`` is a Python ``int`` here and a decimal *string* on the wire —
the canonical proto3 JSON mapping, because an amount is exact or it is wrong
and not every JSON parser keeps integers past 2^53. Enums are their names.
Absent optionals are ``None`` and are omitted from the JSON.

The JSON keys below are checked against ``ledger.proto`` and ``chart.proto`` by
``//proto:sdk_mirrors_test``: a key that the contract does not declare, or a
field the contract declares that this file does not carry, fails the build.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Any, Optional


class AccountType(str, Enum):
    UNSPECIFIED = "ACCOUNT_TYPE_UNSPECIFIED"
    ASSET = "ACCOUNT_TYPE_ASSET"
    LIABILITY = "ACCOUNT_TYPE_LIABILITY"
    EQUITY = "ACCOUNT_TYPE_EQUITY"
    INCOME = "ACCOUNT_TYPE_INCOME"
    EXPENSE = "ACCOUNT_TYPE_EXPENSE"


class Side(str, Enum):
    UNSPECIFIED = "SIDE_UNSPECIFIED"
    DEBIT = "SIDE_DEBIT"
    CREDIT = "SIDE_CREDIT"


# ── JSON helpers ────────────────────────────────────────────────────────────


def _int64(d: dict, key: str) -> int:
    v = d.get(key)
    if v is None:
        return 0
    return _to_int64(v, key)


def _opt_int64(d: dict, key: str) -> Optional[int]:
    v = d.get(key)
    return None if v is None else _to_int64(v, key)


def _to_int64(v: Any, key: str) -> int:
    # bool is an int in Python; it is not an int64 on the wire.
    if isinstance(v, bool) or not isinstance(v, (int, str)):
        raise ValueError(f"{key}: expected an integer, got {v!r}")
    try:
        n = int(v) if isinstance(v, int) else int(str(v).strip(), 10)
    except ValueError:
        raise ValueError(f"{key}: {v!r} is not a 64-bit integer") from None
    if not -(2**63) <= n < 2**63:
        raise ValueError(f"{key}: {n} does not fit in 64 bits")
    return n


def _str(d: dict, key: str) -> str:
    v = d.get(key)
    if v is None:
        return ""
    if not isinstance(v, str):
        raise ValueError(f"{key}: expected a string, got {v!r}")
    return v


def _opt_str(d: dict, key: str) -> Optional[str]:
    v = d.get(key)
    if v is None:
        return None
    if not isinstance(v, str):
        raise ValueError(f"{key}: expected a string, got {v!r}")
    return v


def _enum(d: dict, key: str, cls: type) -> Any:
    v = d.get(key)
    if v is None:
        return list(cls)[0]
    if isinstance(v, str):
        try:
            return cls(v)
        except ValueError:
            raise ValueError(f"{key}: {v!r} is not a known value") from None
    if isinstance(v, int) and not isinstance(v, bool):
        members = list(cls)
        if 0 <= v < len(members):
            return members[v]
    raise ValueError(f"{key}: {v!r} is not a known value")


def _put(o: dict, key: str, v: Any) -> None:
    """Write an int64 as a decimal string, a string if non-empty, an enum as
    its name; skip None and empty strings, as canonical JSON does."""
    if v is None:
        return
    if isinstance(v, bool):
        o[key] = v
    elif isinstance(v, int):
        o[key] = str(v)
    elif isinstance(v, Enum):
        o[key] = v.value
    elif isinstance(v, str):
        if v:
            o[key] = v
    else:
        o[key] = v


def _obj(v: Any) -> dict:
    if not isinstance(v, dict):
        raise ValueError(f"expected a JSON object, got {v!r}")
    return v


# ── ratio.v1.Ledger ─────────────────────────────────────────────────────────


@dataclass(frozen=True)
class Posting:
    """One component of a transaction's integer vector.

    ``dim`` is the conserved dimension (the account); ``amount`` is exact minor
    units. ``currency_code`` names the conservation law the amount is under — two
    currencies are two laws, not one law over a sum; ``None`` is the book's
    untyped group. ``instrument`` partitions further; ``quantity`` is measured,
    not conserved.
    """

    dim: int
    amount: int
    currency_code: Optional[str] = None
    instrument: Optional[str] = None
    quantity: Optional[int] = None

    def to_json(self) -> dict:
        o: dict = {}
        _put(o, "dim", self.dim)
        _put(o, "amount", self.amount)
        _put(o, "currencyCode", self.currency_code)
        _put(o, "instrument", self.instrument)
        _put(o, "quantity", self.quantity)
        return o

    @classmethod
    def from_json(cls, v: Any) -> "Posting":
        d = _obj(v)
        return cls(
            dim=_int64(d, "dim"),
            amount=_int64(d, "amount"),
            currency_code=_opt_str(d, "currencyCode"),
            instrument=_opt_str(d, "instrument"),
            quantity=_opt_int64(d, "quantity"),
        )


@dataclass(frozen=True)
class Transaction:
    """A balanced transaction, as the journal holds it."""

    name: str = ""
    postings: tuple[Posting, ...] = ()
    control_plane_hash: str = ""

    @property
    def id(self) -> str:
        """The segment after ``transactions/``."""
        return self.name.rsplit("/", 1)[-1]

    def to_json(self) -> dict:
        o: dict = {}
        _put(o, "name", self.name)
        o["postings"] = [p.to_json() for p in self.postings]
        _put(o, "controlPlaneHash", self.control_plane_hash)
        return o

    @classmethod
    def from_json(cls, v: Any) -> "Transaction":
        d = _obj(v)
        return cls(
            name=_str(d, "name"),
            postings=tuple(Posting.from_json(p) for p in d.get("postings") or []),
            control_plane_hash=_str(d, "controlPlaneHash"),
        )


@dataclass(frozen=True)
class TransactionPage:
    transactions: tuple[Transaction, ...] = ()
    next_page_token: str = ""

    @classmethod
    def from_json(cls, v: Any) -> "TransactionPage":
        d = _obj(v)
        return cls(
            transactions=tuple(Transaction.from_json(t) for t in d.get("transactions") or []),
            next_page_token=_str(d, "nextPageToken"),
        )


# ── ratio.v1.Chart ──────────────────────────────────────────────────────────


@dataclass(frozen=True)
class Account:
    """A named conserved dimension. ``normal_side`` is derived by the server
    from ``account_type`` — a proved function, never an input."""

    name: str = ""
    dim: int = 0
    display_name: str = ""
    account_type: AccountType = AccountType.UNSPECIFIED
    normal_side: Side = Side.UNSPECIFIED

    def to_json(self) -> dict:
        o: dict = {}
        _put(o, "name", self.name)
        _put(o, "dim", self.dim)
        _put(o, "displayName", self.display_name)
        _put(o, "accountType", self.account_type)
        _put(o, "normalSide", self.normal_side)
        return o

    @classmethod
    def from_json(cls, v: Any) -> "Account":
        d = _obj(v)
        return cls(
            name=_str(d, "name"),
            dim=_int64(d, "dim"),
            display_name=_str(d, "displayName"),
            account_type=_enum(d, "accountType", AccountType),
            normal_side=_enum(d, "normalSide", Side),
        )


@dataclass(frozen=True)
class AccountPage:
    accounts: tuple[Account, ...] = ()
    next_page_token: str = ""

    @classmethod
    def from_json(cls, v: Any) -> "AccountPage":
        d = _obj(v)
        return cls(
            accounts=tuple(Account.from_json(a) for a in d.get("accounts") or []),
            next_page_token=_str(d, "nextPageToken"),
        )


@dataclass(frozen=True)
class AccountBalance:
    """One (account, currency) row of a trial balance."""

    account: str = ""
    debits: int = 0
    credits: int = 0
    currency_code: Optional[str] = None

    def to_json(self) -> dict:
        o: dict = {}
        _put(o, "account", self.account)
        _put(o, "debits", self.debits)
        _put(o, "credits", self.credits)
        _put(o, "currencyCode", self.currency_code)
        return o

    @classmethod
    def from_json(cls, v: Any) -> "AccountBalance":
        d = _obj(v)
        return cls(
            account=_str(d, "account"),
            debits=_int64(d, "debits"),
            credits=_int64(d, "credits"),
            currency_code=_opt_str(d, "currencyCode"),
        )


@dataclass(frozen=True)
class TrialBalance:
    """Totals per side and their difference — zero for any book the kernel
    admitted — with a row per (account, currency)."""

    name: str = ""
    debits: int = 0
    credits: int = 0
    difference: int = 0
    account_balances: tuple[AccountBalance, ...] = ()
    control_plane_hash: str = ""

    def to_json(self) -> dict:
        o: dict = {}
        _put(o, "name", self.name)
        _put(o, "debits", self.debits)
        _put(o, "credits", self.credits)
        _put(o, "difference", self.difference)
        o["accountBalances"] = [b.to_json() for b in self.account_balances]
        _put(o, "controlPlaneHash", self.control_plane_hash)
        return o

    @classmethod
    def from_json(cls, v: Any) -> "TrialBalance":
        d = _obj(v)
        return cls(
            name=_str(d, "name"),
            debits=_int64(d, "debits"),
            credits=_int64(d, "credits"),
            difference=_int64(d, "difference"),
            account_balances=tuple(AccountBalance.from_json(b) for b in d.get("accountBalances") or []),
            control_plane_hash=_str(d, "controlPlaneHash"),
        )
