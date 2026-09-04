#!/usr/bin/env python3
"""Properties the bank-feed mapper must keep.

Test names are sentences. Break the thing a test protects and this
file goes red — a green suite that only checks the books balance is
how a wrong gain ships.

Does not talk to /v1. Does not claim a Connect token is accepted.
"""

from __future__ import annotations

import json
import pathlib
import re
import sys
import unittest
from datetime import date

import mapper as m

# Bazel (or a local run) hands us the files this check is about.
APP_PATH = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).with_name("app.json")
BOOK_RS = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else None
CATALOG = pathlib.Path(sys.argv[3]) if len(sys.argv) > 3 else None

# unittest would otherwise treat leftover argv as a test name.
sys.argv = sys.argv[:1]


def app() -> dict:
    return json.loads(APP_PATH.read_text())


def declared_client(**overrides) -> m.Client:
    c = m.client_from_app(app())
    return m.Client(
        client_id=overrides.get("client_id", c.client_id),
        allowlist=overrides.get("allowlist", c.allowlist),
        scopes=overrides.get("scopes", c.scopes),
    )


def personal(**overrides) -> m.Book:
    return m.Book(
        kind=overrides.get("kind", "PERSONAL"),
        approved_templates=overrides.get("approved_templates", m.PERSONAL_SEEDED_RULES),
        closed_through=overrides.get("closed_through"),
    )


def row(**overrides) -> dict:
    base = {
        "dated": "2026-03-15",
        "amount": "40.00",
        "currency": "USD",
        "kind": "expense",
        "reference": "stmt-1",
    }
    base.update(overrides)
    return base


class ParseMinor(unittest.TestCase):
    def test_an_amount_is_parsed_by_splitting_not_by_floating(self):
        self.assertEqual(m.parse_minor("250000.00"), 25_000_000)
        self.assertEqual(m.parse_minor("0.10"), 10)
        self.assertEqual(m.parse_minor("0.1"), 10)
        self.assertEqual(m.parse_minor("1.5"), 150)
        self.assertEqual(m.parse_minor("42"), 4_200)
        self.assertEqual(m.parse_minor(".5"), 50)
        self.assertEqual(m.parse_minor("$1,204,880.11"), 120_488_011)

    def test_three_decimal_places_are_refused_rather_than_dropped(self):
        with self.assertRaises(m.Refuse) as ctx:
            m.parse_minor("1.005")
        self.assertIn("minor units", str(ctx.exception))

    def test_a_float_object_is_refused(self):
        with self.assertRaises(m.Refuse) as ctx:
            m.parse_minor(1.005)  # type: ignore[arg-type]
        self.assertIn("float", str(ctx.exception))

    def test_a_signed_amount_is_refused_so_kind_cannot_be_inferred(self):
        with self.assertRaises(m.Refuse) as ctx:
            m.parse_minor("-40.00")
        self.assertIn("signed", str(ctx.exception))

    def test_a_zero_amount_is_not_a_posting(self):
        with self.assertRaises(m.Refuse):
            m.parse_minor("0.00")

    def test_an_amount_that_does_not_fit_i64_is_refused(self):
        with self.assertRaises(m.Refuse):
            m.parse_minor("92233720368547758.08")


class Conservation(unittest.TestCase):
    def test_a_personal_template_conserves_in_one_currency(self):
        posts = m.instantiate("living_expense", 4000, "USD")
        self.assertTrue(m.conserves(posts))
        self.assertEqual(posts[0].amount, 4000)
        self.assertEqual(posts[1].amount, -4000)

    def test_usd_plus_eur_minus_is_not_conserved(self):
        posts = (
            m.Posting(10, 100, "USD"),
            m.Posting(1, -100, "EUR"),
        )
        self.assertFalse(m.conserves(posts))

    def test_an_empty_posting_list_is_not_conserved(self):
        self.assertFalse(m.conserves(()))

    def test_a_one_sided_entry_is_not_conserved(self):
        self.assertFalse(m.conserves((m.Posting(10, 4000, "USD"),)))


class Mapping(unittest.TestCase):
    def test_an_expense_row_is_living_expense(self):
        out = m.map_batch([row()], book=personal(), client=declared_client())
        self.assertEqual(len(out), 1)
        self.assertEqual(out[0].rule_id, "living_expense")
        self.assertEqual(out[0].amount, "40.00")
        self.assertEqual(out[0].trade_date, date(2026, 3, 15))
        self.assertTrue(m.conserves(out[0].postings))

    def test_income_is_household_income(self):
        out = m.map_batch(
            [row(kind="income", reference="inc-1")],
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(out[0].rule_id, "household_income")

    def test_a_card_charge_does_not_move_cash(self):
        out = m.map_batch(
            [row(kind="card", reference="card-1")],
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(out[0].rule_id, "card_charge")
        accounts = {p.account for p in out[0].postings}
        self.assertEqual(accounts, {10, 40})
        self.assertNotIn(1, accounts)

    def test_spend_and_receive_use_the_seeded_names(self):
        for kind, rule in (
            ("spend_cash", "spend_cash"),
            ("spend_card", "spend_card"),
            ("receive_income", "receive_income"),
        ):
            out = m.map_batch(
                [row(kind=kind, reference=kind)],
                book=personal(),
                client=declared_client(),
            )
            self.assertEqual(out[0].rule_id, rule, kind)

    def test_a_cash_to_cards_transfer_uses_the_seeded_xfer(self):
        out = m.map_batch(
            [row(**{"kind": "transfer", "from": "cash", "to": "cards", "reference": "xfer-1"})],
            book=personal(),
            client=declared_client(),
        )
        self.assertEqual(out[0].rule_id, "xfer_cash_cards")
        self.assertTrue(m.conserves(out[0].postings))

    def test_a_proposal_is_an_apply_event_shape_not_a_posting_list(self):
        out = m.map_batch([row()], book=personal(), client=declared_client())
        wire = m.as_apply_event(out[0], parent="books/household")
        self.assertEqual(wire["rule_id"], "living_expense")
        self.assertEqual(wire["amount"], "40.00")
        self.assertTrue(wire["validate_only"])
        self.assertNotIn("postings", wire)


class Refusals(unittest.TestCase):
    def test_an_empty_allowlist_refuses_every_post(self):
        with self.assertRaises(m.Refuse) as ctx:
            m.map_batch(
                [row()],
                book=personal(),
                client=declared_client(allowlist=frozenset()),
            )
        self.assertIn("empty", str(ctx.exception))
        self.assertIn("allowlist", str(ctx.exception))

    def test_a_template_off_the_allowlist_is_refused(self):
        with self.assertRaises(m.Refuse) as ctx:
            m.map_batch(
                [row()],
                book=personal(),
                client=declared_client(allowlist=frozenset({"household_income"})),
            )
        self.assertIn("allowlist", str(ctx.exception))

    def test_call_lp_is_refused_on_a_personal_book(self):
        with self.assertRaises(m.Refuse) as ctx:
            m.map_batch(
                [row(kind="call_lp")],
                book=personal(),
                client=declared_client(),
            )
        self.assertIn("call_lp", str(ctx.exception))

    def test_a_template_absent_from_the_ruleset_is_refused(self):
        with self.assertRaises(m.Refuse) as ctx:
            m.map_batch(
                [row()],
                book=personal(approved_templates=frozenset({"household_income"})),
                client=declared_client(),
            )
        self.assertIn("RuleSet", str(ctx.exception))

    def test_a_dated_entry_on_or_before_closed_through_is_refused(self):
        book = personal(closed_through=date(2026, 3, 31))
        with self.assertRaises(m.Refuse) as ctx:
            m.map_batch([row(dated="2026-03-31")], book=book, client=declared_client())
        self.assertIn("closed-through", str(ctx.exception))
        with self.assertRaises(m.Refuse) as ctx:
            m.map_batch([row(dated="2026-03-15")], book=book, client=declared_client())
        self.assertIn("closed-through", str(ctx.exception))

    def test_the_day_after_close_is_accepted(self):
        book = personal(closed_through=date(2026, 3, 31))
        out = m.map_batch(
            [row(dated="2026-04-01")],
            book=book,
            client=declared_client(),
        )
        self.assertEqual(out[0].trade_date, date(2026, 4, 1))

    def test_an_undated_row_is_refused(self):
        with self.assertRaises(m.Refuse) as ctx:
            m.map_batch([row(dated="")], book=personal(), client=declared_client())
        self.assertIn("undated", str(ctx.exception))

    def test_a_closed_row_refuses_the_whole_batch(self):
        book = personal(closed_through=date(2026, 3, 31))
        with self.assertRaises(m.Refuse):
            m.map_batch(
                [
                    row(dated="2026-04-02", reference="open"),
                    row(dated="2026-03-15", reference="closed"),
                ],
                book=book,
                client=declared_client(),
            )

    def test_an_unbalanced_instantiation_refuses_the_batch(self):
        # Sabotage the legs: a one-sided living_expense would look like
        # a cash outflow with no expense. The check must see it.
        saved = m.PERSONAL_LEGS["living_expense"]
        m.PERSONAL_LEGS["living_expense"] = ((10, 1), (1, 1))
        try:
            with self.assertRaises(m.Refuse) as ctx:
                m.map_batch([row()], book=personal(), client=declared_client())
            self.assertIn("conserve", str(ctx.exception))
        finally:
            m.PERSONAL_LEGS["living_expense"] = saved

    def test_journal_append_is_refused_as_a_scope(self):
        with self.assertRaises(m.Refuse) as ctx:
            m.map_batch(
                [row()],
                book=personal(),
                client=declared_client(scopes=frozenset({"books:read", "statements:read", "journal:append"})),
            )
        self.assertIn("journal:append", str(ctx.exception))
        self.assertIn("journals:post", str(ctx.exception))

    def test_missing_statements_read_is_refused_because_close_cannot_be_honored(self):
        with self.assertRaises(m.Refuse) as ctx:
            m.map_batch(
                [row()],
                book=personal(),
                client=declared_client(scopes=frozenset({"books:read", "journals:post"})),
            )
        self.assertIn("statements:read", str(ctx.exception))

    def test_an_investment_book_is_refused(self):
        with self.assertRaises(m.Refuse) as ctx:
            m.map_batch(
                [row()],
                book=personal(kind="INVESTMENT"),
                client=declared_client(),
            )
        self.assertIn("PERSONAL", str(ctx.exception))

    def test_a_missing_kind_is_refused_rather_than_inferred_from_the_sign(self):
        with self.assertRaises(m.Refuse) as ctx:
            m.map_batch([row(kind="")], book=personal(), client=declared_client())
        self.assertIn("Kind", str(ctx.exception))

    def test_deliver_refuses_because_the_grant_path_is_not_built(self):
        out = m.map_batch([row()], book=personal(), client=declared_client())
        with self.assertRaises(m.Refuse) as ctx:
            m.deliver(out, token="connect-access-token")
        msg = str(ctx.exception)
        self.assertIn("grant path is not built", msg)
        self.assertIn("#151", msg)


class ManifestHonesty(unittest.TestCase):
    def test_the_app_declares_only_canonical_scopes(self):
        scopes = app()["workos_connect"]["scopes"]
        self.assertEqual(set(scopes), set(m.CANONICAL_SCOPES))
        for alias in m.REFUSED_ALIASES:
            self.assertNotIn(alias, scopes)

    def test_the_declared_allowlist_is_personal_templates_not_methods(self):
        templates = set(app()["journals_post_allowlist"]["templates"])
        self.assertTrue(templates)
        self.assertTrue(templates <= set(m.PERSONAL_LEGS))
        for forbidden in ("fifo", "hifo", "min_tax", "specific_id", "average_cost", "wash"):
            self.assertNotIn(forbidden, templates)
            self.assertFalse(any(forbidden in t for t in templates), forbidden)

    def test_grant_path_and_bank_oauth_stay_named_as_leftovers(self):
        doc = json.dumps(app())
        self.assertIn("not built", app()["grant_path"]["status"])
        self.assertIn("not wired", app()["bank_oauth"]["status"])
        self.assertIn("#165", doc)
        self.assertIn("#150", doc)

    def test_every_instantiated_template_is_in_createbook_personal(self):
        if BOOK_RS is None or not BOOK_RS.is_file():
            self.skipTest("book.rs not handed to the test")
        src = BOOK_RS.read_text()
        # The Personal config is the first const; ids in the investment
        # block must not count as household templates.
        start = src.index("const PERSONAL_CONFIG")
        end = src.index("const INVESTMENT_CONFIG") if "const INVESTMENT_CONFIG" in src else len(src)
        personal = src[start:end]
        for rule_id in m.PERSONAL_LEGS:
            self.assertIn(
                f'id = "{rule_id}"',
                personal,
                f"{rule_id} is not a CreateBook(Personal) rule — the mapper invented it",
            )

    def test_the_catalog_still_refuses_the_alias_this_issue_named(self):
        if CATALOG is None or not CATALOG.is_file():
            self.skipTest("connect-scopes.md not handed to the test")
        text = CATALOG.read_text()
        self.assertIn("`journals:post`", text)
        self.assertIn("`journal:append`", text)
        self.assertIn("grant path is not built", text)


if __name__ == "__main__":
    unittest.main()
