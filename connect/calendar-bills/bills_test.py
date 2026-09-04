#!/usr/bin/env python3
"""Properties the calendar-bills Connect app must keep.

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

import bills as b

APP_PATH = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).with_name("app.json")
BOOK_RS = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else None
CATALOG = pathlib.Path(sys.argv[3]) if len(sys.argv) > 3 else None
SCREENS = pathlib.Path(sys.argv[4]) if len(sys.argv) > 4 else None
PROTO = pathlib.Path(sys.argv[5]) if len(sys.argv) > 5 else None

sys.argv = sys.argv[:1]


def app() -> dict:
    return json.loads(APP_PATH.read_text())


def declared_client(**overrides) -> b.Client:
    c = b.client_from_app(app())
    return b.Client(
        client_id=overrides.get("client_id", c.client_id),
        allowlist=overrides.get("allowlist", c.allowlist),
        scopes=overrides.get("scopes", c.scopes),
    )


def personal(**overrides) -> b.Book:
    return b.Book(
        kind=overrides.get("kind", "PERSONAL"),
        approved_templates=overrides.get(
            "approved_templates", b.PERSONAL_SEEDED_RULES
        ),
        closed_through=overrides.get("closed_through"),
    )


def row(**overrides) -> dict:
    base = {
        "dated": "2026-04-01",
        "amount": "1800.00",
        "currency": "USD",
        "kind": "bill",
        "reference": "rent-apr",
    }
    base.update(overrides)
    return base


class ParseMinor(unittest.TestCase):
    def test_an_amount_is_parsed_by_splitting_not_by_floating(self):
        self.assertEqual(b.parse_minor("1800.00"), 180_000)
        self.assertEqual(b.parse_minor("0.10"), 10)
        self.assertEqual(b.parse_minor("1.5"), 150)

    def test_three_decimal_places_are_refused_rather_than_dropped(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.parse_minor("1.005")
        self.assertIn("minor units", str(ctx.exception))

    def test_a_float_object_is_refused(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.parse_minor(1.005)  # type: ignore[arg-type]
        self.assertIn("float", str(ctx.exception))

    def test_a_signed_amount_is_refused_so_kind_cannot_be_inferred(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.parse_minor("-1800.00")
        self.assertIn("signed", str(ctx.exception))

    def test_a_zero_amount_is_not_a_posting(self):
        with self.assertRaises(b.Refuse):
            b.parse_minor("0.00")


class Mapping(unittest.TestCase):
    def test_a_bill_row_is_scheduled_spend(self):
        out = b.map_batch([row()], book=personal(), client=declared_client())
        self.assertEqual(len(out), 1)
        self.assertEqual(out[0].rule_id, "scheduled_spend")
        self.assertEqual(out[0].amount, "1800.00")
        self.assertEqual(out[0].trade_date, date(2026, 4, 1))
        self.assertTrue(b.conserves(out[0].postings))
        self.assertEqual(b.cash_delta(out[0].postings), -180_000)

    def test_income_is_scheduled_income(self):
        out = b.map_batch(
            [row(kind="income", amount="40.00", reference="div-1")],
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(out[0].rule_id, "scheduled_income")
        self.assertEqual(b.cash_delta(out[0].postings), 4_000)

    def test_a_proposal_is_an_apply_event_shape_not_a_posting_list(self):
        out = b.map_batch([row()], book=personal(), client=declared_client())
        wire = b.as_apply_event(out[0], parent="books/household")
        self.assertEqual(wire["rule_id"], "scheduled_spend")
        self.assertEqual(wire["amount"], "1800.00")
        self.assertTrue(wire["validate_only"])
        self.assertNotIn("postings", wire)
        self.assertNotIn("kind", wire)

    def test_an_empty_batch_leaves_scheduled_net_unset_not_a_silent_zero(self):
        out = b.map_batch([], book=personal(), client=declared_client())
        self.assertEqual(out, [])
        self.assertIsNone(b.cite_scheduled_net(out))

    def test_a_net_zero_pair_of_scheduled_posts_is_a_real_zero(self):
        out = b.map_batch(
            [
                row(kind="income", amount="50.00", reference="in"),
                row(kind="spend", amount="50.00", reference="out"),
            ],
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(b.cite_scheduled_net(out), 0)


class Refusals(unittest.TestCase):
    def test_an_empty_allowlist_refuses_every_post(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch(
                [row()],
                book=personal(),
                client=declared_client(allowlist=frozenset()),
            )
        self.assertIn("empty", str(ctx.exception))
        self.assertIn("allowlist", str(ctx.exception))

    def test_a_template_off_the_allowlist_is_refused(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch(
                [row()],
                book=personal(),
                client=declared_client(allowlist=frozenset({"scheduled_income"})),
            )
        self.assertIn("allowlist", str(ctx.exception))

    def test_call_lp_is_refused_on_a_personal_book(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch(
                [row(kind="call_lp")],
                book=personal(),
                client=declared_client(),
            )
        self.assertIn("call_lp", str(ctx.exception))

    def test_payroll_kind_is_refused_rather_than_invented(self):
        for kind in ("payroll", "paycheck", "salary", "scheduled_payroll"):
            with self.assertRaises(b.Refuse) as ctx:
                b.map_batch(
                    [row(kind=kind, reference=kind)],
                    book=personal(),
                    client=declared_client(),
                )
            msg = str(ctx.exception).lower()
            self.assertTrue("payroll" in msg or "paycheck" in msg or "salary" in msg, msg)

    def test_envelope_kind_is_refused_rather_than_invented(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch(
                [row(kind="envelope")],
                book=personal(),
                client=declared_client(),
            )
        self.assertIn("envelope", str(ctx.exception).lower())

    def test_an_actual_spend_cash_is_refused_because_a_future_date_is_still_an_actual(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch(
                [row(kind="spend_cash")],
                book=personal(),
                client=declared_client(),
            )
        self.assertIn("actual", str(ctx.exception).lower())

    def test_forecast_templates_are_the_bank_balance_sibling(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch(
                [row(kind="forecast_spend")],
                book=personal(),
                client=declared_client(),
            )
        self.assertIn("bank-balance-predictor", str(ctx.exception))

    def test_an_rrule_is_refused_rather_than_expanded_into_the_journal(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch(
                [row(rrule="FREQ=MONTHLY")],
                book=personal(),
                client=declared_client(),
            )
        self.assertIn("recurrence", str(ctx.exception))
        self.assertIn("dated row", str(ctx.exception))

    def test_a_repeat_field_is_refused(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch(
                [row(repeat="monthly")],
                book=personal(),
                client=declared_client(),
            )
        self.assertIn("recurrence", str(ctx.exception))

    def test_a_template_absent_from_the_ruleset_is_refused(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch(
                [row()],
                book=personal(approved_templates=frozenset({"scheduled_income"})),
                client=declared_client(),
            )
        self.assertIn("RuleSet", str(ctx.exception))

    def test_a_dated_entry_on_or_before_closed_through_is_refused(self):
        book = personal(closed_through=date(2026, 4, 30))
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch([row(dated="2026-04-01")], book=book, client=declared_client())
        self.assertIn("closed-through", str(ctx.exception))

    def test_the_day_after_close_is_accepted(self):
        book = personal(closed_through=date(2026, 3, 31))
        out = b.map_batch(
            [row(dated="2026-04-01")],
            book=book,
            client=declared_client(),
        )
        self.assertEqual(out[0].trade_date, date(2026, 4, 1))

    def test_an_undated_row_is_refused(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch([row(dated="")], book=personal(), client=declared_client())
        self.assertIn("undated", str(ctx.exception))

    def test_a_closed_row_refuses_the_whole_batch(self):
        book = personal(closed_through=date(2026, 3, 31))
        with self.assertRaises(b.Refuse):
            b.map_batch(
                [
                    row(dated="2026-04-02", reference="open"),
                    row(dated="2026-03-15", reference="closed"),
                ],
                book=book,
                client=declared_client(),
            )

    def test_an_unbalanced_instantiation_refuses_the_batch(self):
        saved = b.SCHEDULED_LEGS["scheduled_spend"]
        b.SCHEDULED_LEGS["scheduled_spend"] = ((10, 1), (1, 1))
        try:
            with self.assertRaises(b.Refuse) as ctx:
                b.map_batch([row()], book=personal(), client=declared_client())
            self.assertIn("conserve", str(ctx.exception))
        finally:
            b.SCHEDULED_LEGS["scheduled_spend"] = saved

    def test_journal_append_is_refused_as_a_scope(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch(
                [row()],
                book=personal(),
                client=declared_client(
                    scopes=frozenset({"statements:read", "journal:append"})
                ),
            )
        self.assertIn("journal:append", str(ctx.exception))
        self.assertIn("journals:post", str(ctx.exception))

    def test_missing_statements_read_is_refused_because_close_cannot_be_honored(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch(
                [row()],
                book=personal(),
                client=declared_client(scopes=frozenset({"journals:post"})),
            )
        self.assertIn("statements:read", str(ctx.exception))

    def test_an_investment_book_is_refused(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch(
                [row()],
                book=personal(kind="INVESTMENT"),
                client=declared_client(),
            )
        self.assertIn("PERSONAL", str(ctx.exception))

    def test_a_project_book_is_refused_and_names_the_eac_sibling(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.map_batch(
                [row()],
                book=personal(kind="PROJECT"),
                client=declared_client(),
            )
        self.assertIn("eac-forecast", str(ctx.exception))

    def test_envelope_budget_stays_refused(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.envelope_budget()
        self.assertIn("#164", str(ctx.exception))

    def test_payroll_stays_refused(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.payroll()
        self.assertIn("payroll", str(ctx.exception).lower())

    def test_fetch_statements_refuses_because_the_grant_path_is_not_built(self):
        with self.assertRaises(b.Refuse) as ctx:
            b.fetch_statements(token="connect-access-token")
        msg = str(ctx.exception)
        self.assertIn("grant path is not built", msg)
        self.assertIn("#151", msg)

    def test_deliver_refuses_because_the_grant_path_is_not_built(self):
        out = b.map_batch([row()], book=personal(), client=declared_client())
        with self.assertRaises(b.Refuse) as ctx:
            b.deliver(out, token="connect-access-token")
        msg = str(ctx.exception)
        self.assertIn("grant path is not built", msg)
        self.assertIn("#151", msg)


class ManifestHonesty(unittest.TestCase):
    def test_the_app_declares_only_canonical_scopes(self):
        scopes = app()["workos_connect"]["scopes"]
        self.assertEqual(set(scopes), set(b.CANONICAL_SCOPES))
        for alias in b.REFUSED_ALIASES:
            self.assertNotIn(alias, scopes)
        self.assertIn("statements:read", scopes)
        self.assertIn("journals:post", scopes)

    def test_the_declared_allowlist_is_scheduled_templates_not_methods(self):
        templates = set(app()["journals_post_allowlist"]["templates"])
        self.assertEqual(templates, set(b.SCHEDULED_LEGS))
        for forbidden in (
            "fifo",
            "hifo",
            "min_tax",
            "specific_id",
            "average_cost",
            "wash",
            "payroll",
            "envelope",
            "forecast_spend",
            "forecast_income",
            "living_expense",
        ):
            self.assertNotIn(forbidden, templates)
            self.assertFalse(any(forbidden in t for t in templates), forbidden)

    def test_grant_path_and_calendar_oauth_stay_named_as_leftovers(self):
        doc = json.dumps(app())
        self.assertIn("not built", app()["grant_path"]["status"])
        self.assertIn("not wired", app()["calendar_oauth"]["status"])
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
        for rule_id in b.SCHEDULED_LEGS:
            self.assertIn(
                f'id = "{rule_id}"',
                personal_src,
                f"{rule_id} is not a CreateBook(Personal) rule — the app invented it",
            )
        for invented in ("scheduled_payroll", "scheduled_envelope", "forecast_payroll"):
            self.assertNotIn(f'id = "{invented}"', personal_src, invented)

    def test_the_catalog_still_refuses_the_alias_this_issue_named(self):
        if CATALOG is None or not CATALOG.is_file():
            self.skipTest("connect-scopes.md not handed to the test")
        text = CATALOG.read_text()
        self.assertIn("`journals:post`", text)
        self.assertIn("`journal:append`", text)
        self.assertIn("`statements:read`", text)
        self.assertIn("grant path is not built", text)

    def test_screens_for_personal_was_not_forked_with_a_bills_url(self):
        if SCREENS is None or not SCREENS.is_file():
            self.skipTest("screens.ts not handed to the test")
        src = SCREENS.read_text()
        start = src.index("export const PERSONAL_SCREENS")
        end = src.index("export const PROJECT_SCREENS")
        personal_src = src[start:end]
        self.assertNotIn("calendar", personal_src.lower())
        self.assertNotIn("bills", personal_src.lower())
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
            "rpc CalendarBill",
            "rpc ScheduledBill",
            "message CashForecast ",
            "message CalendarBill ",
        ):
            self.assertNotIn(
                needle,
                src,
                f"{needle} is a kernel forecast RPC — refuse it; /cashflow already cites",
            )


if __name__ == "__main__":
    unittest.main()
