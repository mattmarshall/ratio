"""The REST transport: the contract's ``google.api.http`` bindings, over the
standard library — no dependency, like every first-party Connect app."""

from __future__ import annotations

import json
import urllib.error
import urllib.parse
import urllib.request
from typing import Any, Optional

from .errors import RatioError
from .types import Account, AccountPage, Posting, Transaction, TransactionPage, TrialBalance


class RestTransport:
    def __init__(self, endpoint: str, timeout: float = 30.0) -> None:
        self.endpoint = endpoint.rstrip("/")
        self.timeout = timeout

    # ── the seven calls ──────────────────────────────────────────────────

    def get_transaction(self, name: str) -> Transaction:
        return Transaction.from_json(self._call("GET", f"/v1/{name}"))

    def list_transactions(self, parent: str, page_size: int, page_token: str) -> TransactionPage:
        return TransactionPage.from_json(
            self._call("GET", f"/v1/{parent}/transactions", query=_page(page_size, page_token))
        )

    def create_transaction(
        self, parent: str, postings: list[Posting], transaction_id: str, control_plane_hash: str
    ) -> Transaction:
        body = Transaction(postings=tuple(postings), control_plane_hash=control_plane_hash).to_json()
        query = {"transactionId": transaction_id} if transaction_id else {}
        return Transaction.from_json(self._call("POST", f"/v1/{parent}/transactions", query=query, body=body))

    def get_account(self, name: str) -> Account:
        return Account.from_json(self._call("GET", f"/v1/{name}"))

    def list_accounts(self, parent: str, page_size: int, page_token: str) -> AccountPage:
        return AccountPage.from_json(self._call("GET", f"/v1/{parent}/accounts", query=_page(page_size, page_token)))

    def create_account(self, parent: str, account: Account, account_id: str) -> Account:
        query = {"accountId": account_id} if account_id else {}
        return Account.from_json(self._call("POST", f"/v1/{parent}/accounts", query=query, body=account.to_json()))

    def get_trial_balance(self, name: str) -> TrialBalance:
        return TrialBalance.from_json(self._call("GET", f"/v1/{name}"))

    def close(self) -> None:
        pass

    # ── HTTP ─────────────────────────────────────────────────────────────

    def _call(self, method: str, path: str, query: Optional[dict] = None, body: Optional[dict] = None) -> Any:
        url = self.endpoint + path
        if query:
            url += "?" + urllib.parse.urlencode(query)
        data = json.dumps(body).encode("utf-8") if body is not None else None
        req = urllib.request.Request(url, data=data, method=method)
        req.add_header("accept", "application/json")
        if data is not None:
            req.add_header("content-type", "application/json")
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as res:
                return _decode(res.read())
        except urllib.error.HTTPError as e:
            raise RatioError.from_json(_decode(e.read()), e.code) from None
        except urllib.error.URLError as e:
            raise RatioError(14, f"{url}: {e.reason}") from None


def _page(page_size: int, page_token: str) -> dict:
    query: dict = {}
    if page_size:
        query["pageSize"] = str(page_size)
    if page_token:
        query["pageToken"] = page_token
    return query


def _decode(raw: bytes) -> Any:
    if not raw:
        return None
    try:
        return json.loads(raw.decode("utf-8"))
    except ValueError:
        return raw.decode("utf-8", "replace")
