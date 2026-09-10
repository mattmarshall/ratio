#!/usr/bin/env python3
"""Bounded Google Calendar Events sync for Personal scheduled journals.

Google expands recurrence; this adapter accepts only individual dated instances
with explicit private Ratio metadata. Titles, descriptions, organizers, and
attendees never choose an accounting rule.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from datetime import date, datetime
from typing import Any, Callable, Mapping, Sequence
from urllib.parse import quote, urlencode

import bills

EVENTS_API = "https://www.googleapis.com/calendar/v3/calendars"
GOOGLE_SCOPE = "https://www.googleapis.com/auth/calendar.events.readonly"
DEFAULT_MAX_PAGES = 25
RATIO_KEYS = ("ratio_amount", "ratio_currency", "ratio_kind")


class Refuse(Exception):
    """The provider batch is not accepted and its sync token is unchanged."""


class FullResyncRequired(Refuse):
    """The Google sync token expired; retain the seen index for full resync."""


Transport = Callable[[str, Mapping[str, str]], tuple[int, str]]


@dataclass(frozen=True)
class EventEvidence:
    event_id: str
    etag: str
    status: str
    reason: str


@dataclass(frozen=True)
class SyncResult:
    sync_token: str
    proposed: tuple[bills.ProposedPost, ...]
    seen: Mapping[str, str]
    nonposting: tuple[EventEvidence, ...]


class GoogleCalendar:
    def __init__(
        self,
        *,
        access_token: str,
        transport: Transport,
        max_pages: int = DEFAULT_MAX_PAGES,
    ) -> None:
        if not access_token:
            raise Refuse("Google Calendar access token is required")
        if not isinstance(max_pages, int) or isinstance(max_pages, bool) or max_pages < 1:
            raise Refuse("max_pages must be a positive integer")
        self._access_token = access_token
        self._transport = transport
        self._max_pages = max_pages

    def __repr__(self) -> str:
        return "GoogleCalendar(access_token=<redacted>)"

    def sync(
        self,
        *,
        calendar_id: str,
        sync_token: str = "",
        seen: Mapping[str, str],
        book: bills.Book,
        ratio_client: bills.Client,
    ) -> SyncResult:
        calendar = _calendar_id(calendar_id)
        prior = _seen_index(seen)
        page_token = ""
        final_sync_token: str | None = None
        records: dict[str, Mapping[str, Any]] = {}

        for _page in range(self._max_pages):
            query: dict[str, str] = {
                "singleEvents": "true",
                "showDeleted": "true",
                "maxResults": "2500",
            }
            if sync_token:
                query["syncToken"] = _token(sync_token, "Google sync token")
            if page_token:
                query["pageToken"] = page_token
            url = f"{EVENTS_API}/{quote(calendar, safe='')}/events?{urlencode(query)}"
            payload = self._request(url)
            items = payload.get("items")
            if not isinstance(items, list) or not all(isinstance(item, dict) for item in items):
                raise Refuse("Google Calendar response field 'items' is not an event list")
            for event in items:
                event_id = _identifier(event.get("id"), "Google event id")
                previous = records.get(event_id)
                if previous is not None and _canonical(previous) != _canonical(event):
                    raise Refuse(f"Google event {event_id!r} has conflicting content in one sync")
                records[event_id] = event

            next_page = payload.get("nextPageToken")
            next_sync = payload.get("nextSyncToken")
            if next_page is not None and next_sync is not None:
                raise Refuse("Google Calendar response has both page and sync tokens")
            if next_page is not None:
                page_token = _token(next_page, "Google page token")
                continue
            final_sync_token = _token(next_sync, "Google next sync token")
            break
        else:
            raise Refuse(
                f"Google Calendar sync exceeded the {self._max_pages}-page bound; sync token is unchanged"
            )

        rows: list[Mapping[str, Any]] = []
        updated_seen = dict(prior)
        evidence: list[EventEvidence] = []
        for event_id, event in records.items():
            etag = _identifier(event.get("etag"), "Google event etag")
            status = _status(event.get("status"))
            old_etag = prior.get(event_id)
            if old_etag is not None:
                if old_etag != etag:
                    verb = "deleted" if status == "cancelled" else "changed"
                    raise Refuse(
                        f"previously imported Google event {event_id!r} was {verb}; "
                        "Ratio has no correction/reversal policy, so the sync token is unchanged"
                    )
                evidence.append(EventEvidence(event_id, etag, status, "exact retry"))
                continue
            if status == "cancelled":
                evidence.append(EventEvidence(event_id, etag, status, "cancelled before import"))
                updated_seen[event_id] = etag
                continue
            if status == "tentative":
                evidence.append(EventEvidence(event_id, etag, status, "tentative"))
                continue
            metadata = _metadata(event)
            if metadata is None:
                evidence.append(EventEvidence(event_id, etag, status, "no explicit Ratio metadata"))
                continue
            rows.append(
                {
                    "dated": _event_day(event).isoformat(),
                    "amount": metadata["ratio_amount"],
                    "currency": metadata["ratio_currency"],
                    "kind": metadata["ratio_kind"],
                    "reference": _event_reference(calendar, event_id),
                }
            )
            updated_seen[event_id] = etag

        proposed = bills.map_batch(rows, book=book, client=ratio_client)
        assert final_sync_token is not None
        return SyncResult(final_sync_token, tuple(proposed), updated_seen, tuple(evidence))

    def _request(self, url: str) -> Mapping[str, Any]:
        headers = {"Authorization": f"Bearer {self._access_token}", "Accept": "application/json"}
        try:
            status, raw = self._transport(url, headers)
        except Exception as exc:
            raise Refuse(f"Google Calendar transport failed: {type(exc).__name__}") from exc
        if not isinstance(status, int) or isinstance(status, bool):
            raise Refuse("Google Calendar transport returned a malformed HTTP status")
        try:
            payload = json.loads(raw)
        except (json.JSONDecodeError, TypeError) as exc:
            raise Refuse("Google Calendar returned malformed JSON") from exc
        if not isinstance(payload, dict):
            raise Refuse("Google Calendar returned a non-object response")
        if status == 410:
            raise FullResyncRequired(
                "Google sync token expired; perform a full sync without it while retaining "
                "the prior event-id/etag index so journal history is not reposted"
            )
        if status >= 400:
            raise Refuse(f"Google Calendar returned HTTP {status}")
        return payload


def _calendar_id(value: object) -> str:
    calendar = _identifier(value, "Google calendar id")
    if calendar == "primary":
        raise Refuse("select a concrete Google calendar id; 'primary' is implicit")
    return calendar


def _seen_index(value: Mapping[str, str]) -> dict[str, str]:
    if not isinstance(value, Mapping):
        raise Refuse("the prior Google event-id/etag index is required")
    out: dict[str, str] = {}
    for event_id, etag in value.items():
        out[_identifier(event_id, "prior Google event id")] = _identifier(
            etag, "prior Google event etag"
        )
    return out


def _identifier(value: object, name: str) -> str:
    if not isinstance(value, str) or not value.strip() or len(value) > 1024:
        raise Refuse(f"{name} is missing or malformed")
    return value.strip()


def _token(value: object, name: str) -> str:
    if not isinstance(value, str) or not value or len(value) > 8192:
        raise Refuse(f"{name} is missing or malformed")
    return value


def _status(value: object) -> str:
    if not isinstance(value, str) or not value.strip():
        raise Refuse("Google event status is missing or malformed")
    status = value.strip().lower()
    if status not in {"confirmed", "tentative", "cancelled"}:
        raise Refuse(f"Google event status {status!r} is unsupported")
    return status


def _metadata(event: Mapping[str, Any]) -> Mapping[str, str] | None:
    extended = event.get("extendedProperties")
    if extended is None:
        return None
    if not isinstance(extended, dict):
        raise Refuse("Google event extendedProperties is malformed")
    private = extended.get("private")
    if private is None:
        return None
    if not isinstance(private, dict):
        raise Refuse("Google event private metadata is malformed")
    present = [key for key in RATIO_KEYS if key in private]
    if not present:
        return None
    if len(present) != len(RATIO_KEYS):
        raise Refuse("Google event has incomplete explicit Ratio metadata")
    values = {key: _identifier(private.get(key), f"Google {key}") for key in RATIO_KEYS}
    if values["ratio_kind"].lower() not in {"bill", "income"}:
        raise Refuse("Google ratio_kind must explicitly be 'bill' or 'income'")
    return values


def _event_day(event: Mapping[str, Any]) -> date:
    start = event.get("start")
    if not isinstance(start, dict):
        raise Refuse("Google event has no start")
    if "date" in start and "dateTime" in start:
        raise Refuse("Google event start has both date and dateTime")
    if "date" in start:
        raw = start.get("date")
        try:
            return date.fromisoformat(str(raw))
        except ValueError as exc:
            raise Refuse("Google all-day event has an invalid date") from exc
    raw = start.get("dateTime")
    if not isinstance(raw, str):
        raise Refuse("Google event has no dated occurrence")
    try:
        parsed = datetime.fromisoformat(raw.replace("Z", "+00:00"))
    except ValueError as exc:
        raise Refuse("Google event has an invalid RFC3339 start") from exc
    if parsed.tzinfo is None:
        raise Refuse("Google event start has no time-zone offset")
    return parsed.date()


def _event_reference(calendar_id: str, event_id: str) -> str:
    digest = hashlib.sha256((calendar_id + "\0" + event_id).encode()).hexdigest()
    return "gcal-" + digest[:48]


def _canonical(event: Mapping[str, Any]) -> str:
    return json.dumps(event, sort_keys=True, separators=(",", ":"))
