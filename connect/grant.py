#!/usr/bin/env python3
"""Shared Connect grant path — first-party apps call ConnectApiUrl.

A verified WorkOS Connect access token pulls cites and delivers
against the Connect HTTP API. Membership is still required at `/v1`.
A Connect-shaped token never takes `RATIO_DEMO_OPEN` and never
matches `org:{id}` (#151).

This is not a second IdP. Token mint is WorkOS Connect
(`authorization_code` or M2M `client_credentials`) against
`WORKOS_CONNECT_ISSUER`. AuthKit session tokens stay on DemoUrl.

⭐ ENV NAMES (wired; do not invent a second issuer):

| Name | What |
|---|---|
| `RATIO_CONNECT_API_URL` | ConnectApiUrl (CloudFormation output). Never DemoUrl. |
| `RATIO_API_ORIGIN` | DemoUrl. Read only to refuse a collision. |
| `WORKOS_CONNECT_ISSUER` | AuthKit custom domain. Default `https://auth.ratio.marsh.build`. |
| `WORKOS_CLIENT_ID` | Audience — the Ratio WorkOS project client. Not a Connect-app credential. |
| `WORKOS_CONNECT_CLIENT_ID` | First-party Connect application `client_id` (Dashboard). |
| `WORKOS_CONNECT_CLIENT_SECRET` | Matching Connect application secret. |
| `WORKOS_CONNECT_REDIRECT_URI` | `authorization_code` callback; must match Dashboard exactly. |
| `RATIO_CONNECT_ACCESS_TOKEN` | Already-minted Connect access token (skip the exchange). |
| `RATIO_CONNECT_BOOK` | Optional book id when a caller does not pass one. |

⛔ NEVER READ `RATIO_DEMO_OPEN`. That dial is AuthKit-session only.
⛔ NEVER TREAT `org:{id}` AS MEMBERSHIP. The kernel still decides.

Leftover on issue 22: WorkOS dashboard registration, `DEMO_MEMBERS`
naming a live WorkOS `sub`, unused Cognito CloudFormation resources.
Bank / calendar OAuth product UI stay refused on those apps.
"""

from __future__ import annotations

import json
import os
import urllib.error
import urllib.parse
import urllib.request
from typing import Any, Callable, Mapping
from urllib.request import Request

DEFAULT_CONNECT_ISSUER = "https://auth.ratio.marsh.build"

# method, url, headers, body → (status, response body)
Transport = Callable[[str, str, Mapping[str, str], bytes | None], tuple[int, str]]


class Refuse(Exception):
    """The grant does not open. Message is the reason, not a workaround."""


class FakeTransport:
    """In-process stand-in so a unit test does not hit the network.

    A green helper is not a live walk-through. Dashboard registration
    stays leftover on issue 22.
    """

    def __init__(
        self,
        *,
        status: int = 200,
        body: str = '{"ok":true}',
        responses: Mapping[str, tuple[int, str]] | None = None,
    ) -> None:
        self.default = (status, body)
        self.responses = dict(responses or {})
        self.calls: list[tuple[str, str, dict[str, str], bytes | None]] = []

    def __call__(
        self,
        method: str,
        url: str,
        headers: Mapping[str, str],
        body: bytes | None,
    ) -> tuple[int, str]:
        self.calls.append((method, url, dict(headers), body))
        for needle, resp in self.responses.items():
            if needle in url:
                return resp
        return self.default


def _env(name: str) -> str:
    return os.environ.get(name, "").strip()


def connect_issuer() -> str:
    """WorkOS Connect issuer — AuthKit custom domain, not `/user_management`."""
    raw = _env("WORKOS_CONNECT_ISSUER") or DEFAULT_CONNECT_ISSUER
    return raw.rstrip("/")


def connect_api_url(*, error: type[Exception] = Refuse) -> str:
    """ConnectApiUrl. DemoUrl is refused even when it looks like a `/v1` host."""
    url = _env("RATIO_CONNECT_API_URL").rstrip("/")
    if not url:
        raise error(
            "RATIO_CONNECT_API_URL is unset — first-party Connect apps "
            "call ConnectApiUrl, not DemoUrl. The grant path is built; "
            "the live URL is the leftover (WorkOS dashboard registration "
            "on leftover #22)"
        )
    demo = _env("RATIO_API_ORIGIN").rstrip("/")
    if demo and url == demo:
        raise error(
            "RATIO_CONNECT_API_URL must be ConnectApiUrl, not DemoUrl "
            "(RATIO_API_ORIGIN). AuthKit session tokens stay on DemoUrl; "
            "Connect tokens stay on ConnectApiUrl"
        )
    return url


def book_path(book_id: str | None = None) -> str:
    """`/v1/books` or `/v1/books/{id}`. Empty id lists; it does not invent a book."""
    named = (book_id or _env("RATIO_CONNECT_BOOK")).strip().strip("/")
    if not named:
        return "/v1/books"
    if named.startswith("v1/"):
        named = named[3:]
    if named.startswith("books/") or named.startswith("funds/"):
        return f"/v1/{named}"
    return f"/v1/books/{named}"


def apply_event_path(parent: str) -> str:
    """`/v1/{parent}:applyEvent`. Parent is `funds/{id}`, never an org grant."""
    named = parent.strip().strip("/")
    if named.startswith("v1/"):
        named = named[3:]
    if named.endswith(":applyEvent"):
        return f"/v1/{named}" if not named.startswith("v1/") else f"/{named}"
    if not named.startswith("funds/"):
        named = f"funds/{named}"
    return f"/v1/{named}:applyEvent"


def access_token(
    token: str | None = None,
    *,
    code: str | None = None,
    grant_type: str | None = None,
    scopes: str | None = None,
    transport: Transport | None = None,
    error: type[Exception] = Refuse,
) -> str:
    """Present a Connect access token, or mint one against WorkOS Connect.

    Precedence: explicit `token`, then `RATIO_CONNECT_ACCESS_TOKEN`,
    then `authorization_code` (when `code` is set) or
    `client_credentials`. Missing credentials is not "the grant path
    is not built" — the door is open; the token was not presented.
    """
    presented = (token or "").strip() or _env("RATIO_CONNECT_ACCESS_TOKEN")
    if presented:
        return presented
    return mint_token(
        code=code,
        grant_type=grant_type,
        scopes=scopes,
        transport=transport,
        error=error,
    )


def mint_token(
    *,
    code: str | None = None,
    grant_type: str | None = None,
    scopes: str | None = None,
    transport: Transport | None = None,
    error: type[Exception] = Refuse,
) -> str:
    """POST `{WORKOS_CONNECT_ISSUER}/oauth2/token`. Not a second IdP."""
    client_id = _env("WORKOS_CONNECT_CLIENT_ID")
    client_secret = _env("WORKOS_CONNECT_CLIENT_SECRET")
    if not client_id or not client_secret:
        raise error(
            "no Connect access token — present a verified token, set "
            "RATIO_CONNECT_ACCESS_TOKEN, or set WORKOS_CONNECT_CLIENT_ID "
            "/ WORKOS_CONNECT_CLIENT_SECRET (WorkOS Connect application "
            "credentials from the Dashboard, not a second IdP). The "
            "grant path is built; WorkOS dashboard registration stays "
            "leftover #22"
        )
    kind = (grant_type or "").strip() or (
        "authorization_code" if code else "client_credentials"
    )
    if kind not in ("authorization_code", "client_credentials"):
        raise error(
            "grant_type must be authorization_code or client_credentials "
            "against WorkOS Connect — do not invent a second IdP"
        )
    fields: dict[str, str] = {
        "grant_type": kind,
        "client_id": client_id,
        "client_secret": client_secret,
    }
    if kind == "authorization_code":
        if not (code or "").strip():
            raise error(
                "authorization_code grant needs the callback code — "
                "WorkOS dashboard registration and redirect stay leftover #22"
            )
        redirect = _env("WORKOS_CONNECT_REDIRECT_URI")
        if not redirect:
            raise error(
                "WORKOS_CONNECT_REDIRECT_URI is unset — it must match "
                "the Connect application redirect in the WorkOS Dashboard"
            )
        fields["code"] = code.strip()
        fields["redirect_uri"] = redirect
    if scopes:
        fields["scope"] = scopes
    url = f"{connect_issuer()}/oauth2/token"
    body = urllib.parse.urlencode(fields).encode()
    headers = {"content-type": "application/x-www-form-urlencoded"}
    status, raw = (transport or _urllib_transport)("POST", url, headers, body)
    if status >= 400:
        raise error(
            f"WorkOS Connect token endpoint refused ({status}) — "
            "membership is still required at /v1 and a Connect token "
            "never takes RATIO_DEMO_OPEN or org:{{id}}"
        )
    try:
        payload = json.loads(raw) if raw else {}
    except json.JSONDecodeError as exc:
        raise error("WorkOS Connect token endpoint returned non-JSON") from exc
    minted = str(payload.get("access_token") or "").strip()
    if not minted:
        raise error("WorkOS Connect token endpoint returned no access_token")
    return minted


def request(
    method: str,
    path: str,
    *,
    token: str | None = None,
    body: Any = None,
    transport: Transport | None = None,
    error: type[Exception] = Refuse,
) -> Any:
    """HTTP against ConnectApiUrl. Bearer only — no DemoUrl, no open dial."""
    bearer = access_token(token, transport=transport, error=error)
    origin = connect_api_url(error=error)
    suffix = path if path.startswith("/") else f"/{path}"
    if not suffix.startswith("/v1/"):
        suffix = f"/v1/{suffix.lstrip('/')}"
    url = f"{origin}{suffix}"
    raw: bytes | None = None
    headers = {
        "authorization": f"Bearer {bearer}",
        "accept": "application/json",
    }
    if body is not None:
        raw = json.dumps(body).encode()
        headers["content-type"] = "application/json"
    status, text = (transport or _urllib_transport)(method.upper(), url, headers, raw)
    if status in (401, 403):
        raise error(
            f"ConnectApiUrl {suffix} returned {status} — membership is "
            "still required. A Connect token never takes RATIO_DEMO_OPEN "
            "and never matches org:{id} (#151)"
        )
    if status >= 400:
        raise error(f"ConnectApiUrl {suffix} returned {status}")
    if not text:
        return {}
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return {"raw": text}


def pull(
    *,
    token: str | None = None,
    book_id: str | None = None,
    path: str | None = None,
    transport: Transport | None = None,
    error: type[Exception] = Refuse,
) -> Any:
    """GET cites from ConnectApiUrl. Default path is `/v1/books`."""
    return request(
        "GET",
        path or book_path(book_id),
        token=token,
        transport=transport,
        error=error,
    )


def push(
    *,
    token: str | None = None,
    path: str,
    body: Any,
    transport: Transport | None = None,
    error: type[Exception] = Refuse,
) -> Any:
    """POST a body to ConnectApiUrl. Membership still required."""
    return request(
        "POST",
        path,
        token=token,
        body=body,
        transport=transport,
        error=error,
    )


def deliver_apply_events(
    posts: Any,
    *,
    as_apply_event: Callable[..., dict[str, Any]],
    token: str | None = None,
    parent: str | None = None,
    transport: Transport | None = None,
    error: type[Exception] = Refuse,
) -> list[Any]:
    """POST each proposed ApplyEvent to ConnectApiUrl `/v1/funds/{id}:applyEvent`."""
    named = (parent or _env("RATIO_CONNECT_BOOK")).strip()
    if not named:
        raise error(
            "parent (funds/{id}) is required to deliver ApplyEvent "
            "against ConnectApiUrl — set RATIO_CONNECT_BOOK or pass parent"
        )
    path = apply_event_path(named)
    results = []
    for post in posts:
        results.append(
            push(
                token=token,
                path=path,
                body=as_apply_event(post, parent=named),
                transport=transport,
                error=error,
            )
        )
    return results


def _urllib_transport(
    method: str,
    url: str,
    headers: Mapping[str, str],
    body: bytes | None,
) -> tuple[int, str]:
    req = Request(url, data=body, method=method, headers=dict(headers))
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return resp.status, resp.read().decode()
    except urllib.error.HTTPError as exc:
        return exc.code, exc.read().decode() if exc.fp else ""
