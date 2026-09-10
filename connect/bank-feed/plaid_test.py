#!/usr/bin/env python3
from __future__ import annotations

import json
import pathlib
import sys
import unittest
from decimal import Decimal

import mapper
import plaid

sys.argv = sys.argv[:1]
FIXTURES = pathlib.Path(__file__).with_name("testdata")


def transaction(identifier: str = "txn-1", **changes):
    row = {
        "transaction_id": identifier,
        "account_id": "account-1",
        "date": "2026-03-15",
        "amount": Decimal("40.00"),
        "iso_currency_code": "USD",
        "name": "Grocer",
        "pending": False,
        "personal_finance_category": {"primary": "FOOD_AND_DRINK"},
    }
    row.update(changes)
    return row


def response(*, added=(), modified=(), removed=(), cursor="next", more=False):
    return json.dumps(
        {"added": list(added), "modified": list(modified), "removed": list(removed),
         "next_cursor": cursor, "has_more": more},
        default=str,
    )


class FakeTransport:
    def __init__(self, replies):
        self.replies = list(replies)
        self.calls = []

    def __call__(self, path, body):
        self.calls.append((path, body))
        reply = self.replies.pop(0)
        if isinstance(reply, Exception):
            raise reply
        return reply


def ratio_client():
    return mapper.Client(
        client_id="client_ratio_bank_feed",
        allowlist=mapper.PERSONAL_SEEDED_RULES,
        scopes=mapper.CANONICAL_SCOPES,
    )


def personal():
    return mapper.Book(kind="PERSONAL")


def client(replies, *, max_pages=25):
    transport = FakeTransport(replies)
    return plaid.PlaidClient(client_id="id", secret="secret", access_token="item-token",
                             transport=transport, max_pages=max_pages), transport


class Sync(unittest.TestCase):
    def test_retained_pages_keep_numeric_money_decimal_and_pending_nonposting(self):
        subject, _ = client([
            (200, (FIXTURES / "sync-page-1.json").read_text()),
            (200, (FIXTURES / "sync-page-2.json").read_text()),
        ])
        result = subject.sync(
            choices={"fixture-expense": plaid.RuleChoice("expense")},
            book=personal(),
            ratio_client=ratio_client(),
        )
        self.assertEqual(result.cursor, "fixture-cursor-2")
        self.assertEqual(result.proposed[0].amount, "1.10")
        self.assertEqual(result.pending[0].transaction_id, "fixture-pending")

    def test_pages_form_one_batch_and_return_the_final_cursor(self):
        subject, transport = client([
            (200, response(added=[transaction("one")], cursor="c1", more=True)),
            (200, response(added=[transaction("two")], cursor="c2")),
        ])
        choices = {"one": plaid.RuleChoice("expense"), "two": plaid.RuleChoice("card")}
        result = subject.sync(cursor="c0", choices=choices, book=personal(), ratio_client=ratio_client())
        self.assertEqual(result.cursor, "c2")
        self.assertEqual([p.rule_id for p in result.proposed], ["living_expense", "card_charge"])
        self.assertEqual(result.source_ids, ("one", "two"))
        self.assertEqual([call[1]["cursor"] for call in transport.calls], ["c0", "c1"])

    def test_a_retry_produces_the_same_event_id(self):
        raw = response(added=[transaction()])
        first, _ = client([(200, raw)])
        second, _ = client([(200, raw)])
        args = dict(cursor="", choices={"txn-1": plaid.RuleChoice("expense")},
                    book=personal(), ratio_client=ratio_client())
        self.assertEqual(first.sync(**args).proposed, second.sync(**args).proposed)

    def test_pending_is_visible_and_never_posted(self):
        subject, _ = client([(200, response(added=[transaction(pending=True)]))])
        result = subject.sync(choices={}, book=personal(), ratio_client=ratio_client())
        self.assertEqual(result.proposed, ())
        self.assertEqual(result.pending[0].transaction_id, "txn-1")

    def test_decimal_json_never_becomes_a_float(self):
        raw = response(added=[transaction(amount=Decimal("1.10"))])
        subject, _ = client([(200, raw)])
        result = subject.sync(choices={"txn-1": plaid.RuleChoice("expense")},
                              book=personal(), ratio_client=ratio_client())
        self.assertEqual(result.proposed[0].amount, "1.10")

    def test_provider_category_does_not_choose_the_rule(self):
        subject, _ = client([(200, response(added=[transaction()]))])
        with self.assertRaisesRegex(plaid.Refuse, "explicit user rule choice"):
            subject.sync(choices={}, book=personal(), ratio_client=ratio_client())

    def test_modified_or_removed_records_refuse_the_cursor_advance(self):
        for change in ({"modified": [transaction()]}, {"removed": [{"transaction_id": "txn-1"}]}):
            subject, _ = client([(200, response(**change))])
            with self.assertRaisesRegex(plaid.Refuse, "correction/reversal"):
                subject.sync(cursor="old", choices={}, book=personal(), ratio_client=ratio_client())

    def test_conflicting_duplicate_ids_refuse_the_batch(self):
        subject, _ = client([
            (200, response(added=[transaction()], cursor="c1", more=True)),
            (200, response(added=[transaction(name="Different")], cursor="c2")),
        ])
        with self.assertRaisesRegex(plaid.Refuse, "conflicting content"):
            subject.sync(choices={"txn-1": plaid.RuleChoice("expense")},
                         book=personal(), ratio_client=ratio_client())

    def test_an_exact_duplicate_across_pages_is_one_proposal(self):
        subject, _ = client([
            (200, response(added=[transaction()], cursor="c1", more=True)),
            (200, response(added=[transaction()], cursor="c2")),
        ])
        result = subject.sync(choices={"txn-1": plaid.RuleChoice("expense")},
                              book=personal(), ratio_client=ratio_client())
        self.assertEqual(len(result.proposed), 1)

    def test_mutation_error_names_retry_from_the_original_cursor(self):
        subject, _ = client([(400, json.dumps({
            "error_code": "TRANSACTIONS_SYNC_MUTATION_DURING_PAGINATION",
            "request_id": "safe",
        }))])
        with self.assertRaisesRegex(plaid.Refuse, "original cursor"):
            subject.sync(cursor="old", choices={}, book=personal(), ratio_client=ratio_client())

    def test_page_bound_refuses_an_unending_provider(self):
        subject, _ = client([(200, response(cursor="c1", more=True))], max_pages=1)
        with self.assertRaisesRegex(plaid.Refuse, "1-page bound"):
            subject.sync(choices={}, book=personal(), ratio_client=ratio_client())

    def test_malformed_money_date_currency_and_response_refuse(self):
        bad = [
            transaction(amount=Decimal("1.005")),
            transaction(date="March 15"),
            transaction(iso_currency_code=None, unofficial_currency_code="BTC"),
        ]
        for row in bad:
            subject, _ = client([(200, response(added=[row]))])
            with self.assertRaises(plaid.Refuse):
                subject.sync(choices={"txn-1": plaid.RuleChoice("expense")},
                             book=personal(), ratio_client=ratio_client())
        subject, _ = client([(200, "not-json")])
        with self.assertRaisesRegex(plaid.Refuse, "malformed JSON"):
            subject.sync(choices={}, book=personal(), ratio_client=ratio_client())
        malformed_shapes = [
            {"added": [], "modified": [], "removed": [], "next_cursor": "c", "has_more": "false"},
            {"added": [], "modified": [], "removed": [], "has_more": False},
            {"added": [transaction(pending="false")], "modified": [], "removed": [],
             "next_cursor": "c", "has_more": False},
        ]
        for shape in malformed_shapes:
            subject, _ = client([(200, json.dumps(shape, default=str))])
            with self.assertRaises(plaid.Refuse):
                subject.sync(choices={"txn-1": plaid.RuleChoice("expense")},
                             book=personal(), ratio_client=ratio_client())

    def test_transport_failure_does_not_leak_credentials(self):
        subject, _ = client([RuntimeError("item-token secret")])
        with self.assertRaises(plaid.Refuse) as caught:
            subject.sync(choices={}, book=personal(), ratio_client=ratio_client())
        self.assertNotIn("item-token", str(caught.exception))
        self.assertNotIn("secret", str(caught.exception))
        self.assertEqual(repr(subject), "PlaidClient(credentials=<redacted>)")

    def test_request_parameters_cannot_override_server_credentials(self):
        subject, transport = client([(200, response())])
        subject._request(plaid.SYNC_PATH, {
            "client_id": "attacker",
            "secret": "attacker",
            "access_token": "attacker",
        })
        sent = transport.calls[0][1]
        self.assertEqual(sent["client_id"], "id")
        self.assertEqual(sent["secret"], "secret")
        self.assertEqual(sent["access_token"], "item-token")

    def test_mapper_refusal_stays_inside_the_provider_boundary(self):
        subject, _ = client([(200, response(added=[transaction()]))])
        with self.assertRaises(plaid.Refuse) as caught:
            subject.sync(
                choices={"txn-1": plaid.RuleChoice("not-a-rule")},
                book=personal(),
                ratio_client=ratio_client(),
            )
        self.assertIsInstance(caught.exception.__cause__, mapper.Refuse)

    def test_disconnect_revokes_the_item_and_stops_future_pulls(self):
        subject, transport = client([(200, "{}")])
        subject.disconnect()
        self.assertEqual(transport.calls[0][0], plaid.REMOVE_PATH)
        with self.assertRaisesRegex(plaid.Refuse, "disconnected"):
            subject.sync(choices={}, book=personal(), ratio_client=ratio_client())


if __name__ == "__main__":
    unittest.main()
