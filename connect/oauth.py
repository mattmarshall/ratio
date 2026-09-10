#!/usr/bin/env python3
"""Public PKCE authorization for Ratio's first-party Connect tools.

These tools run locally and act for a signed-in household administrator. A
public WorkOS Connect application plus a fixed loopback callback gives them an
authorization-code grant without a client secret. Tokens remain return values
in this process: this module does not print or persist access, refresh, or ID
tokens.
"""

from __future__ import annotations

import base64
import hashlib
import json
import secrets
import threading
import webbrowser
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from typing import Callable, Mapping
from urllib.parse import parse_qs, urlencode, urlparse
from urllib.request import Request, urlopen

DEFAULT_ISSUER = "https://auth.ratio.marsh.build"
DEFAULT_REDIRECT_URI = "http://127.0.0.1:8765/callback"

# method, URL, headers, encoded body -> status, response body
Transport = Callable[[str, str, Mapping[str, str], bytes | None], tuple[int, str]]
Launch = Callable[[str], object]


class Refuse(Exception):
    """The OAuth claim is unsafe or incomplete; no token is returned."""


@dataclass(frozen=True)
class Application:
    client_id: str
    scopes: tuple[str, ...]
    issuer: str = DEFAULT_ISSUER
    redirect_uri: str = DEFAULT_REDIRECT_URI


@dataclass(frozen=True)
class Attempt:
    state: str
    nonce: str
    verifier: str
    challenge: str


@dataclass(frozen=True)
class Callback:
    code: str
    state: str


def _one(values: Mapping[str, list[str]], name: str) -> str:
    found = values.get(name, [])
    if len(found) != 1 or not found[0].strip():
        raise Refuse(f"OAuth callback must carry exactly one nonempty {name}")
    return found[0].strip()


def _url(raw: str, *, what: str) -> object:
    try:
        parsed = urlparse(raw)
        _ = parsed.port
    except ValueError as exc:
        raise Refuse(f"{what} is not a valid URL") from exc
    if parsed.username or parsed.password or "*" in (parsed.hostname or ""):
        raise Refuse(f"{what} cannot contain credentials or a wildcard host")
    return parsed


def validate_issuer(raw: str, *, allow_loopback_http: bool = False) -> str:
    issuer = raw.strip().rstrip("/")
    parsed = _url(issuer, what="WorkOS Connect issuer")
    loopback = parsed.hostname in {"127.0.0.1", "localhost"}
    if parsed.scheme != "https" and not (
        allow_loopback_http and parsed.scheme == "http" and loopback
    ):
        raise Refuse("WorkOS Connect issuer must use HTTPS")
    if not parsed.hostname or parsed.query or parsed.fragment or parsed.path not in {"", "/"}:
        raise Refuse("WorkOS Connect issuer must be an origin with no path, query, or fragment")
    return issuer


def validate_redirect(raw: str) -> str:
    redirect = raw.strip()
    parsed = _url(redirect, what="Connect redirect URI")
    if (
        parsed.scheme != "http"
        or parsed.hostname != "127.0.0.1"
        or parsed.port is None
        or parsed.port <= 0
        or parsed.path != "/callback"
        or parsed.params
        or parsed.query
        or parsed.fragment
    ):
        raise Refuse(
            "Connect redirect URI must be exactly an http://127.0.0.1:<port>/callback loopback"
        )
    return redirect


def application_from_manifest(
    path: str | Path,
    *,
    client_id: str,
    issuer: str = DEFAULT_ISSUER,
    redirect_uri: str = DEFAULT_REDIRECT_URI,
    allow_loopback_issuer: bool = False,
) -> Application:
    try:
        manifest = json.loads(Path(path).read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise Refuse("Connect app manifest is missing or malformed") from exc
    connect = manifest.get("workos_connect") or {}
    if connect.get("application_type") != "oauth" or connect.get("flow") != "authorization_code":
        raise Refuse("Connect app must declare the OAuth authorization_code flow")
    if connect.get("trust") != "first-party":
        raise Refuse("this public client is only for a declared first-party Connect app")
    if connect.get("public_client") is not True or connect.get("uses_pkce") is not True:
        raise Refuse("Connect app must declare public_client and uses_pkce")
    registered = connect.get("redirect_uri")
    if registered != redirect_uri:
        raise Refuse("runtime redirect URI must exactly match the app manifest")
    raw_scopes = connect.get("scopes")
    if not isinstance(raw_scopes, list) or not raw_scopes:
        raise Refuse("Connect app must declare at least one Ratio scope")
    scopes = tuple(str(scope).strip() for scope in raw_scopes)
    if any(not scope or scope in {"openid", "profile", "email", "offline_access"} for scope in scopes):
        raise Refuse("app.json scopes must contain Ratio resource grants only")
    if len(set(scopes)) != len(scopes):
        raise Refuse("Connect app scopes must be unique")
    named = client_id.strip()
    if not named or not named.startswith("client_"):
        raise Refuse("a WorkOS Connect public client_id is required")
    return Application(
        client_id=named,
        scopes=scopes,
        issuer=validate_issuer(issuer, allow_loopback_http=allow_loopback_issuer),
        redirect_uri=validate_redirect(redirect_uri),
    )


def new_attempt() -> Attempt:
    verifier = secrets.token_urlsafe(64)
    challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).decode().rstrip("=")
    return Attempt(
        state=secrets.token_urlsafe(32),
        nonce=secrets.token_urlsafe(32),
        verifier=verifier,
        challenge=challenge,
    )


def authorization_url(app: Application, attempt: Attempt) -> str:
    # `openid` is the protocol scope WorkOS requires for an OAuth/OIDC user.
    # It is not a Ratio resource grant and app.json remains the exact API scope list.
    query = urlencode(
        {
            "client_id": app.client_id,
            "redirect_uri": app.redirect_uri,
            "response_type": "code",
            "scope": " ".join(("openid", *app.scopes)),
            "state": attempt.state,
            "nonce": attempt.nonce,
            "code_challenge": attempt.challenge,
            "code_challenge_method": "S256",
        }
    )
    return f"{app.issuer}/oauth2/authorize?{query}"


def parse_callback(target: str, *, expected_state: str) -> Callback:
    parsed = urlparse(target)
    if parsed.path != "/callback":
        raise Refuse("OAuth callback arrived on the wrong path")
    values = parse_qs(parsed.query, keep_blank_values=True)
    if "error" in values:
        reason = _one(values, "error")
        raise Refuse(f"WorkOS Connect authorization refused: {reason}")
    state = _one(values, "state")
    if not secrets.compare_digest(state, expected_state):
        raise Refuse("OAuth callback state does not match the authorization attempt")
    return Callback(code=_one(values, "code"), state=state)


class _CallbackServer(HTTPServer):
    expected_state: str
    result: Callback | None
    refusal: Refuse | None


class _CallbackHandler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:  # noqa: N802 — BaseHTTPRequestHandler API
        try:
            self.server.result = parse_callback(  # type: ignore[attr-defined]
                self.path, expected_state=self.server.expected_state  # type: ignore[attr-defined]
            )
            status, message = 200, b"Ratio Connect authorization completed. You may close this tab."
        except Refuse as exc:
            self.server.refusal = exc  # type: ignore[attr-defined]
            status, message = 400, b"Ratio Connect authorization was refused. Return to the terminal."
        self.send_response(status)
        self.send_header("content-type", "text/plain; charset=utf-8")
        self.send_header("content-length", str(len(message)))
        self.end_headers()
        self.wfile.write(message)

    def log_message(self, _format: str, *_args: object) -> None:
        # Query strings contain authorization codes. Never put one in logs.
        return


def callback_server(app: Application, attempt: Attempt, *, timeout: float = 180.0) -> _CallbackServer:
    parsed = urlparse(app.redirect_uri)
    port = parsed.port
    if port is None or port <= 0:
        raise Refuse("Connect redirect URI has no usable loopback port")
    try:
        server = _CallbackServer(("127.0.0.1", port), _CallbackHandler)
    except OSError as exc:
        raise Refuse(f"cannot bind the Connect callback on 127.0.0.1:{port}") from exc
    server.expected_state = attempt.state
    server.result = None
    server.refusal = None
    server.timeout = timeout
    return server


def exchange_code(
    app: Application,
    attempt: Attempt,
    code: str,
    *,
    transport: Transport | None = None,
) -> str:
    fields = {
        "grant_type": "authorization_code",
        "client_id": app.client_id,
        "code": code.strip(),
        "redirect_uri": app.redirect_uri,
        "code_verifier": attempt.verifier,
    }
    if not fields["code"]:
        raise Refuse("authorization code is empty")
    body = urlencode(fields).encode()
    status, raw = (transport or _transport)(
        "POST",
        f"{app.issuer}/oauth2/token",
        {"content-type": "application/x-www-form-urlencoded", "accept": "application/json"},
        body,
    )
    if status >= 400:
        raise Refuse(f"WorkOS Connect token exchange refused with HTTP {status}")
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise Refuse("WorkOS Connect token endpoint returned non-JSON") from exc
    token = str(payload.get("access_token") or "").strip()
    if not token or str(payload.get("token_type") or "").lower() != "bearer":
        raise Refuse("WorkOS Connect token endpoint returned no bearer access token")
    return token


def authorize(
    app: Application,
    *,
    launch: Launch = webbrowser.open,
    transport: Transport | None = None,
    timeout: float = 180.0,
) -> str:
    """Authorize once and return a bearer in memory without printing it."""
    attempt = new_attempt()
    with callback_server(app, attempt, timeout=timeout) as server:
        launched = launch(authorization_url(app, attempt))
        if launched is False:
            raise Refuse("could not open the WorkOS authorization URL")
        server.handle_request()
        if server.refusal is not None:
            raise server.refusal
        if server.result is None:
            raise Refuse("WorkOS Connect authorization timed out before the callback")
        return exchange_code(app, attempt, server.result.code, transport=transport)


def _transport(
    method: str, url: str, headers: Mapping[str, str], body: bytes | None
) -> tuple[int, str]:
    try:
        with urlopen(Request(url, data=body, headers=dict(headers), method=method), timeout=30) as response:
            return response.status, response.read().decode("utf-8")
    except Exception as exc:
        raise Refuse(f"WorkOS Connect request failed: {exc}") from exc


if __name__ == "__main__":
    raise SystemExit(
        "import connect/oauth.py and pass authorize(...) directly to an app; tokens are never printed"
    )
