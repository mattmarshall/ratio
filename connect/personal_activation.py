#!/usr/bin/env python3
"""Local Personal provider activation with encrypted per-membership custody.

WorkOS Connect authorizes access to one Ratio book. Plaid Link and Google
OAuth independently authorize upstream data. This module keeps those grants
separate, verifies the WorkOS subject against ConnectApiUrl, and decrypts an
upstream credential only for the duration of one provider operation.
"""

from __future__ import annotations

import base64
import contextlib
import fcntl
import hashlib
import hmac
import json
import os
import secrets
import tempfile
import time
import webbrowser
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from typing import Any, Callable, Mapping
from urllib.error import HTTPError
from urllib.parse import parse_qs, urlencode, urlparse
from urllib.request import Request, urlopen

import bills
import google_calendar
import grant
import mapper
import oauth
import plaid
import plaid_link

GOOGLE_AUTHORIZATION_ENDPOINT = "https://accounts.google.com/o/oauth2/v2/auth"
GOOGLE_TOKEN_ENDPOINT = "https://oauth2.googleapis.com/token"
GOOGLE_REVOKE_ENDPOINT = "https://oauth2.googleapis.com/revoke"
GOOGLE_REDIRECT_URI = "http://127.0.0.1:8766/google/callback"
TOKEN_KEY_ENV = "RATIO_PERSONAL_TOKEN_KEY"
TOKEN_STORE_ENV = "RATIO_PERSONAL_TOKEN_STORE"
DEFAULT_TOKEN_STORE = "~/.local/share/ratio/personal-providers.json"
_AAD_PREFIX = b"ratio-personal-provider-v1\0"
PLAID_ORIGINS = {
    "sandbox": "https://sandbox.plaid.com",
    "development": "https://development.plaid.com",
    "production": "https://production.plaid.com",
}

ProviderTransport = Callable[
    [str, str, Mapping[str, str], bytes | None], tuple[int, str]
]


class Refuse(Exception):
    """Activation or sync is unsafe or incomplete."""


@dataclass(frozen=True)
class Membership:
    subject: str
    book: str

    def __post_init__(self) -> None:
        subject = self.subject.strip()
        named = self.book.strip().strip("/")
        if named.startswith("v1/"):
            named = named[3:]
        if named and not named.startswith("books/"):
            named = "books/" + named
        if (
            not subject
            or len(subject) > 512
            or "\0" in subject
            or not named
            or len(named) > 512
            or "\0" in named
        ):
            raise Refuse("WorkOS subject and Ratio book must be bounded resource identifiers")
        object.__setattr__(self, "subject", subject)
        object.__setattr__(self, "book", named)

    @property
    def binding(self) -> str:
        return hashlib.sha256(
            (self.subject + "\0" + self.book).encode("utf-8")
        ).hexdigest()


@dataclass(frozen=True)
class PlaidStatus:
    connected: bool
    cursor_present: bool
    pending_count: int


@dataclass(frozen=True)
class GoogleStatus:
    connected: bool
    sync_token_present: bool
    seen_count: int


@dataclass(frozen=True)
class GoogleAttempt:
    state: str
    authorization_url: str


class GoogleGrant:
    """Server-only Google credentials; callers can only deposit or revoke."""

    def __init__(
        self, access_token: str, refresh_token: str, expires_at: float
    ) -> None:
        self._access_token = access_token
        self._refresh_token = refresh_token
        self._expires_at = expires_at

    def __repr__(self) -> str:
        return "GoogleGrant(credentials=<redacted>)"

    def deposit(self, custody: Callable[[Mapping[str, Any]], None]) -> None:
        custody(
            {
                "access_token": self._access_token,
                "refresh_token": self._refresh_token,
                "expires_at": self._expires_at,
            }
        )


class TokenVault:
    """AES-256-GCM records in an atomic mode-0600 local file.

    Subject and book names are represented only by one-way membership
    bindings. Credentials, cursors, and event indexes are inside independently
    authenticated provider records. The outer document contains only those
    bindings, authenticated status counts, and opaque ciphertext.
    """

    def __init__(self, path: str | Path, key: bytes) -> None:
        if len(key) != 32:
            raise Refuse(f"{TOKEN_KEY_ENV} must decode to exactly 32 bytes")
        self.path = Path(path).expanduser()
        self._key = bytes(key)
        self._status_key = hashlib.sha256(
            b"ratio-personal-provider-status-v1\0" + self._key
        ).digest()
        try:
            from cryptography.hazmat.primitives.ciphers.aead import AESGCM
        except ImportError as exc:
            raise Refuse(
                "encrypted provider custody requires the Python cryptography package"
            ) from exc
        self._aead = AESGCM(self._key)

    @classmethod
    def from_environment(cls) -> "TokenVault":
        encoded = os.environ.get(TOKEN_KEY_ENV, "").strip()
        if not encoded:
            raise Refuse(f"{TOKEN_KEY_ENV} is required for encrypted provider custody")
        try:
            key = base64.urlsafe_b64decode(encoded + "=" * (-len(encoded) % 4))
        except Exception as exc:
            raise Refuse(f"{TOKEN_KEY_ENV} must be URL-safe base64") from exc
        return cls(os.environ.get(TOKEN_STORE_ENV, DEFAULT_TOKEN_STORE), key)

    def _get(self, membership: Membership, provider: str) -> dict[str, Any] | None:
        """Open a record for an adapter operation; never expose it to a caller."""
        self._provider(provider)
        with self._locked():
            document = self._read()
            envelope = (
                document.get("memberships", {})
                .get(membership.binding, {})
                .get(provider)
            )
            if envelope is None:
                return None
            return self._open(membership.binding, provider, envelope)

    def metadata(
        self, membership: Membership, provider: str
    ) -> dict[str, Any] | None:
        """Read non-sensitive status without decrypting provider credentials."""
        self._provider(provider)
        with self._locked():
            document = self._read()
            envelope = (
                document.get("memberships", {})
                .get(membership.binding, {})
                .get(provider)
            )
            if envelope is None:
                return None
            metadata = envelope.get("metadata")
            if not isinstance(metadata, dict):
                raise Refuse("provider custody status is malformed")
            expected = hmac.new(
                self._status_key,
                self._aad(membership.binding, provider)
                + b"\0"
                + json.dumps(
                    metadata, sort_keys=True, separators=(",", ":")
                ).encode("utf-8"),
                hashlib.sha256,
            ).digest()
            try:
                presented = base64.urlsafe_b64decode(str(envelope["metadata_tag"]))
            except Exception as exc:
                raise Refuse("provider custody status failed authentication") from exc
            if not hmac.compare_digest(expected, presented):
                raise Refuse("provider custody status failed authentication")
            return dict(metadata)

    def put(
        self, membership: Membership, provider: str, record: Mapping[str, Any]
    ) -> None:
        self._provider(provider)
        with self._locked():
            document = self._read()
            memberships = document.setdefault("memberships", {})
            providers = memberships.setdefault(membership.binding, {})
            providers[provider] = self._seal(
                membership.binding, provider, dict(record)
            )
            self._write(document)

    def delete(self, membership: Membership, provider: str) -> None:
        self._provider(provider)
        with self._locked():
            document = self._read()
            memberships = document.get("memberships", {})
            providers = memberships.get(membership.binding, {})
            providers.pop(provider, None)
            if not providers:
                memberships.pop(membership.binding, None)
            self._write(document)

    def _seal(
        self, binding: str, provider: str, record: Mapping[str, Any]
    ) -> dict[str, str]:
        nonce = secrets.token_bytes(12)
        plaintext = json.dumps(
            record, sort_keys=True, separators=(",", ":")
        ).encode("utf-8")
        ciphertext = self._aead.encrypt(
            nonce, plaintext, self._aad(binding, provider)
        )
        metadata = self._metadata(provider, record)
        metadata_tag = hmac.new(
            self._status_key,
            self._aad(binding, provider)
            + b"\0"
            + json.dumps(
                metadata, sort_keys=True, separators=(",", ":")
            ).encode("utf-8"),
            hashlib.sha256,
        ).digest()
        return {
            "nonce": base64.urlsafe_b64encode(nonce).decode("ascii"),
            "ciphertext": base64.urlsafe_b64encode(ciphertext).decode("ascii"),
            "metadata": metadata,
            "metadata_tag": base64.urlsafe_b64encode(metadata_tag).decode("ascii"),
        }

    @staticmethod
    def _metadata(provider: str, record: Mapping[str, Any]) -> dict[str, Any]:
        if provider == "plaid":
            pending = record.get("pending", {})
            return {
                "cursor_present": bool(record.get("cursor")),
                "pending_count": len(pending) if isinstance(pending, dict) else 0,
            }
        seen = record.get("seen", {})
        return {
            "sync_token_present": bool(record.get("sync_token")),
            "seen_count": len(seen) if isinstance(seen, dict) else 0,
        }

    def _open(
        self, binding: str, provider: str, envelope: Mapping[str, Any]
    ) -> dict[str, Any]:
        try:
            nonce = base64.urlsafe_b64decode(str(envelope["nonce"]))
            ciphertext = base64.urlsafe_b64decode(str(envelope["ciphertext"]))
            plaintext = self._aead.decrypt(
                nonce, ciphertext, self._aad(binding, provider)
            )
            record = json.loads(plaintext)
        except Exception as exc:
            raise Refuse("provider custody record failed authentication") from exc
        if not isinstance(record, dict):
            raise Refuse("provider custody record is malformed")
        return record

    @staticmethod
    def _aad(binding: str, provider: str) -> bytes:
        return _AAD_PREFIX + binding.encode("ascii") + b"\0" + provider.encode("ascii")

    @staticmethod
    def _provider(provider: str) -> None:
        if provider not in {"plaid", "google"}:
            raise Refuse("unsupported provider custody namespace")

    @contextlib.contextmanager
    def _locked(self):
        self.path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        os.chmod(self.path.parent, 0o700)
        lock_path = self.path.with_suffix(self.path.suffix + ".lock")
        with open(lock_path, "a+b") as lock:
            os.chmod(lock_path, 0o600)
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
            try:
                yield
            finally:
                fcntl.flock(lock.fileno(), fcntl.LOCK_UN)

    def _read(self) -> dict[str, Any]:
        if not self.path.exists():
            return {"version": 1, "memberships": {}}
        try:
            document = json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            raise Refuse("provider custody file is unreadable or malformed") from exc
        if (
            not isinstance(document, dict)
            or document.get("version") != 1
            or not isinstance(document.get("memberships"), dict)
        ):
            raise Refuse("provider custody file has an unsupported shape")
        return document

    def _write(self, document: Mapping[str, Any]) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        fd, temporary = tempfile.mkstemp(
            prefix=self.path.name + ".", dir=self.path.parent
        )
        try:
            os.fchmod(fd, 0o600)
            with os.fdopen(fd, "w", encoding="utf-8") as stream:
                json.dump(document, stream, sort_keys=True, separators=(",", ":"))
                stream.write("\n")
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(temporary, self.path)
            directory = os.open(self.path.parent, os.O_RDONLY)
            try:
                os.fsync(directory)
            finally:
                os.close(directory)
        finally:
            with contextlib.suppress(FileNotFoundError):
                os.unlink(temporary)


class GoogleOAuth:
    """Installed-app Google grant, separate from WorkOS Connect."""

    def __init__(
        self,
        *,
        client_id: str,
        client_secret: str,
        transport: ProviderTransport | None = None,
        clock: Callable[[], float] = time.time,
    ) -> None:
        if not client_id.strip() or not client_secret.strip():
            raise Refuse("Google OAuth client id and secret are required")
        self._client_id = client_id.strip()
        self._client_secret = client_secret.strip()
        self._transport = transport or _provider_transport
        self._clock = clock
        self._pending: dict[str, tuple[str, float]] = {}

    @classmethod
    def from_environment(
        cls, *, transport: ProviderTransport | None = None
    ) -> "GoogleOAuth":
        return cls(
            client_id=os.environ.get("GOOGLE_OAUTH_CLIENT_ID", ""),
            client_secret=os.environ.get("GOOGLE_OAUTH_CLIENT_SECRET", ""),
            transport=transport,
        )

    def begin(self, membership: Membership) -> GoogleAttempt:
        state = secrets.token_urlsafe(32)
        self._pending[state] = (membership.binding, self._clock() + 900)
        url = GOOGLE_AUTHORIZATION_ENDPOINT + "?" + urlencode(
            {
                "client_id": self._client_id,
                "redirect_uri": GOOGLE_REDIRECT_URI,
                "response_type": "code",
                "scope": google_calendar.GOOGLE_SCOPE,
                "access_type": "offline",
                "prompt": "consent",
                "include_granted_scopes": "false",
                "state": state,
            }
        )
        return GoogleAttempt(state, url)

    def authorize(
        self,
        membership: Membership,
        *,
        launch: Callable[[str], object] = webbrowser.open,
        timeout: float = 180.0,
    ) -> GoogleGrant:
        """Run the fixed local callback and return credentials to custody."""
        attempt = self.begin(membership)
        try:
            server = _GoogleCallbackServer(
                ("127.0.0.1", 8766), _GoogleCallbackHandler
            )
        except OSError as exc:
            raise Refuse("cannot bind the Google callback on 127.0.0.1:8766") from exc
        server.target = None
        server.expected_state = attempt.state
        server.timeout = timeout
        with server:
            if launch(attempt.authorization_url) is False:
                raise Refuse("could not open the Google authorization URL")
            server.handle_request()
        if server.target is None:
            raise Refuse("Google authorization timed out before the callback")
        return self.complete(server.target, membership)

    def complete(
        self, target: str, membership: Membership
    ) -> GoogleGrant:
        parsed = urlparse(target)
        if parsed.path != "/google/callback":
            raise Refuse("Google callback arrived on the wrong path")
        values = parse_qs(parsed.query, keep_blank_values=True)
        if "error" in values:
            raise Refuse("Google authorization was refused")
        state = _one(values, "state", "Google callback")
        pending = self._pending.pop(state, None)
        if (
            pending is None
            or self._clock() >= pending[1]
            or not secrets.compare_digest(pending[0], membership.binding)
        ):
            raise Refuse("Google callback state, subject, or book does not match")
        code = _one(values, "code", "Google callback")
        payload = self._token_request(
            {
                "grant_type": "authorization_code",
                "client_id": self._client_id,
                "client_secret": self._client_secret,
                "redirect_uri": GOOGLE_REDIRECT_URI,
                "code": code,
            }
        )
        refresh = _secret(payload.get("refresh_token"), "Google refresh token")
        access = _google_access(payload)
        return GoogleGrant(
            access,
            refresh,
            self._clock()
            + _positive_int(payload.get("expires_in"), "Google expires_in"),
        )

    def refresh(self, refresh_token: str) -> GoogleGrant:
        payload = self._token_request(
            {
                "grant_type": "refresh_token",
                "client_id": self._client_id,
                "client_secret": self._client_secret,
                "refresh_token": refresh_token,
            }
        )
        rotated = payload.get("refresh_token")
        next_refresh = (
            _secret(rotated, "Google refresh token")
            if rotated is not None
            else refresh_token
        )
        return GoogleGrant(
            _google_access(payload),
            next_refresh,
            self._clock()
            + _positive_int(payload.get("expires_in"), "Google expires_in"),
        )

    def revoke(self, token: str) -> None:
        status, _ = self._transport(
            "POST",
            GOOGLE_REVOKE_ENDPOINT,
            {"content-type": "application/x-www-form-urlencoded"},
            urlencode({"token": token}).encode("ascii"),
        )
        if status >= 400:
            raise Refuse(f"Google token revocation returned HTTP {status}")

    def _token_request(self, fields: Mapping[str, str]) -> Mapping[str, Any]:
        status, raw = self._transport(
            "POST",
            GOOGLE_TOKEN_ENDPOINT,
            {
                "content-type": "application/x-www-form-urlencoded",
                "accept": "application/json",
            },
            urlencode(fields).encode("ascii"),
        )
        if status >= 400:
            raise Refuse(f"Google token endpoint returned HTTP {status}")
        try:
            payload = json.loads(raw)
        except json.JSONDecodeError as exc:
            raise Refuse("Google token endpoint returned malformed JSON") from exc
        if not isinstance(payload, dict):
            raise Refuse("Google token endpoint returned a non-object")
        return payload


class PersonalActivation:
    """One authenticated Personal membership's provider lifecycle."""

    def __init__(
        self,
        *,
        membership: Membership,
        workos_access_token: str,
        vault: TokenVault,
        ratio_transport: grant.Transport | None = None,
        clock: Callable[[], float] = time.time,
    ) -> None:
        self.membership = membership
        self._workos_access_token = _secret(
            workos_access_token, "WorkOS Connect access token"
        )
        self._vault = vault
        self._ratio_transport = ratio_transport
        self._clock = clock
        self._verify_membership()

    def _verify_membership(self) -> None:
        subject = _jwt_subject(self._workos_access_token)
        if not secrets.compare_digest(subject, self.membership.subject):
            raise Refuse("WorkOS token subject does not match the selected membership")
        try:
            payload = grant.pull(
                token=self._workos_access_token,
                book_id=self.membership.book,
                transport=self._ratio_transport,
                error=Refuse,
            )
        except Exception as exc:
            if isinstance(exc, Refuse):
                raise
            raise Refuse("selected book membership could not be verified") from exc
        book = payload.get("book", payload) if isinstance(payload, dict) else {}
        kind = str(book.get("kind") or "")
        name = str(book.get("name") or "")
        if kind not in {"PERSONAL", "KIND_PERSONAL"}:
            raise Refuse("provider activation requires a Personal book")
        selected = self.membership.book.strip("/")
        if name and name.strip("/") not in {selected, f"books/{selected}"}:
            raise Refuse("ConnectApiUrl returned a different book than selected")

    def plaid_status(self) -> PlaidStatus:
        metadata = self._vault.metadata(self.membership, "plaid")
        if metadata is None:
            return PlaidStatus(False, False, 0)
        return PlaidStatus(
            True,
            bool(metadata.get("cursor_present")),
            int(metadata.get("pending_count") or 0),
        )

    def begin_plaid(
        self, link: plaid_link.PlaidLink, **options: Any
    ) -> plaid_link.LinkSession:
        if self.plaid_status().connected:
            raise Refuse("one Plaid Item is already connected; disconnect it first")
        try:
            return link.create(
                workos_subject=self.membership.subject,
                book=self.membership.book,
                **options,
            )
        except plaid_link.Refuse as exc:
            raise Refuse(str(exc)) from exc

    def complete_plaid(self, link: plaid_link.PlaidLink, **completion: Any) -> str:
        if self.plaid_status().connected:
            raise Refuse("one Plaid Item is already connected; disconnect it first")
        try:
            item = link.complete(
                workos_subject=self.membership.subject,
                book=self.membership.book,
                **completion,
            )
        except plaid_link.Refuse as exc:
            raise Refuse(str(exc)) from exc

        def deposit(item_id: str, access_token: str) -> None:
            self._vault.put(
                self.membership,
                "plaid",
                {
                    "item_id": item_id,
                    "access_token": access_token,
                    "cursor": "",
                    "pending": {},
                },
            )

        item.deposit(deposit)
        return item.item_id

    def sync_plaid(
        self,
        *,
        link: plaid_link.PlaidLink,
        provider_transport: plaid.Transport,
        choices: Mapping[str, plaid.RuleChoice],
        ratio_client: mapper.Client,
        book: mapper.Book,
    ) -> plaid.SyncResult:
        record = self._require("plaid")
        item = plaid_link.LinkedItem(
            _secret(record.get("item_id"), "Plaid Item id"),
            _secret(record.get("access_token"), "Plaid Item access token"),
        )
        client = link.sync_client(item, transport=provider_transport)
        try:
            result = client.sync(
                cursor=str(record.get("cursor") or ""),
                choices=choices,
                book=book,
                ratio_client=ratio_client,
            )
        except plaid.Refuse as exc:
            raise Refuse(str(exc)) from exc
        pending = dict(record.get("pending", {}))
        for pending_id in result.resolved_pending_ids:
            pending.pop(pending_id, None)
        for row in result.pending:
            pending[row.transaction_id] = {
                "account_id": row.account_id,
                "date": row.dated.isoformat(),
            }
        record["cursor"] = result.cursor
        record["pending"] = pending
        self._vault.put(self.membership, "plaid", record)
        return result

    def disconnect_plaid(
        self, *, link: plaid_link.PlaidLink, provider_transport: plaid.Transport
    ) -> None:
        record = self._require("plaid")
        item = plaid_link.LinkedItem(
            _secret(record.get("item_id"), "Plaid Item id"),
            _secret(record.get("access_token"), "Plaid Item access token"),
        )
        client = link.sync_client(item, transport=provider_transport)
        try:
            client.disconnect()
        except plaid.Refuse as exc:
            raise Refuse(str(exc)) from exc
        self._vault.delete(self.membership, "plaid")

    def google_status(self) -> GoogleStatus:
        metadata = self._vault.metadata(self.membership, "google")
        if metadata is None:
            return GoogleStatus(False, False, 0)
        return GoogleStatus(
            True,
            bool(metadata.get("sync_token_present")),
            int(metadata.get("seen_count") or 0),
        )

    def complete_google(
        self, google_oauth: GoogleOAuth, *, callback_target: str, calendar_id: str
    ) -> None:
        if self.google_status().connected:
            raise Refuse("one Google Calendar grant is already connected; disconnect it first")
        if not calendar_id.strip() or calendar_id.strip() == "primary":
            raise Refuse("select one concrete Google calendar")
        credentials = google_oauth.complete(callback_target, self.membership)

        def deposit(record: Mapping[str, Any]) -> None:
            stored = {
                **record,
                "calendar_id": calendar_id.strip(),
                "sync_token": "",
                "seen": {},
            }
            self._vault.put(self.membership, "google", stored)

        credentials.deposit(deposit)

    def connect_google(
        self,
        google_oauth: GoogleOAuth,
        *,
        calendar_id: str,
        launch: Callable[[str], object] = webbrowser.open,
        timeout: float = 180.0,
    ) -> None:
        """Authorize in the browser and deposit tokens without returning them."""
        if self.google_status().connected:
            raise Refuse("one Google Calendar grant is already connected; disconnect it first")
        if not calendar_id.strip() or calendar_id.strip() == "primary":
            raise Refuse("select one concrete Google calendar")
        credentials = google_oauth.authorize(
            self.membership, launch=launch, timeout=timeout
        )

        def deposit(record: Mapping[str, Any]) -> None:
            stored = {
                **record,
                "calendar_id": calendar_id.strip(),
                "sync_token": "",
                "seen": {},
            }
            self._vault.put(self.membership, "google", stored)

        credentials.deposit(deposit)

    def sync_google(
        self,
        *,
        google_oauth: GoogleOAuth,
        provider_transport: google_calendar.Transport,
        ratio_client: bills.Client,
        book: bills.Book,
    ) -> google_calendar.SyncResult:
        record = self._require("google")
        if self._clock() + 60 >= float(record.get("expires_at") or 0):
            refreshed = google_oauth.refresh(
                _secret(record.get("refresh_token"), "Google refresh token")
            )

            def deposit(update: Mapping[str, Any]) -> None:
                record.update(update)

            refreshed.deposit(deposit)
            self._vault.put(self.membership, "google", record)
        client = google_calendar.GoogleCalendar(
            access_token=_secret(record.get("access_token"), "Google access token"),
            transport=provider_transport,
        )
        arguments = {
            "calendar_id": str(record.get("calendar_id") or ""),
            "sync_token": str(record.get("sync_token") or ""),
            "seen": dict(record.get("seen", {})),
            "book": book,
            "ratio_client": ratio_client,
        }
        try:
            try:
                result = client.sync(**arguments)
            except google_calendar.FullResyncRequired:
                arguments["sync_token"] = ""
                result = client.sync(**arguments)
        except google_calendar.Refuse as exc:
            raise Refuse(str(exc)) from exc
        record["sync_token"] = result.sync_token
        record["seen"] = dict(result.seen)
        self._vault.put(self.membership, "google", record)
        return result

    def disconnect_google(self, google_oauth: GoogleOAuth) -> None:
        record = self._require("google")
        token = str(record.get("refresh_token") or record.get("access_token") or "")
        google_oauth.revoke(_secret(token, "Google revocation token"))
        self._vault.delete(self.membership, "google")

    def _require(self, provider: str) -> dict[str, Any]:
        record = self._vault._get(self.membership, provider)
        if record is None:
            raise Refuse(f"{provider} is not connected for this membership")
        return record


def authorize_workos(
    *,
    manifest: str | Path,
    client_id: str,
    subject: str,
    book: str,
    vault: TokenVault,
    issuer: str = oauth.DEFAULT_ISSUER,
    launch: oauth.Launch = __import__("webbrowser").open,
    oauth_transport: oauth.Transport | None = None,
    ratio_transport: grant.Transport | None = None,
) -> PersonalActivation:
    """Complete WorkOS PKCE, then bind its JWT subject and book membership."""
    app = oauth.application_from_manifest(
        manifest, client_id=client_id, issuer=issuer
    )
    token = oauth.authorize(app, launch=launch, transport=oauth_transport)
    return PersonalActivation(
        membership=Membership(subject, book),
        workos_access_token=token,
        vault=vault,
        ratio_transport=ratio_transport,
    )


def plaid_link_from_environment(
    *, transport: plaid_link.Transport | None = None
) -> plaid_link.PlaidLink:
    environment = os.environ.get("PLAID_ENV", "").strip().lower()
    origin = PLAID_ORIGINS.get(environment)
    if origin is None:
        raise Refuse("PLAID_ENV must be sandbox, development, or production")
    return plaid_link.PlaidLink(
        client_id=os.environ.get("PLAID_CLIENT_ID", "").strip(),
        secret=os.environ.get("PLAID_SECRET", "").strip(),
        transport=transport or _plaid_transport(origin),
    )


def _plaid_transport(origin: str) -> plaid_link.Transport:
    def send(path: str, body: Mapping[str, Any]) -> tuple[int, str]:
        request = Request(
            origin + path,
            data=json.dumps(body).encode("utf-8"),
            headers={
                "content-type": "application/json",
                "accept": "application/json",
            },
            method="POST",
        )
        try:
            with urlopen(request, timeout=30) as response:
                return response.status, response.read().decode("utf-8")
        except HTTPError as exc:
            return exc.code, exc.read().decode("utf-8") if exc.fp else ""
        except Exception as exc:
            # Provider errors can retain request data; do not chain them.
            raise Refuse(f"Plaid request failed: {type(exc).__name__}") from None

    return send


def _jwt_subject(token: str) -> str:
    parts = token.split(".")
    if len(parts) != 3:
        raise Refuse("WorkOS Connect access token is not a JWT")
    try:
        payload = json.loads(
            base64.urlsafe_b64decode(parts[1] + "=" * (-len(parts[1]) % 4))
        )
    except Exception as exc:
        raise Refuse("WorkOS Connect access token has a malformed subject claim") from exc
    subject = payload.get("sub") if isinstance(payload, dict) else None
    return _secret(subject, "WorkOS subject")


def _one(values: Mapping[str, list[str]], name: str, what: str) -> str:
    found = values.get(name, [])
    if len(found) != 1 or not found[0].strip():
        raise Refuse(f"{what} must carry exactly one nonempty {name}")
    return found[0].strip()


def _secret(value: object, name: str) -> str:
    if not isinstance(value, str) or not value.strip() or len(value) > 16384:
        raise Refuse(f"{name} is missing or malformed")
    return value.strip()


def _positive_int(value: object, name: str) -> int:
    if isinstance(value, bool):
        raise Refuse(f"{name} is missing or malformed")
    try:
        parsed = int(value)
    except (TypeError, ValueError) as exc:
        raise Refuse(f"{name} is missing or malformed") from exc
    if parsed <= 0 or parsed > 86400:
        raise Refuse(f"{name} is missing or malformed")
    return parsed


def _google_access(payload: Mapping[str, Any]) -> str:
    if str(payload.get("token_type") or "").lower() != "bearer":
        raise Refuse("Google token endpoint returned no bearer access token")
    scope = payload.get("scope")
    if scope is not None and set(str(scope).split()) != {google_calendar.GOOGLE_SCOPE}:
        raise Refuse("Google grant did not return exactly the Calendar read-only scope")
    return _secret(payload.get("access_token"), "Google access token")


def _provider_transport(
    method: str, url: str, headers: Mapping[str, str], body: bytes | None
) -> tuple[int, str]:
    try:
        with urlopen(
            Request(url, data=body, headers=dict(headers), method=method), timeout=30
        ) as response:
            return response.status, response.read().decode("utf-8")
    except Exception as exc:
        raise Refuse(f"provider request failed: {type(exc).__name__}") from exc


class _GoogleCallbackServer(HTTPServer):
    target: str | None
    expected_state: str


class _GoogleCallbackHandler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:  # noqa: N802 — BaseHTTPRequestHandler API
        parsed = urlparse(self.path)
        values = parse_qs(parsed.query, keep_blank_values=True)
        state = values.get("state", [])
        valid = (
            parsed.path == "/google/callback"
            and len(state) == 1
            and secrets.compare_digest(
                state[0], self.server.expected_state  # type: ignore[attr-defined]
            )
        )
        if valid:
            self.server.target = self.path  # type: ignore[attr-defined]
            message = b"Ratio Google authorization completed. You may close this tab."
            status = 200
        else:
            message = b"Ratio Google authorization was refused. Return to the application."
            status = 400
        self.send_response(status)
        self.send_header("content-type", "text/plain; charset=utf-8")
        self.send_header("content-length", str(len(message)))
        self.end_headers()
        self.wfile.write(message)

    def log_message(self, _format: str, *_args: object) -> None:
        # The query carries an authorization code.
        return


if __name__ == "__main__":
    raise SystemExit(
        "Import personal_activation.py from a local UI. It never prints provider tokens."
    )
