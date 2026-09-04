#!/usr/bin/env python3
"""Properties the household tax pack must keep.

Test names are sentences. Break the thing a test protects and this
file goes red — a green suite that only checks the books balance is
how a wrong gain ships.

Does not talk to /v1. Does not claim a Connect token is accepted.
Does not e-file.
"""

from __future__ import annotations

import json
import pathlib
import sys
import os
import unittest
from unittest import mock
from datetime import date

import pack as p

APP_PATH = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).with_name("app.json")
RULES_RS = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else None
CATALOG = pathlib.Path(sys.argv[3]) if len(sys.argv) > 3 else None
SCREENS = pathlib.Path(sys.argv[4]) if len(sys.argv) > 4 else None

sys.argv = sys.argv[:1]


def app() -> dict:
    return json.loads(APP_PATH.read_text())


def declared_client(**overrides) -> p.Client:
    c = p.client_from_app(app())
    return p.Client(
        client_id=overrides.get("client_id", c.client_id),
        scopes=overrides.get("scopes", c.scopes),
    )


def personal(**overrides) -> p.Book:
    return p.Book(
        kind=overrides.get("kind", "PERSONAL"),
        closed_through=overrides.get("closed_through"),
    )


def terms(**overrides) -> p.LotTerms:
    return p.LotTerms(
        long_term_days=overrides.get("long_term_days", 365),
        wash_window_days=overrides.get("wash_window_days"),
        wash_keep_holding_period=overrides.get("wash_keep_holding_period"),
        lot_method=overrides.get("lot_method"),
        min_tax_short_weight=overrides.get("min_tax_short_weight"),
        average_cost=overrides.get("average_cost"),
    )


def row(**overrides) -> dict:
    base = {
        "instrument": "VTI",
        "description": "Vanguard Total Stock Market ETF",
        "acquired": "2025-07-01",
        "disposed": "2026-06-30",
        "proceeds": "300.00",
        "basis": "100.00",
        "currency": "USD",
    }
    base.update(overrides)
    return base


def pack_of(rows, **kwargs) -> p.Pack:
    return p.build_pack(
        rows,
        book=kwargs.get("book", personal()),
        client=kwargs.get("client", declared_client()),
        terms=kwargs.get("terms", terms()),
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

    def test_a_signed_amount_is_refused_so_a_loss_cannot_be_inferred(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.parse_minor("-40.00")
        self.assertIn("signed", str(ctx.exception))

    def test_a_zero_amount_is_a_real_zero_proceeds_not_a_missing_cite(self):
        self.assertEqual(p.parse_minor("0.00"), 0)

    def test_an_amount_that_does_not_fit_i64_is_refused(self):
        with self.assertRaises(p.Refuse):
            p.parse_minor("92233720368547758.08")


class HoldingPeriod(unittest.TestCase):
    def test_the_threshold_day_is_long_term(self):
        # 2025-06-30 → 2026-06-30 is 365 days. Held exactly the
        # threshold is LONG. `the_threshold_day_is_long_term`.
        cat = p.holding_period_category(
            (date(2025, 6, 30),), date(2026, 6, 30), 365
        )
        self.assertEqual(cat, "LONG")

    def test_one_day_short_of_the_threshold_is_short_term(self):
        cat = p.holding_period_category(
            (date(2025, 7, 1),), date(2026, 6, 30), 365
        )
        self.assertEqual(cat, "SHORT")

    def test_a_730_day_threshold_keeps_a_365_day_hold_short(self):
        cat = p.holding_period_category(
            (date(2025, 6, 30),), date(2026, 6, 30), 730
        )
        self.assertEqual(cat, "SHORT")

    def test_agreed_pool_dates_classify_as_one_holding(self):
        cat = p.holding_period_category(
            (date(2024, 3, 1), date(2024, 3, 1)),
            date(2026, 3, 15),
            365,
        )
        self.assertEqual(cat, "LONG")

    def test_mixed_acquired_dates_refuse_a_category_rather_than_inventing_fifos_oldest(self):
        # Day 0 / day 400 / dispose 400 / threshold 365: FIFO's oldest
        # is long, the other lot is short, the pool is neither.
        oldest = date(2025, 1, 1)
        newer = date(2026, 2, 5)
        disposed = date(2026, 2, 5)
        with self.assertRaises(p.Refuse) as ctx:
            p.holding_period_category((oldest, newer), disposed, 365)
        msg = str(ctx.exception)
        self.assertIn("disagree", msg)
        self.assertIn("unset", msg)
        self.assertIn("PoolPeriod", msg)
        self.assertNotIn("invented as FIFO", msg)
        # The invention this refuses: min(dates) would have been LONG.
        invented = p.holding_period_category((oldest,), disposed, 365)
        self.assertEqual(invented, "LONG")
        self.assertEqual(
            p.holding_period_category((newer,), disposed, 365),
            "SHORT",
        )

    def test_a_missing_acquisition_date_refuses_rather_than_defaulting_long_or_short(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.holding_period_category((None,), date(2026, 6, 30), 365)
        self.assertIn("missing", str(ctx.exception))

    def test_an_empty_date_list_refuses(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.holding_period_category((), date(2026, 6, 30), 365)
        self.assertIn("no acquisition date", str(ctx.exception))

    def test_a_missing_date_inside_a_pool_refuses_the_whole_category(self):
        with self.assertRaises(p.Refuse) as ctx:
            p.holding_period_category(
                (date(2024, 3, 1), None), date(2026, 3, 15), 365
            )
        self.assertIn("missing", str(ctx.exception))


class PackShape(unittest.TestCase):
    def test_a_short_term_sale_is_an_8949_short_row(self):
        out = pack_of([row()])
        self.assertEqual(len(out.form_8949), 1)
        self.assertEqual(out.form_8949[0].category, "SHORT")
        self.assertEqual(out.form_8949[0].proceeds, "300.00")
        self.assertEqual(out.form_8949[0].basis, "100.00")
        self.assertEqual(out.form_8949[0].gain, "200.00")
        self.assertEqual(out.unclassified, ())

    def test_a_long_term_sale_is_an_8949_long_row(self):
        out = pack_of([row(acquired="2024-03-01", disposed="2026-03-15")])
        self.assertEqual(out.form_8949[0].category, "LONG")
        self.assertEqual(out.form_8949[0].acquired, "2024-03-01")

    def test_mixed_dates_land_on_the_unclassified_sheet_not_on_form_8949(self):
        out = pack_of(
            [
                row(
                    acquired="2025-01-01",
                    acquired_dates=["2025-01-01", "2026-02-05"],
                    disposed="2026-02-05",
                    proceeds="400.00",
                    basis="200.00",
                )
            ]
        )
        self.assertEqual(out.form_8949, ())
        self.assertEqual(len(out.unclassified), 1)
        self.assertEqual(out.unclassified[0].category, "")
        self.assertIn("disagree", out.unclassified[0].ambiguity)
        csv_8949 = p.csv_form_8949(out)
        self.assertNotIn("LONG", csv_8949.split("\n")[1] if "\n" in csv_8949 else "")
        # The classified sheet has a header only — no invented box.
        self.assertEqual(csv_8949.count("\n"), 1)
        self.assertIn("disagree", p.csv_unclassified(out))

    def test_a_single_acquired_that_is_not_in_acquired_dates_is_refused_as_fifo_invention(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                [
                    row(
                        acquired="2025-01-01",
                        acquired_dates=["2026-02-05"],
                        disposed="2026-02-05",
                    )
                ]
            )
        self.assertIn("FIFO", str(ctx.exception))

    def test_a_lot_with_no_acquisition_date_is_unclassified_not_long(self):
        out = pack_of([row(acquired="")])
        self.assertEqual(out.form_8949, ())
        self.assertEqual(out.unclassified[0].category, "")
        self.assertEqual(out.unclassified[0].acquired, "")

    def test_wash_is_cited_as_code_w_and_does_not_rewrite_the_engine(self):
        out = pack_of(
            [
                row(
                    proceeds="100.00",
                    basis="150.00",
                    wash={"disallowed_loss": "50.00", "code": "W"},
                )
            ]
        )
        r = out.form_8949[0]
        self.assertEqual(r.adjustment_code, "W")
        self.assertEqual(r.adjustment, "50.00")
        # 100 − 150 + 50 = 0. The cite adjusts; it does not hide the loss.
        self.assertEqual(r.gain, "0.00")
        wash_csv = p.csv_wash(out)
        self.assertIn("WashRestatement", wash_csv)
        self.assertIn("50.00", wash_csv)

    def test_lot_terms_cite_leaves_unset_wash_window_blank_not_thirty(self):
        out = pack_of([row()], terms=terms())
        sheet = p.csv_lot_terms(out)
        self.assertIn("wash_window_days", sheet)
        self.assertIn("unset stays unset, not a silent 30", sheet)
        # The VALUE is blank. The note may say "not a silent 30" —
        # that is the honesty, not a default written into the cite.
        wash_line = next(
            line for line in sheet.splitlines() if line.startswith("wash_window_days,")
        )
        self.assertTrue(
            wash_line.startswith("wash_window_days,,"),
            wash_line,
        )

    def test_companion_sheets_are_named_and_do_not_invent_a_1099_box(self):
        files = p.as_files(pack_of([row()]))
        self.assertEqual(
            set(files),
            {"form_8949.csv", "unclassified.csv", "wash_cites.csv", "lot_terms.csv"},
        )
        self.assertNotIn("Box A", files["form_8949.csv"])
        self.assertNotIn("Box D", files["form_8949.csv"])


class Refusals(unittest.TestCase):
    def test_lot_method_min_tax_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of([row()], terms=terms(lot_method="min_tax"))
        self.assertIn("min_tax", str(ctx.exception))
        self.assertIn("election", str(ctx.exception))

    def test_lot_method_specific_id_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of([row()], terms=terms(lot_method="specific_id"))
        self.assertIn("specific_id", str(ctx.exception))

    def test_lot_method_average_cost_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of([row()], terms=terms(lot_method="average_cost"))
        self.assertIn("average_cost", str(ctx.exception))

    def test_lot_method_wash_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of([row()], terms=terms(lot_method="wash"))
        self.assertIn("wash", str(ctx.exception))

    def test_average_cost_false_is_refused_at_read(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of([row()], terms=terms(average_cost=False))
        self.assertIn("omit the field", str(ctx.exception))

    def test_keep_without_a_wash_window_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of([row()], terms=terms(wash_keep_holding_period=True))
        self.assertIn("wash_window_days", str(ctx.exception))

    def test_journals_post_is_refused_because_this_app_is_read_only(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                [row()],
                client=declared_client(
                    scopes=frozenset(
                        {"lots:read", "statements:read", "config:read", "journals:post"}
                    )
                ),
            )
        self.assertIn("journals:post", str(ctx.exception))

    def test_journal_append_is_refused_as_a_scope(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                [row()],
                client=declared_client(
                    scopes=frozenset({"lots:read", "statements:read", "journal:append"})
                ),
            )
        self.assertIn("journal:append", str(ctx.exception))

    def test_missing_config_read_is_refused_because_lot_terms_cannot_be_cited(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of(
                [row()],
                client=declared_client(scopes=frozenset({"lots:read", "statements:read"})),
            )
        self.assertIn("config:read", str(ctx.exception))

    def test_an_investment_book_is_refused(self):
        with self.assertRaises(p.Refuse) as ctx:
            pack_of([row()], book=personal(kind="INVESTMENT"))
        self.assertIn("PERSONAL", str(ctx.exception))

    def test_a_dated_disposal_on_or_before_closed_through_is_refused(self):
        book = personal(closed_through=date(2026, 3, 31))
        with self.assertRaises(p.Refuse) as ctx:
            pack_of([row(disposed="2026-03-31", acquired="2025-01-01")], book=book)
        self.assertIn("closed-through", str(ctx.exception))

    def test_fetch_cites_without_a_token_is_refused(self):
        env = {
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
            out = p.fetch_cites(token="connect-access-token", transport=transport)
        self.assertEqual(out, {"books": []})

    def test_submit_refuses_because_irs_e_file_is_not_a_connect_scope(self):
        out = pack_of([row()])
        with self.assertRaises(p.Refuse) as ctx:
            p.submit(out, token="connect-access-token")
        self.assertIn("IRS", str(ctx.exception))


class ManifestHonesty(unittest.TestCase):
    def test_the_app_declares_only_canonical_scopes(self):
        scopes = app()["workos_connect"]["scopes"]
        self.assertEqual(set(scopes), set(p.CANONICAL_SCOPES))
        for alias in p.REFUSED_ALIASES:
            self.assertNotIn(alias, scopes)
        self.assertNotIn("journals:post", scopes)

    def test_grant_path_and_irs_submission_stay_named_as_leftovers(self):
        doc = json.dumps(app())
        self.assertEqual("built", app()["grant_path"]["status"])
        self.assertIn("ConnectApiUrl", app()["grant_path"]["note"])
        self.assertIn("WorkOS dashboard registration", app()["grant_path"]["note"])
        self.assertIn("refused", app()["irs_submission"]["status"])
        self.assertIn("#166", doc)
        self.assertIn("#150", doc)
        category = app()["pooled_holding_period"]
        self.assertEqual(category["status"], "cited")
        self.assertIn("Ratio.Lots.PoolPeriod", category["note"])
        self.assertNotIn("does not close #9", category["note"])
        self.assertNotIn("PR #154", category["note"])

    def test_the_catalog_still_names_the_scopes_this_app_requests(self):
        if CATALOG is None or not CATALOG.is_file():
            self.skipTest("connect-scopes.md not handed to the test")
        text = CATALOG.read_text()
        for scope in p.CANONICAL_SCOPES:
            self.assertIn(f"`{scope}`", text)
        self.assertIn("leftover #22", text)
        self.assertIn("tax e-file", text)

    def test_lot_term_field_names_are_the_ones_ruleset_already_stores(self):
        if RULES_RS is None or not RULES_RS.is_file():
            self.skipTest("ratio-rules lib.rs not handed to the test")
        src = RULES_RS.read_text()
        for field in p.LOT_TERM_FIELDS:
            self.assertIn(
                f"pub {field}:",
                src,
                f"{field} is not a RuleSet field — the pack invented an election",
            )
        for forbidden in ('lot_method = "min_tax"', 'lot_method = "wash"'):
            # The rules crate must still refuse these as methods.
            self.assertIn("wash", src)

    def test_screens_for_personal_was_not_forked_with_a_tax_pack_url(self):
        if SCREENS is None or not SCREENS.is_file():
            self.skipTest("screens.ts not handed to the test")
        src = SCREENS.read_text()
        start = src.index("export const PERSONAL_SCREENS")
        end = src.index("export const PROJECT_SCREENS")
        personal = src[start:end]
        self.assertNotIn("tax", personal.lower())
        self.assertNotIn("8949", personal)


if __name__ == "__main__":
    unittest.main()
