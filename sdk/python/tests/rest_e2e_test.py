"""End to end over the real binary: `ratio init`, `ratio server --addr :0`, then
the REST client posts, is refused, reads back and ties. The same seven calls
`demo/rehearse.sh` drives, from the SDK a caller would install.

Run: rest_e2e_test.py <path/to/ratio>
"""

import os
import subprocess
import sys
import tempfile
import unittest

import ratio

RATIO = sys.argv.pop(1) if len(sys.argv) > 1 else os.environ.get("RATIO_BIN", "ratio")


class RestEndToEndTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.book = tempfile.mkdtemp(prefix="ratio-sdk-")
        subprocess.run([RATIO, "init", "--kind", "investment", "--book", cls.book], check=True, capture_output=True)
        cls.server = subprocess.Popen(
            [RATIO, "server", "--addr", "127.0.0.1:0", "--book", cls.book],
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        )
        for line in cls.server.stdout:
            if "http://" in line:
                cls.endpoint = "http://" + line.split("http://", 1)[1].strip()
                break
        else:
            raise RuntimeError("the server never announced an address")
        cls.client = ratio.Client(cls.endpoint, book=os.path.basename(cls.book))

    @classmethod
    def tearDownClass(cls):
        cls.server.terminate()
        cls.server.wait(timeout=10)

    def test_post_refuse_read_and_tie(self):
        c = self.client
        chart = {a.dim: a for a in c.accounts}
        self.assertIn(1, chart)
        self.assertEqual(chart[1].normal_side, ratio.Side.DEBIT, "an asset is debit-normal, by theorem")

        txn = c.transactions.create([ratio.Posting(dim=2, amount=-125_000), ratio.Posting(dim=1, amount=125_000)])
        self.assertEqual(txn.name, f"{c.parent}/transactions/txn-0")
        self.assertTrue(txn.control_plane_hash, "the entry names the configuration it was posted under")
        self.assertEqual(c.transactions.get("txn-0"), txn)

        with self.assertRaises(ratio.RatioError) as refused:
            c.transactions.create([ratio.Posting(dim=2, amount=-125_000), ratio.Posting(dim=1, amount=124_999)])
        self.assertEqual(refused.exception.status, "FAILED_PRECONDITION")
        self.assertIn("does not conserve value", refused.exception.message)
        self.assertEqual([t.id for t in c.transactions], ["txn-0"], "the refused entry never reached the journal")

        named = c.transactions.create([ratio.Posting(2, -1, "USD"), ratio.Posting(1, 1, "USD")], id="trade-7")
        self.assertEqual(named.postings[0].currency_code, "USD")
        with self.assertRaises(ratio.RatioError) as again:
            c.transactions.create([ratio.Posting(2, -1), ratio.Posting(1, 1)], id="trade-7")
        self.assertEqual(again.exception.status, "ALREADY_EXISTS")

        with self.assertRaises(ratio.RatioError) as moved:
            c.transactions.create([ratio.Posting(2, -1), ratio.Posting(1, 1)], control_plane_hash="0" * 64)
        self.assertEqual(moved.exception.status, "FAILED_PRECONDITION")

        equity = c.accounts.create(dim=900, display_name="SDK test equity", account_type=ratio.AccountType.EQUITY)
        self.assertEqual(equity.normal_side, ratio.Side.CREDIT)
        self.assertEqual(c.accounts.get(900), equity)

        tb = c.trial_balance()
        self.assertEqual(tb.difference, 0)
        self.assertEqual(tb.debits, tb.credits)
        self.assertTrue(any(r.currency_code == "USD" for r in tb.account_balances))

        with self.assertRaises(ratio.RatioError) as other:
            ratio.Client(self.endpoint, book="somebody-else").trial_balance()
        self.assertEqual(other.exception.status, "NOT_FOUND")


if __name__ == "__main__":
    unittest.main()
