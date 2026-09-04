#!/usr/bin/env python3
"""Properties the vendor / GC portal Connect app must keep.

Test names are sentences. Break the thing a test protects and this
file goes red — a green suite that only checks the books balance is
how a wrong remaining-to-bill ships.

Does not talk to /v1. Does not claim a Connect token is accepted.
Does not grow a vendor user directory or an AIA G702 route.
"""

from __future__ import annotations

import json
import pathlib
import sys
import unittest
from datetime import date

import portal as p

APP_PATH = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).with_name("app.json")
BOOK_RS = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else None
RULES_RS = pathlib.Path(sys.argv[3]) if len(sys.argv) > 3 else None
CATALOG = pathlib.Path(sys.argv[4]) if len(sys.argv) > 4 else None
SCREENS = pathlib.Path(sys.argv[5]) if len(sys.argv) > 5 else None
PROTO = pathlib.Path(sys.argv[6]) if len(sys.argv) > 6 else None

sys.argv = sys.argv[:1]


def app() -> dict:
    return json.loads(APP_PATH.read_text())


def declared_client(**overrides) -> p.Client:
    c = p.client_from_app(app())
    return p.Client(
        client_id=overrides.get("client_id", c.client_id),
        allowlist=overrides.get("allowlist", c.allowlist),
        scopes=overrides.get("scopes", c.scopes),
    )


def project(**overrides) -> p.Book:
    return p.Book(
        kind=overrides.get("kind", "PROJECT"),
        approved_templates=overrides.get("approved_templates", p.PROJECT_SEEDED_RULES),
        closed_through=overrides.get("closed_through"),
    )


def billing(**overrides) -> dict:
    base = {
        "billed": "1000.00",
        "earned": "800.00",
        "retainage_receivable": "100.00",
        "accounts_receivable": "500.00",
    }
    base.update(overrides)
    return base


def budget(**overrides) -> dict:
    base = {
        "original": "10000.00",
        "approved_change_orders": "500.00",
        "incurred": "2000.00",
        "awarded": "1500.00",
    }
    base.update(overrides)
    return base


def invoice(**overrides) -> dict:
    base = {
        "dated": "2026-04-15",
        "amount": "250.00",
        "currency": "USD",
        "kind": "site",
        "reference": "inv-1",
    }
    base.update(overrides)
    return base


def cite(**kwargs) -> p.Statement:
    return p.cite_statement(
        billing=kwargs.get("billing", billing()),
        budget=kwargs.get("budget", budget()),
        book=kwargs.get("book", project()),
        client=kwargs.get("client", declared_client()),
        currency=kwargs.get("currency", "USD"),
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

    def test_a_signed_amount_is_refused_so_a_hold_cannot_be_inferred(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.parse_minor("-40.00")
        self.assertIn("signed", str(ctx.exception))

    def test_a_zero_cite_is_a_real_zero_not_a_missing_figure(self):
        self.assertEqual(p.parse_minor("0.00"), 0)

    def test_a_zero_amount_is_not_a_posting(self):
        with self.assertRaises(p.Refuse):
            p.parse_minor("0.00", allow_zero=False)


class BillingCites(unittest.TestCase):
    def test_a_posted_job_cites_billed_earned_retainage_and_collections(self):
        out = cite()
        self.assertEqual(out.billed, 100_000)
        self.assertEqual(out.earned, 80_000)
        self.assertEqual(out.billed_minus_earned, 20_000)
        self.assertEqual(out.retainage_receivable, 10_000)
        self.assertEqual(out.collections, 40_000)  # 1000 − 500 − 100
        self.assertEqual(out.revised, 1_050_000)  # 10000 + 500
        self.assertEqual(out.remaining_to_bill, 950_000)  # 10500 − 1000
        self.assertEqual(out.remaining_to_spend, 700_000)  # 10500 − 2000 − 1500
        self.assertFalse(
            any(u.startswith("billed") for u in out.unset),
            out.unset,
        )

    def test_an_unbilled_job_leaves_billed_and_remaining_unset_not_the_whole_contract(self):
        out = cite(billing={"earned": "800.00"})
        self.assertIsNone(out.billed)
        self.assertIsNone(out.remaining_to_bill)
        self.assertIsNone(out.collections)
        self.assertIsNone(out.billed_minus_earned)
        self.assertTrue(any("unbilled" in u for u in out.unset), out.unset)
        self.assertTrue(any("whole contract" in u for u in out.unset), out.unset)

    def test_a_real_zero_billed_is_a_figure_not_unset(self):
        out = cite(billing={"billed": "0.00", "accounts_receivable": "0.00"})
        self.assertEqual(out.billed, 0)
        self.assertEqual(out.remaining_to_bill, 1_050_000)
        self.assertEqual(out.collections, 0)

    def test_unset_retainage_is_zero_for_collections_and_blank_on_the_line(self):
        out = cite(
            billing={
                "billed": "1000.00",
                "accounts_receivable": "500.00",
            }
        )
        self.assertIsNone(out.retainage_receivable)
        self.assertEqual(out.collections, 50_000)  # 1000 − 500 − 0
        self.assertTrue(any("retainage receivable" in u for u in out.unset), out.unset)

    def test_unset_ar_cannot_support_collections(self):
        out = cite(billing={"billed": "1000.00", "retainage_receivable": "100.00"})
        self.assertIsNone(out.collections)
        self.assertTrue(any("collections" in u for u in out.unset), out.unset)

    def test_an_unknown_baseline_leaves_remaining_unset(self):
        out = cite(budget={})
        self.assertIsNone(out.original)
        self.assertIsNone(out.revised)
        self.assertIsNone(out.remaining_to_bill)
        self.assertTrue(any("original contract" in u for u in out.unset), out.unset)

    def test_unposted_change_orders_leave_the_line_unset_and_revised_equals_original(self):
        out = cite(budget={"original": "10000.00", "incurred": "2000.00", "awarded": "1500.00"})
        self.assertIsNone(out.approved_change_orders)
        self.assertEqual(out.revised, 1_000_000)
        self.assertTrue(any("approved change orders" in u for u in out.unset), out.unset)

    def test_earned_is_not_substituted_for_billed(self):
        out = cite(billing={"earned": "800.00", "accounts_receivable": "500.00"})
        self.assertIsNone(out.billed)
        self.assertEqual(out.earned, 80_000)
        self.assertIsNone(out.billed_minus_earned)

    def test_a_currency_mismatch_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            cite(currency="US")
        self.assertIn("ISO", str(ctx.exception))

    def test_missing_billing_read_is_refused_because_the_job_cannot_be_cited(self):
        with self.assertRaises(p.Refuse) as ctx:
            cite(client=declared_client(scopes=frozenset({"budget:read", "statements:read"})))
        self.assertIn("billing:read", str(ctx.exception))

    def test_companion_sheets_name_unset_figures_and_do_not_invent_a_g702_box(self):
        files = p.as_files(cite(billing={}))
        self.assertEqual(set(files), {"billing.csv", "budget.csv", "unset.csv"})
        self.assertIn("unbilled", files["unset.csv"])
        self.assertNotIn("G702", files["billing.csv"])
        self.assertNotIn("%", files["billing.csv"])


class InvoicePosts(unittest.TestCase):
    def test_a_site_invoice_maps_to_vendor_invoice_site_and_conserves(self):
        out = p.propose_vendor_invoices(
            [invoice()],
            book=project(),
            client=declared_client(),
        )
        self.assertEqual(out[0].rule_id, "vendor_invoice_site")
        self.assertEqual(out[0].amount, "250.00")
        self.assertTrue(p.conserves(out[0].postings))
        accounts = {post.account for post in out[0].postings}
        self.assertEqual(accounts, {11, 40})

    def test_an_empty_allowlist_refuses_every_post(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.propose_vendor_invoices(
                [invoice()],
                book=project(),
                client=declared_client(allowlist=frozenset()),
            )
        self.assertIn("empty", str(ctx.exception))
        self.assertIn("allowlist", str(ctx.exception))

    def test_a_template_off_the_allowlist_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.propose_vendor_invoices(
                [invoice(kind="finishes")],
                book=project(),
                client=declared_client(allowlist=frozenset({"vendor_invoice_site"})),
            )
        self.assertIn("allowlist", str(ctx.exception))

    def test_a_closed_row_refuses_the_whole_batch(self):
        book = project(closed_through=date(2026, 3, 31))
        with self.assertRaises(p.Refuse) as ctx:
            p.propose_vendor_invoices(
                [
                    invoice(dated="2026-04-02", reference="open"),
                    invoice(dated="2026-03-15", reference="closed"),
                ],
                book=book,
                client=declared_client(),
            )
        self.assertIn("closed-through", str(ctx.exception))

    def test_an_undated_row_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.propose_vendor_invoices(
                [invoice(dated="")],
                book=project(),
                client=declared_client(),
            )
        self.assertIn("undated", str(ctx.exception))

    def test_call_lp_is_refused_on_a_project_book(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.propose_vendor_invoices(
                [invoice(kind="call_lp")],
                book=project(),
                client=declared_client(),
            )
        self.assertIn("call_lp", str(ctx.exception))

    def test_a_template_absent_from_the_ruleset_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.propose_vendor_invoices(
                [invoice()],
                book=project(approved_templates=frozenset({"vendor_invoice"})),
                client=declared_client(),
            )
        self.assertIn("RuleSet", str(ctx.exception))

    def test_journals_post_is_required_to_propose_a_write(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.propose_vendor_invoices(
                [invoice()],
                book=project(),
                client=declared_client(scopes=frozenset(p.READ_SCOPES)),
            )
        self.assertIn("journals:post", str(ctx.exception))

    def test_journal_append_is_refused_as_a_scope(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.propose_vendor_invoices(
                [invoice()],
                book=project(),
                client=declared_client(
                    scopes=frozenset({"billing:read", "budget:read", "statements:read", "journal:append"})
                ),
            )
        self.assertIn("journal:append", str(ctx.exception))
        self.assertIn("journals:post", str(ctx.exception))

    def test_projects_billing_read_is_refused_as_a_scope(self):
        with self.assertRaises(p.Refuse) as ctx:
            cite(
                client=declared_client(
                    scopes=frozenset(
                        {
                            "projects:billing:read",
                            "budget:read",
                            "statements:read",
                            "journals:post",
                        }
                    )
                )
            )
        self.assertIn("projects:billing:read", str(ctx.exception))

    def test_a_personal_book_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            cite(book=project(kind="PERSONAL"))
        self.assertIn("PROJECT", str(ctx.exception))

    def test_an_unbalanced_instantiation_refuses_the_batch(self):
        saved = p.VENDOR_INVOICE_LEGS["vendor_invoice_site"]
        p.VENDOR_INVOICE_LEGS["vendor_invoice_site"] = ((11, 1), (40, 1))
        try:
            with self.assertRaises(p.Refuse) as ctx:
                p.propose_vendor_invoices(
                    [invoice()],
                    book=project(),
                    client=declared_client(),
                )
            self.assertIn("conserve", str(ctx.exception))
        finally:
            p.VENDOR_INVOICE_LEGS["vendor_invoice_site"] = saved


class ProductRefusals(unittest.TestCase):
    def test_fetch_cites_refuses_because_the_grant_path_is_not_built(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.fetch_cites(token="connect-access-token")
        msg = str(ctx.exception)
        self.assertIn("grant path is not built", msg)
        self.assertIn("#22", msg)

    def test_deliver_refuses_because_the_grant_path_is_not_built(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.deliver(cite(), token="connect-access-token")
        msg = str(ctx.exception)
        self.assertIn("grant path is not built", msg)
        self.assertIn("#22", msg)

    def test_render_g702_is_refused_because_that_door_is_184(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.render_g702(cite())
        self.assertIn("#184", str(ctx.exception))
        self.assertIn("G702", str(ctx.exception))

    def test_eac_is_refused_as_a_forecast(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.eac(cite())
        self.assertIn("#169", str(ctx.exception))

    def test_a_forecast_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.forecast(cite())
        self.assertIn("forecast", str(ctx.exception))

    def test_a_vendor_directory_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.vendor_directory()
        self.assertIn("vendor user directory", str(ctx.exception))
        self.assertIn("#172", str(ctx.exception))


class ManifestHonesty(unittest.TestCase):
    def test_the_app_declares_only_canonical_scopes(self):
        scopes = app()["workos_connect"]["scopes"]
        self.assertEqual(set(scopes), set(p.CANONICAL_SCOPES))
        for alias in p.REFUSED_ALIASES:
            self.assertNotIn(alias, scopes)
        self.assertIn("billing:read", scopes)
        self.assertIn("budget:read", scopes)
        self.assertIn("statements:read", scopes)
        self.assertIn("journals:post", scopes)
        self.assertNotIn("journal:append", scopes)

    def test_the_declared_allowlist_is_vendor_invoice_templates_not_methods(self):
        templates = set(app()["journals_post_allowlist"]["templates"])
        self.assertTrue(templates)
        self.assertTrue(templates <= set(p.VENDOR_INVOICE_LEGS))
        for forbidden in ("fifo", "hifo", "min_tax", "specific_id", "average_cost", "wash"):
            self.assertNotIn(forbidden, templates)
            self.assertFalse(any(forbidden in t for t in templates), forbidden)

    def test_grant_path_and_refusals_stay_named_as_leftovers(self):
        doc = json.dumps(app())
        self.assertIn("not built", app()["grant_path"]["status"])
        self.assertIn("refused", app()["aia_g702"]["status"])
        self.assertIn("refused", app()["eac_forecast"]["status"])
        self.assertIn("refused", app()["vendor_directory"]["status"])
        self.assertIn("#172", doc)
        self.assertIn("#150", doc)
        self.assertIn("#184", doc)
        self.assertIn("#169", doc)
        self.assertIn("#22", doc)

    def test_every_instantiated_template_is_in_createbook_project(self):
        if BOOK_RS is None or not BOOK_RS.is_file():
            self.skipTest("book.rs not handed to the test")
        src = BOOK_RS.read_text()
        start = src.index("const PROJECT_CONFIG")
        end = src.index("const OPERATING_CONFIG") if "const OPERATING_CONFIG" in src else len(src)
        project_src = src[start:end]
        for rule_id in p.VENDOR_INVOICE_LEGS:
            self.assertIn(
                f'id = "{rule_id}"',
                project_src,
                f"{rule_id} is not a CreateBook(Project) rule — the app invented it",
            )

    def test_project_term_field_names_are_the_ones_ruleset_already_stores(self):
        if RULES_RS is None or not RULES_RS.is_file():
            self.skipTest("ratio-rules lib.rs not handed to the test")
        src = RULES_RS.read_text()
        self.assertIn("pub struct ProjectTerms", src)
        for field in p.PROJECT_TERM_FIELDS:
            self.assertIn(
                f"pub {field}:",
                src,
                f"{field} is not a ProjectTerms field — the portal invented a baseline",
            )

    def test_the_catalog_still_refuses_the_alias_this_issue_named(self):
        if CATALOG is None or not CATALOG.is_file():
            self.skipTest("connect-scopes.md not handed to the test")
        text = CATALOG.read_text()
        for scope in p.CANONICAL_SCOPES:
            self.assertIn(f"`{scope}`", text)
        self.assertIn("`journal:append`", text)
        self.assertIn("`projects:billing:read`", text)
        self.assertIn("grant path is not built", text)
        self.assertIn("vendor portal", text)

    def test_screens_for_project_was_not_forked_with_a_vendor_portal_url(self):
        if SCREENS is None or not SCREENS.is_file():
            self.skipTest("screens.ts not handed to the test")
        src = SCREENS.read_text()
        start = src.index("export const PROJECT_SCREENS")
        end = src.index("export const INVESTMENT_SCREENS")
        project_src = src[start:end]
        self.assertNotIn("vendor", project_src.lower())
        self.assertNotIn("portal", project_src.lower())
        self.assertNotIn("g702", project_src.lower())
        self.assertIn("billing", project_src)
        self.assertIn("budget", project_src)

    def test_the_kernel_did_not_grow_a_vendor_portal_rpc(self):
        if PROTO is None or not PROTO.is_file():
            self.skipTest("console.proto not handed to the test")
        src = PROTO.read_text()
        self.assertIn("rpc ProjectProgress", src)
        for needle in (
            "rpc VendorPortal",
            "rpc VendorDirectory",
            "rpc GcPortal",
            "rpc PayApp",
            "message VendorUser ",
            "message VendorDirectory ",
        ):
            self.assertNotIn(
                needle,
                src,
                f"{needle} is a kernel vendor-portal RPC — refuse it; this app is the door",
            )


if __name__ == "__main__":
    unittest.main()
