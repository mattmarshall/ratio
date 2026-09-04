#!/usr/bin/env python3
"""Properties the LP / investor portal Connect app must keep.

Test names are sentences. Break the thing a test protects and this
file goes red — a green suite that only checks the books balance is
how a silent 1/N of book NAV ships.

Talks to ConnectApiUrl only through the shared grant helper.
Does not grow an HTML portal, an LP directory, or a drip election.
"""

from __future__ import annotations

import json
import os
import pathlib
import sys
import unittest
from datetime import date
from unittest import mock

import portal as p

APP_PATH = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).with_name("app.json")
RULES_RS = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else None
CATALOG = pathlib.Path(sys.argv[3]) if len(sys.argv) > 3 else None
SCREENS = pathlib.Path(sys.argv[4]) if len(sys.argv) > 4 else None
PROTO = pathlib.Path(sys.argv[5]) if len(sys.argv) > 5 else None
TYPES = pathlib.Path(sys.argv[6]) if len(sys.argv) > 6 else None

sys.argv = sys.argv[:1]


def app() -> dict:
    return json.loads(APP_PATH.read_text())


def declared_client(**overrides) -> p.Client:
    c = p.client_from_app(app())
    return p.Client(
        client_id=overrides.get("client_id", c.client_id),
        scopes=overrides.get("scopes", c.scopes),
    )


def book(**overrides) -> p.Book:
    return p.Book(
        book_id=overrides.get("book_id", "fund-a"),
        kind=overrides.get("kind", "INVESTMENT"),
        member=overrides.get("member", True),
        org_id=overrides.get("org_id"),
        closed_through=overrides.get("closed_through"),
    )


def lp(**overrides) -> dict:
    base = {
        "grain": "LP",
        "beginning": "100.00",
        "contributions": "40.00",
        "distributions": "10.00",
        "ending": "130.00",
        "units": "10",
    }
    base.update(overrides)
    return base


def gp(**overrides) -> dict:
    base = {
        "grain": "GP",
        "beginning": "0.00",
        "contributions": "10.00",
        "distributions": "0.00",
        "ending": "10.00",
        "units": "2",
    }
    base.update(overrides)
    return base


def nav(**overrides) -> dict:
    base = {
        "net_asset_value": "140.00",
        "journal_digest": "cafe" * 8,
        "journal_position": 12,
        "config_digest": "abba" * 8,
        "beginning": "100.00",
        "contributions": "50.00",
        "distributions": "10.00",
        "income": "5.00",
        "expense": "2.00",
        "unrealized": "3.00",
        "ending": "146.00",
    }
    base.update(overrides)
    return base


def notice(**overrides) -> dict:
    base = {
        "kind": "call",
        "amount": "40.00",
        "digest": "beef" * 8,
        "amounts": [{"partner": "LP", "amount": "32.00"}, {"partner": "GP", "amount": "8.00"}],
        "entry_id": "call-1",
    }
    base.update(overrides)
    return base


def cite(**kwargs) -> p.Statement:
    return p.cite_statement(
        partners=kwargs.get("partners", (lp(), gp())),
        nav=kwargs.get("nav", nav()),
        book=kwargs.get("book", book()),
        client=kwargs.get("client", declared_client()),
        remaining_commitment=kwargs.get("remaining_commitment", "60.00"),
        remaining_undrawn=kwargs.get("remaining_undrawn", "60.00"),
        notices=kwargs.get("notices", (notice(),)),
        currency=kwargs.get("currency", "USD"),
        partner_cut=kwargs.get("partner_cut"),
        book_income=kwargs.get("book_income"),
        book_expense=kwargs.get("book_expense"),
        book_unrealized=kwargs.get("book_unrealized"),
    )


class ParseMinor(unittest.TestCase):
    def test_an_amount_is_parsed_by_splitting_not_by_floating(self):
        self.assertEqual(p.parse_minor("250000.00"), 25_000_000)
        self.assertEqual(p.parse_minor("0.10"), 10)
        self.assertEqual(p.parse_minor("0.1"), 10)
        self.assertEqual(p.parse_minor("1.5"), 150)
        self.assertEqual(p.parse_minor("42"), 4_200)
        self.assertEqual(p.parse_minor(".5"), 50)
        self.assertEqual(p.parse_minor("$1,204,880.11"), 120_488_011)

    def test_three_decimal_places_are_refused_rather_than_dropped(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.parse_minor("1.005")
        self.assertIn("minor units", str(ctx.exception))

    def test_a_float_object_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.parse_minor(1.005)  # type: ignore[arg-type]
        self.assertIn("float", str(ctx.exception))

    def test_a_signed_magnitude_is_refused_unless_asked(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.parse_minor("-40.00")
        self.assertIn("signed", str(ctx.exception))

    def test_a_zero_cite_is_a_real_zero_not_a_missing_figure(self):
        self.assertEqual(p.parse_minor("0.00"), 0)
        self.assertIsNone(p.parse_optional_minor(""))
        self.assertIsNone(p.parse_optional_minor(None))
        self.assertEqual(p.parse_optional_minor("0.00"), 0)


class DigestHonesty(unittest.TestCase):
    def test_an_empty_digest_is_unset_not_history_intact(self):
        self.assertIsNone(p.parse_optional_digest(""))
        self.assertIsNone(p.parse_optional_digest("   "))
        self.assertIsNone(p.parse_optional_digest(None))
        self.assertEqual(p.parse_optional_digest("cafe" * 8), "cafe" * 8)

    def test_a_non_string_digest_is_refused_so_a_hash_cannot_be_invented(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.parse_optional_digest(0)  # type: ignore[arg-type]
        self.assertIn("digest", str(ctx.exception))


class PartnerCut(unittest.TestCase):
    def test_a_named_cut_fills_allocated_plugs_exactly(self):
        cut = (p.PartnerShare("LP", 80), p.PartnerShare("GP", 20))
        self.assertEqual(p.apply_cut(30_000, cut), {"LP": 24_000, "GP": 6_000})

    def test_an_empty_cut_is_unset_not_one_over_n(self):
        self.assertIsNone(p.apply_cut(30_000, ()))
        self.assertIsNone(p.apply_cut(30_000, None))
        self.assertIsNone(p.apply_cut(None, (p.PartnerShare("LP", 80),)))

    def test_a_figure_that_will_not_divide_stays_unset_not_rounded(self):
        cut = (p.PartnerShare("LP", 80), p.PartnerShare("GP", 20))
        self.assertIsNone(p.apply_cut(101, cut))

    def test_equal_split_is_refused_because_that_is_the_defect(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.equal_split(30_000, ("LP", "GP"))
        self.assertIn("1/N", str(ctx.exception))
        self.assertIn("#180", str(ctx.exception))


class CapitalCites(unittest.TestCase):
    def test_a_posted_book_cites_partner_capital_commitment_and_nav(self):
        out = cite()
        self.assertEqual(len(out.partners), 2)
        self.assertEqual(out.partners[0].grain, "LP")
        self.assertEqual(out.partners[0].beginning, 10_000)
        self.assertEqual(out.partners[0].contributions, 4_000)
        self.assertEqual(out.partners[0].distributions, 1_000)
        self.assertEqual(out.partners[0].ending, 13_000)
        self.assertEqual(out.remaining_commitment, 6_000)
        self.assertEqual(out.remaining_undrawn, 6_000)
        self.assertEqual(out.nav.net_asset_value, 14_000)
        self.assertEqual(out.nav.journal_digest, "cafe" * 8)
        self.assertEqual(out.notices[0].kind, "call")
        self.assertFalse(any(u.startswith("NAV strike") for u in out.unset), out.unset)

    def test_allocated_plugs_stay_unset_without_a_named_cut(self):
        out = cite()
        self.assertIsNone(out.partners[0].allocated_income)
        self.assertIsNone(out.partners[1].allocated_income)
        self.assertTrue(any("1/N of book NAV" in u for u in out.unset), out.unset)
        self.assertTrue(
            out.partners[0].allocated_income != 7_000,
            "a silent half of book income would still look like a cite",
        )

    def test_a_named_cut_fills_allocated_income_from_the_book_figure(self):
        out = cite(
            partner_cut=(p.PartnerShare("LP", 80), p.PartnerShare("GP", 20)),
            book_income="30.00",
        )
        self.assertEqual(out.partners[0].allocated_income, 2_400)
        self.assertEqual(out.partners[1].allocated_income, 600)
        self.assertFalse(any("LP allocated income" in u for u in out.unset), out.unset)

    def test_a_missing_nav_strike_stays_unset_not_nav_zero(self):
        out = cite(nav={})
        self.assertIsNone(out.nav.net_asset_value)
        self.assertIsNone(out.nav.journal_digest)
        self.assertTrue(any(u.startswith("NAV strike") for u in out.unset), out.unset)
        self.assertNotEqual(out.nav.net_asset_value, 0)
        files = p.as_files(out)
        self.assertIn("NAV strike", files["unset.csv"])
        self.assertIn("net_asset_value,,", files["nav.csv"].replace(" ", ""))

    def test_an_empty_digest_is_unset_not_history_intact(self):
        out = cite(nav=nav(journal_digest=""))
        self.assertIsNone(out.nav.journal_digest)
        self.assertTrue(any("history-intact" in u for u in out.unset), out.unset)

    def test_a_book_that_never_committed_leaves_undrawn_unset_not_callable_zero(self):
        out = cite(remaining_commitment=None, remaining_undrawn=None)
        self.assertIsNone(out.remaining_commitment)
        self.assertIsNone(out.remaining_undrawn)
        self.assertTrue(any("callable zero" in u for u in out.unset), out.unset)
        self.assertNotEqual(out.remaining_undrawn, 0)

    def test_a_fully_drawn_commitment_is_a_real_zero(self):
        out = cite(remaining_commitment="0.00", remaining_undrawn="0.00")
        self.assertEqual(out.remaining_commitment, 0)
        self.assertEqual(out.remaining_undrawn, 0)
        self.assertFalse(any("remaining undrawn" in u for u in out.unset), out.unset)

    def test_activity_shaped_beginning_stays_unset_not_a_fake_zero_stock(self):
        out = cite(partners=(lp(beginning=""), gp(beginning="")))
        self.assertIsNone(out.partners[0].beginning)
        self.assertIsNone(out.partners[1].beginning)
        self.assertTrue(any("fake zero stock" in u for u in out.unset), out.unset)
        self.assertNotEqual(out.partners[0].beginning, 0)

    def test_an_unposted_partner_leaves_ending_unset_not_ending_zero(self):
        out = cite(partners=(lp(), {"grain": "SP"}))
        sp = next(r for r in out.partners if r.grain == "SP")
        self.assertIsNone(sp.ending)
        self.assertIsNone(sp.contributions)
        self.assertTrue(any(u.startswith("SP ending") for u in out.unset), out.unset)

    def test_empty_notices_are_unset_not_a_silent_waterfall(self):
        out = cite(notices=())
        self.assertEqual(out.notices, ())
        self.assertTrue(any("not a waterfall" in u for u in out.unset), out.unset)

    def test_a_posted_zero_nav_is_a_real_zero_not_a_missing_strike(self):
        out = cite(nav=nav(net_asset_value="0.00"))
        self.assertEqual(out.nav.net_asset_value, 0)
        self.assertFalse(any(u.startswith("NAV strike") for u in out.unset), out.unset)


class ScopeAndKind(unittest.TestCase):
    def test_journal_append_is_refused_as_an_alias(self):
        with self.assertRaises(p.Refuse) as ctx:
            cite(
                client=declared_client(
                    scopes=frozenset(
                        {"partners:read", "statements:read", "nav:read", "journal:append"}
                    )
                )
            )
        self.assertIn("journal:append", str(ctx.exception))

    def test_journals_post_is_refused_because_this_app_is_read_only(self):
        with self.assertRaises(p.Refuse) as ctx:
            cite(
                client=declared_client(
                    scopes=frozenset(
                        {
                            "partners:read",
                            "statements:read",
                            "nav:read",
                            "journals:post",
                        }
                    )
                )
            )
        self.assertIn("unknown scope", str(ctx.exception))
        self.assertIn("journals:post", str(ctx.exception))

    def test_missing_partners_read_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            cite(
                client=declared_client(
                    scopes=frozenset({"statements:read", "nav:read"})
                )
            )
        self.assertIn("partners:read", str(ctx.exception))

    def test_books_read_is_optional_and_a_named_book_still_cites(self):
        out = cite(
            client=declared_client(
                scopes=frozenset({"partners:read", "statements:read", "nav:read"})
            )
        )
        self.assertEqual(out.partners[0].ending, 13_000)

    def test_a_personal_book_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            cite(book=book(kind="PERSONAL"))
        self.assertIn("INVESTMENT", str(ctx.exception))

    def test_an_org_id_claim_is_not_membership(self):
        with self.assertRaises(p.Refuse) as ctx:
            cite(book=book(member=False, org_id="org_acme"))
        self.assertIn("org_id", str(ctx.exception))
        self.assertIn("membership", str(ctx.exception))


class GrantPath(unittest.TestCase):
    def test_fetch_cites_refuses_without_a_token_and_does_not_say_unbuilt(self):
        env = {
            "RATIO_CONNECT_API_URL": "https://connect.example",
            "RATIO_CONNECT_ACCESS_TOKEN": "",
            "WORKOS_CONNECT_CLIENT_ID": "",
            "WORKOS_CONNECT_CLIENT_SECRET": "",
        }
        with mock.patch.dict(os.environ, env, clear=False):
            with self.assertRaises(p.Refuse) as ctx:
                p.fetch_cites()
        self.assertIn("no Connect access token", str(ctx.exception))
        self.assertNotIn("grant path is not built", str(ctx.exception))

    def test_fetch_cites_pulls_connect_api_url_when_a_token_is_presented(self):
        transport = p._grant.FakeTransport(body='{"books":[]}')
        env = {"RATIO_CONNECT_API_URL": "https://connect.example"}
        with mock.patch.dict(os.environ, env, clear=False):
            self.assertEqual(
                p.fetch_cites(token="connect-access-token", transport=transport),
                {"books": []},
            )
        self.assertEqual(transport.calls[0][1], "https://connect.example/v1/books")
        self.assertEqual(
            transport.calls[0][2]["authorization"], "Bearer connect-access-token"
        )

    def test_deliver_confirms_connect_api_url_when_a_token_is_presented(self):
        transport = p._grant.FakeTransport(body='{"ok":true}')
        env = {"RATIO_CONNECT_API_URL": "https://connect.example"}
        with mock.patch.dict(os.environ, env, clear=False):
            p.deliver(cite(), token="connect-access-token", transport=transport)
        self.assertEqual(
            transport.calls[0][2]["authorization"], "Bearer connect-access-token"
        )

    def test_a_connect_token_never_reads_demo_open(self):
        src = pathlib.Path(p.__file__).read_text()
        self.assertNotIn('os.environ', src)
        self.assertNotIn("getenv", src)
        self.assertIn("never takes", src)
        grant = (pathlib.Path(p.__file__).resolve().parent.parent / "grant.py").read_text()
        self.assertIn("NEVER READ `RATIO_DEMO_OPEN`", grant)


class Refusals(unittest.TestCase):
    def test_irr_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.irr()
        self.assertIn("IRR", str(ctx.exception))

    def test_tvpi_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.tvpi()
        self.assertIn("TVPI", str(ctx.exception))

    def test_waterfall_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.waterfall()
        self.assertIn("waterfall", str(ctx.exception))

    def test_drip_stays_leftover_on_161(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.drip()
        self.assertIn("#161", str(ctx.exception))
        with self.assertRaises(p.Refuse) as ctx:
            p.drip_election()
        self.assertIn("#161", str(ctx.exception))

    def test_kernel_portal_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.kernel_portal()
        self.assertIn("ratio watch", str(ctx.exception))
        with self.assertRaises(p.Refuse) as ctx:
            p.html_portal()
        self.assertIn("Connect", str(ctx.exception))

    def test_lp_directory_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.lp_directory()
        self.assertIn("#161", str(ctx.exception))

    def test_document_vault_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.document_vault()
        self.assertIn("document vault", str(ctx.exception))

    def test_payment_initiation_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.payments_initiate()
        self.assertIn("payments:initiate", str(ctx.exception))


class RenderHonesty(unittest.TestCase):
    def test_csv_leaves_missing_nav_blank_and_names_it_on_unset(self):
        files = p.as_files(cite(nav={}))
        self.assertIn("net_asset_value", files["nav.csv"])
        self.assertIn("NAV strike", files["unset.csv"])
        amount = files["nav.csv"].splitlines()[1].split(",")[1]
        self.assertEqual(amount, "")

    def test_json_keeps_missing_figures_null_not_zero(self):
        payload = p.as_json(cite(nav={}, remaining_commitment=None, remaining_undrawn=None))
        self.assertIsNone(payload["nav"]["net_asset_value"])
        self.assertIsNone(payload["remaining_undrawn"])
        self.assertNotEqual(payload["nav"]["net_asset_value"], "0.00")


class ManifestHonesty(unittest.TestCase):
    def test_the_app_declares_only_canonical_scopes(self):
        scopes = app()["workos_connect"]["scopes"]
        self.assertEqual(set(scopes), set(p.CANONICAL_SCOPES))
        for alias in p.REFUSED_ALIASES:
            self.assertNotIn(alias, scopes)
        self.assertNotIn("journals:post", scopes)
        self.assertNotIn("journal:append", scopes)
        self.assertEqual(app()["journals_post_allowlist"]["templates"], [])
        self.assertIn("partners:read", scopes)
        self.assertIn("statements:read", scopes)
        self.assertIn("nav:read", scopes)
        self.assertIn("books:read", scopes)

    def test_grant_path_and_refusals_stay_named_as_leftovers(self):
        doc = json.dumps(app())
        self.assertEqual("built", app()["grant_path"]["status"])
        self.assertIn("ConnectApiUrl", app()["grant_path"]["note"])
        self.assertIn("WorkOS dashboard registration", app()["grant_path"]["note"])
        self.assertIn("refused", app()["drip"]["status"])
        self.assertIn("refused", app()["kernel_portal"]["status"])
        self.assertIn("refused", app()["irr_tvpi_waterfall"]["status"])
        self.assertIn("#161", doc)
        self.assertIn("#150", doc)
        self.assertIn("#22", doc)
        self.assertIn("does not reopen #151", doc)
        self.assertEqual(app()["issue"], 161)

    def test_the_catalog_still_names_the_scopes_this_app_requests(self):
        if CATALOG is None or not CATALOG.is_file():
            self.skipTest("connect-scopes.md not handed to the test")
        text = CATALOG.read_text()
        for scope in p.CANONICAL_SCOPES:
            self.assertIn(f"`{scope}`", text)
        self.assertIn("`journal:append`", text)
        self.assertIn("leftover #22", text)
        self.assertIn("An `org_id` claim is not membership", text)
        self.assertIn("`partners:read`", text)

    def test_partner_cut_field_names_are_the_ones_ruleset_already_stores(self):
        if RULES_RS is None or not RULES_RS.is_file():
            self.skipTest("ratio-rules lib.rs not handed to the test")
        src = RULES_RS.read_text()
        self.assertIn("partner_cut", src)
        self.assertIn("special_allocation", src)
        self.assertNotIn("pub irr:", src)
        self.assertNotIn("pub tvpi:", src)
        self.assertNotIn("pub waterfall:", src)

    def test_screens_for_investment_was_not_forked_with_a_portal_url(self):
        if SCREENS is None or not SCREENS.is_file():
            self.skipTest("screens.ts not handed to the test")
        src = SCREENS.read_text()
        start = src.index("export const INVESTMENT_SCREENS")
        end = src.index("export const OPERATING_SCREENS")
        investment = src[start:end]
        lowered = investment.lower()
        self.assertNotIn('segment: "portal"', lowered)
        self.assertNotIn('segment: "lp"', lowered)
        self.assertNotIn('segment: "investor"', lowered)
        self.assertNotIn('segment: "k1"', lowered)
        self.assertNotIn("lp-portal", lowered)
        self.assertIn('segment: "capital"', investment)
        self.assertIn('segment: "nav"', investment)

    def test_the_kernel_did_not_grow_an_lp_portal_rpc(self):
        if PROTO is None or not PROTO.is_file():
            self.skipTest("console.proto not handed to the test")
        src = PROTO.read_text()
        self.assertIn("message CapitalNotice", src)
        self.assertIn("message PartnerShare", src)
        self.assertIn("net_asset_value", src)
        for field in p.PARTNER_PROTO_FIELDS:
            self.assertIn(field, src)
        for field in p.NOTICE_PROTO_FIELDS:
            self.assertIn(field, src)
        for field in p.STRIKE_PROTO_FIELDS:
            self.assertIn(field, src)
        for needle in (
            "rpc LpPortal",
            "rpc InvestorPortal",
            "rpc ClientPortal",
            "message LpPortal",
            "message InvestorPortal",
        ):
            self.assertNotIn(
                needle,
                src,
                f"{needle} is a kernel portal RPC — refuse it; this app is the door",
            )

    def test_console_wire_types_still_name_the_cites_this_app_copies(self):
        if TYPES is None or not TYPES.is_file():
            self.skipTest("types.ts not handed to the test")
        src = TYPES.read_text()
        self.assertIn("export interface CapitalNotice", src)
        self.assertIn("export interface PartnerShare", src)
        self.assertIn("export interface NavStrike", src)
        self.assertIn("partnerCut", src)
        self.assertIn("netAssetValue", src)


if __name__ == "__main__":
    unittest.main()
