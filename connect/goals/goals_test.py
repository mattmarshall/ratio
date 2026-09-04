#!/usr/bin/env python3
"""Properties the net-worth goals Connect app must keep.

Test names are sentences. Break the thing a test protects and this
file goes red — a green suite that only checks the books balance is
how a wrong gain ships.

Does not talk to /v1. Does not claim a Connect token is accepted.
Does not grow a kernel Goal RPC.
"""

from __future__ import annotations

import json
import pathlib
import sys
import unittest
from datetime import date

import goals as g

APP_PATH = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).with_name("app.json")
BOOK_RS = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else None
CATALOG = pathlib.Path(sys.argv[3]) if len(sys.argv) > 3 else None
SCREENS = pathlib.Path(sys.argv[4]) if len(sys.argv) > 4 else None
PROTO = pathlib.Path(sys.argv[5]) if len(sys.argv) > 5 else None

sys.argv = sys.argv[:1]


def app() -> dict:
    return json.loads(APP_PATH.read_text())


def declared_client(**overrides) -> g.Client:
    c = g.client_from_app(app())
    return g.Client(
        client_id=overrides.get("client_id", c.client_id),
        allowlist=overrides.get("allowlist", c.allowlist),
        scopes=overrides.get("scopes", c.scopes),
    )


def personal(**overrides) -> g.Book:
    return g.Book(
        kind=overrides.get("kind", "PERSONAL"),
        approved_templates=overrides.get("approved_templates", g.PERSONAL_SEEDED_RULES),
        closed_through=overrides.get("closed_through"),
    )


def sheet(**overrides) -> g.Statement:
    base = {
        "currency": "USD",
        "as_of": "2026-03-31",
        "net_worth": "50000.00",
        "cash": "8000.00",
        "ending_net_worth": "50000.00",
        "ending_cash": "8000.00",
    }
    base.update(overrides)
    return g.statement_from_cite(base)


def goal(**overrides) -> dict:
    base = {
        "name": "emergency fund",
        "target": "75000.00",
        "target_date": "2026-12-31",
        "currency": "USD",
    }
    base.update(overrides)
    return base


def move(**overrides) -> dict:
    base = {
        "dated": "2026-04-15",
        "amount": "500.00",
        "currency": "USD",
        "kind": "income",
        "reference": "what-if-1",
    }
    base.update(overrides)
    return base


class ParseMinor(unittest.TestCase):
    def test_an_amount_is_parsed_by_splitting_not_by_floating(self):
        self.assertEqual(g.parse_minor("250000.00"), 25_000_000)
        self.assertEqual(g.parse_minor("0.10"), 10)
        self.assertEqual(g.parse_minor("0.1"), 10)
        self.assertEqual(g.parse_minor("1.5"), 150)
        self.assertEqual(g.parse_minor("42"), 4_200)
        self.assertEqual(g.parse_minor(".5"), 50)
        self.assertEqual(g.parse_minor("$1,204,880.11"), 120_488_011)

    def test_three_decimal_places_are_refused_rather_than_dropped(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.parse_minor("1.005")
        self.assertIn("minor units", str(ctx.exception))

    def test_a_float_object_is_refused(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.parse_minor(1.005)  # type: ignore[arg-type]
        self.assertIn("float", str(ctx.exception))

    def test_a_signed_amount_is_refused_so_kind_cannot_be_inferred(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.parse_minor("-40.00")
        self.assertIn("signed", str(ctx.exception))

    def test_a_zero_amount_is_not_a_posting(self):
        with self.assertRaises(g.Refuse):
            g.parse_minor("0.00")


class GoalProgress(unittest.TestCase):
    def test_a_short_goal_cites_current_target_and_gap_without_a_percentage(self):
        out = g.evaluate_goal(
            goal(),
            statement=sheet(),
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(out.current, 5_000_000)
        self.assertEqual(out.target, 7_500_000)
        self.assertEqual(out.gap, 2_500_000)
        self.assertEqual(out.status, "short")
        self.assertEqual(out.name, "emergency fund")
        self.assertNotIn("%", g.format_minor(out.gap))

    def test_a_met_goal_is_met_when_the_sheet_is_at_or_above_the_target(self):
        out = g.evaluate_goal(
            goal(target="40000.00"),
            statement=sheet(net_worth="50000.00"),
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(out.status, "met")
        self.assertEqual(out.gap, -1_000_000)

    def test_an_unset_sheet_leaves_progress_unset_not_a_silent_zero(self):
        out = g.evaluate_goal(
            goal(),
            statement=g.Statement(currency="USD"),
            book=personal(),
            client=declared_client(),
        )
        self.assertIsNone(out.current)
        self.assertIsNone(out.gap)
        self.assertEqual(out.status, "unset")
        self.assertEqual(out.target, 7_500_000)

    def test_a_real_zero_net_worth_is_a_figure_not_unset(self):
        out = g.evaluate_goal(
            goal(),
            statement=sheet(net_worth="0.00"),
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(out.current, 0)
        self.assertEqual(out.status, "short")
        self.assertEqual(out.gap, 7_500_000)

    def test_a_currency_mismatch_is_refused(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.evaluate_goal(
                goal(),
                statement=sheet(currency="EUR"),
                book=personal(),
                client=declared_client(),
            )
        self.assertIn("EUR", str(ctx.exception))

    def test_missing_statements_read_is_refused_because_the_sheet_cannot_be_cited(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.evaluate_goal(
                goal(),
                statement=sheet(),
                book=personal(),
                client=declared_client(scopes=frozenset({"journals:post"})),
            )
        self.assertIn("statements:read", str(ctx.exception))


class ScenarioOverlay(unittest.TestCase):
    def test_extra_income_raises_projected_net_worth_and_cash(self):
        out = g.overlay_scenario(
            [move(kind="income", amount="500.00")],
            statement=sheet(),
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(out.net_worth_delta, 50_000)
        self.assertEqual(out.cash_delta, 50_000)
        self.assertEqual(out.projected_net_worth, 5_050_000)
        self.assertEqual(out.projected_cash, 850_000)
        self.assertEqual(out.posts[0].rule_id, "household_income")
        self.assertTrue(g.conserves(out.posts[0].postings))

    def test_a_living_expense_lowers_net_worth_and_cash(self):
        out = g.overlay_scenario(
            [move(kind="expense", amount="200.00", reference="exp-1")],
            statement=sheet(),
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(out.posts[0].rule_id, "living_expense")
        self.assertEqual(out.net_worth_delta, -20_000)
        self.assertEqual(out.cash_delta, -20_000)

    def test_a_card_charge_lowers_net_worth_and_does_not_move_cash(self):
        out = g.overlay_scenario(
            [move(kind="card", amount="200.00", reference="card-1")],
            statement=sheet(),
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(out.posts[0].rule_id, "card_charge")
        self.assertEqual(out.net_worth_delta, -20_000)
        self.assertEqual(out.cash_delta, 0)
        accounts = {p.account for p in out.posts[0].postings}
        self.assertEqual(accounts, {10, 40})
        self.assertNotIn(1, accounts)

    def test_a_cash_to_investments_transfer_does_not_change_net_worth(self):
        out = g.overlay_scenario(
            [move(**{"kind": "transfer", "from": "cash", "to": "investments", "reference": "xfer-1"})],
            statement=sheet(),
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(out.posts[0].rule_id, "xfer_cash_investments")
        self.assertEqual(out.net_worth_delta, 0)
        self.assertEqual(out.cash_delta, -50_000)
        self.assertEqual(out.projected_net_worth, 5_000_000)
        self.assertTrue(g.conserves(out.posts[0].postings))

    def test_an_unset_sheet_leaves_projected_net_worth_unset(self):
        out = g.overlay_scenario(
            [move()],
            statement=g.Statement(currency="USD"),
            book=personal(),
            client=declared_client(),
        )
        self.assertIsNone(out.projected_net_worth)
        self.assertIsNone(out.projected_cash)
        self.assertEqual(out.net_worth_delta, 50_000)

    def test_overlay_does_not_require_journals_post(self):
        out = g.overlay_scenario(
            [move()],
            statement=sheet(),
            book=personal(),
            client=declared_client(scopes=frozenset({"statements:read"})),
        )
        self.assertEqual(out.posts[0].rule_id, "household_income")

    def test_usd_plus_eur_minus_is_not_conserved(self):
        posts = (
            g.Posting(10, 100, "USD"),
            g.Posting(1, -100, "EUR"),
        )
        self.assertFalse(g.conserves(posts))


class OptInPosts(unittest.TestCase):
    def test_non_opt_in_must_not_post(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.propose_scenario_posts(
                [move()],
                book=personal(),
                client=declared_client(),
                opt_in=False,
            )
        self.assertIn("opts in", str(ctx.exception))
        self.assertIn("must not post", str(ctx.exception))

    def test_opt_in_proposes_an_apply_event_shape_not_a_posting_list(self):
        out = g.propose_scenario_posts(
            [move()],
            book=personal(),
            client=declared_client(),
            opt_in=True,
        )
        self.assertEqual(len(out), 1)
        self.assertEqual(out[0].rule_id, "household_income")
        wire = g.as_apply_event(out[0], parent="books/household")
        self.assertEqual(wire["rule_id"], "household_income")
        self.assertEqual(wire["amount"], "500.00")
        self.assertTrue(wire["validate_only"])
        self.assertNotIn("postings", wire)

    def test_an_empty_allowlist_refuses_every_post(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.propose_scenario_posts(
                [move()],
                book=personal(),
                client=declared_client(allowlist=frozenset()),
                opt_in=True,
            )
        self.assertIn("empty", str(ctx.exception))
        self.assertIn("allowlist", str(ctx.exception))

    def test_a_template_off_the_allowlist_is_refused(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.propose_scenario_posts(
                [move(kind="expense")],
                book=personal(),
                client=declared_client(allowlist=frozenset({"household_income"})),
                opt_in=True,
            )
        self.assertIn("allowlist", str(ctx.exception))

    def test_a_dated_entry_on_or_before_closed_through_is_refused(self):
        book = personal(closed_through=date(2026, 3, 31))
        with self.assertRaises(g.Refuse) as ctx:
            g.propose_scenario_posts(
                [move(dated="2026-03-31")],
                book=book,
                client=declared_client(),
                opt_in=True,
            )
        self.assertIn("closed-through", str(ctx.exception))
        with self.assertRaises(g.Refuse) as ctx:
            g.propose_scenario_posts(
                [move(dated="2026-03-15")],
                book=book,
                client=declared_client(),
                opt_in=True,
            )
        self.assertIn("closed-through", str(ctx.exception))

    def test_the_day_after_close_is_accepted_when_opted_in(self):
        book = personal(closed_through=date(2026, 3, 31))
        out = g.propose_scenario_posts(
            [move(dated="2026-04-01")],
            book=book,
            client=declared_client(),
            opt_in=True,
        )
        self.assertEqual(out[0].trade_date, date(2026, 4, 1))

    def test_an_undated_row_is_refused(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.propose_scenario_posts(
                [move(dated="")],
                book=personal(),
                client=declared_client(),
                opt_in=True,
            )
        self.assertIn("undated", str(ctx.exception))

    def test_a_closed_row_refuses_the_whole_batch(self):
        book = personal(closed_through=date(2026, 3, 31))
        with self.assertRaises(g.Refuse):
            g.propose_scenario_posts(
                [
                    move(dated="2026-04-02", reference="open"),
                    move(dated="2026-03-15", reference="closed"),
                ],
                book=book,
                client=declared_client(),
                opt_in=True,
            )

    def test_call_lp_is_refused_on_a_personal_book(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.propose_scenario_posts(
                [move(kind="call_lp")],
                book=personal(),
                client=declared_client(),
                opt_in=True,
            )
        self.assertIn("call_lp", str(ctx.exception))

    def test_a_template_absent_from_the_ruleset_is_refused(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.propose_scenario_posts(
                [move()],
                book=personal(approved_templates=frozenset({"living_expense"})),
                client=declared_client(),
                opt_in=True,
            )
        self.assertIn("RuleSet", str(ctx.exception))

    def test_journals_post_is_required_to_propose_a_write(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.propose_scenario_posts(
                [move()],
                book=personal(),
                client=declared_client(scopes=frozenset({"statements:read"})),
                opt_in=True,
            )
        self.assertIn("journals:post", str(ctx.exception))

    def test_journal_append_is_refused_as_a_scope(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.propose_scenario_posts(
                [move()],
                book=personal(),
                client=declared_client(
                    scopes=frozenset({"statements:read", "journal:append"})
                ),
                opt_in=True,
            )
        self.assertIn("journal:append", str(ctx.exception))
        self.assertIn("journals:post", str(ctx.exception))

    def test_an_investment_book_is_refused(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.propose_scenario_posts(
                [move()],
                book=personal(kind="INVESTMENT"),
                client=declared_client(),
                opt_in=True,
            )
        self.assertIn("PERSONAL", str(ctx.exception))

    def test_an_unbalanced_instantiation_refuses_the_batch(self):
        saved = g.PERSONAL_LEGS["household_income"]
        g.PERSONAL_LEGS["household_income"] = ((1, 1), (30, 1))
        try:
            with self.assertRaises(g.Refuse) as ctx:
                g.propose_scenario_posts(
                    [move()],
                    book=personal(),
                    client=declared_client(),
                    opt_in=True,
                )
            self.assertIn("conserve", str(ctx.exception))
        finally:
            g.PERSONAL_LEGS["household_income"] = saved

    def test_overlay_on_a_closed_date_is_not_a_mutation(self):
        # A what-if dated inside a closed period is still a read.
        # Posting that date is the thing the gate refuses.
        book = personal(closed_through=date(2026, 3, 31))
        out = g.overlay_scenario(
            [move(dated="2026-03-15")],
            statement=sheet(),
            book=book,
            client=declared_client(),
        )
        self.assertEqual(out.posts[0].trade_date, date(2026, 3, 15))
        with self.assertRaises(g.Refuse) as ctx:
            g.propose_scenario_posts(
                [move(dated="2026-03-15")],
                book=book,
                client=declared_client(),
                opt_in=True,
            )
        self.assertIn("closed-through", str(ctx.exception))


class ForecastRefusals(unittest.TestCase):
    def test_required_savings_is_refused_as_a_cash_forecast(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.required_savings(goal(), statement=sheet())
        self.assertIn("cash forecast", str(ctx.exception))

    def test_a_fire_number_is_refused(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.fire_number(goal())
        self.assertIn("FIRE", str(ctx.exception))

    def test_fetch_statements_refuses_because_the_grant_path_is_not_built(self):
        with self.assertRaises(g.Refuse) as ctx:
            g.fetch_statements(token="connect-access-token")
        msg = str(ctx.exception)
        self.assertIn("grant path is not built", msg)
        self.assertIn("#151", msg)

    def test_deliver_refuses_because_the_grant_path_is_not_built(self):
        out = g.propose_scenario_posts(
            [move()],
            book=personal(),
            client=declared_client(),
            opt_in=True,
        )
        with self.assertRaises(g.Refuse) as ctx:
            g.deliver(out, token="connect-access-token")
        msg = str(ctx.exception)
        self.assertIn("grant path is not built", msg)
        self.assertIn("#151", msg)


class ManifestHonesty(unittest.TestCase):
    def test_the_app_declares_only_canonical_scopes(self):
        scopes = app()["workos_connect"]["scopes"]
        self.assertEqual(set(scopes), set(g.CANONICAL_SCOPES))
        for alias in g.REFUSED_ALIASES:
            self.assertNotIn(alias, scopes)
        self.assertIn("statements:read", scopes)
        self.assertIn("journals:post", scopes)

    def test_the_declared_allowlist_is_personal_templates_not_methods(self):
        templates = set(app()["journals_post_allowlist"]["templates"])
        self.assertTrue(templates)
        self.assertTrue(templates <= set(g.PERSONAL_LEGS))
        for forbidden in ("fifo", "hifo", "min_tax", "specific_id", "average_cost", "wash"):
            self.assertNotIn(forbidden, templates)
            self.assertFalse(any(forbidden in t for t in templates), forbidden)

    def test_grant_path_and_opt_in_stay_named_as_leftovers(self):
        doc = json.dumps(app())
        self.assertIn("not built", app()["grant_path"]["status"])
        self.assertIn("opt-in only", app()["scenario_journals"]["status"])
        self.assertIn("#168", doc)
        self.assertIn("#150", doc)

    def test_every_instantiated_template_is_in_createbook_personal(self):
        if BOOK_RS is None or not BOOK_RS.is_file():
            self.skipTest("book.rs not handed to the test")
        src = BOOK_RS.read_text()
        start = src.index("const PERSONAL_CONFIG")
        end = src.index("const INVESTMENT_CONFIG") if "const INVESTMENT_CONFIG" in src else len(src)
        personal_src = src[start:end]
        for rule_id in g.PERSONAL_LEGS:
            self.assertIn(
                f'id = "{rule_id}"',
                personal_src,
                f"{rule_id} is not a CreateBook(Personal) rule — the app invented it",
            )

    def test_the_catalog_still_refuses_the_alias_this_issue_named(self):
        if CATALOG is None or not CATALOG.is_file():
            self.skipTest("connect-scopes.md not handed to the test")
        text = CATALOG.read_text()
        self.assertIn("`journals:post`", text)
        self.assertIn("`journal:append`", text)
        self.assertIn("`statements:read`", text)
        self.assertIn("grant path is not built", text)

    def test_screens_for_personal_was_not_forked_with_a_goals_url(self):
        if SCREENS is None or not SCREENS.is_file():
            self.skipTest("screens.ts not handed to the test")
        src = SCREENS.read_text()
        start = src.index("export const PERSONAL_SCREENS")
        end = src.index("export const PROJECT_SCREENS")
        personal_src = src[start:end]
        self.assertNotIn("goal", personal_src.lower())
        self.assertNotIn("scenario", personal_src.lower())
        self.assertNotIn("what-if", personal_src.lower())
        self.assertIn("bridge", personal_src)
        self.assertIn("cashflow", personal_src)
        self.assertIn("sheet", personal_src)

    def test_the_kernel_did_not_grow_a_goal_tracking_rpc(self):
        if PROTO is None or not PROTO.is_file():
            self.skipTest("console.proto not handed to the test")
        src = PROTO.read_text()
        # ProjectProgress is a job figure, not a household goal.
        self.assertIn("rpc ProjectProgress", src)
        for needle in (
            "rpc Goal",
            "rpc NetWorthGoal",
            "rpc TrackGoal",
            "rpc Scenario",
            "rpc WhatIf",
            "message Goal ",
            "message Scenario ",
        ):
            self.assertNotIn(
                needle,
                src,
                f"{needle} is a kernel goal-tracking RPC — refuse it; this app is the door",
            )


if __name__ == "__main__":
    unittest.main()
