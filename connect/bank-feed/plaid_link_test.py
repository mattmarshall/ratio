#!/usr/bin/env python3
from __future__ import annotations

import json
import pathlib
import sys
import unittest

import plaid_link as link

sys.argv = sys.argv[:1]
FIXTURES = pathlib.Path(__file__).with_name("testdata")


class Clock:
    def __init__(self, now=1000.0):
        self.now = now

    def __call__(self):
        return self.now


class Transport:
    def __init__(self, replies):
        self.replies = list(replies)
        self.calls = []

    def __call__(self, path, body):
        self.calls.append((path, body))
        reply = self.replies.pop(0)
        if isinstance(reply, Exception):
            raise reply
        return reply


def subject(replies, *, clock=None, seconds=900):
    transport = Transport(replies)
    client = link.PlaidLink(client_id="client-id", secret="plaid-secret",
                            transport=transport, clock=clock or Clock(),
                            session_seconds=seconds)
    return client, transport


class PlaidLink(unittest.TestCase):
    def test_link_creation_is_minimal_and_hashes_the_membership_identity(self):
        client, transport = subject([(200, (FIXTURES / "link-create.json").read_text())])
        session = client.create(workos_subject="user_123@example.com", book="books/home",
                                country_codes=("US",), language="en",
                                redirect_uri="https://ratio.marsh.build/connect/plaid")
        path, body = transport.calls[0]
        self.assertEqual(path, link.LINK_CREATE_PATH)
        self.assertEqual(body["products"], ["transactions"])
        self.assertEqual(body["country_codes"], ["US"])
        self.assertTrue(body["user"]["client_user_id"].startswith("ratio-"))
        self.assertNotIn("user_123", json.dumps(body))
        self.assertNotIn("books/home", json.dumps(body))
        self.assertEqual(session.link_token, "link-sandbox-fixture")
        self.assertNotEqual(session.state, session.link_token)

    def test_success_exchanges_once_and_keeps_item_token_server_only(self):
        client, transport = subject([
            (200, (FIXTURES / "link-create.json").read_text()),
            (200, (FIXTURES / "token-exchange.json").read_text()),
        ])
        session = client.create(workos_subject="user", book="books/home")
        item = client.complete(state=session.state, workos_subject="user", book="books/home",
                               public_token="public-once",
                               expected_item_id="item-fixture")
        self.assertEqual(item.item_id, "item-fixture")
        self.assertNotIn("access-fixture", repr(item))
        self.assertEqual(transport.calls[1][0], link.TOKEN_EXCHANGE_PATH)
        sync = client.sync_client(item, transport=lambda _path, _body: (200, "{}"))
        self.assertNotIn("access-fixture", repr(sync))
        with self.assertRaisesRegex(link.Refuse, "unknown or mismatched"):
            client.complete(state=session.state, workos_subject="user", book="books/home",
                            public_token="public-once")

    def test_state_mismatch_expiry_and_cancellation_never_exchange(self):
        clock = Clock()
        client, transport = subject([(200, '{"link_token":"link"}')], clock=clock, seconds=10)
        session = client.create(workos_subject="user", book="book")
        with self.assertRaisesRegex(link.Refuse, "unknown or mismatched"):
            client.complete(state="other", workos_subject="user", book="book",
                            public_token="public")
        clock.now += 11
        with self.assertRaisesRegex(link.Refuse, "expired"):
            client.complete(state=session.state, workos_subject="user", book="book",
                            public_token="public")
        self.assertEqual(len(transport.calls), 1)

        client, transport = subject([(200, '{"link_token":"link"}')])
        session = client.create(workos_subject="user", book="book")
        client.cancel(state=session.state, workos_subject="user", book="book",
                      provider_error="USER_EXIT")
        with self.assertRaisesRegex(link.Refuse, "unknown or mismatched"):
            client.complete(state=session.state, workos_subject="user", book="book",
                            public_token="public")
        self.assertEqual(len(transport.calls), 1)

    def test_a_wrong_item_refuses_after_one_exchange(self):
        client, _ = subject([(200, '{"link_token":"link"}'),
                             (200, '{"access_token":"access","item_id":"other"}')])
        session = client.create(workos_subject="user", book="book")
        with self.assertRaisesRegex(link.Refuse, "different Item"):
            client.complete(state=session.state, workos_subject="user", book="book",
                            public_token="public", expected_item_id="expected")

    def test_pending_sessions_are_bounded_and_expired_slots_are_reclaimed(self):
        clock = Clock()
        transport = Transport([(200, '{"link_token":"one"}'),
                               (200, '{"link_token":"two"}'),
                               (200, '{"link_token":"three"}')])
        client = link.PlaidLink(client_id="id", secret="secret", transport=transport,
                                clock=clock, session_seconds=10, max_pending=1)
        client.create(workos_subject="user", book="one")
        with self.assertRaisesRegex(link.Refuse, "too many pending"):
            client.create(workos_subject="user", book="two")
        clock.now += 10
        session = client.create(workos_subject="user", book="three")
        self.assertTrue(session.state)
        self.assertEqual(len(transport.calls), 2)

    def test_completion_is_bound_to_the_workos_subject_and_book(self):
        for subject_change, book_change in (("other", "book"), ("user", "other")):
            client, transport = subject([(200, '{"link_token":"link"}')])
            session = client.create(workos_subject="user", book="book")
            with self.assertRaisesRegex(link.Refuse, "unknown or mismatched"):
                client.complete(
                    state=session.state,
                    workos_subject=subject_change,
                    book=book_change,
                    public_token="public",
                )
            self.assertEqual(len(transport.calls), 1)

    def test_request_parameters_cannot_override_server_credentials(self):
        client, transport = subject([(200, '{}')])
        client._request(link.LINK_CREATE_PATH, {
            "client_id": "attacker",
            "secret": "attacker",
        })
        sent = transport.calls[0][1]
        self.assertEqual(sent["client_id"], "client-id")
        self.assertEqual(sent["secret"], "plaid-secret")

    def test_redirect_country_language_and_identity_are_validated(self):
        for kwargs in (
            {"redirect_uri": "http://ratio.example/callback"},
            {"redirect_uri": "https://ratio.example/callback?state=x"},
            {"country_codes": ("XX",)},
            {"language": "xx"},
            {"workos_subject": ""},
            {"book": ""},
        ):
            client, _ = subject([])
            args = {"workos_subject": "user", "book": "book", **kwargs}
            with self.assertRaises(link.Refuse):
                client.create(**args)

    def test_malformed_provider_responses_and_transport_failures_are_redacted(self):
        replies = [(200, "not-json"), (200, "{}"), (500, '{"error_code":"INVALID_REQUEST"}'),
                   RuntimeError("plaid-secret public-token")]
        for reply in replies:
            client, _ = subject([reply])
            with self.assertRaises(link.Refuse) as caught:
                client.create(workos_subject="user", book="book")
            self.assertNotIn("plaid-secret", str(caught.exception))
            self.assertNotIn("public-token", str(caught.exception))
        client, _ = subject([])
        self.assertEqual(repr(client), "PlaidLink(credentials=<redacted>)")

    def test_exchange_response_requires_both_server_only_identifiers(self):
        for response in ('{"item_id":"item"}', '{"access_token":"access"}', '[]'):
            client, _ = subject([(200, '{"link_token":"link"}'), (200, response)])
            session = client.create(workos_subject="user", book="book")
            with self.assertRaises(link.Refuse):
                client.complete(state=session.state, workos_subject="user", book="book",
                                public_token="public")


if __name__ == "__main__":
    unittest.main()
