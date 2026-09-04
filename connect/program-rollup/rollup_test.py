#!/usr/bin/env python3
"""Properties the Project program roll-up must keep.

Test names are sentences. Break the thing a test protects and this
file goes red — a green suite that only checks the books balance is
how a missing billed cite ships as a program billed of 0.00.

Does not talk to /v1. Does not claim a Connect token is accepted.
Does not invent a mega-book.
"""

from __future__ import annotations

import json
import pathlib
import sys
import unittest

import rollup as r

APP_PATH = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).with_name("app.json")
RULES_RS = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else None
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
        scopes=overrides.get("scopes", c.scopes),
    )


def budget(**overrides) -> r.BudgetCite:
    return r.BudgetCite(
        original=overrides.get("original", 1_000_000),
        approved_change_orders=overrides.get("approved_change_orders", 50_000),
        incurred=overrides.get("incurred", 200_000),
        awarded=overrides.get("awarded", 150_000),
    )


def billing(**overrides) -> r.BillingCite:
    return r.BillingCite(
        billed=overrides.get("billed", 100_000),
        earned=overrides.get("earned", 90_000),
        retainage_receivable=overrides.get("retainage_receivable", 10_000),
        accounts_receivable=overrides.get("accounts_receivable", 40_000),
    )


def visible(**overrides) -> r.VisibleBook:
    return r.VisibleBook(
        book_id=overrides.get("book_id", "job-a"),
        kind=overrides.get("kind", "PROJECT"),
        member=overrides.get("member", True),
        org_id=overrides.get("org_id"),
        budget=overrides.get("budget", budget()),
        billing=overrides.get("billing", billing()),
    )


def rollup_of(**kwargs) -> r.Rollup:
    return r.build_rollup(
        client=kwargs.get("client", declared_client()),
        books=kwargs.get("books", (visible(),)),
    )


def program_amount(out: r.Rollup, name: str) -> str:
    return next(line.amount for line in out.program if line.figure == name)


def program_cited(out: r.Rollup, name: str) -> int:
    return next(line.cited_books for line in out.program if line.figure == name)


def program_note(out: r.Rollup, name: str) -> str:
    return next(line.note for line in out.program if line.figure == name)


class ParseMinor(unittest.TestCase):
    def test_an_amount_is_parsed_by_splitting_not_by_floating(self):
        self.assertEqual(r.parse_minor("1000.00"), 100_000)
        self.assertEqual(r.parse_minor("0.10"), 10)
        self.assertEqual(r.parse_minor("0.1"), 10)
        self.assertEqual(r.parse_minor("1.5"), 150)
        self.assertEqual(r.parse_minor("42"), 4_200)
        self.assertEqual(r.parse_minor(".5"), 50)
        self.assertEqual(r.parse_minor("$1,204,880.11"), 120_488_011)

    def test_three_decimal_places_are_refused_rather_than_dropped(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.parse_minor("1.005")
        self.assertIn("minor units", str(ctx.exception))

    def test_a_float_object_is_refused(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.parse_minor(1.005)  # type: ignore[arg-type]
        self.assertIn("float", str(ctx.exception))

    def test_a_signed_magnitude_is_refused_so_a_hold_cannot_be_inferred(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.parse_minor("-40.00")
        self.assertIn("signed", str(ctx.exception))

    def test_a_signed_change_order_net_is_a_deduct_not_a_refused_amount(self):
        self.assertEqual(r.parse_minor("-250.00", allow_signed=True), -25_000)

    def test_a_zero_amount_is_a_real_zero_not_a_missing_cite(self):
        self.assertEqual(r.parse_minor("0.00"), 0)
        self.assertEqual(r.parse_optional_minor("0.00"), 0)
        self.assertIsNone(r.parse_optional_minor(""))
        self.assertIsNone(r.parse_optional_minor(None))

    def test_an_amount_that_does_not_fit_i64_is_refused(self):
        with self.assertRaises(r.Refuse):
            r.parse_minor("92233720368547758.08")


class PerBookCuts(unittest.TestCase):
    def test_revised_equals_original_plus_approved_when_both_are_set(self):
        self.assertEqual(r.revised_contract(1_000_000, 50_000), 1_050_000)

    def test_revised_equals_original_when_no_change_order_has_posted(self):
        self.assertEqual(r.revised_contract(1_000_000, None), 1_000_000)

    def test_an_unknown_baseline_cannot_price_a_revised_contract(self):
        self.assertIsNone(r.revised_contract(None, 50_000))
        self.assertIsNone(r.revised_contract(None, None))

    def test_remaining_to_bill_stays_unset_when_billed_is_missing(self):
        # Treating billed as 0 would print the whole contract as remaining.
        self.assertIsNone(r.remaining_to_bill(1_000_000, None))
        self.assertEqual(r.remaining_to_bill(1_000_000, 100_000), 900_000)

    def test_remaining_to_spend_stays_unset_when_awarded_cannot_support_the_cut(self):
        # Treating awarded as 0 would print budget − actual as headroom.
        self.assertIsNone(r.remaining_to_spend(1_050_000, 200_000, None))
        self.assertEqual(r.remaining_to_spend(1_050_000, 200_000, 150_000), 700_000)

    def test_collected_stays_unset_when_billed_or_ar_is_missing(self):
        self.assertIsNone(r.collected_against_billed(None, 40_000, 10_000))
        self.assertIsNone(r.collected_against_billed(100_000, None, 10_000))
        self.assertEqual(r.collected_against_billed(100_000, 40_000, 10_000), 50_000)

    def test_unheld_retainage_is_zero_for_the_subtraction_and_not_an_unknown_hold(self):
        self.assertEqual(r.collected_against_billed(100_000, 40_000, None), 60_000)


class MembershipListing(unittest.TestCase):
    def test_list_program_books_keeps_only_project_books_the_subject_administers(self):
        kept = r.list_program_books(
            (
                visible(book_id="job-a", kind="PROJECT", member=True),
                visible(book_id="job-b", kind="PROJECT", member=False, org_id="org_1"),
                visible(book_id="fund-1", kind="INVESTMENT", member=True),
                visible(book_id="house", kind="PERSONAL", member=True),
                visible(book_id="ops", kind="OPERATING", member=True),
                visible(book_id="job-c", kind="PROJECT", member=True),
            )
        )
        self.assertEqual([b.book_id for b in kept], ["job-a", "job-c"])

    def test_an_org_id_claim_does_not_add_a_book_the_subject_does_not_administer(self):
        kept = r.list_program_books(
            (
                visible(book_id="secret-job", kind="PROJECT", member=False, org_id="org_1"),
                visible(book_id="mine", kind="PROJECT", member=True, org_id="org_1"),
            )
        )
        self.assertEqual([b.book_id for b in kept], ["mine"])

    def test_books_from_org_claim_is_refused_because_org_id_is_not_membership(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.books_from_org_claim("org_1")
        msg = str(ctx.exception)
        self.assertIn("org_id claim is not membership", msg)
        self.assertIn("#151", msg)


class RollupShape(unittest.TestCase):
    def test_two_fixture_jobs_map_to_per_book_cites_and_a_program_sum(self):
        # Job A: original 10,000 / CO 500 / billed 1,000 / retainage 100 /
        # AR 400 → revised 10,500, remaining-to-bill 9,500, collected 500.
        # Job B: original 4,000 / no CO / billed 500 / AR 200 / no retainage
        # → revised 4,000, remaining-to-bill 3,500, collected 300.
        out = rollup_of(
            books=(
                visible(
                    book_id="job-a",
                    budget=budget(
                        original=1_000_000,
                        approved_change_orders=50_000,
                        incurred=200_000,
                        awarded=150_000,
                    ),
                    billing=billing(
                        billed=100_000,
                        earned=90_000,
                        retainage_receivable=10_000,
                        accounts_receivable=40_000,
                    ),
                ),
                visible(
                    book_id="job-b",
                    budget=budget(
                        original=400_000,
                        approved_change_orders=None,
                        incurred=80_000,
                        awarded=40_000,
                    ),
                    billing=billing(
                        billed=50_000,
                        earned=45_000,
                        retainage_receivable=None,
                        accounts_receivable=20_000,
                    ),
                ),
            )
        )
        self.assertEqual(out.books_in_program, 2)
        self.assertEqual(out.books[0].book_id, "job-a")
        self.assertEqual(out.books[0].revised, "10500.00")
        self.assertEqual(out.books[0].remaining_to_bill, "9500.00")
        self.assertEqual(out.books[0].collected, "500.00")
        self.assertEqual(out.books[0].remaining_to_spend, "7000.00")
        self.assertEqual(out.books[1].revised, "4000.00")
        self.assertEqual(out.books[1].change_orders, "")
        self.assertEqual(out.books[1].remaining_to_bill, "3500.00")
        self.assertEqual(out.books[1].collected, "300.00")
        self.assertEqual(program_amount(out, "revised"), "14500.00")
        self.assertEqual(program_amount(out, "billed"), "1500.00")
        self.assertEqual(program_amount(out, "remaining_to_bill"), "13000.00")
        self.assertEqual(program_amount(out, "collected"), "800.00")
        self.assertEqual(program_cited(out, "billed"), 2)

    def test_a_book_that_lacks_billed_does_not_invent_a_program_billed_of_zero(self):
        out = rollup_of(
            books=(
                visible(
                    book_id="billed-job",
                    billing=billing(billed=100_000, accounts_receivable=40_000),
                ),
                visible(
                    book_id="unbilled-job",
                    billing=r.BillingCite(),
                    budget=budget(original=400_000, approved_change_orders=None),
                ),
            )
        )
        self.assertEqual(out.books[1].billed, "")
        self.assertEqual(out.books[1].collected, "")
        self.assertEqual(out.books[1].remaining_to_bill, "")
        self.assertEqual(program_amount(out, "billed"), "1000.00")
        self.assertEqual(program_cited(out, "billed"), 1)
        self.assertIn("1 of 2", program_note(out, "billed"))
        self.assertIn("not 0.00", program_note(out, "billed"))
        billed_line = next(
            line for line in r.csv_books(out).splitlines() if line.startswith("unbilled-job,")
        )
        self.assertIn(",,", billed_line)
        self.assertNotIn("unbilled-job,4000.00,,,4000.00,,,,0.00", billed_line)

    def test_program_remaining_is_not_recomputed_from_mixed_program_totals(self):
        # If program remaining were program_revised − program_billed, the
        # unbilled job's revised 4,000 would be treated as billed-zero
        # leftover. Remaining is the sum of per-book remaining cites.
        out = rollup_of(
            books=(
                visible(
                    book_id="billed-job",
                    budget=budget(original=1_000_000, approved_change_orders=None),
                    billing=billing(billed=100_000, accounts_receivable=40_000),
                ),
                visible(
                    book_id="unbilled-job",
                    budget=budget(original=400_000, approved_change_orders=None),
                    billing=r.BillingCite(),
                ),
            )
        )
        self.assertEqual(program_amount(out, "revised"), "14000.00")
        self.assertEqual(program_amount(out, "billed"), "1000.00")
        self.assertEqual(program_amount(out, "remaining_to_bill"), "9000.00")
        self.assertNotEqual(program_amount(out, "remaining_to_bill"), "13000.00")
        self.assertEqual(program_cited(out, "remaining_to_bill"), 1)

    def test_missing_cites_on_every_book_leave_the_program_total_unset(self):
        out = rollup_of(
            books=(
                visible(book_id="a", budget=r.BudgetCite(), billing=r.BillingCite()),
                visible(book_id="b", budget=r.BudgetCite(), billing=r.BillingCite()),
            )
        )
        for name in ("billed", "collected", "remaining_to_bill", "revised", "earned"):
            self.assertEqual(program_amount(out, name), "", name)
            self.assertEqual(program_cited(out, name), 0, name)
            self.assertIn("not a fake roll-up zero", program_note(out, name))
        billed_line = next(
            line for line in r.csv_program(out).splitlines() if line.startswith("billed,")
        )
        self.assertTrue(billed_line.startswith("billed,,"), billed_line)
        collected_line = next(
            line for line in r.csv_program(out).splitlines() if line.startswith("collected,")
        )
        self.assertTrue(collected_line.startswith("collected,,"), collected_line)
        payload = json.loads(r.as_json(out))
        billed = next(p for p in payload["program"] if p["figure"] == "billed")
        self.assertEqual(billed["amount"], "")
        self.assertNotEqual(billed["amount"], "0.00")

    def test_a_posted_zero_billed_is_a_figure_and_enters_the_program_sum(self):
        out = rollup_of(
            books=(
                visible(
                    book_id="zero-billed",
                    billing=billing(billed=0, earned=None, accounts_receivable=0),
                ),
            )
        )
        self.assertEqual(out.books[0].billed, "0.00")
        self.assertEqual(program_amount(out, "billed"), "0.00")
        self.assertEqual(program_cited(out, "billed"), 1)
        self.assertEqual(out.books[0].remaining_to_bill, "10500.00")

    def test_a_non_member_project_book_does_not_enter_the_program(self):
        out = rollup_of(
            books=(
                visible(book_id="mine", billing=billing(billed=100_000)),
                visible(
                    book_id="theirs",
                    member=False,
                    org_id="org_1",
                    billing=billing(billed=9_000_000),
                ),
            )
        )
        self.assertEqual(out.books_in_program, 1)
        self.assertEqual(out.books[0].book_id, "mine")
        self.assertEqual(program_amount(out, "billed"), "1000.00")

    def test_a_personal_book_the_subject_can_see_is_not_a_job_in_the_program(self):
        out = rollup_of(
            books=(
                visible(book_id="job-a"),
                visible(book_id="house", kind="PERSONAL", budget=r.BudgetCite(), billing=r.BillingCite()),
            )
        )
        self.assertEqual([b.book_id for b in out.books], ["job-a"])
        self.assertEqual(out.books_in_program, 1)

    def test_an_empty_membership_is_an_empty_program_not_a_silent_zero(self):
        out = rollup_of(books=())
        self.assertEqual(out.books_in_program, 0)
        self.assertEqual(out.books, ())
        self.assertEqual(program_amount(out, "billed"), "")
        self.assertIn("no PROJECT book", program_note(out, "billed"))

    def test_companion_sheets_are_named_and_do_not_invent_a_percent_complete(self):
        files = r.as_files(rollup_of())
        self.assertEqual(set(files), {"books.csv", "program.csv", "unset.csv", "program.json"})
        self.assertNotIn("%", files["program.csv"].splitlines()[0])
        self.assertNotIn("Percent", files["books.csv"])
        self.assertIn("Remaining to bill", files["books.csv"])
        payload = json.loads(files["program.json"])
        self.assertIn("books_in_program", payload)
        self.assertIn("program", payload)

    def test_cite_from_fixture_reads_the_list_books_plus_cite_shape(self):
        out = r.cite_from_fixture(
            {
                "app": app(),
                "books": [
                    {
                        "book_id": "job-a",
                        "kind": "PROJECT",
                        "member": True,
                        "budget": "10000.00",
                        "approved_change_orders": "500.00",
                        "incurred": "2000.00",
                        "awarded": "1500.00",
                        "progress": {
                            "billed": "1000.00",
                            "earned": "900.00",
                            "retainage_receivable": "100.00",
                            "accounts_receivable": "400.00",
                        },
                    },
                    {
                        "id": "job-b",
                        "kind": "PROJECT",
                        "member": True,
                        "original": "4000.00",
                        "billed": "500.00",
                        "accounts_receivable": "200.00",
                    },
                ],
            }
        )
        self.assertEqual(out.books[0].revised, "10500.00")
        self.assertEqual(out.books[0].collected, "500.00")
        self.assertEqual(out.books[1].billed, "500.00")
        self.assertEqual(program_amount(out, "billed"), "1500.00")


class Refusals(unittest.TestCase):
    def test_journals_post_is_refused_because_this_app_is_read_only(self):
        with self.assertRaises(r.Refuse) as ctx:
            rollup_of(
                client=declared_client(
                    scopes=frozenset(
                        {
                            "books:read",
                            "budget:read",
                            "billing:read",
                            "journals:post",
                        }
                    )
                )
            )
        self.assertIn("journals:post", str(ctx.exception))
        self.assertIn("read-only", str(ctx.exception))

    def test_journal_append_is_refused_as_a_scope(self):
        with self.assertRaises(r.Refuse) as ctx:
            rollup_of(
                client=declared_client(
                    scopes=frozenset({"books:read", "budget:read", "journal:append"})
                )
            )
        self.assertIn("journal:append", str(ctx.exception))

    def test_journal_read_is_refused_as_a_scope(self):
        with self.assertRaises(r.Refuse) as ctx:
            rollup_of(
                client=declared_client(
                    scopes=frozenset({"books:read", "budget:read", "journal:read"})
                )
            )
        self.assertIn("journal:read", str(ctx.exception))

    def test_projects_budget_read_is_refused_as_the_catalog_alias(self):
        with self.assertRaises(r.Refuse) as ctx:
            rollup_of(
                client=declared_client(
                    scopes=frozenset(
                        {"books:read", "projects:budget:read", "billing:read"}
                    )
                )
            )
        self.assertIn("projects:budget:read", str(ctx.exception))

    def test_projects_billing_read_is_refused_as_the_catalog_alias(self):
        with self.assertRaises(r.Refuse) as ctx:
            rollup_of(
                client=declared_client(
                    scopes=frozenset(
                        {"books:read", "budget:read", "projects:billing:read"}
                    )
                )
            )
        self.assertIn("projects:billing:read", str(ctx.exception))

    def test_missing_books_read_is_refused_because_membership_cannot_be_listed(self):
        with self.assertRaises(r.Refuse) as ctx:
            rollup_of(
                client=declared_client(scopes=frozenset({"budget:read", "billing:read"}))
            )
        self.assertIn("books:read", str(ctx.exception))

    def test_missing_budget_read_is_refused_because_the_original_cannot_be_cited(self):
        with self.assertRaises(r.Refuse) as ctx:
            rollup_of(
                client=declared_client(scopes=frozenset({"books:read", "billing:read"}))
            )
        self.assertIn("budget:read", str(ctx.exception))

    def test_missing_billing_read_is_refused_because_billed_cannot_be_cited(self):
        with self.assertRaises(r.Refuse) as ctx:
            rollup_of(
                client=declared_client(scopes=frozenset({"books:read", "budget:read"}))
            )
        self.assertIn("billing:read", str(ctx.exception))

    def test_fetch_cites_refuses_because_the_grant_path_is_not_built(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.fetch_cites(token="connect-access-token")
        msg = str(ctx.exception)
        self.assertIn("grant path is not built", msg)
        self.assertIn("#22", msg)

    def test_deliver_refuses_because_the_grant_path_is_not_built(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.deliver(rollup_of(), token="connect-access-token")
        self.assertIn("grant path is not built", str(ctx.exception))

    def test_mega_book_is_refused_because_books_stay_independent(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.mega_book()
        msg = str(ctx.exception)
        self.assertIn("mega-book", msg)
        self.assertIn("independent", msg)

    def test_merge_journals_is_refused_because_a_program_is_a_cite_of_cites(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.merge_journals()
        self.assertIn("not merged", str(ctx.exception))

    def test_eac_is_refused_because_forecast_stays_on_169(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.eac()
        self.assertIn("#169", str(ctx.exception))

    def test_render_g702_is_refused_because_pay_app_stays_on_184(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.render_g702()
        self.assertIn("#184", str(ctx.exception))

    def test_vendor_directory_is_refused_because_that_door_is_172(self):
        with self.assertRaises(r.Refuse) as ctx:
            r.vendor_directory()
        self.assertIn("#172", str(ctx.exception))


class ManifestHonesty(unittest.TestCase):
    def test_the_app_declares_only_canonical_scopes(self):
        scopes = app()["workos_connect"]["scopes"]
        self.assertEqual(set(scopes), set(r.CANONICAL_SCOPES))
        for alias in r.REFUSED_ALIASES:
            self.assertNotIn(alias, scopes)
        self.assertNotIn("journals:post", scopes)
        self.assertNotIn("journal:append", scopes)
        self.assertNotIn("statements:read", scopes)

    def test_grant_path_and_mega_book_stay_named_as_leftovers(self):
        doc = json.dumps(app())
        self.assertIn("not built", app()["grant_path"]["status"])
        self.assertIn("refused", app()["mega_book"]["status"])
        self.assertIn("#179", doc)
        self.assertIn("#150", doc)
        self.assertIn("#22", doc)
        self.assertIn("does not reopen #151", doc)
        self.assertIn("#169", doc)
        self.assertIn("#172", doc)
        self.assertIn("#184", doc)

    def test_the_catalog_still_names_the_scopes_this_app_requests(self):
        if CATALOG is None or not CATALOG.is_file():
            self.skipTest("connect-scopes.md not handed to the test")
        text = CATALOG.read_text()
        for scope in r.CANONICAL_SCOPES:
            self.assertIn(f"`{scope}`", text)
        self.assertIn("`journal:append`", text)
        self.assertIn("`projects:budget:read`", text)
        self.assertIn("`projects:billing:read`", text)
        self.assertIn("leftover #22", text)
        self.assertIn("An `org_id` claim is not membership", text)

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
            "a project budget is not a lot Method — the roll-up invented an election",
        )
        self.assertNotIn("program", project_src.lower())
        self.assertNotIn("mega", project_src.lower())

    def test_screens_for_project_was_not_forked_with_a_program_url(self):
        if SCREENS is None or not SCREENS.is_file():
            self.skipTest("screens.ts not handed to the test")
        src = SCREENS.read_text()
        start = src.index("export const PROJECT_SCREENS")
        end = src.index("export const INVESTMENT_SCREENS")
        project_src = src[start:end]
        lowered = project_src.lower()
        self.assertNotIn('segment: "program"', lowered)
        self.assertNotIn('segment: "rollup"', lowered)
        self.assertNotIn('segment: "roll-up"', lowered)
        self.assertNotIn('segment: "g702"', lowered)
        self.assertNotIn("multi-contract", lowered)
        self.assertNotIn("program roll", lowered)
        self.assertIn("budget", project_src)
        self.assertIn("billing", project_src)

    def test_the_kernel_did_not_grow_a_program_rpc_or_a_fifth_kind(self):
        if PROTO is None or not PROTO.is_file():
            self.skipTest("console.proto not handed to the test")
        src = PROTO.read_text()
        self.assertIn("KIND_PROJECT = 3", src)
        self.assertIn("KIND_OPERATING = 4", src)
        self.assertNotIn("KIND_PROGRAM", src)
        self.assertNotIn("KIND_MEGA", src)
        for needle in (
            "rpc ProgramRollup",
            "rpc Rollup",
            "rpc MegaBook",
            "message ProgramRollup",
            "message MegaBook",
        ):
            self.assertNotIn(
                needle,
                src,
                f"{needle} is a kernel program RPC — refuse it; this app is the door",
            )

    def test_console_wire_types_did_not_grow_a_fifth_kind(self):
        if TYPES is None or not TYPES.is_file():
            self.skipTest("types.ts not handed to the test")
        src = TYPES.read_text()
        self.assertIn(
            'export type BookKind = "PERSONAL" | "INVESTMENT" | "PROJECT" | "OPERATING" | "UNSPECIFIED"',
            src,
        )
        self.assertNotIn("PROGRAM", src)
        self.assertNotIn("MEGA", src)


if __name__ == "__main__":
    unittest.main()
