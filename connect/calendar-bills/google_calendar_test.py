#!/usr/bin/env python3
from __future__ import annotations

import json
import pathlib
import sys
import unittest

import bills
import google_calendar as google

sys.argv = sys.argv[:1]
FIXTURES = pathlib.Path(__file__).with_name("testdata")


class Transport:
    def __init__(self, replies):
        self.replies = list(replies)
        self.calls = []

    def __call__(self, url, headers):
        self.calls.append((url, headers))
        reply = self.replies.pop(0)
        if isinstance(reply, Exception):
            raise reply
        return reply


def calendar(replies, *, max_pages=25):
    transport = Transport(replies)
    return google.GoogleCalendar(access_token="google-access-token", transport=transport,
                                 max_pages=max_pages), transport


def personal():
    return bills.Book(kind="PERSONAL")


def ratio_client():
    return bills.Client(client_id="client_ratio_calendar_bills",
                        allowlist=frozenset(bills.SCHEDULED_LEGS),
                        scopes=bills.CANONICAL_SCOPES)


def event(identifier="event-1", **changes):
    value = {
        "id": identifier,
        "etag": '"etag-1"',
        "status": "confirmed",
        "start": {"date": "2026-04-01"},
        "summary": "Title must not decide accounting",
        "extendedProperties": {"private": {
            "ratio_amount": "1800.00", "ratio_currency": "USD", "ratio_kind": "bill"
        }},
    }
    value.update(changes)
    return value


def page(*, items=(), page_token=None, sync_token=None):
    value = {"items": list(items)}
    if page_token is not None:
        value["nextPageToken"] = page_token
    if sync_token is not None:
        value["nextSyncToken"] = sync_token
    return json.dumps(value)


class CalendarSync(unittest.TestCase):
    def test_retained_pages_expand_occurrences_and_return_only_the_final_sync_token(self):
        client, transport = calendar([
            (200, (FIXTURES / "events-page-1.json").read_text()),
            (200, (FIXTURES / "events-page-2.json").read_text()),
        ])
        result = client.sync(calendar_id="household@example.com", seen={},
                             book=personal(), ratio_client=ratio_client())
        self.assertEqual(result.sync_token, "sync-final")
        self.assertEqual([p.rule_id for p in result.proposed],
                         ["scheduled_spend", "scheduled_income"])
        self.assertEqual([p.trade_date.isoformat() for p in result.proposed],
                         ["2026-04-01", "2026-04-15"])
        self.assertIn("singleEvents=true", transport.calls[0][0])
        self.assertIn("showDeleted=true", transport.calls[0][0])
        self.assertIn("pageToken=page-2", transport.calls[1][0])
        self.assertNotIn("recurrence", json.dumps([p.event_id for p in result.proposed]))

    def test_incremental_sync_keeps_stable_parameters_and_exact_retry_is_nonposting(self):
        unchanged = event()
        client, transport = calendar([(200, page(items=[unchanged], sync_token="new-sync"))])
        result = client.sync(calendar_id="calendar-id", sync_token="old-sync",
                             seen={"event-1": '"etag-1"'}, book=personal(),
                             ratio_client=ratio_client())
        self.assertEqual(result.proposed, ())
        self.assertEqual(result.nonposting[0].reason, "exact retry")
        self.assertIn("syncToken=old-sync", transport.calls[0][0])
        self.assertNotIn("timeMin", transport.calls[0][0])

    def test_title_description_and_organizer_never_supply_missing_metadata(self):
        untagged = event(extendedProperties=None, summary="Rent USD 1800 bill",
                         description="ratio_kind=bill", organizer={"email": "owner@example.com"})
        client, _ = calendar([(200, page(items=[untagged], sync_token="sync"))])
        result = client.sync(calendar_id="calendar-id", seen={}, book=personal(),
                             ratio_client=ratio_client())
        self.assertEqual(result.proposed, ())
        self.assertEqual(result.nonposting[0].reason, "no explicit Ratio metadata")

    def test_tentative_and_newly_cancelled_events_are_visible_and_nonposting(self):
        client, _ = calendar([
            (200, page(items=[event("tentative", status="tentative"),
                              event("cancelled", status="cancelled")], sync_token="sync")),
            (200, page(items=[event("cancelled", etag='"etag-2"')], sync_token="sync-2")),
        ])
        result = client.sync(calendar_id="calendar-id", seen={}, book=personal(),
                             ratio_client=ratio_client())
        self.assertEqual(result.proposed, ())
        self.assertEqual({row.status for row in result.nonposting}, {"tentative", "cancelled"})
        self.assertEqual(result.seen, {})
        confirmed = client.sync(calendar_id="calendar-id", sync_token=result.sync_token,
                                seen=result.seen, book=personal(), ratio_client=ratio_client())
        self.assertEqual(len(confirmed.proposed), 1)
        self.assertEqual(confirmed.seen["cancelled"], '"etag-2"')

    def test_changed_or_deleted_imported_occurrence_refuses_the_batch(self):
        for changed in (event(etag='"etag-2"'), event(etag='"etag-2"', status="cancelled")):
            client, _ = calendar([(200, page(items=[changed], sync_token="new"))])
            with self.assertRaisesRegex(google.Refuse, "correction/reversal"):
                client.sync(calendar_id="calendar-id", sync_token="old",
                            seen={"event-1": '"etag-1"'}, book=personal(),
                            ratio_client=ratio_client())

    def test_conflicting_duplicate_event_ids_refuse(self):
        client, _ = calendar([
            (200, page(items=[event()], page_token="p2")),
            (200, page(items=[event(etag='"different"')], sync_token="sync")),
        ])
        with self.assertRaisesRegex(google.Refuse, "conflicting content"):
            client.sync(calendar_id="calendar-id", seen={}, book=personal(),
                        ratio_client=ratio_client())

    def test_expired_sync_token_requires_full_sync_with_the_seen_index_retained(self):
        for body in ('{"error":{"code":410}}', "provider non-json body"):
            client, _ = calendar([(410, body)])
            with self.assertRaises(google.FullResyncRequired) as caught:
                client.sync(calendar_id="calendar-id", sync_token="expired",
                            seen={"event-1": '"etag-1"'}, book=personal(),
                            ratio_client=ratio_client())
            self.assertIn("retaining", str(caught.exception))
            self.assertIn("event-id/etag", str(caught.exception))

    def test_incomplete_metadata_invalid_dates_and_page_shapes_refuse(self):
        bad_events = [
            event(extendedProperties={"private": {"ratio_amount": "1.00"}}),
            event(start={"dateTime": "2026-04-01T10:00:00"}),
            event(start={"date": "bad"}),
            event(status=None),
        ]
        for bad in bad_events:
            client, _ = calendar([(200, page(items=[bad], sync_token="sync"))])
            with self.assertRaises(google.Refuse):
                client.sync(calendar_id="calendar-id", seen={}, book=personal(),
                            ratio_client=ratio_client())
        for raw in ("not-json", "[]", '{"items":[],"nextPageToken":"p","nextSyncToken":"s"}'):
            client, _ = calendar([(200, raw)])
            with self.assertRaises(google.Refuse):
                client.sync(calendar_id="calendar-id", seen={}, book=personal(),
                            ratio_client=ratio_client())

    def test_transport_and_credentials_are_redacted(self):
        client, _ = calendar([RuntimeError("google-access-token calendar contents")])
        with self.assertRaises(google.Refuse) as caught:
            client.sync(calendar_id="calendar-id", seen={}, book=personal(),
                        ratio_client=ratio_client())
        self.assertNotIn("google-access-token", str(caught.exception))
        self.assertNotIn("calendar contents", str(caught.exception))
        self.assertIsNone(caught.exception.__cause__)
        self.assertEqual(repr(client), "GoogleCalendar(access_token=<redacted>)")

        client, transport = calendar([(200, page(sync_token="sync"))])
        client.sync(calendar_id="calendar-id", seen={}, book=personal(),
                    ratio_client=ratio_client())
        url, headers = transport.calls[0]
        self.assertEqual(headers["Authorization"], "Bearer google-access-token")
        self.assertEqual(headers["Accept"], "application/json")
        self.assertNotIn("google-access-token", url)

        client, _ = calendar([(200, "google-access-token calendar contents")])
        with self.assertRaises(google.Refuse) as caught:
            client.sync(calendar_id="calendar-id", seen={}, book=personal(),
                        ratio_client=ratio_client())
        self.assertIsNone(caught.exception.__cause__)

    def test_mapper_refusal_stays_inside_the_provider_boundary(self):
        client, _ = calendar([(200, page(items=[event()], sync_token="sync"))])
        denied = bills.Client(
            client_id="client_ratio_calendar_bills",
            allowlist=frozenset(),
            scopes=bills.CANONICAL_SCOPES,
        )
        with self.assertRaises(google.Refuse) as caught:
            client.sync(
                calendar_id="calendar-id",
                seen={},
                book=personal(),
                ratio_client=denied,
            )
        self.assertIsInstance(caught.exception.__cause__, bills.Refuse)

    def test_page_count_and_concrete_calendar_are_bounded(self):
        client, _ = calendar([(200, page(page_token="again"))], max_pages=1)
        with self.assertRaisesRegex(google.Refuse, "1-page bound"):
            client.sync(calendar_id="calendar-id", seen={}, book=personal(),
                        ratio_client=ratio_client())
        client, _ = calendar([])
        with self.assertRaisesRegex(google.Refuse, "concrete"):
            client.sync(calendar_id="primary", seen={}, book=personal(),
                        ratio_client=ratio_client())


if __name__ == "__main__":
    unittest.main()
