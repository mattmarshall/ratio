"""The gRPC transport: the same seven calls over ``grpcio``, with the wire
format from :mod:`ratio._wire` — so ``pip install 'ratio-sdk[grpc]'`` is the
whole setup, and there is no stub to regenerate when the contract moves."""

from __future__ import annotations

from typing import Callable

from . import _wire
from .errors import RatioError
from .types import Account, AccountPage, Posting, Transaction, TransactionPage, TrialBalance


class GrpcTransport:
    def __init__(self, endpoint: str, timeout: float = 30.0) -> None:
        try:
            import grpc  # noqa: F401 — optional; REST needs nothing
        except ImportError:
            raise ImportError(
                "the gRPC transport needs grpcio — pip install 'ratio-sdk[grpc]' — "
                "or use transport='rest', which needs nothing"
            ) from None
        self._grpc = grpc
        self._timeout = timeout
        if endpoint.startswith("https://"):
            self._channel = grpc.secure_channel(endpoint[len("https://"):], grpc.ssl_channel_credentials())
        else:
            target = endpoint[len("http://"):] if endpoint.startswith("http://") else endpoint
            self._channel = grpc.insecure_channel(target)

    def _unary(self, service: str, method: str, request: bytes, decode: Callable[[bytes], object]) -> object:
        # No serializers: bytes in, bytes out. The wire module is the codec.
        call = self._channel.unary_unary(f"/ratio.v1.{service}/{method}")
        try:
            return decode(call(request, timeout=self._timeout))
        except self._grpc.RpcError as e:
            raise RatioError(e.code().value[0], e.details() or "") from None

    # ── the seven calls ──────────────────────────────────────────────────

    def get_transaction(self, name: str) -> Transaction:
        return self._unary("Ledger", "GetTransaction", _wire.get_request(name), _wire.decode_transaction)  # type: ignore[return-value]

    def list_transactions(self, parent: str, page_size: int, page_token: str) -> TransactionPage:
        return self._unary(
            "Ledger", "ListTransactions", _wire.list_request(parent, page_size, page_token), _wire.decode_transaction_page
        )  # type: ignore[return-value]

    def create_transaction(
        self, parent: str, postings: list[Posting], transaction_id: str, control_plane_hash: str
    ) -> Transaction:
        t = Transaction(postings=tuple(postings), control_plane_hash=control_plane_hash)
        return self._unary(
            "Ledger", "CreateTransaction", _wire.create_transaction_request(parent, t, transaction_id), _wire.decode_transaction
        )  # type: ignore[return-value]

    def get_account(self, name: str) -> Account:
        return self._unary("Chart", "GetAccount", _wire.get_request(name), _wire.decode_account)  # type: ignore[return-value]

    def list_accounts(self, parent: str, page_size: int, page_token: str) -> AccountPage:
        return self._unary(
            "Chart", "ListAccounts", _wire.list_request(parent, page_size, page_token), _wire.decode_account_page
        )  # type: ignore[return-value]

    def create_account(self, parent: str, account: Account, account_id: str) -> Account:
        return self._unary(
            "Chart", "CreateAccount", _wire.create_account_request(parent, account, account_id), _wire.decode_account
        )  # type: ignore[return-value]

    def get_trial_balance(self, name: str) -> TrialBalance:
        return self._unary("Chart", "GetTrialBalance", _wire.get_request(name), _wire.decode_trial_balance)  # type: ignore[return-value]

    def close(self) -> None:
        self._channel.close()
