"""The JSON view: int64 as strings out, either in; enums as names; absent is absent."""

import unittest

from ratio import Account, AccountType, Posting, RatioError, Side, Transaction, TrialBalance


class JsonTest(unittest.TestCase):
    def test_int64_is_a_string_out_and_either_in(self):
        p = Posting(dim=2, amount=-125_000)
        j = p.to_json()
        self.assertEqual(j, {"dim": "2", "amount": "-125000"})
        self.assertEqual(Posting.from_json(j), p)
        self.assertEqual(Posting.from_json({"dim": 2, "amount": -125000}), p)
        with self.assertRaises(ValueError):
            Posting.from_json({"dim": 2, "amount": 1.5})
        with self.assertRaises(ValueError):
            Posting.from_json({"dim": 2, "amount": True})
        with self.assertRaises(ValueError):
            Posting.from_json({"dim": 2, "amount": str(2**63)})

    def test_every_posting_field_round_trips(self):
        p = Posting(dim=1, amount=7, currency_code="USD", instrument="AAPL", quantity=3)
        self.assertEqual(p.to_json()["quantity"], "3")
        self.assertEqual(Posting.from_json(p.to_json()), p)

    def test_a_transaction_names_its_configuration(self):
        t = Transaction(name="books/b/transactions/t", postings=(Posting(1, 7),), control_plane_hash="abc")
        j = t.to_json()
        self.assertEqual(j["controlPlaneHash"], "abc")
        self.assertEqual(Transaction.from_json(j), t)
        self.assertEqual(t.id, "t")

    def test_enums_are_names_out_and_names_or_numbers_in(self):
        a = Account(name="books/b/accounts/1", dim=1, display_name="Cash", account_type=AccountType.ASSET, normal_side=Side.DEBIT)
        j = a.to_json()
        self.assertEqual(j["accountType"], "ACCOUNT_TYPE_ASSET")
        self.assertEqual(j["normalSide"], "SIDE_DEBIT")
        self.assertEqual(Account.from_json(j), a)
        self.assertEqual(Account.from_json({"dim": "1", "accountType": 1}).account_type, AccountType.ASSET)
        with self.assertRaises(ValueError):
            Account.from_json({"accountType": "ASSET"})

    def test_a_trial_balance_reads_its_rows(self):
        tb = TrialBalance.from_json({
            "name": "books/b/trialBalance", "debits": "5", "credits": "5", "difference": "0",
            "accountBalances": [{"account": "books/b/accounts/1", "debits": "5", "credits": "0", "currencyCode": "USD"}],
        })
        self.assertEqual(tb.difference, 0)
        self.assertEqual(tb.account_balances[0].currency_code, "USD")

    def test_errors_are_google_rpc_status(self):
        e = RatioError.from_json({"error": {"code": 9, "message": "nope", "status": "FAILED_PRECONDITION"}}, 400)
        self.assertEqual((e.code, e.status, e.message), (9, "FAILED_PRECONDITION", "nope"))
        self.assertEqual(RatioError(5, "gone").status, "NOT_FOUND")
        self.assertEqual(RatioError.from_json("<html>", 502).code, 2)


if __name__ == "__main__":
    unittest.main()
