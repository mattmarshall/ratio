#!/usr/bin/env python3
from __future__ import annotations

import base64
import json
import os
import pathlib
import sys
import tempfile
import unittest
from decimal import Decimal
from urllib.parse import parse_qs, urlparse

import bills
import mapper
import personal_activation as activation
import plaid
import plaid_link

DOC = pathlib.Path(sys.argv[1])
sys.argv = sys.argv[:1]


def jwt(subject: str) -> str:
    encoded = base64.urlsafe_b64encode(
        json.dumps({"sub": subject}).encode()
    ).decode().rstrip("=")
    return f"header.{encoded}.signature"


class RatioTransport:
    def __init__(self, *, kind: str = "PERSONAL", name: str = "books/home"):
        self.kind = kind
        self.name = name
        self.calls = []

    def __call__(self, method, url, headers, body):
        self.calls.append((method, url, headers, body))
        return 200, json.dumps({"name": self.name, "kind": self.kind})


class Replies:
    def __init__(self, *replies):
        self.replies = list(replies)
        self.calls = []

    def __call__(self, *arguments):
        self.calls.append(arguments)
        reply = self.replies.pop(0)
        if isinstance(reply, Exception):
            raise reply
        return reply


def plaid_page(*, added=(), cursor="next"):
    return json.dumps(
        {
            "added": list(added),
            "modified": [],
            "removed": [],
            "next_cursor": cursor,
            "has_more": False,
        },
        default=str,
    )


def transaction(identifier: str, *, pending: bool, pending_id=None):
    row = {
        "transaction_id": identifier,
        "account_id": "checking",
        "date": "2026-09-01",
        "amount": Decimal("12.34"),
        "iso_currency_code": "USD",
        "name": "Merchant",
        "pending": pending,
    }
    if pending_id is not None:
        row["pending_transaction_id"] = pending_id
    return row


def calendar_page():
    return json.dumps(
        {
            "items": [
                {
                    "id": "occurrence-1",
                    "etag": '"one"',
                    "status": "confirmed",
                    "start": {"date": "2026-10-01"},
                    "extendedProperties": {
                        "private": {
                            "ratio_amount": "55.00",
                            "ratio_currency": "USD",
                            "ratio_kind": "bill",
                        }
                    },
                }
            ],
            "nextSyncToken": "sync-new",
        }
    )


class ActivationTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = pathlib.Path(self.temp.name) / "providers.json"
        self.key = bytes(range(32))
        self.membership = activation.Membership("user_123", "books/home")
        self.old_api = os.environ.get("RATIO_CONNECT_API_URL")
        os.environ["RATIO_CONNECT_API_URL"] = "https://connect.example.test"
        self.addCleanup(self._restore_environment)

    def _restore_environment(self):
        if self.old_api is None:
            os.environ.pop("RATIO_CONNECT_API_URL", None)
        else:
            os.environ["RATIO_CONNECT_API_URL"] = self.old_api

    def vault(self):
        return activation.TokenVault(self.path, self.key)

    def runner(self, *, membership=None, token=None, ratio=None, clock=lambda: 1000):
        return activation.PersonalActivation(
            membership=membership or self.membership,
            workos_access_token=token or jwt((membership or self.membership).subject),
            vault=self.vault(),
            ratio_transport=ratio or RatioTransport(),
            clock=clock,
        )

    def test_custody_is_authenticated_durable_and_isolated_by_membership(self):
        vault = self.vault()
        secret = {
            "access_token": "plaid-access-secret",
            "item_id": "item-secret",
            "cursor": "",
            "pending": {},
        }
        vault.put(self.membership, "plaid", secret)
        raw = self.path.read_text()
        self.assertNotIn("user_123", raw)
        self.assertNotIn("books/home", raw)
        self.assertNotIn("plaid-access-secret", raw)
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(
            activation.TokenVault(self.path, self.key)._get(
                self.membership, "plaid"
            ),
            secret,
        )
        other = activation.Membership("user_456", "books/home")
        self.assertIsNone(vault._get(other, "plaid"))
        vault.put(other, "plaid", {**secret, "access_token": "other-secret"})
        self.assertEqual(
            vault._get(self.membership, "plaid")["access_token"],
            "plaid-access-secret",
        )

        document = json.loads(raw)
        envelope = document["memberships"][self.membership.binding]["plaid"]
        envelope["metadata"]["pending_count"] = 99
        self.path.write_text(json.dumps(document))
        with self.assertRaisesRegex(activation.Refuse, "status failed authentication"):
            vault.metadata(self.membership, "plaid")

        document = json.loads(raw)
        envelope = document["memberships"][self.membership.binding]["plaid"]
        ciphertext = bytearray(base64.urlsafe_b64decode(envelope["ciphertext"]))
        ciphertext[0] ^= 1
        envelope["ciphertext"] = base64.urlsafe_b64encode(ciphertext).decode()
        self.path.write_text(json.dumps(document))
        with self.assertRaisesRegex(activation.Refuse, "authentication"):
            vault._get(self.membership, "plaid")

    def test_workos_subject_book_and_personal_kind_are_checked_before_activation(self):
        with self.assertRaisesRegex(activation.Refuse, "subject"):
            self.runner(token=jwt("other"))
        with self.assertRaisesRegex(activation.Refuse, "Personal"):
            self.runner(ratio=RatioTransport(kind="PROJECT"))
        with self.assertRaisesRegex(activation.Refuse, "different book"):
            self.runner(ratio=RatioTransport(name="books/other"))

    def test_plaid_completion_sync_restart_pending_transition_and_disconnect(self):
        link_transport = Replies(
            (200, '{"link_token":"link-safe"}'),
            (200, '{"item_id":"item-1","access_token":"plaid-secret"}'),
        )
        link = plaid_link.PlaidLink(
            client_id="plaid-client",
            secret="plaid-client-secret",
            transport=link_transport,
        )
        runner = self.runner()
        session = runner.begin_plaid(link)
        item_id = runner.complete_plaid(
            link,
            state=session.state,
            public_token="public-once",
            expected_item_id="item-1",
        )
        self.assertEqual(item_id, "item-1")
        self.assertNotIn("plaid-secret", repr(runner.plaid_status()))

        pending_transport = Replies(
            (200, plaid_page(added=[transaction("pending-1", pending=True)], cursor="c1"))
        )
        first = runner.sync_plaid(
            link=link,
            provider_transport=pending_transport,
            choices={},
            ratio_client=mapper.Client(
                "client_ratio_bank_feed",
                mapper.PERSONAL_SEEDED_RULES,
                mapper.CANONICAL_SCOPES,
            ),
            book=mapper.Book("PERSONAL"),
        )
        self.assertEqual(first.proposed, ())
        self.assertEqual(runner.plaid_status().pending_count, 1)

        restarted = self.runner()
        with self.assertRaisesRegex(activation.Refuse, "original cursor"):
            restarted.sync_plaid(
                link=link,
                provider_transport=Replies(
                    (
                        400,
                        json.dumps(
                            {
                                "error_code": "TRANSACTIONS_SYNC_MUTATION_DURING_PAGINATION"
                            }
                        ),
                    )
                ),
                choices={},
                ratio_client=mapper.Client(
                    "client_ratio_bank_feed",
                    mapper.PERSONAL_SEEDED_RULES,
                    mapper.CANONICAL_SCOPES,
                ),
                book=mapper.Book("PERSONAL"),
            )
        self.assertEqual(
            self.vault()._get(self.membership, "plaid")["cursor"], "c1"
        )
        posted_transport = Replies(
            (
                200,
                plaid_page(
                    added=[
                        transaction(
                            "posted-1", pending=False, pending_id="pending-1"
                        )
                    ],
                    cursor="c2",
                ),
            ),
            (200, "{}"),
        )
        second = restarted.sync_plaid(
            link=link,
            provider_transport=posted_transport,
            choices={"posted-1": plaid.RuleChoice("expense")},
            ratio_client=mapper.Client(
                "client_ratio_bank_feed",
                mapper.PERSONAL_SEEDED_RULES,
                mapper.CANONICAL_SCOPES,
            ),
            book=mapper.Book("PERSONAL"),
        )
        self.assertEqual(second.cursor, "c2")
        self.assertEqual(second.resolved_pending_ids, ("pending-1",))
        self.assertEqual(restarted.plaid_status().pending_count, 0)
        self.assertEqual(posted_transport.calls[0][1]["cursor"], "c1")
        restarted.disconnect_plaid(
            link=link, provider_transport=posted_transport
        )
        self.assertFalse(restarted.plaid_status().connected)

    def test_provider_completion_refuses_the_wrong_state_subject_or_book(self):
        link_transport = Replies((200, '{"link_token":"link-safe"}'))
        link = plaid_link.PlaidLink(
            client_id="id", secret="secret", transport=link_transport
        )
        session = link.create(
            workos_subject=self.membership.subject, book=self.membership.book
        )
        wrong = activation.Membership("user_123", "books/other")
        with self.assertRaisesRegex(activation.Refuse, "unknown or mismatched"):
            self.runner(
                membership=wrong,
                ratio=RatioTransport(name="books/other"),
            ).complete_plaid(
                link, state=session.state, public_token="public", expected_item_id=None,
            )
        self.assertEqual(len(link_transport.calls), 1)

        google_transport = Replies()
        google = activation.GoogleOAuth(
            client_id="google-id",
            client_secret="google-secret",
            transport=google_transport,
            clock=lambda: 1000,
        )
        attempt = google.begin(self.membership)
        with self.assertRaisesRegex(activation.Refuse, "state, subject, or book"):
            google.complete(
                f"/google/callback?code=code&state={attempt.state}", wrong
            )
        self.assertEqual(google_transport.calls, [])

    def test_google_grant_refresh_expired_sync_recovery_restart_and_revoke(self):
        oauth_transport = Replies(
            (
                200,
                json.dumps(
                    {
                        "access_token": "google-access-old",
                        "refresh_token": "google-refresh-secret",
                        "expires_in": 10,
                        "token_type": "Bearer",
                        "scope": "https://www.googleapis.com/auth/calendar.events.readonly",
                    }
                ),
            ),
            (
                200,
                json.dumps(
                    {
                        "access_token": "google-access-new",
                        "expires_in": 3600,
                        "token_type": "Bearer",
                        "scope": "https://www.googleapis.com/auth/calendar.events.readonly",
                    }
                ),
            ),
            (200, ""),
        )
        google = activation.GoogleOAuth(
            client_id="google-id",
            client_secret="google-secret",
            transport=oauth_transport,
            clock=lambda: 1000,
        )
        attempt = google.begin(self.membership)
        query = parse_qs(urlparse(attempt.authorization_url).query)
        self.assertEqual(query["redirect_uri"], [activation.GOOGLE_REDIRECT_URI])
        self.assertEqual(query["scope"], ["https://www.googleapis.com/auth/calendar.events.readonly"])
        self.assertEqual(query["access_type"], ["offline"])
        runner = self.runner()
        runner.complete_google(
            google,
            callback_target=f"/google/callback?code=one-time&state={attempt.state}",
            calendar_id="household@example.com",
        )
        self.assertNotIn("google-access-old", self.path.read_text())
        self.assertNotIn("google-refresh-secret", self.path.read_text())

        # Seed an expired incremental cursor. A 410 retries a full sync while
        # preserving the prior seen index, then commits only the successful cut.
        record = self.vault()._get(self.membership, "google")
        record["sync_token"] = "expired-sync"
        record["seen"] = {"already-imported": '"old"'}
        self.vault().put(self.membership, "google", record)
        provider = Replies((410, "sensitive provider body"), (200, calendar_page()))
        restarted = self.runner()
        result = restarted.sync_google(
            google_oauth=google,
            provider_transport=provider,
            ratio_client=bills.Client(
                "client_ratio_calendar_bills",
                frozenset(bills.SCHEDULED_LEGS),
                bills.CANONICAL_SCOPES,
            ),
            book=bills.Book("PERSONAL"),
        )
        self.assertEqual(result.sync_token, "sync-new")
        self.assertIn("syncToken=expired-sync", provider.calls[0][0])
        self.assertNotIn("syncToken=", provider.calls[1][0])
        self.assertEqual(restarted.google_status().seen_count, 2)
        refresh_fields = parse_qs(oauth_transport.calls[1][3].decode())
        self.assertEqual(refresh_fields["refresh_token"], ["google-refresh-secret"])

        restarted.disconnect_google(google)
        self.assertFalse(restarted.google_status().connected)
        revoke_fields = parse_qs(oauth_transport.calls[2][3].decode())
        self.assertEqual(revoke_fields["token"], ["google-refresh-secret"])

    def test_failed_sync_and_failed_revoke_keep_retry_state(self):
        self.vault().put(
            self.membership,
            "google",
            {
                "access_token": "access",
                "refresh_token": "refresh",
                "expires_at": 5000,
                "calendar_id": "calendar",
                "sync_token": "old",
                "seen": {},
            },
        )
        runner = self.runner()
        google = activation.GoogleOAuth(
            client_id="id",
            client_secret="secret",
            transport=Replies((500, "do not echo refresh")),
            clock=lambda: 1000,
        )
        with self.assertRaises(activation.Refuse):
            runner.sync_google(
                google_oauth=google,
                provider_transport=Replies((500, "provider body")),
                ratio_client=bills.Client(
                    "client_ratio_calendar_bills",
                    frozenset(bills.SCHEDULED_LEGS),
                    bills.CANONICAL_SCOPES,
                ),
                book=bills.Book("PERSONAL"),
            )
        self.assertTrue(runner.google_status().sync_token_present)
        with self.assertRaisesRegex(activation.Refuse, "HTTP 500"):
            runner.disconnect_google(google)
        self.assertTrue(runner.google_status().connected)

    def test_production_contract_names_exact_callbacks_scopes_and_secrets(self):
        text = DOC.read_text()
        for required in (
            "http://127.0.0.1:8765/callback",
            "http://127.0.0.1:8766/google/callback",
            "https://ratio.marsh.build/connect/plaid",
            "https://www.googleapis.com/auth/calendar.events.readonly",
            "WORKOS_CONNECT_CLIENT_ID",
            "RATIO_CONNECT_API_URL",
            "PLAID_CLIENT_ID",
            "PLAID_SECRET",
            "PLAID_ENV",
            "PLAID_REDIRECT_URI",
            "GOOGLE_OAUTH_CLIENT_ID",
            "GOOGLE_OAUTH_CLIENT_SECRET",
            "RATIO_PERSONAL_TOKEN_KEY",
            "RATIO_PERSONAL_TOKEN_STORE",
        ):
            self.assertIn(required, text)
        self.assertIn("no client secret", text)


if __name__ == "__main__":
    unittest.main()
