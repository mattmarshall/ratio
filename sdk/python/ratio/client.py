"""``ratio.Client``: one book, over one transport."""

from __future__ import annotations

from typing import Iterator, Optional

from .transport import Transport
from .types import Account, AccountPage, AccountType, Posting, Transaction, TransactionPage, TrialBalance


class Transactions:
    """``client.transactions`` — the journal."""

    def __init__(self, transport: Transport, parent: str) -> None:
        self._t = transport
        self._parent = parent

    def create(
        self,
        postings: list[Posting],
        *,
        id: Optional[str] = None,
        control_plane_hash: str = "",
    ) -> Transaction:
        """Post a transaction. Refused with ``FAILED_PRECONDITION`` unless every
        conserved dimension nets to zero; ``ALREADY_EXISTS`` if ``id`` is taken;
        ``FAILED_PRECONDITION`` if ``control_plane_hash`` is not the active
        configuration."""
        return self._t.create_transaction(self._parent, list(postings), id or "", control_plane_hash)

    def get(self, id: str) -> Transaction:
        return self._t.get_transaction(f"{self._parent}/transactions/{id}")

    def list(self, page_size: int = 0, page_token: str = "") -> TransactionPage:
        return self._t.list_transactions(self._parent, page_size, page_token)

    def __iter__(self) -> Iterator[Transaction]:
        """Every transaction, in posting order, page by page."""
        token = ""
        while True:
            page = self.list(page_token=token)
            yield from page.transactions
            if not page.next_page_token:
                return
            token = page.next_page_token


class Accounts:
    """``client.accounts`` — the chart."""

    def __init__(self, transport: Transport, parent: str) -> None:
        self._t = transport
        self._parent = parent

    def create(self, dim: int, display_name: str, account_type: AccountType) -> Account:
        """Add an account. The id is the dimension; the normal side comes back
        derived from the type."""
        account = Account(dim=dim, display_name=display_name, account_type=account_type)
        return self._t.create_account(self._parent, account, "")

    def get(self, dim: int) -> Account:
        return self._t.get_account(f"{self._parent}/accounts/{dim}")

    def list(self, page_size: int = 0, page_token: str = "") -> AccountPage:
        return self._t.list_accounts(self._parent, page_size, page_token)

    def __iter__(self) -> Iterator[Account]:
        token = ""
        while True:
            page = self.list(page_token=token)
            yield from page.accounts
            if not page.next_page_token:
                return
            token = page.next_page_token


class Client:
    """A ``ratio server``, scoped to ``books/{book}``.

    ``transport`` is ``"rest"`` (default, standard library only) or ``"grpc"``
    (needs ``grpcio``). Both reach the same server on the same port.
    """

    def __init__(
        self,
        endpoint: str,
        book: str,
        *,
        transport: str = "rest",
        timeout: float = 30.0,
    ) -> None:
        if transport == "rest":
            from .rest import RestTransport

            self._transport: Transport = RestTransport(endpoint, timeout)
        elif transport == "grpc":
            from .grpc import GrpcTransport

            self._transport = GrpcTransport(endpoint, timeout)
        else:
            raise ValueError(f"transport must be 'rest' or 'grpc', got {transport!r}")
        self.book = book
        self.parent = f"books/{book}"
        self.transactions = Transactions(self._transport, self.parent)
        self.accounts = Accounts(self._transport, self.parent)

    def trial_balance(self) -> TrialBalance:
        """Totals per side and their difference — zero for any book the kernel
        admitted — with a row per (account, currency)."""
        return self._transport.get_trial_balance(f"{self.parent}/trialBalance")

    def close(self) -> None:
        self._transport.close()

    def __enter__(self) -> "Client":
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()
