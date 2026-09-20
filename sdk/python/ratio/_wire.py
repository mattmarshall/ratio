"""The protobuf wire format for the ``ratio.v1`` messages, by hand.

Seven RPCs over five messages need two wire types: varints (every int64,
int32 and enum) and length-delimited fields (every string and message). That
is under a hundred lines, and it means the gRPC transport needs ``grpcio`` and
nothing else — no generated ``_pb2`` modules, no ``protobuf`` runtime, no
build step between ``pip install`` and a call.

Field numbers are the contract's (``proto/ratio/v1/ledger.proto``,
``chart.proto``). A message this file does not know a field of is decoded by
skipping it, so a newer server does not break an older client.
"""

from __future__ import annotations

from typing import Callable, Iterator, Optional

from .types import (
    Account,
    AccountBalance,
    AccountPage,
    AccountType,
    Posting,
    Side,
    Transaction,
    TransactionPage,
    TrialBalance,
)

_MASK64 = (1 << 64) - 1


# ── encoding ────────────────────────────────────────────────────────────────


def _varint(n: int) -> bytes:
    out = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        if n:
            out.append(b | 0x80)
        else:
            out.append(b)
            return bytes(out)


def _int_field(num: int, v: Optional[int]) -> bytes:
    """int64 / int32 / enum: a varint of the two's-complement 64-bit value.
    Zero is the proto3 default and is not written; ``None`` is absent."""
    if not v:
        return b""
    return _varint(num << 3) + _varint(v & _MASK64)


def _present_int_field(num: int, v: Optional[int]) -> bytes:
    """A proto3 ``optional`` int64: written when set, zero included."""
    if v is None:
        return b""
    return _varint(num << 3) + _varint(v & _MASK64)


def _len_field(num: int, payload: bytes) -> bytes:
    return _varint((num << 3) | 2) + _varint(len(payload)) + payload


def _str_field(num: int, s: Optional[str]) -> bytes:
    if not s:
        return b""
    return _len_field(num, s.encode("utf-8"))


def posting(p: Posting) -> bytes:
    return (
        _int_field(1, p.dim)
        + _int_field(2, p.amount)
        + _str_field(3, p.currency_code)
        + _str_field(4, p.instrument)
        + _present_int_field(5, p.quantity)
    )


def transaction(t: Transaction) -> bytes:
    return (
        _str_field(1, t.name)
        + b"".join(_len_field(2, posting(p)) for p in t.postings)
        + _str_field(3, t.control_plane_hash)
    )


def get_request(name: str) -> bytes:
    """GetTransactionRequest, GetAccountRequest, GetTrialBalanceRequest: ``name = 1``."""
    return _str_field(1, name)


def list_request(parent: str, page_size: int, page_token: str) -> bytes:
    """ListTransactionsRequest / ListAccountsRequest: parent 1, page_size 2, page_token 3."""
    return _str_field(1, parent) + _int_field(2, page_size) + _str_field(3, page_token)


def create_transaction_request(parent: str, t: Transaction, transaction_id: str) -> bytes:
    return _str_field(1, parent) + _len_field(2, transaction(t)) + _str_field(3, transaction_id)


def account(a: Account) -> bytes:
    return (
        _str_field(1, a.name)
        + _int_field(2, a.dim)
        + _str_field(3, a.display_name)
        + _int_field(4, list(AccountType).index(a.account_type))
        + _int_field(5, list(Side).index(a.normal_side))
    )


def create_account_request(parent: str, a: Account, account_id: str) -> bytes:
    return _str_field(1, parent) + _len_field(2, account(a)) + _str_field(3, account_id)


# ── decoding ────────────────────────────────────────────────────────────────


def _read_varint(b: bytes, i: int) -> tuple[int, int]:
    shift, n = 0, 0
    while True:
        if i >= len(b):
            raise ValueError("truncated varint")
        c = b[i]
        i += 1
        n |= (c & 0x7F) << shift
        if not c & 0x80:
            return n, i
        shift += 7
        if shift > 70:
            raise ValueError("varint too long")


def fields(b: bytes) -> Iterator[tuple[int, int, object]]:
    """Yield ``(field number, wire type, value)`` — an int for varints, bytes
    for length-delimited fields; fixed-width fields are skipped."""
    i = 0
    while i < len(b):
        tag, i = _read_varint(b, i)
        num, wt = tag >> 3, tag & 7
        if wt == 0:
            v, i = _read_varint(b, i)
            yield num, wt, v
        elif wt == 2:
            n, i = _read_varint(b, i)
            if i + n > len(b):
                raise ValueError("truncated field")
            yield num, wt, b[i : i + n]
            i += n
        elif wt == 1:
            i += 8
        elif wt == 5:
            i += 4
        else:
            raise ValueError(f"unsupported wire type {wt}")


def _i64(v: object) -> int:
    n = int(v)  # type: ignore[arg-type]
    return n - (1 << 64) if n >= (1 << 63) else n


def _s(v: object) -> str:
    return bytes(v).decode("utf-8")  # type: ignore[arg-type]


def _enum(v: object, cls: type) -> object:
    members = list(cls)
    n = int(v)  # type: ignore[arg-type]
    return members[n] if 0 <= n < len(members) else members[0]


def decode_posting(b: bytes) -> Posting:
    dim = amount = 0
    currency = instrument = None
    quantity = None
    for num, wt, v in fields(b):
        if num == 1 and wt == 0:
            dim = _i64(v)
        elif num == 2 and wt == 0:
            amount = _i64(v)
        elif num == 3 and wt == 2:
            currency = _s(v)
        elif num == 4 and wt == 2:
            instrument = _s(v)
        elif num == 5 and wt == 0:
            quantity = _i64(v)
    return Posting(dim, amount, currency, instrument, quantity)


def decode_transaction(b: bytes) -> Transaction:
    name = hash_ = ""
    postings: list[Posting] = []
    for num, wt, v in fields(b):
        if num == 1 and wt == 2:
            name = _s(v)
        elif num == 2 and wt == 2:
            postings.append(decode_posting(bytes(v)))  # type: ignore[arg-type]
        elif num == 3 and wt == 2:
            hash_ = _s(v)
    return Transaction(name, tuple(postings), hash_)


def decode_transaction_page(b: bytes) -> TransactionPage:
    items: list[Transaction] = []
    token = ""
    for num, wt, v in fields(b):
        if num == 1 and wt == 2:
            items.append(decode_transaction(bytes(v)))  # type: ignore[arg-type]
        elif num == 2 and wt == 2:
            token = _s(v)
    return TransactionPage(tuple(items), token)


def decode_account(b: bytes) -> Account:
    name = display = ""
    dim = 0
    t: object = AccountType.UNSPECIFIED
    side: object = Side.UNSPECIFIED
    for num, wt, v in fields(b):
        if num == 1 and wt == 2:
            name = _s(v)
        elif num == 2 and wt == 0:
            dim = _i64(v)
        elif num == 3 and wt == 2:
            display = _s(v)
        elif num == 4 and wt == 0:
            t = _enum(v, AccountType)
        elif num == 5 and wt == 0:
            side = _enum(v, Side)
    return Account(name, dim, display, t, side)  # type: ignore[arg-type]


def decode_account_page(b: bytes) -> AccountPage:
    items: list[Account] = []
    token = ""
    for num, wt, v in fields(b):
        if num == 1 and wt == 2:
            items.append(decode_account(bytes(v)))  # type: ignore[arg-type]
        elif num == 2 and wt == 2:
            token = _s(v)
    return AccountPage(tuple(items), token)


def decode_account_balance(b: bytes) -> AccountBalance:
    acct = ""
    debits = credits = 0
    currency = None
    for num, wt, v in fields(b):
        if num == 1 and wt == 2:
            acct = _s(v)
        elif num == 2 and wt == 0:
            debits = _i64(v)
        elif num == 3 and wt == 0:
            credits = _i64(v)
        elif num == 4 and wt == 2:
            currency = _s(v)
    return AccountBalance(acct, debits, credits, currency)


def decode_trial_balance(b: bytes) -> TrialBalance:
    name = hash_ = ""
    debits = credits = difference = 0
    rows: list[AccountBalance] = []
    for num, wt, v in fields(b):
        if num == 1 and wt == 2:
            name = _s(v)
        elif num == 2 and wt == 0:
            debits = _i64(v)
        elif num == 3 and wt == 0:
            credits = _i64(v)
        elif num == 4 and wt == 0:
            difference = _i64(v)
        elif num == 5 and wt == 2:
            rows.append(decode_account_balance(bytes(v)))  # type: ignore[arg-type]
        elif num == 6 and wt == 2:
            hash_ = _s(v)
    return TrialBalance(name, debits, credits, difference, tuple(rows), hash_)


Decoder = Callable[[bytes], object]
