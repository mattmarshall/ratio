#!/usr/bin/env python3
"""Properties the fund-ops-alerts Connect app must keep.

Test names are sentences. Break the thing a test protects and this
file goes red — a green suite that only checks the books balance is
how a missing break report ships as reconciled.

Talks to ConnectApiUrl only through the shared grant helper.
A green cite is not a live WorkOS dashboard registration and not
a Slack / email / PagerDuty delivery.
"""

from __future__ import annotations

import json
import os
import pathlib
import sys
import unittest
from datetime import date
from unittest import mock

import alerts as a

APP_PATH = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).with_name("app.json")
RULES_RS = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else None
CATALOG = pathlib.Path(sys.argv[3]) if len(sys.argv) > 3 else None
SCREENS = pathlib.Path(sys.argv[4]) if len(sys.argv) > 4 else None
PROTO = pathlib.Path(sys.argv[5]) if len(sys.argv) > 5 else None
TYPES = pathlib.Path(sys.argv[6]) if len(sys.argv) > 6 else None
RECON = pathlib.Path(sys.argv[7]) if len(sys.argv) > 7 else None

sys.argv = sys.argv[:1]


def app() -> dict:
    return json.loads(APP_PATH.read_text())


def declared_client(**overrides) -> a.Client:
    c = a.client_from_app(app())
    return a.Client(
        client_id=overrides.get("client_id", c.client_id),
        scopes=overrides.get("scopes", c.scopes),
    )


def book(**overrides) -> a.Book:
    return a.Book(
        book_id=overrides.get("book_id", "fund-a"),
        kind=overrides.get("kind", "INVESTMENT"),
        member=overrides.get("member", True),
        org_id=overrides.get("org_id"),
    )


def brk(**overrides) -> a.BreakCite:
    return a.BreakCite(
        name=overrides.get("name", "cash"),
        account=overrides.get("account", "Cash"),
        severity=overrides.get("severity", "HIGH"),
        explained=overrides.get("explained", False),
        cause=overrides.get("cause", "custodian cash disagrees"),
        ratio_amount=overrides.get("ratio_amount", 500_000),
        reported_amount=overrides.get("reported_amount", 300_000),
        difference=overrides.get("difference", 200_000),
        config_digest=overrides.get("config_digest", "abba" * 8),
    )


def gate(**overrides) -> a.NavGateCite:
    return a.NavGateCite(
        unexplained_breaks=overrides.get(
            "unexplained_breaks", ("Cash: custodian cash disagrees",)
        ),
        unresolved_trades=overrides.get("unresolved_trades", ()),
        unpriced=overrides.get("unpriced", ()),
        valuation_date=overrides.get("valuation_date"),
    )


def strike(**overrides) -> a.StrikeCite:
    return a.StrikeCite(
        name=overrides.get("name", "2026-03-31"),
        view=overrides.get("view", "ib"),
        net_asset_value=overrides.get("net_asset_value", 100_000),
        journal_digest=overrides.get("journal_digest", "cafe" * 8),
        journal_position=overrides.get("journal_position", 12),
        config_digest=overrides.get("config_digest", "abba" * 8),
    )


def pack_of(**kwargs) -> a.AlertPack:
    return a.build_pack(
        book=kwargs.get("book", book()),
        client=kwargs.get("client", declared_client()),
        breaks=kwargs.get("breaks", (brk(),)),
        nav_gate=kwargs.get("nav_gate", gate()),
        strike=kwargs.get("strike", strike()),
    )


class ParseMinor(unittest.TestCase):
    def test_an_amount_is_parsed_by_splitting_not_by_floating(self):
        self.assertEqual(a.parse_minor("1000.00"), 100_000)
        self.assertEqual(a.parse_minor("0.10"), 10)
        self.assertEqual(a.parse_minor("0.1"), 10)
        self.assertEqual(a.parse_minor("1.5"), 150)
        self.assertEqual(a.parse_minor("42"), 4_200)
        self.assertEqual(a.parse_minor(".5"), 50)
        self.assertEqual(a.parse_minor("$1,204,880.11"), 120_488_011)

    def test_three_decimal_places_are_refused_rather_than_dropped(self):
        with self.assertRaises(a.Refuse) as ctx:
            a.parse_minor("1.005")
        self.assertIn("minor units", str(ctx.exception))

    def test_a_float_object_is_refused(self):
        with self.assertRaises(a.Refuse) as ctx:
            a.parse_minor(1.005)  # type: ignore[arg-type]
        self.assertIn("float", str(ctx.exception))

    def test_a_signed_magnitude_is_refused_unless_asked(self):
        with self.assertRaises(a.Refuse) as ctx:
            a.parse_minor("-40.00")
        self.assertIn("signed", str(ctx.exception))

    def test_a_signed_difference_is_a_real_loss_not_an_inference(self):
        self.assertEqual(a.parse_minor("-40.00", allow_signed=True), -4_000)

    def test_a_zero_amount_is_a_real_zero_not_a_missing_cite(self):
        self.assertEqual(a.parse_minor("0.00"), 0)
        self.assertIsNone(a.parse_optional_minor(""))
        self.assertIsNone(a.parse_optional_minor(None))
        self.assertEqual(a.parse_optional_minor("0.00"), 0)

    def test_an_amount_that_does_not_fit_i64_is_refused(self):
        with self.assertRaises(a.Refuse):
            a.parse_minor("92233720368547758.08")


class DigestHonesty(unittest.TestCase):
    def test_an_empty_digest_is_unset_not_history_intact(self):
        self.assertIsNone(a.parse_optional_digest(""))
        self.assertIsNone(a.parse_optional_digest("   "))
        self.assertIsNone(a.parse_optional_digest(None))
        self.assertEqual(a.parse_optional_digest("cafe" * 8), "cafe" * 8)

    def test_a_non_string_digest_is_refused_so_a_hash_cannot_be_invented(self):
        with self.assertRaises(a.Refuse) as ctx:
            a.parse_optional_digest(0)  # type: ignore[arg-type]
        self.assertIn("digest", str(ctx.exception))


class CiteMapping(unittest.TestCase):
    def test_a_break_cite_copies_the_kernel_fields(self):
        b = a.break_from_cite(
            {
                "name": "cash",
                "account": "Cash",
                "severity": "HIGH",
                "explained": False,
                "cause": "custodian cash disagrees",
                "ratio_amount": "5000.00",
                "reported_amount": "3000.00",
                "difference": "2000.00",
                "config_digest": "abba" * 8,
            }
        )
        self.assertEqual(b.difference, 200_000)
        self.assertFalse(b.explained)
        self.assertEqual(b.config_digest, "abba" * 8)

    def test_a_nav_gate_cite_copies_the_three_first_class_reasons(self):
        g = a.nav_gate_from_cite(
            {
                "unexplainedBreaks": ["Cash: custodian cash disagrees"],
                "unresolvedTrades": ["trade-9 unidentified"],
                "unpriced": ["ACME"],
                "valuation_date": "2026-03-31",
            }
        )
        assert g is not None
        self.assertEqual(g.unexplained_breaks, ("Cash: custodian cash disagrees",))
        self.assertEqual(g.unresolved_trades, ("trade-9 unidentified",))
        self.assertEqual(g.unpriced, ("ACME",))
        self.assertEqual(g.valuation_date, date(2026, 3, 31))

    def test_unpriced_without_a_valuation_date_is_refused_not_invented(self):
        with self.assertRaises(a.Refuse) as ctx:
            a.nav_gate_from_cite({"unpriced": ["ACME"]})
        self.assertIn("valuation date", str(ctx.exception))

    def test_unpriced_stays_empty_when_no_date_is_named(self):
        g = a.nav_gate_from_cite({"unexplained_breaks": ["Cash"]})
        assert g is not None
        self.assertEqual(g.unpriced, ())
        self.assertIsNone(g.valuation_date)

    def test_a_missing_nav_stays_unset_not_zero(self):
        s = a.strike_from_cite({"name": "2026-03-31", "view": "ib"})
        assert s is not None
        self.assertIsNone(s.net_asset_value)


class PackShape(unittest.TestCase):
    def test_a_fixture_break_maps_to_breaks_csv(self):
        files = a.as_files(pack_of())
        self.assertIn("breaks.csv", files)
        self.assertIn("Cash", files["breaks.csv"])
        self.assertIn("2000.00", files["breaks.csv"])

    def test_unexplained_breaks_are_the_alert_lines(self):
        explained = brk(name="fee", account="Fees", explained=True, cause="accepted")
        open_line = brk()
        out = pack_of(breaks=(explained, open_line))
        lines = a.unexplained_breaks(out.breaks)
        self.assertEqual(len(lines), 1)
        self.assertEqual(lines[0].name, "cash")
        payload = a.as_json(out)
        self.assertEqual(payload["unexplained"][0]["name"], "cash")

    def test_a_missing_break_report_is_unset_not_a_silent_reconciled_list(self):
        out = pack_of(breaks=None)
        files = a.as_files(out)
        self.assertNotIn("breaks.csv", files)
        self.assertEqual(out.sections["breaks"].status, "unset")
        self.assertTrue(any(line.startswith("breaks:") for line in out.unset))
        self.assertIn("silent reconciled", out.sections["breaks"].note)
        payload = a.as_json(out)
        self.assertIsNone(payload["breaks"])

    def test_a_cited_empty_break_report_is_reconciled_and_the_pack_says_so(self):
        out = pack_of(breaks=())
        files = a.as_files(out)
        self.assertNotIn("breaks.csv", files)
        self.assertEqual(out.sections["breaks"].status, "cited-empty")
        self.assertIn("reconciled", out.sections["breaks"].note)
        self.assertIn("cited-empty", files["manifest.json"])

    def test_a_missing_nav_gate_is_unset_not_an_all_clear(self):
        out = pack_of(nav_gate=None)
        files = a.as_files(out)
        self.assertNotIn("nav_gate.csv", files)
        self.assertEqual(out.sections["nav_gate"].status, "unset")
        self.assertTrue(any("all-clear" in line for line in out.unset))
        payload = a.as_json(out)
        self.assertIsNone(payload["nav_gate"])

    def test_a_cited_empty_nav_gate_is_nothing_blocks_and_the_pack_says_so(self):
        out = pack_of(nav_gate=gate(unexplained_breaks=(), unresolved_trades=(), unpriced=()))
        files = a.as_files(out)
        self.assertNotIn("nav_gate.csv", files)
        self.assertEqual(out.sections["nav_gate"].status, "cited-empty")
        self.assertIn("nothing blocks", out.sections["nav_gate"].note)

    def test_unpriced_on_a_named_day_is_cited(self):
        out = pack_of(
            nav_gate=gate(
                unexplained_breaks=(),
                unpriced=("ACME",),
                valuation_date=date(2026, 3, 31),
            )
        )
        files = a.as_files(out)
        self.assertIn("nav_gate.csv", files)
        self.assertIn("ACME", files["nav_gate.csv"])
        self.assertIn("2026-03-31", files["nav_gate.csv"])

    def test_a_missing_strike_is_unset_not_nav_zero(self):
        out = pack_of(strike=None)
        files = a.as_files(out)
        self.assertNotIn("strike.csv", files)
        self.assertEqual(out.sections["strike"].status, "unset")
        self.assertTrue(any("NAV 0.00" in line for line in out.unset))
        payload = a.as_json(out)
        self.assertIsNone(payload["strike"])

    def test_an_empty_journal_digest_is_unset_not_history_intact(self):
        out = pack_of(strike=strike(journal_digest=None))
        files = a.as_files(out)
        self.assertNotIn("strike.csv", files)
        self.assertTrue(any("journal_digest" in line for line in out.unset))

    def test_a_posted_zero_difference_is_a_figure(self):
        out = pack_of(breaks=(brk(difference=0),))
        files = a.as_files(out)
        self.assertIn("0.00", files["breaks.csv"])

    def test_a_personal_book_leaves_fund_ops_cites_unset_when_none_are_handed(self):
        out = pack_of(book=book(kind="PERSONAL"), breaks=None, nav_gate=None, strike=None)
        self.assertEqual(out.book.kind, "PERSONAL")
        self.assertEqual(out.sections["breaks"].status, "unset")
        self.assertEqual(out.sections["nav_gate"].status, "unset")
        self.assertEqual(out.sections["strike"].status, "unset")
        files = a.as_files(out)
        self.assertNotIn("breaks.csv", files)
        self.assertNotIn("nav_gate.csv", files)
        self.assertNotIn("strike.csv", files)

    def test_json_keeps_missing_figures_null_not_zero(self):
        payload = a.as_json(pack_of(strike=None, nav_gate=None, breaks=None))
        self.assertIsNone(payload["strike"])
        self.assertIsNone(payload["nav_gate"])
        self.assertIsNone(payload["breaks"])
        self.assertNotEqual(payload["strike"], "0.00")


class Refusals(unittest.TestCase):
    def test_journal_read_is_refused_as_an_alias(self):
        with self.assertRaises(a.Refuse) as ctx:
            pack_of(client=declared_client(scopes=frozenset({"breaks:read", "journal:read"})))
        self.assertIn("journal:read", str(ctx.exception))

    def test_journal_append_is_refused_as_an_alias(self):
        with self.assertRaises(a.Refuse) as ctx:
            pack_of(client=declared_client(scopes=frozenset({"breaks:read", "journal:append"})))
        self.assertIn("journal:append", str(ctx.exception))

    def test_journals_post_is_refused_because_this_app_is_read_only(self):
        scopes = set(a.CANONICAL_SCOPES) | {"journals:post"}
        with self.assertRaises(a.Refuse) as ctx:
            pack_of(client=declared_client(scopes=frozenset(scopes)))
        self.assertIn("journals:post", str(ctx.exception))

    def test_missing_breaks_read_is_refused(self):
        scopes = set(a.CANONICAL_SCOPES) - {"breaks:read"}
        with self.assertRaises(a.Refuse) as ctx:
            pack_of(client=declared_client(scopes=frozenset(scopes)))
        self.assertIn("breaks:read", str(ctx.exception))

    def test_missing_webhooks_journal_is_refused(self):
        scopes = set(a.CANONICAL_SCOPES) - {"webhooks:journal"}
        with self.assertRaises(a.Refuse) as ctx:
            pack_of(client=declared_client(scopes=frozenset(scopes)))
        self.assertIn("webhooks:journal", str(ctx.exception))

    def test_a_non_member_book_is_refused_even_when_org_id_matches(self):
        with self.assertRaises(a.Refuse) as ctx:
            pack_of(book=book(member=False, org_id="org_shared"))
        msg = str(ctx.exception)
        self.assertIn("membership", msg)
        self.assertIn("org_id", msg)

    def test_unspecified_is_refused_as_a_hidden_fifth_kind(self):
        with self.assertRaises(a.Refuse) as ctx:
            pack_of(book=book(kind="UNSPECIFIED"))
        self.assertIn("UNSPECIFIED", str(ctx.exception))

    def test_subscribe_is_refused_because_the_kernel_webhook_surface_is_not_built(self):
        with self.assertRaises(a.Refuse) as ctx:
            a.subscribe()
        self.assertIn("webhooks:journal", str(ctx.exception))
        self.assertIn("not built", str(ctx.exception))

    def test_kernel_notify_is_refused(self):
        with self.assertRaises(a.Refuse) as ctx:
            a.kernel_notify()
        self.assertIn("notification", str(ctx.exception))

    def test_chatbot_is_refused(self):
        with self.assertRaises(a.Refuse) as ctx:
            a.chatbot()
        self.assertIn("chatbot", str(ctx.exception))

    def test_html_alerts_are_refused(self):
        with self.assertRaises(a.Refuse) as ctx:
            a.html_alerts()
        self.assertIn("ratio watch", str(ctx.exception))

    def test_inventing_a_break_explanation_is_refused(self):
        with self.assertRaises(a.Refuse) as ctx:
            a.explain_break()
        self.assertIn("BreakExplanation", str(ctx.exception))

    def test_rewriting_a_strike_is_refused(self):
        with self.assertRaises(a.Refuse) as ctx:
            a.rewrite_strike()
        self.assertIn("rewrite", str(ctx.exception))

    def test_slack_is_refused_without_a_configured_destination(self):
        with self.assertRaises(a.Refuse) as ctx:
            a.slack()
        self.assertIn("Slack", str(ctx.exception))
        self.assertIn("configured destination", str(ctx.exception))

    def test_email_is_refused_without_a_configured_destination(self):
        with self.assertRaises(a.Refuse) as ctx:
            a.email()
        self.assertIn("email", str(ctx.exception))

    def test_pagerduty_is_refused_without_a_configured_destination(self):
        with self.assertRaises(a.Refuse) as ctx:
            a.pagerduty()
        self.assertIn("PagerDuty", str(ctx.exception))


class GrantPath(unittest.TestCase):
    def test_fetch_cites_without_a_token_is_refused(self):
        env = {
            "RATIO_CONNECT_ACCESS_TOKEN": "",
            "WORKOS_CONNECT_CLIENT_ID": "",
            "WORKOS_CONNECT_CLIENT_SECRET": "",
        }
        with mock.patch.dict(os.environ, env, clear=False):
            with self.assertRaises(a.Refuse) as ctx:
                a.fetch_cites()
        self.assertIn("no Connect access token", str(ctx.exception))
        self.assertNotIn("grant path is not built", str(ctx.exception))

    def test_fetch_cites_pulls_connect_api_url_when_a_token_is_presented(self):
        transport = a._grant.FakeTransport(body='{"books":[]}')
        env = {
            "RATIO_CONNECT_API_URL": "https://connect.example",
            "RATIO_API_ORIGIN": "https://demo.example",
        }
        with mock.patch.dict(os.environ, env, clear=False):
            out = a.fetch_cites(token="connect-access-token", transport=transport)
        self.assertEqual(out["book"], {"books": []})
        self.assertEqual(transport.calls[0][1], "https://connect.example/v1/books")
        self.assertEqual(
            transport.calls[0][2]["authorization"], "Bearer connect-access-token"
        )

    def test_deliver_writes_a_cite_pack_after_the_grant_can_read_connect_api_url(self):
        transport = a._grant.FakeTransport(body='{"name":"books/a"}')
        env = {"RATIO_CONNECT_API_URL": "https://connect.example"}
        with mock.patch.dict(os.environ, env, clear=False):
            files = a.deliver(pack_of(), token="connect-access-token", transport=transport)
        self.assertIn("manifest.json", files)
        self.assertIn("unset.csv", files)
        self.assertEqual(
            transport.calls[0][2]["authorization"], "Bearer connect-access-token"
        )

    def test_dry_run_is_the_same_local_cite_pack(self):
        transport = a._grant.FakeTransport(body='{"ok":true}')
        env = {"RATIO_CONNECT_API_URL": "https://connect.example"}
        with mock.patch.dict(os.environ, env, clear=False):
            files = a.dry_run(pack_of(), token="connect-access-token", transport=transport)
        self.assertIn("manifest.json", files)
        self.assertNotIn("slack.json", files)
        self.assertNotIn("email.json", files)
        self.assertNotIn("pagerduty.json", files)

    def test_a_connect_token_never_reads_demo_open(self):
        src = pathlib.Path(a.__file__).read_text()
        self.assertNotIn("os.environ", src)
        self.assertNotIn("getenv", src)
        self.assertIn("never takes", src)
        grant = (pathlib.Path(a.__file__).resolve().parent.parent / "grant.py").read_text()
        self.assertIn("NEVER READ `RATIO_DEMO_OPEN`", grant)


class ManifestHonesty(unittest.TestCase):
    def test_the_app_declares_only_canonical_scopes(self):
        scopes = app()["workos_connect"]["scopes"]
        self.assertEqual(set(scopes), set(a.CANONICAL_SCOPES))
        for alias in a.REFUSED_ALIASES:
            self.assertNotIn(alias, scopes)
        self.assertNotIn("journals:post", scopes)
        self.assertNotIn("journal:append", scopes)
        self.assertNotIn("journal:read", scopes)
        self.assertNotIn("breaks:explain", scopes)
        self.assertEqual(app()["journals_post_allowlist"]["templates"], [])
        self.assertIn("webhooks:journal", scopes)
        self.assertIn("breaks:read", scopes)
        self.assertIn("nav:read", scopes)
        self.assertIn("views:read", scopes)
        self.assertIn("books:read", scopes)

    def test_grant_path_and_refusals_stay_named_as_leftovers(self):
        doc = json.dumps(app())
        self.assertEqual("built", app()["grant_path"]["status"])
        self.assertIn("ConnectApiUrl", app()["grant_path"]["note"])
        self.assertIn("WorkOS dashboard registration", app()["grant_path"]["note"])
        self.assertIn("refused", app()["destinations"]["status"])
        self.assertIn("refused", app()["kernel_notifier"]["status"])
        self.assertEqual("reserved", app()["subscribe"]["status"])
        self.assertIn("#162", doc)
        self.assertIn("#150", doc)
        self.assertIn("#22", doc)
        self.assertIn("does not reopen #151", doc)
        self.assertEqual(app()["issue"], 162)

    def test_the_catalog_still_names_the_scopes_this_app_requests(self):
        if CATALOG is None or not CATALOG.is_file():
            self.skipTest("connect-scopes.md not handed to the test")
        text = CATALOG.read_text()
        for scope in a.CANONICAL_SCOPES:
            self.assertIn(f"`{scope}`", text)
        self.assertIn("`journal:read`", text)
        self.assertIn("leftover #22", text)
        self.assertIn("An `org_id` claim is not membership", text)
        self.assertIn("`webhooks:journal`", text)
        self.assertIn("`breaks:read`", text)

    def test_screens_for_was_not_forked_with_an_alerts_url(self):
        if SCREENS is None or not SCREENS.is_file():
            self.skipTest("screens.ts not handed to the test")
        src = SCREENS.read_text()
        lowered = src.lower()
        for needle in (
            "fund-ops-alerts",
            "ops-alerts",
            'segment: "alerts"',
            'segment: "pager"',
            'segment: "slack"',
        ):
            self.assertNotIn(needle, lowered)
        self.assertIn('segment: "breaks"', src)
        self.assertIn('segment: "strikes"', src)

    def test_the_kernel_did_not_grow_an_alerts_rpc(self):
        if PROTO is None or not PROTO.is_file():
            self.skipTest("console.proto not handed to the test")
        src = PROTO.read_text()
        self.assertIn("message NavGate", src)
        self.assertIn("unexplained_breaks", src)
        self.assertIn("unresolved_trades", src)
        self.assertIn("unpriced", src)
        for field in a.NAV_GATE_PROTO_FIELDS:
            self.assertIn(field, src)
        for field in a.BREAK_PROTO_FIELDS:
            self.assertIn(field, src)
        for field in a.STRIKE_PROTO_FIELDS:
            self.assertIn(field, src)
        for needle in (
            "rpc FundOpsAlerts",
            "rpc OpsAlerts",
            "rpc NotifyOps",
            "rpc SlackAlert",
            "message FundOpsAlert",
            "message OpsNotification",
        ):
            self.assertNotIn(
                needle,
                src,
                f"{needle} is a kernel notifier RPC — refuse it; this app is the door",
            )

    def test_the_wire_types_still_name_the_cites_this_pack_copies(self):
        if TYPES is None or not TYPES.is_file():
            self.skipTest("types.ts not handed to the test")
        src = TYPES.read_text()
        self.assertIn("export interface NavGate", src)
        self.assertIn("export interface Break", src)
        self.assertIn("unexplainedBreaks", src)
        self.assertIn("unresolvedTrades", src)
        self.assertIn("unpriced", src)
        self.assertIn("navGate", src)

    def test_a_break_report_empty_list_is_still_the_kernel_reconciled_meaning(self):
        if RECON is None or not RECON.is_file():
            self.skipTest("recon.proto not handed to the test")
        src = RECON.read_text()
        self.assertIn("message BreakReport", src)
        self.assertIn("Empty means the period reconciled", src)

    def test_ruleset_did_not_grow_a_lot_method_for_alerts(self):
        if RULES_RS is None or not RULES_RS.is_file():
            self.skipTest("ratio-rules lib.rs not handed to the test")
        src = RULES_RS.read_text()
        self.assertNotIn("pub ops_alert", src)
        self.assertNotIn("pub pagerduty", src)


if __name__ == "__main__":
    unittest.main()
