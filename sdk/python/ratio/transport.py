"""What a transport does: the seven calls, on typed messages.

Both transports implement this, so :class:`ratio.Client` does not know which
one it holds — and neither adds a rule the server does not have.
"""

from __future__ import annotations

from typing import Optional, Protocol

from .types import Account, AccountPage, Posting, Transaction, TransactionPage, TrialBalance


class Transport(Protocol):
    def get_transaction(self, name: str) -> Transaction: ...

    def list_transactions(self, parent: str, page_size: int, page_token: str) -> TransactionPage: ...

    def create_transaction(
        self, parent: str, postings: list[Posting], transaction_id: str, control_plane_hash: str
    ) -> Transaction: ...

    def get_account(self, name: str) -> Account: ...

    def list_accounts(self, parent: str, page_size: int, page_token: str) -> AccountPage: ...

    def create_account(self, parent: str, account: Account, account_id: str) -> Account: ...

    def get_trial_balance(self, name: str) -> TrialBalance: ...

    def close(self) -> None: ...
