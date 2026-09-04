#!/usr/bin/env python3
"""Properties the audit evidence pack must keep.

Test names are sentences. Break the thing a test protects and this
file goes red — a green suite that only checks the books balance is
how a missing digest ships as history-intact.

Does not talk to /v1. Does not claim a Connect token is accepted.
Does not deliver a live ZIP.
"""

from __future__ import annotations

import io
import json
import pathlib
import sys
import unittest
import zipfile
from datetime import date

import pack as p

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
    )


def journal(**overrides) -> p.JournalCite:
    return p.JournalCite(
        position=overrides.get("position", 12),
        digest=overrides.get("digest", "cafe" * 8),
    )


def config(**overrides) -> p.ConfigCite:
    return p.ConfigCite(
        digest=overrides.get("digest", "abba" * 8),
        lot_method=overrides.get("lot_method"),
        long_term_days=overrides.get("long_term_days"),
        wash_window_days=overrides.get("wash_window_days"),
        wash_keep_holding_period=overrides.get("wash_keep_holding_period"),
        min_tax_short_weight=overrides.get("min_tax_short_weight"),
        average_cost=overrides.get("average_cost"),
    )


def close(**overrides) -> p.CloseCite:
    return p.CloseCite(
        name=overrides.get("name", "2026-03-31"),
        view=overrides.get("view", "ib"),
        closed_date=overrides.get("closed_date", date(2026, 3, 31)),
        actor=overrides.get("actor", "user_admin"),
        journal_position=overrides.get("journal_position", 12),
        journal_digest=overrides.get("journal_digest", "cafe" * 8),
        config_digest=overrides.get("config_digest", "abba" * 8),
        closing_entry=overrides.get("closing_entry", "close-2026-03"),
        equity_destination=overrides.get("equity_destination", "25"),
        surplus=overrides.get("surplus", 10_000),
    )


def strike(**overrides) -> p.StrikeCite:
    return p.StrikeCite(
        name=overrides.get("name", "2026-03-31"),
        view=overrides.get("view", "ib"),
        valuation_time=overrides.get("valuation_time", "2026-03-31T21:00:00Z"),
        actor=overrides.get("actor", "user_admin"),
        journal_position=overrides.get("journal_position", 12),
        journal_digest=overrides.get("journal_digest", "cafe" * 8),
        net_asset_value=overrides.get("net_asset_value", 100_000),
        trial_balance_difference=overrides.get("trial_balance_difference", 0),
        config_digest=overrides.get("config_digest", "abba" * 8),
        qualification=overrides.get("qualification", ()),
        wash_qualified=overrides.get("wash_qualified", False),
        wash_restatement_original=overrides.get("wash_restatement_original"),
        wash_restatement_moved=overrides.get("wash_restatement_moved"),
    )


def explanation(**overrides) -> p.ExplanationCite:
    return p.ExplanationCite(
        text=overrides.get("text", "custodian unsettled dividend"),
        actor=overrides.get("actor", "user_admin"),
        accept_time=overrides.get("accept_time", "2026-03-30T12:00:00Z"),
        difference=overrides.get("difference", 200_000),
        config_digest=overrides.get("config_digest", "abba" * 8),
        journal_position=overrides.get("journal_position", 11),
        journal_digest=overrides.get("journal_digest", "beef" * 8),
    )


def brk(**overrides) -> p.BreakCite:
    return p.BreakCite(
        name=overrides.get("name", "cash"),
        account=overrides.get("account", "Cash"),
        severity=overrides.get("severity", "HIGH"),
        explained=overrides.get("explained", True),
        cause=overrides.get("cause", "custodian cash disagrees"),
        ratio_amount=overrides.get("ratio_amount", 500_000),
        reported_amount=overrides.get("reported_amount", 300_000),
        difference=overrides.get("difference", 200_000),
        config_digest=overrides.get("config_digest", "abba" * 8),
        explanation=overrides.get("explanation", explanation()),
    )


def pack_of(**kwargs) -> p.Pack:
    return p.build_pack(
        book=kwargs.get("book", book()),
        client=kwargs.get("client", declared_client()),
        journal=kwargs.get("journal", journal()),
        config=kwargs.get("config", config()),
        closes=kwargs.get("closes", (close(),)),
        strikes=kwargs.get("strikes", (strike(),)),
        breaks=kwargs.get("breaks", (brk(),)),
    )


class ParseMinor(unittest.TestCase):
    def test_an_amount_is_parsed_by_splitting_not_by_floating(self):
        self.assertEqual(p.parse_minor("1000.00"), 100_000)
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

    def test_a_signed_surplus_is_a_real_loss_not_an_inference(self):
        self.assertEqual(p.parse_minor("-40.00", allow_signed=True), -4_000)

    def test_a_zero_amount_is_a_real_zero_not_a_missing_cite(self):
        self.assertEqual(p.parse_minor("0.00"), 0)
        self.assertIsNone(p.parse_optional_minor(""))
        self.assertIsNone(p.parse_optional_minor(None))
        self.assertEqual(p.parse_optional_minor("0.00"), 0)

    def test_an_amount_that_does_not_fit_i64_is_refused(self):
        with self.assertRaises(p.Refuse):
            p.parse_minor("92233720368547758.08")


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


class CiteMapping(unittest.TestCase):
    def test_a_period_close_cite_copies_the_kernel_fields(self):
        c = p.close_from_cite(
            {
                "name": "2026-03-31",
                "view": "ib",
                "closed_date": "2026-03-31",
                "actor": "user_admin",
                "journal_position": 12,
                "journal_digest": "cafe" * 8,
                "config_digest": "abba" * 8,
                "closing_entry": "close-2026-03",
                "equity_destination": "25",
                "surplus": "100.00",
            }
        )
        self.assertEqual(c.closed_date, date(2026, 3, 31))
        self.assertEqual(c.journal_position, 12)
        self.assertEqual(c.surplus, 10_000)
        self.assertEqual(c.journal_digest, "cafe" * 8)

    def test_a_proto_date_object_is_accepted_as_the_close_day(self):
        c = p.close_from_cite(
            {
                "name": "2026-03-31",
                "view": "ib",
                "closed_date": {"year": 2026, "month": 3, "day": 31},
                "surplus": "0.00",
            }
        )
        self.assertEqual(c.closed_date, date(2026, 3, 31))
        self.assertEqual(c.surplus, 0)

    def test_an_empty_surplus_stays_unset_not_a_measured_zero(self):
        c = p.close_from_cite({"name": "2026-03-31", "view": "ib", "surplus": ""})
        self.assertIsNone(c.surplus)

    def test_a_nav_strike_cite_does_not_rewrite_a_wash_restatement(self):
        s = p.strike_from_cite(
            {
                "name": "2026-03-31",
                "view": "ib",
                "net_asset_value": "1000.00",
                "wash_qualified": True,
                "wash_restatement_original": "40.00",
                "wash_restatement_moved": "10.00",
                "journal_digest": "cafe" * 8,
            }
        )
        self.assertEqual(s.net_asset_value, 100_000)
        self.assertTrue(s.wash_qualified)
        self.assertEqual(s.wash_restatement_original, 4_000)
        self.assertEqual(s.wash_restatement_moved, 1_000)

    def test_a_missing_nav_stays_unset_not_zero(self):
        s = p.strike_from_cite({"name": "2026-03-31", "view": "ib"})
        self.assertIsNone(s.net_asset_value)

    def test_a_break_explanation_is_copied_not_invented(self):
        b = p.break_from_cite(
            {
                "name": "cash",
                "account": "Cash",
                "difference": "2000.00",
                "explanation": {
                    "text": "custodian unsettled dividend",
                    "actor": "user_admin",
                    "difference": "2000.00",
                },
            }
        )
        self.assertIsNotNone(b.explanation)
        assert b.explanation is not None
        self.assertEqual(b.explanation.actor, "user_admin")
        self.assertEqual(b.explanation.difference, 200_000)

    def test_an_explanation_without_an_actor_is_refused_not_invented(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.break_from_cite(
                {
                    "name": "cash",
                    "account": "Cash",
                    "explanation": {"text": "looks fine", "actor": ""},
                }
            )
        self.assertIn("person-attributed", str(ctx.exception))


class PackShape(unittest.TestCase):
    def test_a_fixture_close_maps_to_closes_csv(self):
        out = pack_of()
        files = p.as_files(out)
        self.assertIn("closes.csv", files)
        self.assertIn("2026-03-31", files["closes.csv"])
        self.assertIn("100.00", files["closes.csv"])
        self.assertIn("cafe" * 8, files["closes.csv"])

    def test_a_fixture_strike_maps_to_strikes_csv_without_rewriting_nav(self):
        out = pack_of(
            strikes=(
                strike(
                    wash_qualified=True,
                    wash_restatement_original=4_000,
                    wash_restatement_moved=1_000,
                ),
            )
        )
        files = p.as_files(out)
        self.assertIn("1000.00", files["strikes.csv"])
        self.assertIn("40.00", files["strikes.csv"])
        self.assertIn("10.00", files["strikes.csv"])
        self.assertEqual(out.strikes[0].net_asset_value, 100_000)

    def test_a_fixture_break_maps_to_breaks_csv_with_the_person(self):
        out = pack_of()
        files = p.as_files(out)
        self.assertIn("user_admin", files["breaks.csv"])
        self.assertIn("custodian unsettled dividend", files["breaks.csv"])

    def test_a_missing_close_cite_is_unset_not_a_silent_empty_file(self):
        out = pack_of(closes=None)
        files = p.as_files(out)
        self.assertNotIn("closes.csv", files)
        self.assertEqual(out.sections["closes"].status, "unset")
        self.assertTrue(any(line.startswith("closes:") for line in out.unset))
        self.assertIn("closes", files["unset.csv"])

    def test_a_cited_empty_close_list_is_not_a_fake_closed_period(self):
        out = pack_of(closes=())
        files = p.as_files(out)
        self.assertNotIn("closes.csv", files)
        self.assertEqual(out.sections["closes"].status, "cited-empty")
        self.assertTrue(any("fake closed period" in line for line in out.unset))

    def test_a_missing_strike_cite_is_unset_not_nav_zero(self):
        out = pack_of(strikes=None)
        files = p.as_files(out)
        self.assertNotIn("strikes.csv", files)
        self.assertEqual(out.sections["strikes"].status, "unset")
        self.assertTrue(any(line.startswith("strikes:") for line in out.unset))
        self.assertIn("NAV of 0.00", out.sections["strikes"].note)

    def test_a_cited_empty_strike_list_is_not_nav_zero(self):
        out = pack_of(strikes=())
        self.assertEqual(out.sections["strikes"].status, "cited-empty")
        self.assertNotIn("strikes.csv", p.as_files(out))
        self.assertTrue(any("NAV 0.00" in line for line in out.unset))

    def test_a_missing_break_report_is_unset_not_a_silent_reconciled_file(self):
        out = pack_of(breaks=None)
        files = p.as_files(out)
        self.assertNotIn("breaks.csv", files)
        self.assertEqual(out.sections["breaks"].status, "unset")
        self.assertTrue(any(line.startswith("breaks:") for line in out.unset))
        self.assertIn("silent reconciled", out.sections["breaks"].note)

    def test_a_cited_empty_break_report_is_reconciled_and_the_manifest_says_so(self):
        out = pack_of(breaks=())
        files = p.as_files(out)
        self.assertNotIn("breaks.csv", files)
        self.assertEqual(out.sections["breaks"].status, "cited-empty")
        self.assertIn("reconciled", out.sections["breaks"].note)
        self.assertIn("cited-empty", files["manifest.json"])

    def test_an_empty_journal_digest_is_unset_not_history_intact(self):
        out = pack_of(journal=journal(digest=None))
        files = p.as_files(out)
        self.assertNotIn("journal.csv", files)
        self.assertTrue(any("journal.digest" in line for line in out.unset))
        manifest = json.loads(files["manifest.json"])
        self.assertNotEqual(manifest["sections"]["journal"]["note"].find("not history-intact"), -1)

    def test_a_missing_config_digest_is_unset_not_a_pin(self):
        out = pack_of(config=config(digest=None))
        files = p.as_files(out)
        self.assertNotIn("config.csv", files)
        self.assertEqual(out.sections["config"].status, "unset")
        self.assertTrue(any("config.digest" in line for line in out.unset))

    def test_a_posted_zero_surplus_is_a_figure(self):
        out = pack_of(closes=(close(surplus=0),))
        files = p.as_files(out)
        self.assertIn("0.00", files["closes.csv"])
        self.assertFalse(any("surplus: unset" in line for line in out.unset))

    def test_a_personal_book_leaves_strikes_unset_when_none_are_cited(self):
        out = pack_of(book=book(kind="PERSONAL"), strikes=None)
        self.assertEqual(out.book.kind, "PERSONAL")
        self.assertEqual(out.sections["strikes"].status, "unset")
        self.assertNotIn("strikes.csv", p.as_files(out))

    def test_the_zip_contains_only_cited_sheets_plus_manifest_and_unset(self):
        out = pack_of(closes=None, strikes=None, breaks=())
        raw = p.as_zip(out)
        with zipfile.ZipFile(io.BytesIO(raw)) as zf:
            names = set(zf.namelist())
        self.assertEqual(
            names,
            {"manifest.json", "unset.csv", "journal.csv", "config.csv"},
        )
        self.assertNotIn("closes.csv", names)
        self.assertNotIn("strikes.csv", names)
        self.assertNotIn("breaks.csv", names)

    def test_a_full_cite_zip_names_every_sheet(self):
        raw = p.as_zip(pack_of())
        with zipfile.ZipFile(io.BytesIO(raw)) as zf:
            names = set(zf.namelist())
        self.assertTrue(
            {
                "manifest.json",
                "unset.csv",
                "journal.csv",
                "config.csv",
                "closes.csv",
                "strikes.csv",
                "breaks.csv",
            }.issubset(names)
        )


class Refusals(unittest.TestCase):
    def test_journal_read_is_refused_as_an_alias(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                client=declared_client(
                    scopes=frozenset({"audit:export", "journal:read"})
                )
            )
        self.assertIn("journal:read", str(ctx.exception))

    def test_journal_append_is_refused_as_an_alias(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                client=declared_client(
                    scopes=frozenset({"audit:export", "journal:append"})
                )
            )
        self.assertIn("journal:append", str(ctx.exception))

    def test_journals_post_is_refused_because_this_app_is_read_only(self):
        scopes = set(p.CANONICAL_SCOPES) | {"journals:post"}
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(client=declared_client(scopes=frozenset(scopes)))
        self.assertIn("journals:post", str(ctx.exception))

    def test_missing_audit_export_is_refused(self):
        scopes = set(p.CANONICAL_SCOPES) - {"audit:export"}
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(client=declared_client(scopes=frozenset(scopes)))
        self.assertIn("audit:export", str(ctx.exception))

    def test_missing_closes_read_is_refused(self):
        scopes = set(p.CANONICAL_SCOPES) - {"closes:read"}
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(client=declared_client(scopes=frozenset(scopes)))
        self.assertIn("closes:read", str(ctx.exception))

    def test_a_non_member_book_is_refused_even_when_org_id_matches(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(book=book(member=False, org_id="org_shared"))
        msg = str(ctx.exception)
        self.assertIn("membership", msg)
        self.assertIn("org_id", msg)

    def test_unspecified_is_refused_as_a_hidden_fifth_kind(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(book=book(kind="UNSPECIFIED"))
        self.assertIn("UNSPECIFIED", str(ctx.exception))

    def test_lot_method_min_tax_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(config=config(lot_method="min_tax"))
        self.assertIn("min_tax", str(ctx.exception))
        self.assertIn("election", str(ctx.exception))

    def test_lot_method_wash_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(config=config(lot_method="wash"))
        self.assertIn("wash", str(ctx.exception))

    def test_fetch_cites_refuses_because_the_grant_path_is_not_built(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.fetch_cites(token="connect-access-token")
        msg = str(ctx.exception)
        self.assertIn("grant path is not built", msg)
        self.assertIn("#22", msg)
        self.assertIn("not a live token", msg)

    def test_deliver_refuses_because_the_grant_path_is_not_built(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.deliver(pack_of(), token="connect-access-token")
        self.assertIn("grant path is not built", str(ctx.exception))

    def test_store_blob_is_refused_because_there_is_no_kernel_blob_store(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.store_blob(b"zip")
        self.assertIn("blob store", str(ctx.exception))

    def test_close_period_is_refused_because_this_app_does_not_replace_close(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.close_period()
        self.assertIn("period close", str(ctx.exception))

    def test_lp_portal_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.lp_portal()
        self.assertIn("#161", str(ctx.exception))

    def test_esign_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.esign()
        self.assertIn("e-sign", str(ctx.exception))

    def test_a_second_journal_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.second_journal()
        self.assertIn("second journal", str(ctx.exception))


class ManifestHonesty(unittest.TestCase):
    def test_the_app_declares_only_canonical_scopes(self):
        scopes = app()["workos_connect"]["scopes"]
        self.assertEqual(set(scopes), set(p.CANONICAL_SCOPES))
        for alias in p.REFUSED_ALIASES:
            self.assertNotIn(alias, scopes)
        self.assertNotIn("journals:post", scopes)
        self.assertNotIn("journal:append", scopes)
        self.assertNotIn("journal:read", scopes)
        self.assertEqual(app()["journals_post_allowlist"]["templates"], [])

    def test_grant_path_and_blob_store_stay_named_as_leftovers(self):
        doc = json.dumps(app())
        self.assertIn("not built", app()["grant_path"]["status"])
        self.assertIn("refused", app()["blob_store"]["status"])
        self.assertIn("refused", app()["lp_portal"]["status"])
        self.assertIn("#185", doc)
        self.assertIn("#150", doc)
        self.assertIn("#22", doc)
        self.assertIn("does not reopen #151", doc)
        self.assertEqual(app()["issue"], 185)

    def test_the_catalog_still_names_the_scopes_this_app_requests(self):
        if CATALOG is None or not CATALOG.is_file():
            self.skipTest("connect-scopes.md not handed to the test")
        text = CATALOG.read_text()
        for scope in p.CANONICAL_SCOPES:
            self.assertIn(f"`{scope}`", text)
        self.assertIn("`journal:read`", text)
        self.assertIn("leftover #22", text)
        self.assertIn("audit:export", text)

    def test_config_pin_field_names_are_the_ones_ruleset_already_stores(self):
        if RULES_RS is None or not RULES_RS.is_file():
            self.skipTest("ratio-rules lib.rs not handed to the test")
        src = RULES_RS.read_text()
        for field in p.CONFIG_PIN_FIELDS:
            self.assertIn(
                f"pub {field}:",
                src,
                f"{field} is not a RuleSet field — the pack invented an election",
            )

    def test_screens_for_was_not_forked_with_an_audit_zip_url(self):
        if SCREENS is None or not SCREENS.is_file():
            self.skipTest("screens.ts not handed to the test")
        src = SCREENS.read_text()
        for needle in ("audit-export", "audit_export", "evidence.zip", "evidence-pack"):
            self.assertNotIn(needle, src.lower())

    def test_the_kernel_did_not_grow_an_audit_export_rpc(self):
        if PROTO is None or not PROTO.is_file():
            self.skipTest("console.proto not handed to the test")
        src = PROTO.read_text()
        self.assertIn("rpc ListPeriodCloses", src)
        self.assertIn("rpc GetPeriodClose", src)
        self.assertIn("message NavStrike", src)
        self.assertIn("message BreakExplanation", src)
        for needle in (
            "rpc AuditExport",
            "rpc ExportEvidence",
            "rpc EvidencePack",
            "message AuditExport",
            "message EvidenceZip",
        ):
            self.assertNotIn(
                needle,
                src,
                f"{needle} is a kernel audit RPC — refuse it; this app is the door",
            )

    def test_the_wire_types_still_name_the_cites_this_pack_copies(self):
        if TYPES is None or not TYPES.is_file():
            self.skipTest("types.ts not handed to the test")
        src = TYPES.read_text()
        self.assertIn("export interface PeriodClose", src)
        self.assertIn("export interface NavStrike", src)
        self.assertIn("export interface BreakExplanation", src)
        self.assertIn("journalDigest", src)
        self.assertIn("configDigest", src)
        self.assertIn("washRestatementOriginal", src)
        self.assertIn("missing_surplus_is_unset", src)

    def test_a_break_report_empty_list_is_still_the_kernel_reconciled_meaning(self):
        if RECON is None or not RECON.is_file():
            self.skipTest("recon.proto not handed to the test")
        src = RECON.read_text()
        self.assertIn("message BreakReport", src)
        self.assertIn("Empty means the period reconciled", src)

    def test_period_close_and_nav_strike_field_names_match_the_proto(self):
        if PROTO is None or not PROTO.is_file():
            self.skipTest("console.proto not handed to the test")
        src = PROTO.read_text()
        for field in p.CLOSE_PROTO_FIELDS:
            self.assertIn(field, src)
        for field in p.STRIKE_PROTO_FIELDS:
            self.assertIn(field, src)
        for field in p.BREAK_PROTO_FIELDS:
            self.assertIn(field, src)
        for field in p.EXPLANATION_PROTO_FIELDS:
            self.assertIn(field, src)


if __name__ == "__main__":
    unittest.main()
