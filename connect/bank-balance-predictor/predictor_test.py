#!/usr/bin/env python3
"""Properties the bank-balance predictor Connect app must keep.

Test names are sentences. Break the thing a test protects and this
file goes red — a green suite that only checks the books balance is
how a wrong gain ships.

Does not talk to /v1. Does not claim a Connect token is accepted.
Does not grow a kernel forecast RPC. Does not reopen #164.
"""

from __future__ import annotations

import json
import pathlib
import sys
import unittest
from datetime import date

import predictor as p

APP_PATH = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).with_name("app.json")
BOOK_RS = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else None
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
        allowlist=overrides.get("allowlist", c.allowlist),
        scopes=overrides.get("scopes", c.scopes),
    )


def personal(**overrides) -> p.Book:
    return p.Book(
        kind=overrides.get("kind", "PERSONAL"),
        approved_templates=overrides.get(
            "approved_templates", p.PERSONAL_SEEDED_RULES
        ),
        closed_through=overrides.get("closed_through"),
    )


def row(**overrides) -> dict:
    base = {
        "dated": "2026-04-15",
        "amount": "200.00",
        "currency": "USD",
        "kind": "income",
        "reference": "pred-1",
    }
    base.update(overrides)
    return base


def sheet(**overrides) -> p.Statement:
    base = {
        "currency": "USD",
        "as_of": "2026-03-31",
        "cash": "8000.00",
    }
    base.update(overrides)
    return p.statement_from_cite(base)


class ParseMinor(unittest.TestCase):
    def test_an_amount_is_parsed_by_splitting_not_by_floating(self):
        self.assertEqual(p.parse_minor("250000.00"), 25_000_000)
        self.assertEqual(p.parse_minor("0.10"), 10)
        self.assertEqual(p.parse_minor("1.5"), 150)
        self.assertEqual(p.parse_minor("$1,204.11"), 120_411)

    def test_three_decimal_places_are_refused_rather_than_dropped(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.parse_minor("1.005")
        self.assertIn("minor units", str(ctx.exception))

    def test_a_float_object_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.parse_minor(1.005)  # type: ignore[arg-type]
        self.assertIn("float", str(ctx.exception))

    def test_a_signed_amount_is_refused_so_kind_cannot_be_inferred(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.parse_minor("-40.00")
        self.assertIn("signed", str(ctx.exception))

    def test_a_zero_amount_is_not_a_posting(self):
        with self.assertRaises(p.Refuse):
            p.parse_minor("0.00")


class Mapping(unittest.TestCase):
    def test_an_income_row_is_forecast_income(self):
        out = p.map_batch([row()], book=personal(), client=declared_client())
        self.assertEqual(len(out), 1)
        self.assertEqual(out[0].rule_id, "forecast_income")
        self.assertEqual(out[0].amount, "200.00")
        self.assertEqual(out[0].trade_date, date(2026, 4, 15))
        self.assertTrue(p.conserves(out[0].postings))
        self.assertEqual(p.cash_delta(out[0].postings), 20_000)

    def test_a_spend_row_is_forecast_spend(self):
        out = p.map_batch(
            [row(kind="spend", reference="pred-out")],
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(out[0].rule_id, "forecast_spend")
        self.assertEqual(p.cash_delta(out[0].postings), -20_000)

    def test_a_proposal_is_an_apply_event_shape_not_a_posting_list(self):
        out = p.map_batch([row()], book=personal(), client=declared_client())
        wire = p.as_apply_event(out[0], parent="books/household")
        self.assertEqual(wire["rule_id"], "forecast_income")
        self.assertEqual(wire["amount"], "200.00")
        self.assertTrue(wire["validate_only"])
        self.assertNotIn("postings", wire)
        self.assertNotIn("kind", wire)

    def test_an_empty_batch_leaves_forecast_net_unset_not_a_silent_zero(self):
        out = p.map_batch([], book=personal(), client=declared_client())
        self.assertEqual(out, [])
        self.assertIsNone(p.cite_forecast_net(out))

    def test_a_net_zero_pair_of_forecast_posts_is_a_real_zero(self):
        out = p.map_batch(
            [
                row(kind="income", amount="50.00", reference="in"),
                row(kind="spend", amount="50.00", reference="out"),
            ],
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(p.cite_forecast_net(out), 0)


class PredictedBalance(unittest.TestCase):
    def test_a_higher_predicted_balance_is_forecast_income(self):
        out = p.from_predicted_balance(
            predicted="8500.00",
            dated="2026-04-30",
            statement=sheet(),
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(out.rule_id, "forecast_income")
        self.assertEqual(out.amount, "500.00")
        self.assertEqual(p.cash_delta(out.postings), 50_000)

    def test_a_lower_predicted_balance_is_forecast_spend(self):
        out = p.from_predicted_balance(
            predicted="7500.00",
            dated="2026-04-30",
            statement=sheet(),
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(out.rule_id, "forecast_spend")
        self.assertEqual(out.amount, "500.00")

    def test_unset_cited_cash_is_not_a_silent_zero_baseline(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.from_predicted_balance(
                predicted="8500.00",
                dated="2026-04-30",
                statement=p.Statement(currency="USD"),
                book=personal(),
                client=declared_client(),
            )
        self.assertIn("unset", str(ctx.exception))

    def test_a_predicted_balance_equal_to_cited_cash_is_not_a_posting(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.from_predicted_balance(
                predicted="8000.00",
                dated="2026-04-30",
                statement=sheet(),
                book=personal(),
                client=declared_client(),
            )
        self.assertIn("zero", str(ctx.exception))


class Refusals(unittest.TestCase):
    def test_an_empty_allowlist_refuses_every_post(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.map_batch(
                [row()],
                book=personal(),
                client=declared_client(allowlist=frozenset()),
            )
        self.assertIn("empty", str(ctx.exception))
        self.assertIn("allowlist", str(ctx.exception))

    def test_a_template_off_the_allowlist_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.map_batch(
                [row()],
                book=personal(),
                client=declared_client(allowlist=frozenset({"forecast_spend"})),
            )
        self.assertIn("allowlist", str(ctx.exception))

    def test_call_lp_is_refused_on_a_personal_book(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.map_batch(
                [row(kind="call_lp")],
                book=personal(),
                client=declared_client(),
            )
        self.assertIn("call_lp", str(ctx.exception))

    def test_payroll_kind_is_refused_rather_than_invented(self):
        for kind in ("payroll", "paycheck", "forecast_payroll"):
            with self.assertRaises(p.Refuse) as ctx:
                p.map_batch(
                    [row(kind=kind, reference=kind)],
                    book=personal(),
                    client=declared_client(),
                )
            self.assertIn("payroll", str(ctx.exception).lower())

    def test_envelope_kind_is_refused_rather_than_invented(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.map_batch(
                [row(kind="envelope")],
                book=personal(),
                client=declared_client(),
            )
        self.assertIn("envelope", str(ctx.exception).lower())

    def test_an_actual_spend_cash_is_refused_because_a_future_date_is_still_an_actual(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.map_batch(
                [row(kind="spend_cash")],
                book=personal(),
                client=declared_client(),
            )
        msg = str(ctx.exception)
        self.assertIn("actual", msg.lower())

    def test_scheduled_templates_are_the_calendar_bills_sibling(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.map_batch(
                [row(kind="scheduled_spend")],
                book=personal(),
                client=declared_client(),
            )
        self.assertIn("calendar-bills", str(ctx.exception))

    def test_a_template_absent_from_the_ruleset_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.map_batch(
                [row()],
                book=personal(approved_templates=frozenset({"forecast_spend"})),
                client=declared_client(),
            )
        self.assertIn("RuleSet", str(ctx.exception))

    def test_a_dated_entry_on_or_before_closed_through_is_refused(self):
        book = personal(closed_through=date(2026, 4, 30))
        with self.assertRaises(p.Refuse) as ctx:
            p.map_batch([row(dated="2026-04-30")], book=book, client=declared_client())
        self.assertIn("closed-through", str(ctx.exception))
        with self.assertRaises(p.Refuse) as ctx:
            p.map_batch([row(dated="2026-04-15")], book=book, client=declared_client())
        self.assertIn("closed-through", str(ctx.exception))

    def test_the_day_after_close_is_accepted(self):
        book = personal(closed_through=date(2026, 3, 31))
        out = p.map_batch(
            [row(dated="2026-04-01")],
            book=book,
            client=declared_client(),
        )
        self.assertEqual(out[0].trade_date, date(2026, 4, 1))

    def test_an_undated_row_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.map_batch([row(dated="")], book=personal(), client=declared_client())
        self.assertIn("undated", str(ctx.exception))

    def test_a_closed_row_refuses_the_whole_batch(self):
        book = personal(closed_through=date(2026, 3, 31))
        with self.assertRaises(p.Refuse):
            p.map_batch(
                [
                    row(dated="2026-04-02", reference="open"),
                    row(dated="2026-03-15", reference="closed"),
                ],
                book=book,
                client=declared_client(),
            )

    def test_an_unbalanced_instantiation_refuses_the_batch(self):
        saved = p.FORECAST_LEGS["forecast_income"]
        p.FORECAST_LEGS["forecast_income"] = ((1, 1), (30, 1))
        try:
            with self.assertRaises(p.Refuse) as ctx:
                p.map_batch([row()], book=personal(), client=declared_client())
            self.assertIn("conserve", str(ctx.exception))
        finally:
            p.FORECAST_LEGS["forecast_income"] = saved

    def test_journal_append_is_refused_as_a_scope(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.map_batch(
                [row()],
                book=personal(),
                client=declared_client(
                    scopes=frozenset({"statements:read", "journal:append"})
                ),
            )
        self.assertIn("journal:append", str(ctx.exception))
        self.assertIn("journals:post", str(ctx.exception))

    def test_missing_statements_read_is_refused_because_close_cannot_be_honored(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.map_batch(
                [row()],
                book=personal(),
                client=declared_client(scopes=frozenset({"journals:post"})),
            )
        self.assertIn("statements:read", str(ctx.exception))

    def test_an_investment_book_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.map_batch(
                [row()],
                book=personal(kind="INVESTMENT"),
                client=declared_client(),
            )
        self.assertIn("PERSONAL", str(ctx.exception))

    def test_a_project_book_is_refused_and_names_the_eac_sibling(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.map_batch(
                [row()],
                book=personal(kind="PROJECT"),
                client=declared_client(),
            )
        self.assertIn("eac-forecast", str(ctx.exception))

    def test_envelope_budget_stays_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.envelope_budget()
        self.assertIn("#164", str(ctx.exception))

    def test_payroll_stays_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.payroll()
        self.assertIn("payroll", str(ctx.exception).lower())

    def test_fetch_statements_refuses_because_the_grant_path_is_not_built(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.fetch_statements(token="connect-access-token")
        msg = str(ctx.exception)
        self.assertIn("grant path is not built", msg)
        self.assertIn("#151", msg)

    def test_deliver_refuses_because_the_grant_path_is_not_built(self):
        out = p.map_batch([row()], book=personal(), client=declared_client())
        with self.assertRaises(p.Refuse) as ctx:
            p.deliver(out, token="connect-access-token")
        msg = str(ctx.exception)
        self.assertIn("grant path is not built", msg)
        self.assertIn("#151", msg)


class ManifestHonesty(unittest.TestCase):
    def test_the_app_declares_only_canonical_scopes(self):
        scopes = app()["workos_connect"]["scopes"]
        self.assertEqual(set(scopes), set(p.CANONICAL_SCOPES))
        for alias in p.REFUSED_ALIASES:
            self.assertNotIn(alias, scopes)
        self.assertIn("statements:read", scopes)
        self.assertIn("journals:post", scopes)

    def test_the_declared_allowlist_is_forecast_templates_not_methods(self):
        templates = set(app()["journals_post_allowlist"]["templates"])
        self.assertEqual(templates, set(p.FORECAST_LEGS))
        for forbidden in (
            "fifo",
            "hifo",
            "min_tax",
            "specific_id",
            "average_cost",
            "wash",
            "payroll",
            "envelope",
            "scheduled_spend",
            "scheduled_income",
            "living_expense",
        ):
            self.assertNotIn(forbidden, templates)
            self.assertFalse(any(forbidden in t for t in templates), forbidden)

    def test_grant_path_and_bank_oauth_stay_named_as_leftovers(self):
        doc = json.dumps(app())
        self.assertIn("not built", app()["grant_path"]["status"])
        self.assertIn("not wired", app()["bank_oauth"]["status"])
        self.assertIn("#163", doc)
        self.assertIn("#150", doc)
        self.assertIn("#164", doc)

    def test_every_instantiated_template_is_in_createbook_personal(self):
        if BOOK_RS is None or not BOOK_RS.is_file():
            self.skipTest("book.rs not handed to the test")
        src = BOOK_RS.read_text()
        start = src.index("const PERSONAL_CONFIG")
        end = (
            src.index("const INVESTMENT_CONFIG")
            if "const INVESTMENT_CONFIG" in src
            else len(src)
        )
        personal_src = src[start:end]
        for rule_id in p.FORECAST_LEGS:
            self.assertIn(
                f'id = "{rule_id}"',
                personal_src,
                f"{rule_id} is not a CreateBook(Personal) rule — the app invented it",
            )
        for invented in ("forecast_payroll", "forecast_envelope", "scheduled_payroll"):
            self.assertNotIn(f'id = "{invented}"', personal_src, invented)

    def test_the_catalog_still_refuses_the_alias_this_issue_named(self):
        if CATALOG is None or not CATALOG.is_file():
            self.skipTest("connect-scopes.md not handed to the test")
        text = CATALOG.read_text()
        self.assertIn("`journals:post`", text)
        self.assertIn("`journal:append`", text)
        self.assertIn("`statements:read`", text)
        self.assertIn("leftover #22", text)

    def test_screens_for_personal_was_not_forked_with_a_predictor_url(self):
        if SCREENS is None or not SCREENS.is_file():
            self.skipTest("screens.ts not handed to the test")
        src = SCREENS.read_text()
        start = src.index("export const PERSONAL_SCREENS")
        end = src.index("export const PROJECT_SCREENS")
        personal_src = src[start:end]
        self.assertNotIn("predictor", personal_src.lower())
        self.assertNotIn("bank-balance", personal_src.lower())
        self.assertNotIn("envelope", personal_src.lower())
        self.assertNotIn("payroll", personal_src.lower())
        self.assertIn("cashflow", personal_src)

    def test_the_kernel_did_not_grow_a_forecast_rpc(self):
        if PROTO is None or not PROTO.is_file():
            self.skipTest("console.proto not handed to the test")
        src = PROTO.read_text()
        for needle in (
            "rpc Forecast",
            "rpc CashForecast",
            "rpc PredictCash",
            "rpc BankBalance",
            "message CashForecast ",
            "message Forecast ",
        ):
            self.assertNotIn(
                needle,
                src,
                f"{needle} is a kernel forecast RPC — refuse it; /cashflow already cites",
            )


if __name__ == "__main__":
    unittest.main()
