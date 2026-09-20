"""ratio — the Python SDK for the Ratio accounting kernel.

    import ratio

    client = ratio.Client("http://127.0.0.1:50051", book="fund-1")
    txn = client.transactions.create([
        ratio.Posting(dim=2, amount=-125_000),   # cash out
        ratio.Posting(dim=1, amount=125_000),    # investments up
    ])
    print(txn.name)                              # books/fund-1/transactions/txn-0
    assert client.trial_balance().difference == 0

An entry that does not conserve value raises :class:`RatioError` with
``status == "FAILED_PRECONDITION"`` and the store's own message; nothing
reaches the journal. That is the whole API.

Two transports serve the same seven calls: REST (the default; standard library
only) and gRPC (``transport="grpc"``; needs ``grpcio``). Both talk to one
``ratio server`` on one port.
"""

from .client import Accounts, Client, Transactions
from .errors import RatioError
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

__all__ = [
    "Account",
    "AccountBalance",
    "AccountPage",
    "AccountType",
    "Accounts",
    "Client",
    "Posting",
    "RatioError",
    "Side",
    "Transaction",
    "TransactionPage",
    "Transactions",
    "TrialBalance",
]
