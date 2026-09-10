#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import pathlib
import sys
import threading
import urllib.parse
import urllib.request
import unittest


OAUTH = pathlib.Path(sys.argv[1])
MANIFESTS = [pathlib.Path(path) for path in sys.argv[2:]]
spec = importlib.util.spec_from_file_location("ratio_connect_oauth", OAUTH)
assert spec and spec.loader
oauth = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = oauth
spec.loader.exec_module(oauth)


class OAuthTest(unittest.TestCase):
    def app(self, manifest: pathlib.Path | None = None, **overrides):
        return oauth.application_from_manifest(
            manifest or MANIFESTS[0],
            client_id=overrides.get("client_id", "client_personal_test"),
            issuer=overrides.get("issuer", oauth.DEFAULT_ISSUER),
            redirect_uri=overrides.get("redirect_uri", oauth.DEFAULT_REDIRECT_URI),
            allow_loopback_issuer=overrides.get("allow_loopback_issuer", False),
        )

    def test_every_personal_app_declares_one_public_pkce_shape_and_exact_scopes(self):
        for manifest in MANIFESTS:
            raw = json.loads(manifest.read_text())
            connect = raw["workos_connect"]
            app = self.app(manifest)
            self.assertTrue(connect["public_client"], manifest)
            self.assertTrue(connect["uses_pkce"], manifest)
            self.assertEqual(connect["redirect_uri"], oauth.DEFAULT_REDIRECT_URI, manifest)
            self.assertEqual(list(app.scopes), connect["scopes"], manifest)

    def test_authorization_url_uses_state_nonce_pkce_and_only_protocol_plus_manifest_scopes(self):
        app = self.app()
        attempt = oauth.Attempt("state-secret", "nonce-secret", "verifier-secret", "challenge")
        parsed = urllib.parse.urlparse(oauth.authorization_url(app, attempt))
        q = urllib.parse.parse_qs(parsed.query)
        self.assertEqual(parsed.path, "/oauth2/authorize")
        self.assertEqual(q["client_id"], [app.client_id])
        self.assertEqual(q["redirect_uri"], [oauth.DEFAULT_REDIRECT_URI])
        self.assertEqual(q["state"], [attempt.state])
        self.assertEqual(q["nonce"], [attempt.nonce])
        self.assertEqual(q["code_challenge"], [attempt.challenge])
        self.assertEqual(q["code_challenge_method"], ["S256"])
        self.assertEqual(q["scope"][0].split(), ["openid", *app.scopes])
        self.assertNotIn("offline_access", q["scope"][0])

    def test_pkce_challenge_is_the_unpadded_sha256_of_the_verifier(self):
        attempt = oauth.new_attempt()
        expected = oauth.base64.urlsafe_b64encode(
            oauth.hashlib.sha256(attempt.verifier.encode()).digest()
        ).decode().rstrip("=")
        self.assertEqual(attempt.challenge, expected)
        self.assertNotIn("=", attempt.challenge)
        self.assertGreaterEqual(len(attempt.verifier), 43)

    def test_callback_requires_exact_path_state_and_one_code(self):
        got = oauth.parse_callback("/callback?code=one&state=right", expected_state="right")
        self.assertEqual(got.code, "one")
        for target, message in [
            ("/other?code=one&state=right", "wrong path"),
            ("/callback?code=one&state=wrong", "does not match"),
            ("/callback?state=right", "exactly one"),
            ("/callback?code=one&code=two&state=right", "exactly one"),
            ("/callback?error=access_denied&state=right", "access_denied"),
        ]:
            with self.subTest(target=target), self.assertRaisesRegex(oauth.Refuse, message):
                oauth.parse_callback(target, expected_state="right")

    def test_exchange_is_public_pkce_and_never_sends_a_secret(self):
        app = self.app()
        attempt = oauth.Attempt("state", "nonce", "verifier", "challenge")
        calls = []

        def transport(method, url, headers, body):
            calls.append((method, url, headers, urllib.parse.parse_qs(body.decode())))
            return 200, '{"access_token":"bearer-value","token_type":"Bearer","refresh_token":"ignored"}'

        self.assertEqual(oauth.exchange_code(app, attempt, "auth-code", transport=transport), "bearer-value")
        fields = calls[0][3]
        self.assertEqual(fields["code_verifier"], ["verifier"])
        self.assertEqual(fields["redirect_uri"], [oauth.DEFAULT_REDIRECT_URI])
        self.assertNotIn("client_secret", fields)
        self.assertNotIn("refresh_token", fields)

    def test_token_response_must_be_json_with_a_bearer(self):
        app = self.app()
        attempt = oauth.Attempt("state", "nonce", "verifier", "challenge")
        cases = [
            ((400, "{}"), "HTTP 400"),
            ((200, "not-json"), "non-JSON"),
            ((200, '{"token_type":"Bearer"}'), "no bearer"),
            ((200, '{"access_token":"x","token_type":"MAC"}'), "no bearer"),
        ]
        for response, message in cases:
            with self.subTest(response=response), self.assertRaisesRegex(oauth.Refuse, message):
                oauth.exchange_code(app, attempt, "code", transport=lambda *_: response)

    def test_issuer_and_redirect_refuse_confused_or_remote_origins(self):
        for issuer in ["http://auth.example.test", "https://*.example.test", "https://u:p@example.test"]:
            with self.subTest(issuer=issuer), self.assertRaises(oauth.Refuse):
                oauth.validate_issuer(issuer)
        self.assertEqual(
            oauth.validate_issuer("http://127.0.0.1:9999", allow_loopback_http=True),
            "http://127.0.0.1:9999",
        )
        for redirect in [
            "https://127.0.0.1:8765/callback",
            "http://localhost:8765/callback",
            "http://127.0.0.1:8765/other",
            "http://127.0.0.1:8765/callback?next=x",
            "http://*.example.test/callback",
        ]:
            with self.subTest(redirect=redirect), self.assertRaises(oauth.Refuse):
                oauth.validate_redirect(redirect)

    def test_loopback_server_receives_one_callback_and_does_not_log_the_code(self):
        app = self.app()
        attempt = oauth.Attempt("right-state", "nonce", "verifier", "challenge")
        with oauth.callback_server(app, attempt, timeout=2) as server:
            thread = threading.Thread(target=server.handle_request)
            thread.start()
            with urllib.request.urlopen(
                oauth.DEFAULT_REDIRECT_URI + "?code=sensitive-code&state=right-state"
            ) as response:
                self.assertEqual(response.status, 200)
                self.assertNotIn(b"sensitive-code", response.read())
            thread.join(timeout=2)
            self.assertFalse(thread.is_alive())
            self.assertEqual(server.result.code, "sensitive-code")

    def test_authorize_completes_the_loopback_and_returns_only_the_access_token(self):
        app = self.app()
        callback_threads = []

        def launch(url):
            query = urllib.parse.parse_qs(urllib.parse.urlparse(url).query)

            def complete():
                target = (
                    oauth.DEFAULT_REDIRECT_URI
                    + "?code=one-time-code&state="
                    + urllib.parse.quote(query["state"][0])
                )
                with urllib.request.urlopen(target) as response:
                    self.assertEqual(response.status, 200)

            thread = threading.Thread(target=complete)
            callback_threads.append(thread)
            thread.start()
            return True

        def token_transport(method, url, headers, body):
            fields = urllib.parse.parse_qs(body.decode())
            self.assertEqual(fields["code"], ["one-time-code"])
            self.assertIn("code_verifier", fields)
            self.assertNotIn("client_secret", fields)
            return 200, json.dumps(
                {
                    "access_token": "memory-only-bearer",
                    "refresh_token": "must-not-be-returned",
                    "id_token": "must-not-be-returned",
                    "token_type": "Bearer",
                }
            )

        token = oauth.authorize(app, launch=launch, transport=token_transport, timeout=2)
        for thread in callback_threads:
            thread.join(timeout=2)
        self.assertEqual(token, "memory-only-bearer")


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]])
