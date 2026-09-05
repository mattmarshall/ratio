#!/usr/bin/env python3
"""Properties the Operating bank-rec Connect app must keep.

Test names are sentences. Break the thing a test protects and this
file goes red — a green suite that only checks the books balance is
how a silent cleared $0.00 ships.

Does not talk to /v1 except through the shared grant helper.
Does not grow a kernel BankRec RPC. Does not invent payroll.
"""

from __future__ import annotations

import json
import os
import pathlib
import sys
import unittest
from datetime import date
from unittest import mock

import rec as r

APP_PATH = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).with_name("app.json")
BOOK_RS = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else None
CATALOG = pathlib.Path(sys.argv[3]) if len(sys.argv) > 3 else None
SCREENS = pathlib.Path(sys.argv[4]) if len(sys.argv) > 4 else None
PROTO = pathlib.Path(sys.argv[5]) if len(sys.argv) > 5 else None
TYPES = pathlib.Path(sys.argv[6]) if len(sys.argv) > 6 else None

sys.argv = sys.argv[:1]


def app() -> dict:
    return json.loads(APP_PATH.read_text())


def declared_client(**overrides) -> r.Client:
    c = r.client_from_app(app())
    return r.Client(
        client_id=overrides.get("client_id", c.client_id),
        allowlist=overrides.get("allowlist", c.allowlist),
        scopes=overrides.get("scopes", c.scopes),
    )


def operating(**overrides) -> r.Book:
    return r.Book(
        kind=overrides.get("kind", "OPERATING"),
        member=overrides.get("member", True),
        org_id=overrides.get("org_id"),
        approved_templates=overrides.get(
            "approved_templates", r.OPERATING_SEEDED_RULES
        ),
        closed_through=overrides.get("closed_through"),
    )


def report(**overrides) -> r.Report:
    base = dict(
        book=operating(),
        client=declared_client(),
        book_cash="1000.00",
        bank_ending="800.00",
        receivable={"control": "250.00"},
        payable={"control": "100.00"},
        journal_digest="cafe" * 8,
        journal_position=12,
        closed_through="2026-03-31",
        as_of="2026-04-30",
        currency="USD",
    )
    base.update(overrides)
    return r.reconcile(**base)


def move(**overrides) -> dict:
    base = {
        "dated": "2026-04-15",
        "amount": "25.00",
        "currency": "USD",
        "kind": "expense",
        "reference": "bank-fee-1",
    }
    base.update(overrides)
    return base


class ParseMinor(unittest.TestCase):
    def test_an_amount_is_parsed_by_splitting_not_by_floating(self):
        self.assertEqual(r.parse_minor("1000.00"), 100_000)
        self.assertEqual(r.parse_minor("0.10"), 10)
        self.assertEqual(r.parse_minor("0.1"), 10)
        self.assertEqual(r.parse_minor("$1,204.11"), 120_411)

    def test_three_decimal_places_are_refused_rather_than_dropped(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.parse_minor("1.005")
        self.assertIn("minor units", str(ctx.exception))

    def test_a_float_object_is_refused(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.parse_minor(1.005)  # type: ignore[arg-type]
        self.assertIn("float", str(ctx.exception))

    def test_a_zero_amount_is_not_a_posting(self):
        with self.assertRaises(r.Refuse):
            r.parse_minor("0.00", allow_zero=False)

    def test_a_cited_zero_is_a_figure(self):
        self.assertEqual(r.parse_optional_minor("0.00"), 0)
        self.assertIsNone(r.parse_optional_minor(""))
        self.assertIsNone(r.parse_optional_minor(None))


class ReconReport(unittest.TestCase):
    def test_a_mismatch_is_open_and_cites_the_difference(self):
        out = report()
        self.assertEqual(out.book_cash, 100_000)
        self.assertEqual(out.bank_ending, 80_000)
        self.assertEqual(out.difference, 20_000)
        self.assertEqual(out.remaining, 20_000)
        self.assertEqual(out.status, "open")
        self.assertEqual(out.open_ar, 25_000)
        self.assertEqual(out.open_ap, 10_000)
        self.assertEqual(out.journal_digest, "cafe" * 8)

    def test_named_outstanding_deposits_explain_the_difference(self):
        out = report(
            outstanding=[{"kind": "deposit", "amount": "200.00", "reference": "dep-1"}]
        )
        self.assertEqual(out.outstanding_net, 20_000)
        self.assertEqual(out.remaining, 0)
        self.assertEqual(out.status, "tied")
        self.assertEqual(out.outstanding[0].kind, "deposit")

    def test_an_outstanding_check_widens_book_over_bank(self):
        out = report(
            outstanding=[{"kind": "check", "amount": "50.00", "reference": "chk-1"}]
        )
        self.assertEqual(out.outstanding_net, -5_000)
        self.assertEqual(out.remaining, 25_000)
        self.assertEqual(out.status, "open")

    def test_a_real_zero_book_and_bank_with_a_digest_is_tied(self):
        out = report(book_cash="0.00", bank_ending="0.00")
        self.assertEqual(out.book_cash, 0)
        self.assertEqual(out.bank_ending, 0)
        self.assertEqual(out.difference, 0)
        self.assertEqual(out.status, "tied")

    def test_a_missing_book_cash_leaves_status_unset_not_cleared_zero(self):
        out = report(book_cash=None)
        self.assertIsNone(out.book_cash)
        self.assertIsNone(out.difference)
        self.assertEqual(out.status, "unset")
        self.assertTrue(any("book cash" in u for u in out.unset))
        self.assertNotEqual(out.status, "tied")

    def test_a_missing_bank_statement_is_unset_not_reconciled_empty(self):
        out = report(bank_ending=None)
        self.assertIsNone(out.bank_ending)
        self.assertIsNone(out.difference)
        self.assertEqual(out.status, "unset")
        self.assertTrue(any("bank ending" in u for u in out.unset))
        self.assertTrue(any("reconciled-empty" in u for u in out.unset))

    def test_an_empty_digest_is_unset_not_success(self):
        out = report(journal_digest="")
        self.assertIsNone(out.journal_digest)
        self.assertEqual(out.status, "unset")
        self.assertTrue(any("digest" in u for u in out.unset))
        self.assertTrue(any("success" in u for u in out.unset))
        # Cash figures are still cited — the pin is what is missing.
        self.assertEqual(out.book_cash, 100_000)
        self.assertEqual(out.difference, 20_000)

    def test_open_ar_is_not_a_silent_reconciling_item(self):
        out = report(receivable={"control": "200.00"})
        self.assertEqual(out.open_ar, 20_000)
        self.assertEqual(out.difference, 20_000)
        self.assertEqual(out.remaining, 20_000)
        self.assertEqual(out.status, "open")
        self.assertEqual(out.outstanding, ())

    def test_unset_aging_stays_unset_not_ar_zero(self):
        out = report(receivable=None, payable=None)
        self.assertIsNone(out.open_ar)
        self.assertIsNone(out.open_ap)
        self.assertTrue(any("open AR" in u for u in out.unset))
        self.assertTrue(any("open AP" in u for u in out.unset))
        self.assertTrue(any("reconciling item" in u for u in out.unset))

    def test_a_real_zero_aging_control_is_a_figure(self):
        out = report(receivable={"control": "0.00"}, payable={"control": "0.00"})
        self.assertEqual(out.open_ar, 0)
        self.assertEqual(out.open_ap, 0)
        self.assertFalse(any("open AR" in u for u in out.unset))

    def test_ar_ap_cannot_be_passed_as_outstanding_kind(self):
        with self.assertRaises(r.Refuse) as ctx:
            report(outstanding=[{"kind": "ar", "amount": "200.00", "reference": "inv-1"}])
        self.assertIn("AR/AP", str(ctx.exception))

    def test_missing_statements_read_is_refused(self):
        with self.assertRaises(r.Refuse) as ctx:
            report(client=declared_client(scopes=frozenset({"journals:post"})))
        self.assertIn("statements:read", str(ctx.exception))

    def test_journal_append_is_refused_as_a_scope(self):
        with self.assertRaises(r.Refuse) as ctx:
            report(
                client=declared_client(
                    scopes=frozenset({"statements:read", "journal:append"})
                )
            )
        self.assertIn("journal:append", str(ctx.exception))
        self.assertIn("journals:post", str(ctx.exception))

    def test_an_investment_book_is_refused(self):
        with self.assertRaises(r.Refuse) as ctx:
            report(book=operating(kind="INVESTMENT"))
        self.assertIn("OPERATING", str(ctx.exception))

    def test_a_non_member_with_a_matching_org_id_is_refused(self):
        with self.assertRaises(r.Refuse) as ctx:
            report(book=operating(member=False, org_id="org_demo"))
        self.assertIn("org_id", str(ctx.exception))
        self.assertIn("membership", str(ctx.exception))

    def test_report_does_not_require_journals_post(self):
        out = report(client=declared_client(scopes=frozenset({"statements:read"})))
        self.assertEqual(out.status, "open")


class OptInPosts(unittest.TestCase):
    def test_non_opt_in_must_not_post(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.propose_recon_posts(
                [move()],
                book=operating(),
                client=declared_client(),
                opt_in=False,
            )
        self.assertIn("opts in", str(ctx.exception))
        self.assertIn("must not post", str(ctx.exception))

    def test_opt_in_proposes_an_apply_event_shape_not_a_posting_list(self):
        out = r.propose_recon_posts(
            [move()],
            book=operating(),
            client=declared_client(),
            opt_in=True,
        )
        self.assertEqual(len(out), 1)
        self.assertEqual(out[0].rule_id, "pay_expense")
        self.assertTrue(r.conserves(out[0].postings))
        wire = r.as_apply_event(out[0], parent="books/studio")
        self.assertEqual(wire["rule_id"], "pay_expense")
        self.assertEqual(wire["amount"], "25.00")
        self.assertTrue(wire["validate_only"])
        self.assertNotIn("postings", wire)

    def test_an_empty_allowlist_refuses_every_post(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.propose_recon_posts(
                [move()],
                book=operating(),
                client=declared_client(allowlist=frozenset()),
                opt_in=True,
            )
        self.assertIn("empty", str(ctx.exception))
        self.assertIn("allowlist", str(ctx.exception))

    def test_a_template_off_the_allowlist_is_refused(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.propose_recon_posts(
                [move(kind="contribute")],
                book=operating(),
                client=declared_client(allowlist=frozenset({"pay_expense"})),
                opt_in=True,
            )
        self.assertIn("allowlist", str(ctx.exception))

    def test_a_dated_entry_on_or_before_closed_through_is_refused(self):
        book = operating(closed_through=date(2026, 3, 31))
        with self.assertRaises(r.Refuse) as ctx:
            r.propose_recon_posts(
                [move(dated="2026-03-31")],
                book=book,
                client=declared_client(),
                opt_in=True,
            )
        self.assertIn("closed-through", str(ctx.exception))
        with self.assertRaises(r.Refuse) as ctx:
            r.propose_recon_posts(
                [move(dated="2026-03-15")],
                book=book,
                client=declared_client(),
                opt_in=True,
            )
        self.assertIn("closed-through", str(ctx.exception))

    def test_the_day_after_close_is_accepted_when_opted_in(self):
        book = operating(closed_through=date(2026, 3, 31))
        out = r.propose_recon_posts(
            [move(dated="2026-04-01")],
            book=book,
            client=declared_client(),
            opt_in=True,
        )
        self.assertEqual(out[0].trade_date, date(2026, 4, 1))

    def test_an_undated_row_is_refused(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.propose_recon_posts(
                [move(dated="")],
                book=operating(),
                client=declared_client(),
                opt_in=True,
            )
        self.assertIn("undated", str(ctx.exception))

    def test_a_closed_row_refuses_the_whole_batch(self):
        book = operating(closed_through=date(2026, 3, 31))
        with self.assertRaises(r.Refuse):
            r.propose_recon_posts(
                [
                    move(dated="2026-04-02", reference="open"),
                    move(dated="2026-03-15", reference="closed"),
                ],
                book=book,
                client=declared_client(),
                opt_in=True,
            )

    def test_call_lp_is_refused_on_an_operating_book(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.propose_recon_posts(
                [move(kind="call_lp")],
                book=operating(),
                client=declared_client(),
                opt_in=True,
            )
        self.assertIn("call_lp", str(ctx.exception))

    def test_invoice_customer_is_not_a_recon_adjustment(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.propose_recon_posts(
                [move(kind="invoice")],
                book=operating(),
                client=declared_client(),
                opt_in=True,
            )
        self.assertIn("invoice_customer", str(ctx.exception))

    def test_journals_post_is_required_to_propose_a_write(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.propose_recon_posts(
                [move()],
                book=operating(),
                client=declared_client(scopes=frozenset({"statements:read"})),
                opt_in=True,
            )
        self.assertIn("journals:post", str(ctx.exception))

    def test_usd_plus_eur_minus_is_not_conserved(self):
        posts = (
            r.Posting(10, 100, "USD"),
            r.Posting(1, -100, "EUR"),
        )
        self.assertFalse(r.conserves(posts))

    def test_an_unbalanced_instantiation_refuses_the_batch(self):
        saved = r.OPERATING_CASH_LEGS["pay_expense"]
        r.OPERATING_CASH_LEGS["pay_expense"] = ((10, 1), (1, 1))
        try:
            with self.assertRaises(r.Refuse) as ctx:
                r.propose_recon_posts(
                    [move()],
                    book=operating(),
                    client=declared_client(),
                    opt_in=True,
                )
            self.assertIn("conserve", str(ctx.exception))
        finally:
            r.OPERATING_CASH_LEGS["pay_expense"] = saved


class LeftoverRefusals(unittest.TestCase):
    def test_payroll_is_refused(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.payroll()
        self.assertIn("payroll", str(ctx.exception))
        self.assertIn("#174", str(ctx.exception))

    def test_tax_filing_is_refused(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.tax_filing()
        self.assertIn("tax", str(ctx.exception))
        self.assertIn("#174", str(ctx.exception))

    def test_inventory_and_cogs_are_refused(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.inventory()
        self.assertIn("COGS", str(ctx.exception))
        with self.assertRaises(r.Refuse) as ctx:
            r.cogs()
        self.assertIn("COGS", str(ctx.exception))

    def test_bank_oauth_is_refused_and_does_not_absorb_165(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.bank_oauth()
        self.assertIn("#165", str(ctx.exception))
        self.assertIn("does not absorb", str(ctx.exception))

    def test_a_kernel_recon_rpc_is_refused(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.kernel_recon()
        self.assertIn("kernel", str(ctx.exception))

    def test_fetch_cites_without_a_token_is_refused(self):
        env = {
            "RATIO_CONNECT_ACCESS_TOKEN": "",
            "WORKOS_CONNECT_CLIENT_ID": "",
            "WORKOS_CONNECT_CLIENT_SECRET": "",
        }
        with mock.patch.dict(os.environ, env, clear=False):
            with self.assertRaises(r.Refuse) as ctx:
                r.fetch_cites()
        self.assertIn("no Connect access token", str(ctx.exception))
        self.assertNotIn("grant path is not built", str(ctx.exception))

    def test_fetch_cites_pulls_connect_api_url_when_a_token_is_presented(self):
        transport = r._grant.FakeTransport(body='{"books":[]}')
        env = {"RATIO_CONNECT_API_URL": "https://connect.example"}
        with mock.patch.dict(os.environ, env, clear=False):
            self.assertEqual(
                r.fetch_cites(token="connect-access-token", transport=transport),
                {"books": []},
            )

    def test_deliver_posts_apply_event_to_connect_api_url_when_a_token_is_presented(self):
        out = r.propose_recon_posts(
            [move()],
            book=operating(),
            client=declared_client(),
            opt_in=True,
        )
        transport = r._grant.FakeTransport(body='{"name":"entries/1"}')
        env = {"RATIO_CONNECT_API_URL": "https://connect.example"}
        with mock.patch.dict(os.environ, env, clear=False):
            r.deliver(
                out,
                token="connect-access-token",
                parent="funds/alpha",
                transport=transport,
            )
        self.assertEqual(
            transport.calls[0][1],
            "https://connect.example/v1/funds/alpha:applyEvent",
        )


class RenderHonesty(unittest.TestCase):
    def test_csv_leaves_missing_cash_blank_and_names_it_on_unset(self):
        files = r.as_files(report(book_cash=None))
        amount = files["recon.csv"].splitlines()[1].split(",")[2]
        self.assertEqual(amount, "")
        self.assertIn("book cash", files["unset.csv"])
        self.assertIn("unset", files["recon.csv"])

    def test_json_keeps_missing_figures_null_not_zero(self):
        payload = r.as_json(report(book_cash=None, bank_ending=None, journal_digest=""))
        self.assertIsNone(payload["book_cash"])
        self.assertIsNone(payload["bank_ending"])
        self.assertIsNone(payload["journal_digest"])
        self.assertEqual(payload["status"], "unset")
        self.assertNotEqual(payload["book_cash"], "0.00")


class ManifestHonesty(unittest.TestCase):
    def test_the_app_declares_only_canonical_scopes(self):
        scopes = app()["workos_connect"]["scopes"]
        self.assertEqual(set(scopes), set(r.CANONICAL_SCOPES))
        for alias in r.REFUSED_ALIASES:
            self.assertNotIn(alias, scopes)
        self.assertIn("statements:read", scopes)
        self.assertIn("journals:post", scopes)
        self.assertNotIn("journal:append", scopes)

    def test_the_declared_allowlist_is_operating_cash_templates_not_methods(self):
        templates = set(app()["journals_post_allowlist"]["templates"])
        self.assertTrue(templates)
        self.assertTrue(templates <= set(r.OPERATING_CASH_LEGS))
        self.assertNotIn("invoice_customer", templates)
        self.assertNotIn("vendor_bill", templates)
        for forbidden in ("fifo", "hifo", "min_tax", "specific_id", "average_cost", "wash"):
            self.assertNotIn(forbidden, templates)
            self.assertFalse(any(forbidden in t for t in templates), forbidden)

    def test_grant_path_and_leftovers_stay_named(self):
        doc = json.dumps(app())
        self.assertEqual("built", app()["grant_path"]["status"])
        self.assertIn("ConnectApiUrl", app()["grant_path"]["note"])
        self.assertIn("WorkOS dashboard registration", app()["grant_path"]["note"])
        self.assertIn("opt-in only", app()["recon_adjustments"]["status"])
        self.assertIn("read-only by default", app()["recon_report"]["status"])
        self.assertEqual("refused", app()["payroll"]["status"])
        self.assertEqual("refused", app()["tax_filing"]["status"])
        self.assertEqual("refused", app()["inventory_cogs"]["status"])
        self.assertEqual(app()["issue"], 174)
        self.assertIn("#174", doc)
        self.assertIn("#150", doc)
        self.assertIn("#22", doc)
        self.assertIn("does not absorb #22", doc)
        self.assertIn("does not reopen #151", doc)

    def test_every_instantiated_template_is_in_createbook_operating(self):
        if BOOK_RS is None or not BOOK_RS.is_file():
            self.skipTest("book.rs not handed to the test")
        src = BOOK_RS.read_text()
        start = src.index("const OPERATING_CONFIG")
        operating_src = src[start:]
        for rule_id in r.OPERATING_CASH_LEGS:
            self.assertIn(
                f'id = "{rule_id}"',
                operating_src,
                f"{rule_id} is not a CreateBook(Operating) rule — the app invented it",
            )
        self.assertIn('id = "invoice_customer"', operating_src)
        self.assertIn('id = "vendor_bill"', operating_src)

    def test_the_catalog_still_refuses_the_alias_this_issue_named(self):
        if CATALOG is None or not CATALOG.is_file():
            self.skipTest("connect-scopes.md not handed to the test")
        text = CATALOG.read_text()
        self.assertIn("`journals:post`", text)
        self.assertIn("`journal:append`", text)
        self.assertIn("`statements:read`", text)
        self.assertIn("leftover #22", text)
        self.assertIn("An `org_id` claim is not membership", text)

    def test_screens_for_operating_was_not_forked_with_a_bank_rec_url(self):
        if SCREENS is None or not SCREENS.is_file():
            self.skipTest("screens.ts not handed to the test")
        src = SCREENS.read_text()
        start = src.index("export const OPERATING_SCREENS")
        end = src.index("export const SCREENS")
        operating = src[start:end]
        lowered = operating.lower()
        self.assertNotIn("bank-rec", lowered)
        self.assertNotIn("bankrec", lowered)
        self.assertNotIn("reconcil", lowered)
        self.assertNotIn("payroll", lowered)
        self.assertNotIn('segment: "tax"', lowered)
        self.assertIn('segment: "sheet"', operating)
        self.assertIn('segment: "cashflow"', operating)
        self.assertIn('segment: "aging"', operating)
        self.assertIn('segment: "accounts"', operating)

    def test_the_kernel_did_not_grow_a_bank_rec_or_payroll_rpc(self):
        if PROTO is None or not PROTO.is_file():
            self.skipTest("console.proto not handed to the test")
        src = PROTO.read_text()
        self.assertIn("rpc OperatingAging", src)
        self.assertIn("message AgingSchedule", src)
        self.assertIn("message PeriodClose", src)
        for field in r.AGING_PROTO_FIELDS:
            self.assertIn(field, src)
        for field in r.CLOSE_PROTO_FIELDS:
            self.assertIn(field, src)
        for needle in (
            "rpc BankRec",
            "rpc BankReconciliation",
            "rpc ReconcileBank",
            "rpc Payroll",
            "rpc FileTax",
            "rpc OperatingTax",
            "message BankRec",
            "message PayrollRun",
        ):
            self.assertNotIn(
                needle,
                src,
                f"{needle} is a kernel bank-rec / payroll RPC — refuse it; this app is the door",
            )

    def test_console_wire_types_still_name_the_cites_this_app_copies(self):
        if TYPES is None or not TYPES.is_file():
            self.skipTest("types.ts not handed to the test")
        src = TYPES.read_text()
        self.assertIn("export interface OperatingAgingResponse", src)
        self.assertIn("export interface AgingSchedule", src)
        self.assertIn("control", src)
        self.assertIn("journalPosition", src)


if __name__ == "__main__":
    unittest.main()
