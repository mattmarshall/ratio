#!/usr/bin/env python3
"""Plaid Link token and public-token exchange boundary.

WorkOS authorizes a subject for a Ratio book. Plaid authorizes a bank Item.
Neither grant implies the other. The WorkOS subject and book name are hashed
into Plaid's non-PII client_user_id and are never sent in clear text.
"""

from __future__ import annotations

import hashlib
import hmac
import json
import secrets
import time
from dataclasses import dataclass
from typing import Any, Callable, Mapping, Sequence
from urllib.parse import urlparse

LINK_CREATE_PATH = "/link/token/create"
TOKEN_EXCHANGE_PATH = "/item/public_token/exchange"
PRODUCTS = ("transactions",)
SUPPORTED_COUNTRIES = frozenset(
    {"US", "GB", "ES", "NL", "FR", "IE", "CA", "DE", "IT", "PL", "DK", "NO", "SE", "EE", "LT", "LV", "PT", "BE", "AT", "FI"}
)
SUPPORTED_LANGUAGES = frozenset(
    {"da", "nl", "en", "et", "fr", "de", "hi", "it", "lv", "lt", "no", "pl", "pt", "ro", "es", "sv", "vi"}
)
DEFAULT_SESSION_SECONDS = 15 * 60
DEFAULT_MAX_PENDING = 64


class Refuse(Exception):
    """No Link session or Item credential is accepted."""


Transport = Callable[[str, Mapping[str, Any]], tuple[int, str]]
Clock = Callable[[], float]


@dataclass(frozen=True)
class LinkSession:
    """The only object safe for the browser-facing caller."""

    state: str
    link_token: str
    expires_at: float


class LinkedItem:
    """Server-only, runtime-only Item credential."""

    def __init__(self, item_id: str, access_token: str) -> None:
        self._item_id = item_id
        self._access_token = access_token

    @property
    def item_id(self) -> str:
        return self._item_id

    def __repr__(self) -> str:
        return "LinkedItem(item_id=<redacted>, access_token=<redacted>)"

    def deposit(self, custody: Callable[[str, str], None]) -> None:
        """Move the credential into encrypted custody without returning it."""
        custody(self._item_id, self._access_token)


@dataclass(frozen=True)
class _Pending:
    expires_at: float
    membership_binding: str


class PlaidLink:
    def __init__(
        self,
        *,
        client_id: str,
        secret: str,
        transport: Transport,
        clock: Clock = time.time,
        session_seconds: int = DEFAULT_SESSION_SECONDS,
        max_pending: int = DEFAULT_MAX_PENDING,
    ) -> None:
        if not client_id or not secret:
            raise Refuse("Plaid client_id and secret are required")
        if not isinstance(session_seconds, int) or isinstance(session_seconds, bool) or session_seconds < 1:
            raise Refuse("session_seconds must be a positive integer")
        if not isinstance(max_pending, int) or isinstance(max_pending, bool) or max_pending < 1:
            raise Refuse("max_pending must be a positive integer")
        self._client_id = client_id
        self._secret = secret
        self._transport = transport
        self._clock = clock
        self._session_seconds = session_seconds
        self._max_pending = max_pending
        self._pending: dict[str, _Pending] = {}

    def __repr__(self) -> str:
        return "PlaidLink(credentials=<redacted>)"

    def create(
        self,
        *,
        workos_subject: str,
        book: str,
        country_codes: Sequence[str] = ("US",),
        language: str = "en",
        redirect_uri: str | None = None,
    ) -> LinkSession:
        client_user_id = _client_user_id(workos_subject, book)
        countries = _countries(country_codes)
        lang = _language(language)
        payload: dict[str, Any] = {
            "client_name": "Ratio",
            "language": lang,
            "country_codes": list(countries),
            "products": list(PRODUCTS),
            "user": {"client_user_id": client_user_id},
        }
        if redirect_uri is not None:
            payload["redirect_uri"] = _redirect_uri(redirect_uri)
        now = self._clock()
        self._pending = {
            key: pending for key, pending in self._pending.items() if pending.expires_at > now
        }
        if len(self._pending) >= self._max_pending:
            raise Refuse("too many pending Link sessions")
        response = self._request(LINK_CREATE_PATH, payload)
        link_token = _token(response, "link_token")
        state = secrets.token_urlsafe(32)
        while state in self._pending:
            state = secrets.token_urlsafe(32)
        expires_at = now + self._session_seconds
        self._pending[state] = _Pending(expires_at, client_user_id)
        return LinkSession(state, link_token, expires_at)

    def complete(
        self,
        *,
        state: str,
        workos_subject: str,
        book: str,
        public_token: str,
        expected_item_id: str | None = None,
    ) -> LinkedItem:
        self._claim(state, workos_subject=workos_subject, book=book)
        token = _nonempty(public_token, "Plaid public token", maximum=4096)
        response = self._request(TOKEN_EXCHANGE_PATH, {"public_token": token})
        item_id = _nonempty(response.get("item_id"), "Plaid Item id", maximum=256)
        access_token = _token(response, "access_token")
        if expected_item_id is not None and item_id != expected_item_id:
            raise Refuse("Plaid exchanged a different Item than the Link completion named")
        return LinkedItem(item_id, access_token)

    def cancel(
        self,
        *,
        state: str,
        workos_subject: str,
        book: str,
        provider_error: str | None = None,
    ) -> None:
        self._claim(state, workos_subject=workos_subject, book=book)
        if provider_error:
            _nonempty(provider_error, "Plaid Link error", maximum=256)

    def sync_client(self, item: LinkedItem, *, transport: Any, max_pages: int = 25) -> Any:
        """Move the server-only token directly into the Transactions adapter."""
        if not isinstance(item, LinkedItem):
            raise Refuse("a validated server-only LinkedItem is required")
        import plaid

        return plaid.PlaidClient(
            client_id=self._client_id,
            secret=self._secret,
            access_token=item._access_token,
            transport=transport,
            max_pages=max_pages,
        )

    def _claim(self, state: str, *, workos_subject: str, book: str) -> None:
        key = _nonempty(state, "Link state", maximum=256)
        expected_binding = _client_user_id(workos_subject, book)
        # Pop before exchange. After an uncertain network result, replaying a
        # public token could attach the wrong outcome to a second request.
        pending = self._pending.pop(key, None)
        if pending is None:
            raise Refuse("Link state is unknown or mismatched")
        if self._clock() >= pending.expires_at:
            raise Refuse("Link state expired")
        if not hmac.compare_digest(pending.membership_binding, expected_binding):
            raise Refuse("Link state is unknown or mismatched")

    def _request(self, path: str, body: Mapping[str, Any]) -> Mapping[str, Any]:
        request = {**body, "client_id": self._client_id, "secret": self._secret}
        try:
            status, raw = self._transport(path, request)
        except Exception as exc:
            raise Refuse(f"Plaid {path} transport failed: {type(exc).__name__}") from exc
        if not isinstance(status, int) or isinstance(status, bool):
            raise Refuse(f"Plaid {path} transport returned a malformed HTTP status")
        try:
            response = json.loads(raw)
        except (json.JSONDecodeError, TypeError) as exc:
            raise Refuse(f"Plaid {path} returned malformed JSON") from exc
        if not isinstance(response, dict):
            raise Refuse(f"Plaid {path} returned a non-object response")
        if status >= 400:
            code = str(response.get("error_code") or "HTTP_ERROR")
            raise Refuse(f"Plaid {path} returned HTTP {status} ({code})")
        return response


def _client_user_id(subject: object, book: object) -> str:
    sub = _nonempty(subject, "WorkOS subject", maximum=512)
    book_name = _nonempty(book, "Ratio book", maximum=512)
    digest = hashlib.sha256((sub + "\0" + book_name).encode()).hexdigest()
    return "ratio-" + digest


def _countries(values: Sequence[str]) -> tuple[str, ...]:
    if isinstance(values, (str, bytes)) or not values:
        raise Refuse("at least one Plaid country code is required")
    countries = tuple(dict.fromkeys(str(value).strip().upper() for value in values))
    invalid = [code for code in countries if code not in SUPPORTED_COUNTRIES]
    if invalid:
        raise Refuse("unsupported Plaid country code " + ", ".join(invalid))
    return countries


def _language(value: object) -> str:
    language = str(value or "").strip().lower()
    if language not in SUPPORTED_LANGUAGES:
        raise Refuse(f"unsupported Plaid Link language {language!r}")
    return language


def _redirect_uri(value: object) -> str:
    uri = _nonempty(value, "Plaid redirect URI", maximum=2048)
    parsed = urlparse(uri)
    if parsed.scheme != "https" or not parsed.hostname or parsed.username or parsed.password:
        raise Refuse("Plaid production redirect URI must be HTTPS with no user information")
    if parsed.query or parsed.fragment:
        raise Refuse("Plaid redirect URI cannot contain a query or fragment")
    return uri


def _token(response: Mapping[str, Any], name: str) -> str:
    return _nonempty(response.get(name), f"Plaid {name}", maximum=8192)


def _nonempty(value: object, name: str, *, maximum: int) -> str:
    if not isinstance(value, str) or not value.strip() or len(value) > maximum:
        raise Refuse(f"{name} is missing or malformed")
    return value.strip()
