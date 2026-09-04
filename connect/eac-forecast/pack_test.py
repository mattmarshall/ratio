#!/usr/bin/env python3
"""Properties the Project EAC / forecast pack must keep.

Test names are sentences. Break the thing a test protects and this
file goes red — a green suite that only checks the books balance is
how a wrong remaining-to-spend ships as a complete EAC of 0.00.

Does not talk to /v1. Does not claim a Connect token is accepted.
Does not add EAC fields on /budget.
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


def budget(**overrides) -> p.BudgetCite:
    return p.BudgetCite(
        original=overrides.get("original", 1_000_000_00),
        approved_change_orders=overrides.get("approved_change_orders", 50_000_00),
        incurred=overrides.get("incurred", 200_000_00),
        awarded=overrides.get("awarded", 150_000_00),
    )


def billing(**overrides) -> p.BillingCite:
    return p.BillingCite(
        billed=overrides.get("billed", 100_000_00),
        earned=overrides.get("earned", 90_000_00),
        accounts_receivable=overrides.get("accounts_receivable", 40_000_00),
    )


def phase(**overrides) -> p.PhaseCite:
    return p.PhaseCite(
        display_name=overrides.get("display_name", "Site and mobilization"),
        budget=overrides.get("budget", 300_000_00),
        approved_change_orders=overrides.get("approved_change_orders", 10_000_00),
        incurred=overrides.get("incurred", 80_000_00),
        awarded=overrides.get("awarded", 40_000_00),
    )


def pack_of(**kwargs) -> p.Pack:
    return p.build_pack(
        book=kwargs.get("book", project()),
        client=kwargs.get("client", declared_client()),
        budget=kwargs.get("budget", budget()),
        billing=kwargs.get("billing", billing()),
        phases=kwargs.get("phases", (phase(),)),
    )


def cite_amount(out: p.Pack, name: str) -> str:
    return next(line.amount for line in out.cites if line.figure == name)


def forecast_amount(out: p.Pack, name: str) -> str:
    return next(line.amount for line in out.forecast if line.figure == name)


def companion_amount(out: p.Pack, name: str) -> str:
    return next(line.amount for line in out.companions if line.figure == name)


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


class RemainingToSpend(unittest.TestCase):
    def test_remaining_is_revised_minus_incurred_minus_awarded(self):
        # Same identity /budget already cites: original 10,000 + CO 500
        # = revised 10,500; incurred 2,000; awarded 1,500 → remaining 7,000.
        self.assertEqual(p.remaining_to_spend(1_050_000, 200_000, 150_000), 700_000)

    def test_remaining_stays_unset_when_awarded_cannot_support_the_cut(self):
        # Treating awarded as 0 would print budget − actual as headroom.
        self.assertIsNone(p.remaining_to_spend(1_000_000_00, 200_000_00, None))
        self.assertIsNone(p.remaining_to_spend(1_000_000_00, 0, None))

    def test_remaining_stays_unset_when_revised_is_missing(self):
        self.assertIsNone(p.remaining_to_spend(None, 200_000_00, 150_000_00))

    def test_remaining_stays_unset_when_incurred_is_missing(self):
        self.assertIsNone(p.remaining_to_spend(1_000_000_00, None, 150_000_00))

    def test_a_real_zero_remaining_is_a_figure_when_every_side_is_set(self):
        self.assertEqual(p.remaining_to_spend(100_000, 100_000, 0), 0)
        self.assertEqual(p.remaining_to_spend(0, 0, 0), 0)


class ForecastCuts(unittest.TestCase):
    def test_eac_equals_revised_when_remaining_can_be_cited(self):
        remaining = p.remaining_to_spend(1_050_000_00, 200_000_00, 150_000_00)
        etc = p.estimate_to_complete(remaining, 150_000_00)
        eac = p.estimate_at_completion(200_000_00, etc)
        self.assertEqual(remaining, 700_000_00)
        self.assertEqual(etc, 850_000_00)
        self.assertEqual(eac, 1_050_000_00)

    def test_an_unset_remaining_is_not_a_silent_eac_of_zero(self):
        self.assertIsNone(p.estimate_to_complete(None, 150_000_00))
        self.assertIsNone(p.estimate_to_complete(None, None))
        self.assertIsNone(p.estimate_at_completion(200_000_00, None))
        self.assertIsNone(p.estimate_at_completion(None, None))

    def test_an_unset_etc_is_not_substituted_from_awarded_alone(self):
        # ETC = awarded without remaining would invent a cost-to-complete.
        self.assertIsNone(p.estimate_to_complete(None, 150_000_00))


class PackShape(unittest.TestCase):
    def test_a_fixture_project_budget_maps_to_eac_rows(self):
        # original 10,000 / CO 500 / incurred 2,000 / awarded 1,500
        # → remaining 7,000, ETC 8,500, EAC 10,500.
        out = pack_of(
            budget=budget(
                original=1_000_000,
                approved_change_orders=50_000,
                incurred=200_000,
                awarded=150_000,
            )
        )
        self.assertEqual(cite_amount(out, "original_contract"), "10000.00")
        self.assertEqual(cite_amount(out, "approved_change_orders"), "500.00")
        self.assertEqual(cite_amount(out, "revised_contract"), "10500.00")
        self.assertEqual(cite_amount(out, "incurred"), "2000.00")
        self.assertEqual(cite_amount(out, "awarded"), "1500.00")
        self.assertEqual(cite_amount(out, "remaining_to_spend"), "7000.00")
        self.assertEqual(forecast_amount(out, "etc"), "8500.00")
        self.assertEqual(forecast_amount(out, "eac"), "10500.00")
        self.assertEqual(out.eac, 1_050_000)
        self.assertIn("finishes at the revised contract", next(
            line.note for line in out.forecast if line.figure == "eac"
        ))

    def test_missing_cites_stay_unset_and_do_not_invent_a_fake_eac_of_zero(self):
        out = pack_of(
            budget=p.BudgetCite(),
            billing=p.BillingCite(),
            phases=(p.PhaseCite(display_name="Site and mobilization"),),
        )
        for name in (
            "original_contract",
            "approved_change_orders",
            "revised_contract",
            "incurred",
            "awarded",
            "remaining_to_spend",
        ):
            self.assertEqual(cite_amount(out, name), "", name)
        self.assertEqual(forecast_amount(out, "eac"), "")
        self.assertEqual(forecast_amount(out, "etc"), "")
        self.assertIsNone(out.eac)
        self.assertIsNone(out.remaining_to_spend)
        self.assertIn("EAC", " ".join(out.unset))
        csv_eac = p.csv_forecast(out)
        eac_line = next(line for line in csv_eac.splitlines() if line.startswith("eac,"))
        self.assertTrue(eac_line.startswith("eac,,"), eac_line)
        remaining_line = next(
            line for line in p.csv_cites(out).splitlines()
            if line.startswith("remaining_to_spend,")
        )
        self.assertTrue(remaining_line.startswith("remaining_to_spend,,"), remaining_line)
        row = out.phases[0]
        self.assertEqual(row.eac, "")
        self.assertEqual(row.remaining_to_spend, "")
        self.assertIn("not a silent 0.00", row.assumption)

    def test_an_unawarded_job_does_not_print_budget_minus_actual_as_headroom(self):
        out = pack_of(
            budget=budget(original=1_000_000_00, approved_change_orders=None, incurred=200_000_00, awarded=None)
        )
        self.assertEqual(cite_amount(out, "revised_contract"), "1000000.00")
        self.assertEqual(cite_amount(out, "incurred"), "200000.00")
        self.assertEqual(cite_amount(out, "awarded"), "")
        self.assertEqual(cite_amount(out, "remaining_to_spend"), "")
        self.assertEqual(forecast_amount(out, "eac"), "")
        self.assertIn("headroom", next(
            line.note for line in out.cites if line.figure == "awarded"
        ))

    def test_a_posted_zero_award_is_a_figure_and_remaining_equals_revised_minus_incurred(self):
        out = pack_of(
            budget=budget(original=1_000_000_00, approved_change_orders=None, incurred=200_000_00, awarded=0)
        )
        self.assertEqual(cite_amount(out, "awarded"), "0.00")
        self.assertEqual(cite_amount(out, "remaining_to_spend"), "800000.00")
        self.assertEqual(forecast_amount(out, "etc"), "800000.00")
        self.assertEqual(forecast_amount(out, "eac"), "1000000.00")

    def test_a_real_zero_eac_is_when_revised_is_zero_and_the_cut_is_supported(self):
        out = pack_of(
            budget=budget(original=0, approved_change_orders=None, incurred=0, awarded=0)
        )
        self.assertEqual(cite_amount(out, "remaining_to_spend"), "0.00")
        self.assertEqual(forecast_amount(out, "eac"), "0.00")
        self.assertEqual(forecast_amount(out, "etc"), "0.00")

    def test_billed_and_earned_are_not_substituted_for_missing_incurred(self):
        out = pack_of(
            budget=budget(incurred=None, awarded=150_000_00),
            billing=billing(billed=100_000_00, earned=90_000_00),
        )
        self.assertEqual(cite_amount(out, "incurred"), "")
        self.assertEqual(cite_amount(out, "remaining_to_spend"), "")
        self.assertEqual(forecast_amount(out, "eac"), "")
        self.assertEqual(companion_amount(out, "billed"), "100000.00")
        self.assertEqual(companion_amount(out, "earned"), "90000.00")

    def test_phase_eac_stays_unset_when_awarded_is_missing(self):
        out = pack_of(
            phases=(
                phase(
                    display_name="Structure",
                    budget=400_000_00,
                    approved_change_orders=None,
                    incurred=120_000_00,
                    awarded=None,
                ),
            )
        )
        row = out.phases[0]
        self.assertEqual(row.description, "Structure")
        self.assertEqual(row.revised, "400000.00")
        self.assertEqual(row.incurred, "120000.00")
        self.assertEqual(row.awarded, "")
        self.assertEqual(row.remaining_to_spend, "")
        self.assertEqual(row.eac, "")

    def test_phase_eac_equals_revised_when_the_cut_is_supported(self):
        out = pack_of(
            phases=(phase(incurred=80_000_00, awarded=40_000_00, approved_change_orders=10_000_00),)
        )
        row = out.phases[0]
        self.assertEqual(row.revised, "310000.00")
        self.assertEqual(row.remaining_to_spend, "190000.00")
        self.assertEqual(row.etc, "230000.00")
        self.assertEqual(row.eac, "310000.00")

    def test_companion_sheets_are_named_and_do_not_invent_a_percent_complete(self):
        files = p.as_files(pack_of())
        self.assertEqual(
            set(files),
            {"cites.csv", "eac.csv", "companions.csv", "phases.csv", "unset.csv", "eac.json"},
        )
        self.assertNotIn("%", files["eac.csv"].splitlines()[0])
        self.assertNotIn("Percent", files["eac.csv"])
        self.assertIn("Assumption", files["eac.csv"])
        self.assertIn("remaining_to_spend", files["cites.csv"])
        payload = json.loads(files["eac.json"])
        self.assertIn("eac", payload)
        self.assertIn("assumption", payload["forecast"][0])

    def test_cite_from_fixture_reads_the_budget_page_shape(self):
        out = p.cite_from_fixture(
            {
                "kind": "PROJECT",
                "budget": "10000.00",
                "approved_change_orders": "500.00",
                "incurred": "2000.00",
                "awarded": "1500.00",
                "progress": {
                    "billed": "1000.00",
                    "earned": "900.00",
                },
                "phases": [
                    {
                        "display_name": "Site and mobilization",
                        "budget": "3000.00",
                        "approved_change_orders": "100.00",
                        "cost": "800.00",
                        "awarded": "400.00",
                    }
                ],
                "app": app(),
            }
        )
        self.assertEqual(cite_amount(out, "original_contract"), "10000.00")
        self.assertEqual(cite_amount(out, "remaining_to_spend"), "7000.00")
        self.assertEqual(forecast_amount(out, "eac"), "10500.00")
        self.assertEqual(out.phases[0].incurred, "800.00")
        self.assertEqual(out.phases[0].eac, "3100.00")


class Refusals(unittest.TestCase):
    def test_journals_post_is_refused_because_this_app_is_read_only(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                client=declared_client(
                    scopes=frozenset(
                        {
                            "budget:read",
                            "billing:read",
                            "statements:read",
                            "journals:post",
                        }
                    )
                )
            )
        self.assertIn("journals:post", str(ctx.exception))
        self.assertIn("second ledger", str(ctx.exception))

    def test_journal_append_is_refused_as_a_scope(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                client=declared_client(
                    scopes=frozenset({"budget:read", "billing:read", "journal:append"})
                )
            )
        self.assertIn("journal:append", str(ctx.exception))

    def test_projects_budget_read_is_refused_as_the_catalog_alias(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                client=declared_client(
                    scopes=frozenset(
                        {"projects:budget:read", "billing:read", "statements:read"}
                    )
                )
            )
        self.assertIn("projects:budget:read", str(ctx.exception))

    def test_missing_budget_read_is_refused_because_remaining_cannot_be_cited(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                client=declared_client(scopes=frozenset({"billing:read", "statements:read"}))
            )
        self.assertIn("budget:read", str(ctx.exception))

    def test_missing_billing_read_is_refused_because_companions_cannot_be_cited(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                client=declared_client(scopes=frozenset({"budget:read", "statements:read"}))
            )
        self.assertIn("billing:read", str(ctx.exception))

    def test_a_personal_book_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(book=project(kind="PERSONAL"))
        self.assertIn("PROJECT", str(ctx.exception))
        self.assertIn("#163", str(ctx.exception))

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

    def test_post_forecast_refuses_because_there_is_no_forecast_template(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.post_forecast()
        msg = str(ctx.exception)
        self.assertIn("journal:append", msg)
        self.assertIn("second ledger", msg)
        self.assertIn("#169", msg)

    def test_cpi_eac_is_refused_because_a_percentage_is_a_rounded_figure(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.cpi_eac()
        self.assertIn("percent", str(ctx.exception).lower())


class ManifestHonesty(unittest.TestCase):
    def test_the_app_declares_only_canonical_scopes(self):
        scopes = app()["workos_connect"]["scopes"]
        self.assertEqual(set(scopes), set(p.CANONICAL_SCOPES))
        for alias in p.REFUSED_ALIASES:
            self.assertNotIn(alias, scopes)
        self.assertNotIn("journals:post", scopes)
        self.assertNotIn("journal:append", scopes)

    def test_grant_path_and_forecast_journals_stay_named_as_leftovers(self):
        doc = json.dumps(app())
        self.assertIn("not built", app()["grant_path"]["status"])
        self.assertIn("refused", app()["forecast_journals"]["status"])
        self.assertIn("#169", doc)
        self.assertIn("#150", doc)
        self.assertIn("#22", doc)
        self.assertIn("does not reopen #151", doc)
        self.assertIn("#163", doc)

    def test_the_catalog_still_names_the_scopes_this_app_requests(self):
        if CATALOG is None or not CATALOG.is_file():
            self.skipTest("connect-scopes.md not handed to the test")
        text = CATALOG.read_text()
        for scope in p.CANONICAL_SCOPES:
            self.assertIn(f"`{scope}`", text)
        self.assertIn("`journal:append`", text)
        self.assertIn("leftover #22", text)
        self.assertIn("Not EAC, not a forecast", text)

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
        self.assertNotIn("eac", project_src.lower())
        self.assertNotIn("forecast", project_src.lower())

    def test_screens_for_project_was_not_forked_with_an_eac_url(self):
        if SCREENS is None or not SCREENS.is_file():
            self.skipTest("screens.ts not handed to the test")
        src = SCREENS.read_text()
        start = src.index("export const PROJECT_SCREENS")
        end = src.index("export const INVESTMENT_SCREENS")
        project_src = src[start:end]
        lowered = project_src.lower()
        self.assertNotIn('segment: "eac"', lowered)
        self.assertNotIn('segment: "forecast"', lowered)
        self.assertNotIn('segment: "etc"', lowered)
        self.assertNotIn("estimate at completion", lowered)
        self.assertIn("budget", project_src)
        self.assertIn("billing", project_src)

    def test_the_kernel_did_not_grow_an_eac_rpc(self):
        if PROTO is None or not PROTO.is_file():
            self.skipTest("console.proto not handed to the test")
        src = PROTO.read_text()
        self.assertIn("rpc ProjectProgress", src)
        for needle in (
            "rpc Eac",
            "rpc EAC",
            "rpc Forecast",
            "rpc EstimateAtCompletion",
            "rpc CostToComplete",
            "message Eac",
            "message Forecast",
            "message EstimateAtCompletion",
        ):
            self.assertNotIn(
                needle,
                src,
                f"{needle} is a kernel forecast RPC — refuse it; this app is the door",
            )


if __name__ == "__main__":
    unittest.main()
