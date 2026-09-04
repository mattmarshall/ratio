#!/usr/bin/env python3
"""Properties the shared Connect grant helper must keep.

Test names are sentences. A green helper is not a live walk-through —
WorkOS dashboard registration stays leftover on issue 22.
"""

from __future__ import annotations

import json
import os
import unittest
import urllib.parse
from unittest import mock

import grant as g


def _clear_grant_env() -> dict[str, str]:
    keys = (
        "RATIO_CONNECT_API_URL",
        "RATIO_API_ORIGIN",
        "RATIO_CONNECT_ACCESS_TOKEN",
        "RATIO_CONNECT_BOOK",
        "RATIO_DEMO_OPEN",
        "WORKOS_CONNECT_ISSUER",
        "WORKOS_CLIENT_ID",
        "WORKOS_CONNECT_CLIENT_ID",
        "WORKOS_CONNECT_CLIENT_SECRET",
        "WORKOS_CONNECT_REDIRECT_URI",
    )
    return {k: "" for k in keys}


class TokenPresentation(unittest.TestCase):
    def test_a_missing_token_is_refused_because_no_token_was_presented(self):
        with mock.patch.dict(os.environ, _clear_grant_env(), clear=False):
            with self.assertRaises(g.Refuse) as ctx:
                g.access_token()
        msg = str(ctx.exception)
        self.assertIn("no Connect access token", msg)
        self.assertIn("RATIO_CONNECT_ACCESS_TOKEN", msg)
        self.assertIn("WORKOS_CONNECT_CLIENT_ID", msg)
        self.assertNotIn("grant path is not built", msg)

    def test_an_explicit_token_is_used_without_minting(self):
        with mock.patch.dict(os.environ, _clear_grant_env(), clear=False):
            self.assertEqual(g.access_token("  connect-access-token  "), "connect-access-token")

    def test_RATIO_CONNECT_ACCESS_TOKEN_is_used_when_no_argument(self):
        env = _clear_grant_env()
        env["RATIO_CONNECT_ACCESS_TOKEN"] = "from-env"
        with mock.patch.dict(os.environ, env, clear=False):
            self.assertEqual(g.access_token(), "from-env")

    def test_RATIO_DEMO_OPEN_does_not_mint_or_bypass_a_missing_token(self):
        env = _clear_grant_env()
        env["RATIO_DEMO_OPEN"] = "1"
        with mock.patch.dict(os.environ, env, clear=False):
            with self.assertRaises(g.Refuse) as ctx:
                g.access_token()
        self.assertIn("no Connect access token", str(ctx.exception))
        self.assertNotIn("RATIO_DEMO_OPEN", str(ctx.exception).split("never")[0])

    def test_client_credentials_mints_against_the_connect_issuer(self):
        env = _clear_grant_env()
        env["WORKOS_CONNECT_CLIENT_ID"] = "client_connect_app"
        env["WORKOS_CONNECT_CLIENT_SECRET"] = "secret"
        env["WORKOS_CONNECT_ISSUER"] = "https://auth.ratio.marsh.build"
        transport = g.FakeTransport(body='{"access_token":"minted-m2m"}')
        with mock.patch.dict(os.environ, env, clear=False):
            token = g.access_token(transport=transport)
        self.assertEqual(token, "minted-m2m")
        method, url, headers, body = transport.calls[0]
        self.assertEqual(method, "POST")
        self.assertEqual(url, "https://auth.ratio.marsh.build/oauth2/token")
        self.assertIn("application/x-www-form-urlencoded", headers["content-type"])
        fields = {k: urllib.parse.unquote(v) for k, v in (p.split("=", 1) for p in body.decode().split("&"))}
        self.assertEqual(fields["grant_type"], "client_credentials")
        self.assertEqual(fields["client_id"], "client_connect_app")
        self.assertEqual(fields["client_secret"], "secret")

    def test_authorization_code_needs_the_redirect_and_the_code(self):
        env = _clear_grant_env()
        env["WORKOS_CONNECT_CLIENT_ID"] = "client_connect_app"
        env["WORKOS_CONNECT_CLIENT_SECRET"] = "secret"
        with mock.patch.dict(os.environ, env, clear=False):
            with self.assertRaises(g.Refuse) as ctx:
                g.access_token(grant_type="authorization_code")
        self.assertIn("authorization_code", str(ctx.exception))
        env["WORKOS_CONNECT_REDIRECT_URI"] = "https://app.example/callback"
        transport = g.FakeTransport(body='{"access_token":"minted-user"}')
        with mock.patch.dict(os.environ, env, clear=False):
            token = g.access_token(code="auth-code", transport=transport)
        self.assertEqual(token, "minted-user")
        fields = {k: urllib.parse.unquote(v) for k, v in (p.split("=", 1) for p in transport.calls[0][3].decode().split("&"))}
        self.assertEqual(fields["grant_type"], "authorization_code")
        self.assertEqual(fields["code"], "auth-code")
        self.assertEqual(fields["redirect_uri"], "https://app.example/callback")

    def test_an_invented_grant_type_is_refused(self):
        env = _clear_grant_env()
        env["WORKOS_CONNECT_CLIENT_ID"] = "client_connect_app"
        env["WORKOS_CONNECT_CLIENT_SECRET"] = "secret"
        with mock.patch.dict(os.environ, env, clear=False):
            with self.assertRaises(g.Refuse) as ctx:
                g.access_token(grant_type="password")
        self.assertIn("authorization_code", str(ctx.exception))
        self.assertIn("client_credentials", str(ctx.exception))
        self.assertIn("second IdP", str(ctx.exception))


class ConnectApiUrlFence(unittest.TestCase):
    def test_a_missing_connect_api_url_is_refused(self):
        with mock.patch.dict(os.environ, _clear_grant_env(), clear=False):
            with self.assertRaises(g.Refuse) as ctx:
                g.connect_api_url()
        msg = str(ctx.exception)
        self.assertIn("RATIO_CONNECT_API_URL", msg)
        self.assertIn("ConnectApiUrl", msg)
        self.assertNotIn("grant path is not built", msg)

    def test_demo_url_is_refused_as_the_connect_host(self):
        env = _clear_grant_env()
        env["RATIO_CONNECT_API_URL"] = "https://demo.example"
        env["RATIO_API_ORIGIN"] = "https://demo.example"
        with mock.patch.dict(os.environ, env, clear=False):
            with self.assertRaises(g.Refuse) as ctx:
                g.connect_api_url()
        self.assertIn("not DemoUrl", str(ctx.exception))

    def test_connect_api_url_is_accepted_when_it_is_not_demo_url(self):
        env = _clear_grant_env()
        env["RATIO_CONNECT_API_URL"] = "https://connect.example/"
        env["RATIO_API_ORIGIN"] = "https://demo.example"
        with mock.patch.dict(os.environ, env, clear=False):
            self.assertEqual(g.connect_api_url(), "https://connect.example")


class LivePullAndDeliver(unittest.TestCase):
    def test_pull_sends_bearer_to_connect_api_url_and_not_demo_url(self):
        env = _clear_grant_env()
        env["RATIO_CONNECT_API_URL"] = "https://connect.example"
        env["RATIO_API_ORIGIN"] = "https://demo.example"
        env["RATIO_DEMO_OPEN"] = "1"
        transport = g.FakeTransport(body='{"books":[]}')
        with mock.patch.dict(os.environ, env, clear=False):
            out = g.pull(token="connect-access-token", transport=transport)
        self.assertEqual(out, {"books": []})
        method, url, headers, body = transport.calls[0]
        self.assertEqual(method, "GET")
        self.assertEqual(url, "https://connect.example/v1/books")
        self.assertEqual(headers["authorization"], "Bearer connect-access-token")
        self.assertNotIn("org:", json.dumps(headers))
        self.assertIsNone(body)
        self.assertTrue(all("demo.example" not in call[1] for call in transport.calls))

    def test_pull_of_a_named_book_uses_the_books_path(self):
        env = _clear_grant_env()
        env["RATIO_CONNECT_API_URL"] = "https://connect.example"
        transport = g.FakeTransport(body='{"name":"books/alpha"}')
        with mock.patch.dict(os.environ, env, clear=False):
            g.pull(token="t", book_id="alpha", transport=transport)
        self.assertEqual(transport.calls[0][1], "https://connect.example/v1/books/alpha")

    def test_a_401_is_membership_not_an_open_dial(self):
        env = _clear_grant_env()
        env["RATIO_CONNECT_API_URL"] = "https://connect.example"
        transport = g.FakeTransport(status=401, body="unauthorized")
        with mock.patch.dict(os.environ, env, clear=False):
            with self.assertRaises(g.Refuse) as ctx:
                g.pull(token="t", transport=transport)
        msg = str(ctx.exception)
        self.assertIn("membership", msg)
        self.assertIn("RATIO_DEMO_OPEN", msg)
        self.assertIn("org:{id}", msg)

    def test_deliver_apply_events_posts_to_apply_event_on_connect_api_url(self):
        env = _clear_grant_env()
        env["RATIO_CONNECT_API_URL"] = "https://connect.example"
        transport = g.FakeTransport(body='{"name":"entries/1"}')

        def as_apply_event(post, *, parent):
            return {"parent": parent, "rule_id": post}

        with mock.patch.dict(os.environ, env, clear=False):
            out = g.deliver_apply_events(
                ["living_expense"],
                as_apply_event=as_apply_event,
                token="t",
                parent="funds/alpha",
                transport=transport,
            )
        self.assertEqual(out, [{"name": "entries/1"}])
        method, url, headers, body = transport.calls[0]
        self.assertEqual(method, "POST")
        self.assertEqual(url, "https://connect.example/v1/funds/alpha:applyEvent")
        self.assertEqual(headers["authorization"], "Bearer t")
        self.assertEqual(json.loads(body), {"parent": "funds/alpha", "rule_id": "living_expense"})

    def test_deliver_apply_events_refuses_without_a_parent(self):
        env = _clear_grant_env()
        env["RATIO_CONNECT_API_URL"] = "https://connect.example"
        with mock.patch.dict(os.environ, env, clear=False):
            with self.assertRaises(g.Refuse) as ctx:
                g.deliver_apply_events(
                    ["x"],
                    as_apply_event=lambda p, *, parent: {},
                    token="t",
                )
        self.assertIn("parent", str(ctx.exception))
        self.assertNotIn("grant path is not built", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()
