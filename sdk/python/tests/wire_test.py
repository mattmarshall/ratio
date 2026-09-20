"""The wire codec: known bytes for known values, and round trips."""

import unittest

from ratio import _wire
from ratio.types import Account, AccountType, Posting, Side, Transaction


class WireTest(unittest.TestCase):
    def test_negative_int64_is_ten_bytes(self):
        # tag 0x08 (field 1, varint), 1; tag 0x10 (field 2, varint), -1 as 2^64-1.
        self.assertEqual(_wire.posting(Posting(1, -1)), b"\x08\x01\x10" + b"\xff" * 9 + b"\x01")

    def test_absent_optionals_are_not_on_the_wire_and_present_zero_is(self):
        self.assertEqual(_wire.posting(Posting(0, 0)), b"")
        self.assertEqual(_wire.posting(Posting(0, 0, quantity=0)), b"\x28\x00")

    def test_a_transaction_round_trips(self):
        t = Transaction(
            name="books/b/transactions/t",
            postings=(Posting(2, -125_000, "USD"), Posting(1, 125_000, "USD", "AAPL", 10)),
            control_plane_hash="abc",
        )
        self.assertEqual(_wire.decode_transaction(_wire.transaction(t)), t)

    def test_create_request_nests_the_transaction(self):
        t = Transaction(postings=(Posting(1, 5),))
        b = _wire.create_transaction_request("books/b", t, "trade-1")
        seen = {num: v for num, _, v in _wire.fields(b)}
        self.assertEqual(seen[1], b"books/b")
        self.assertEqual(_wire.decode_transaction(seen[2]), t)
        self.assertEqual(seen[3], b"trade-1")

    def test_accounts_carry_enums_as_numbers(self):
        a = Account("books/b/accounts/20", 20, "Capital", AccountType.EQUITY, Side.CREDIT)
        b = _wire.account(a)
        self.assertIn(b"\x20\x03", b)  # field 4 = 3 (EQUITY)
        self.assertIn(b"\x28\x02", b)  # field 5 = 2 (CREDIT)
        self.assertEqual(_wire.decode_account(b), a)

    def test_unknown_fields_are_skipped(self):
        # field 9 varint, field 10 fixed64, field 11 bytes — none in Posting.
        extra = b"\x48\x07" + b"\x51" + b"\x00" * 8 + b"\x5a\x02hi"
        self.assertEqual(_wire.decode_posting(_wire.posting(Posting(3, 4)) + extra), Posting(3, 4))

    def test_truncation_is_an_error(self):
        with self.assertRaises(ValueError):
            list(_wire.fields(b"\x12\x05ab"))


if __name__ == "__main__":
    unittest.main()
