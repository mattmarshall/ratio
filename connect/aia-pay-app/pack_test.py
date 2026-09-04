#!/usr/bin/env python3
"""Properties the Project AIA pay-app pack must keep.

Test names are sentences. Break the thing a test protects and this
file goes red — a green suite that only checks the books balance is
how a wrong billed figure ships as a complete G702.

Does not talk to /v1. Does not claim a Connect token is accepted.
Does not emit a licensed AIA form.
"""

from __future__ import annotations

import json
import pathlib
import sys
import unittest

import pack as p

APP_PATH = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).with_name("app.json")
RULES_RS = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else None
CATALOG = pathlib.Path(sys.argv[3]) if len(sys.argv) > 3 else None
SCREENS = pathlib.Path(sys.argv[4]) if len(sys.argv) > 4 else None
PROTO = pathlib.Path(sys.argv[5]) if len(sys.argv) > 5 else None

sys.argv = sys.argv[:1]


def app() -> dict:
    return json.loads(APP_PATH.read_text())


def declared_client(**overrides) -> p.Client:
    c = p.client_from_app(app())
    return p.Client(
        client_id=overrides.get("client_id", c.client_id),
        scopes=overrides.get("scopes", c.scopes),
    )


def project(**overrides) -> p.Book:
    return p.Book(
        kind=overrides.get("kind", "PROJECT"),
        closed_through=overrides.get("closed_through"),
    )


def contract(**overrides) -> p.ContractCite:
    return p.ContractCite(
        original=overrides.get("original", 1_000_000_00),
        approved_change_orders=overrides.get("approved_change_orders", 50_000_00),
    )


def billing(**overrides) -> p.BillingCite:
    return p.BillingCite(
        billed=overrides.get("billed", 100_000_00),
        earned=overrides.get("earned", 90_000_00),
        retainage_receivable=overrides.get("retainage_receivable", 10_000_00),
        retainage_payable=overrides.get("retainage_payable"),
        accounts_receivable=overrides.get("accounts_receivable", 40_000_00),
    )


def phase(**overrides) -> p.PhaseCite:
    return p.PhaseCite(
        display_name=overrides.get("display_name", "Site and mobilization"),
        budget=overrides.get("budget", 300_000_00),
        approved_change_orders=overrides.get("approved_change_orders", 10_000_00),
        cost=overrides.get("cost", 80_000_00),
        completed=overrides.get("completed"),
        prior_completed=overrides.get("prior_completed"),
    )


def pack_of(**kwargs) -> p.Pack:
    return p.build_pack(
        book=kwargs.get("book", project()),
        client=kwargs.get("client", declared_client()),
        contract=kwargs.get("contract", contract()),
        billing=kwargs.get("billing", billing()),
        prior=kwargs.get("prior", ...),
        phases=kwargs.get("phases", (phase(),)),
    )


def g702_amount(out: p.Pack, name: str) -> str:
    return next(line.amount for line in out.g702 if line.line == name)


def companion_amount(out: p.Pack, name: str) -> str:
    return next(line.amount for line in out.companions if line.line == name)


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

    def test_a_signed_magnitude_is_refused_so_a_hold_cannot_be_inferred(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.parse_minor("-40.00")
        self.assertIn("signed", str(ctx.exception))

    def test_a_signed_change_order_net_is_a_deduct_not_a_refused_amount(self):
        self.assertEqual(p.parse_minor("-250.00", allow_signed=True), -25_000)

    def test_a_zero_amount_is_a_real_zero_not_a_missing_cite(self):
        self.assertEqual(p.parse_minor("0.00"), 0)
        self.assertEqual(p.parse_optional_minor("0.00"), 0)
        self.assertIsNone(p.parse_optional_minor(""))
        self.assertIsNone(p.parse_optional_minor(None))

    def test_an_amount_that_does_not_fit_i64_is_refused(self):
        with self.assertRaises(p.Refuse):
            p.parse_minor("92233720368547758.08")


class ContractCuts(unittest.TestCase):
    def test_revised_equals_original_plus_approved_when_both_are_set(self):
        self.assertEqual(p.revised_contract(1_000_000_00, 50_000_00), 1_050_000_00)

    def test_revised_equals_original_when_no_change_order_has_posted(self):
        self.assertEqual(p.revised_contract(1_000_000_00, None), 1_000_000_00)

    def test_an_unknown_baseline_cannot_price_a_revised_contract(self):
        self.assertIsNone(p.revised_contract(None, 50_000_00))
        self.assertIsNone(p.revised_contract(None, None))

    def test_remaining_to_bill_stays_unset_when_billed_is_missing(self):
        # Treating billed as 0 would print the whole contract as remaining.
        self.assertIsNone(p.remaining_to_bill(1_000_000_00, None))
        self.assertEqual(p.remaining_to_bill(1_000_000_00, 100_000_00), 900_000_00)

    def test_billed_less_retainage_treats_unheld_retainage_as_zero_for_the_subtraction(self):
        self.assertEqual(p.billed_less_retainage(100_000_00, None), 100_000_00)
        self.assertEqual(p.billed_less_retainage(100_000_00, 10_000_00), 90_000_00)
        self.assertIsNone(p.billed_less_retainage(None, 10_000_00))

    def test_an_omitted_prior_does_not_make_current_due_equal_billed_to_date(self):
        self.assertIsNone(p.current_payment_due(90_000_00, None))
        self.assertEqual(p.current_payment_due(90_000_00, 36_000_00), 54_000_00)


class PackShape(unittest.TestCase):
    def test_a_fixture_project_billing_maps_to_g702_rows(self):
        # Same identity /billing already cites: original 10,000.00,
        # approved CO 500.00, billed 1,000.00, retainage 100.00,
        # AR 400.00 → collected 500.00, remaining 9,500.00.
        out = pack_of(
            contract=contract(original=1_000_000, approved_change_orders=50_000),
            billing=billing(
                billed=100_000,
                earned=90_000,
                retainage_receivable=10_000,
                accounts_receivable=40_000,
            ),
        )
        self.assertEqual(g702_amount(out, "original_contract"), "10000.00")
        self.assertEqual(g702_amount(out, "net_change_orders"), "500.00")
        self.assertEqual(g702_amount(out, "contract_sum_to_date"), "10500.00")
        self.assertEqual(g702_amount(out, "total_billed_to_date"), "1000.00")
        self.assertEqual(g702_amount(out, "retainage_held"), "100.00")
        self.assertEqual(g702_amount(out, "billed_less_retainage"), "900.00")
        self.assertEqual(g702_amount(out, "balance_to_finish"), "9500.00")
        self.assertEqual(companion_amount(out, "earned_to_date"), "900.00")
        self.assertEqual(companion_amount(out, "billed_minus_earned"), "100.00")
        self.assertEqual(companion_amount(out, "collected"), "500.00")

    def test_a_prior_application_cut_fills_previous_certificates_and_current_due(self):
        out = pack_of(
            billing=billing(billed=100_000_00, retainage_receivable=10_000_00),
            prior=p.ApplicationCite(billed=40_000_00, retainage_receivable=4_000_00),
        )
        self.assertEqual(g702_amount(out, "previous_certificates"), "36000.00")
        self.assertEqual(g702_amount(out, "current_payment_due"), "54000.00")

    def test_an_omitted_prior_leaves_previous_and_current_due_unset(self):
        out = pack_of()
        self.assertEqual(g702_amount(out, "previous_certificates"), "")
        self.assertEqual(g702_amount(out, "current_payment_due"), "")
        self.assertIn("previous_certificates", out.unset)
        self.assertIn("current_payment_due", out.unset)
        self.assertIn("not a silent first pay-app", next(
            line.note for line in out.g702 if line.line == "previous_certificates"
        ))

    def test_a_cited_prior_of_zero_is_a_real_zero_previous_not_an_omitted_cut(self):
        out = pack_of(
            billing=billing(billed=100_000_00, retainage_receivable=10_000_00),
            prior=p.ApplicationCite(billed=0, retainage_receivable=0),
        )
        self.assertEqual(g702_amount(out, "previous_certificates"), "0.00")
        self.assertEqual(g702_amount(out, "current_payment_due"), "90000.00")

    def test_missing_cites_stay_unset_and_do_not_invent_fake_zeros(self):
        out = pack_of(
            contract=p.ContractCite(),
            billing=p.BillingCite(),
            phases=(p.PhaseCite(display_name="Site and mobilization"),),
        )
        for name in (
            "original_contract",
            "net_change_orders",
            "contract_sum_to_date",
            "total_billed_to_date",
            "retainage_held",
            "billed_less_retainage",
            "previous_certificates",
            "current_payment_due",
            "balance_to_finish",
        ):
            self.assertEqual(g702_amount(out, name), "", name)
        self.assertEqual(companion_amount(out, "earned_to_date"), "")
        self.assertEqual(companion_amount(out, "collected"), "")
        row = out.g703[0]
        self.assertEqual(row.scheduled_value, "")
        self.assertEqual(row.change_orders, "")
        self.assertEqual(row.revised_value, "")
        self.assertEqual(row.completed_and_stored, "")
        self.assertEqual(row.previous_completed, "")
        self.assertEqual(row.this_period, "")
        csv_702 = p.csv_g702(out)
        # The VALUE is blank. Notes may say "not a silent zero" —
        # that is the honesty, not a default written into the cite.
        billed_line = next(
            line for line in csv_702.splitlines() if line.startswith("total_billed_to_date,")
        )
        self.assertTrue(billed_line.startswith("total_billed_to_date,,"), billed_line)
        co_line = next(
            line for line in csv_702.splitlines() if line.startswith("net_change_orders,")
        )
        self.assertTrue(co_line.startswith("net_change_orders,,"), co_line)

    def test_a_posted_zero_billed_is_a_figure_and_remaining_is_the_whole_revised(self):
        out = pack_of(
            contract=contract(original=1_000_000_00, approved_change_orders=None),
            billing=billing(billed=0, earned=None, retainage_receivable=None, accounts_receivable=None),
        )
        self.assertEqual(g702_amount(out, "total_billed_to_date"), "0.00")
        self.assertEqual(g702_amount(out, "contract_sum_to_date"), "1000000.00")
        self.assertEqual(g702_amount(out, "net_change_orders"), "")
        self.assertEqual(g702_amount(out, "balance_to_finish"), "1000000.00")
        self.assertEqual(g702_amount(out, "retainage_held"), "")

    def test_earned_is_not_substituted_for_missing_billed(self):
        out = pack_of(
            billing=billing(billed=None, earned=90_000_00, retainage_receivable=None, accounts_receivable=None)
        )
        self.assertEqual(g702_amount(out, "total_billed_to_date"), "")
        self.assertEqual(companion_amount(out, "earned_to_date"), "90000.00")
        self.assertEqual(g702_amount(out, "billed_less_retainage"), "")
        self.assertEqual(companion_amount(out, "billed_minus_earned"), "")

    def test_phase_cost_is_not_used_as_completed_and_stored(self):
        out = pack_of(
            phases=(
                phase(
                    display_name="Structure",
                    budget=400_000_00,
                    approved_change_orders=None,
                    cost=120_000_00,
                    completed=None,
                ),
            )
        )
        row = out.g703[0]
        self.assertEqual(row.description, "Structure")
        self.assertEqual(row.scheduled_value, "400000.00")
        self.assertEqual(row.change_orders, "")
        self.assertEqual(row.revised_value, "400000.00")
        self.assertEqual(row.cost_to_date, "120000.00")
        self.assertEqual(row.completed_and_stored, "")
        self.assertEqual(row.this_period, "")
        self.assertEqual(row.balance_to_finish, "")
        self.assertEqual(row.materials_stored, "")
        self.assertEqual(row.retainage, "")

    def test_a_seeded_phase_cost_of_zero_is_a_true_zero_not_unset(self):
        out = pack_of(phases=(phase(cost=0, completed=None, approved_change_orders=None),))
        self.assertEqual(out.g703[0].cost_to_date, "0.00")
        self.assertEqual(out.g703[0].completed_and_stored, "")

    def test_completed_by_line_and_a_prior_cut_fill_this_period(self):
        out = pack_of(
            phases=(
                phase(completed=80_000_00, prior_completed=30_000_00, cost=80_000_00),
            )
        )
        row = out.g703[0]
        self.assertEqual(row.completed_and_stored, "80000.00")
        self.assertEqual(row.previous_completed, "30000.00")
        self.assertEqual(row.this_period, "50000.00")
        self.assertEqual(row.revised_value, "310000.00")
        self.assertEqual(row.balance_to_finish, "230000.00")

    def test_companion_sheets_are_named_and_do_not_invent_a_percent_complete(self):
        files = p.as_files(pack_of())
        self.assertEqual(
            set(files),
            {"g702.csv", "g703.csv", "companions.csv", "unset.csv"},
        )
        self.assertNotIn("%", files["g703.csv"].splitlines()[0])
        self.assertNotIn("Percent", files["g703.csv"])
        self.assertIn("Cost to date", files["g703.csv"])
        self.assertIn("original_contract", files["g702.csv"])

    def test_cite_from_fixture_reads_the_billing_page_shape(self):
        out = p.cite_from_fixture(
            {
                "kind": "PROJECT",
                "budget": "10000.00",
                "approved_change_orders": "500.00",
                "progress": {
                    "billed": "1000.00",
                    "earned": "900.00",
                    "retainage_receivable": "100.00",
                    "accounts_receivable": "400.00",
                },
                "phases": [
                    {
                        "display_name": "Site and mobilization",
                        "budget": "3000.00",
                        "approved_change_orders": "100.00",
                        "cost": "800.00",
                    }
                ],
                "app": app(),
            }
        )
        self.assertEqual(g702_amount(out, "original_contract"), "10000.00")
        self.assertEqual(g702_amount(out, "net_change_orders"), "500.00")
        self.assertEqual(g702_amount(out, "total_billed_to_date"), "1000.00")
        self.assertEqual(out.g703[0].scheduled_value, "3000.00")
        self.assertEqual(out.g703[0].change_orders, "100.00")
        self.assertEqual(out.g703[0].cost_to_date, "800.00")
        self.assertEqual(out.g703[0].completed_and_stored, "")


class Refusals(unittest.TestCase):
    def test_journals_post_is_refused_because_this_app_is_read_only(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                client=declared_client(
                    scopes=frozenset(
                        {
                            "billing:read",
                            "budget:read",
                            "statements:read",
                            "journals:post",
                        }
                    )
                )
            )
        self.assertIn("journals:post", str(ctx.exception))

    def test_journal_append_is_refused_as_a_scope(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                client=declared_client(
                    scopes=frozenset({"billing:read", "budget:read", "journal:append"})
                )
            )
        self.assertIn("journal:append", str(ctx.exception))

    def test_projects_billing_read_is_refused_as_the_catalog_alias(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                client=declared_client(
                    scopes=frozenset(
                        {"projects:billing:read", "budget:read", "statements:read"}
                    )
                )
            )
        self.assertIn("projects:billing:read", str(ctx.exception))

    def test_missing_billing_read_is_refused_because_billed_cannot_be_cited(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                client=declared_client(scopes=frozenset({"budget:read", "statements:read"}))
            )
        self.assertIn("billing:read", str(ctx.exception))

    def test_missing_budget_read_is_refused_because_the_original_cannot_be_cited(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                client=declared_client(scopes=frozenset({"billing:read", "statements:read"}))
            )
        self.assertIn("budget:read", str(ctx.exception))

    def test_a_personal_book_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(book=project(kind="PERSONAL"))
        self.assertIn("PROJECT", str(ctx.exception))

    def test_an_investment_book_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(book=project(kind="INVESTMENT"))
        self.assertIn("PROJECT", str(ctx.exception))

    def test_fetch_cites_refuses_because_the_grant_path_is_not_built(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.fetch_cites(token="connect-access-token")
        msg = str(ctx.exception)
        self.assertIn("grant path is not built", msg)
        self.assertIn("#22", msg)

    def test_deliver_refuses_because_the_grant_path_is_not_built(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.deliver(pack_of(), token="connect-access-token")
        self.assertIn("grant path is not built", str(ctx.exception))

    def test_render_form_refuses_because_a_licensed_aia_pdf_is_not_a_connect_scope(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.render_form(pack_of(), token="connect-access-token")
        self.assertIn("AIA", str(ctx.exception))


class ManifestHonesty(unittest.TestCase):
    def test_the_app_declares_only_canonical_scopes(self):
        scopes = app()["workos_connect"]["scopes"]
        self.assertEqual(set(scopes), set(p.CANONICAL_SCOPES))
        for alias in p.REFUSED_ALIASES:
            self.assertNotIn(alias, scopes)
        self.assertNotIn("journals:post", scopes)
        self.assertNotIn("journal:append", scopes)

    def test_grant_path_and_licensed_form_stay_named_as_leftovers(self):
        doc = json.dumps(app())
        self.assertIn("not built", app()["grant_path"]["status"])
        self.assertIn("refused", app()["licensed_aia_form"]["status"])
        self.assertIn("#184", doc)
        self.assertIn("#150", doc)
        self.assertIn("#22", doc)
        self.assertIn("does not reopen #151", doc)

    def test_the_catalog_still_names_the_scopes_this_app_requests(self):
        if CATALOG is None or not CATALOG.is_file():
            self.skipTest("connect-scopes.md not handed to the test")
        text = CATALOG.read_text()
        for scope in p.CANONICAL_SCOPES:
            self.assertIn(f"`{scope}`", text)
        self.assertIn("`projects:billing:read`", text)
        self.assertIn("grant path is not built", text)
        self.assertIn("AIA G702 product UI", text)

    def test_project_term_field_names_are_the_ones_ruleset_already_stores(self):
        if RULES_RS is None or not RULES_RS.is_file():
            self.skipTest("ratio-rules lib.rs not handed to the test")
        src = RULES_RS.read_text()
        self.assertIn("pub struct ProjectTerms", src)
        self.assertIn("pub budget: Option<i64>", src)
        self.assertIn("pub struct PhaseBudget", src)
        start = src.index("pub struct ProjectTerms")
        end = src.index("impl ProjectTerms")
        project_src = src[start:end]
        self.assertNotIn(
            "lot_method",
            project_src,
            "a project budget is not a lot Method — the pack invented an election",
        )

    def test_screens_for_project_was_not_forked_with_a_g702_url(self):
        if SCREENS is None or not SCREENS.is_file():
            self.skipTest("screens.ts not handed to the test")
        src = SCREENS.read_text()
        start = src.index("export const PROJECT_SCREENS")
        end = src.index("export const INVESTMENT_SCREENS")
        project_src = src[start:end]
        self.assertNotIn("g702", project_src.lower())
        self.assertNotIn("g703", project_src.lower())
        self.assertNotIn("pay-app", project_src.lower())
        self.assertNotIn("payapp", project_src.lower())
        self.assertNotIn("aia", project_src.lower())
        self.assertIn("billing", project_src)
        self.assertIn("budget", project_src)

    def test_the_kernel_did_not_grow_a_g702_rpc(self):
        if PROTO is None or not PROTO.is_file():
            self.skipTest("console.proto not handed to the test")
        src = PROTO.read_text()
        self.assertIn("rpc ProjectProgress", src)
        for needle in (
            "rpc PayApp",
            "rpc G702",
            "rpc G703",
            "rpc ApplicationForPayment",
            "message PayApp",
            "message G702",
            "message G703",
        ):
            self.assertNotIn(
                needle,
                src,
                f"{needle} is a kernel G702 RPC — refuse it; this app is the door",
            )


if __name__ == "__main__":
    unittest.main()
