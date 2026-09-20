import assert from "node:assert/strict";
import { test } from "node:test";
import * as wire from "./wire.js";
import type { Account, Transaction } from "./types.js";

test("a negative int64 is ten bytes", () => {
  assert.deepEqual(wire.encodePosting({ dim: "1", amount: "-1" }), [0x08, 0x01, 0x10, ...Array(9).fill(0xff), 0x01]);
});

test("absent optionals are not on the wire and a present zero is", () => {
  assert.deepEqual(wire.encodePosting({ dim: "0", amount: "0" }), []);
  assert.deepEqual(wire.encodePosting({ dim: "0", amount: "0", quantity: "0" }), [0x28, 0x00]);
});

test("a transaction round trips", () => {
  const t: Transaction = {
    name: "books/b/transactions/t",
    postings: [
      { dim: "2", amount: "-125000", currencyCode: "USD" },
      { dim: "1", amount: "125000", currencyCode: "USD", instrument: "AAPL", quantity: "10" },
    ],
    controlPlaneHash: "abc",
  };
  assert.deepEqual(wire.decodeTransaction(Uint8Array.from(wire.encodeTransaction(t))), t);
});

test("a create request nests the transaction", () => {
  const t: Transaction = { postings: [{ dim: "1", amount: "5" }] };
  const seen = new Map<number, Uint8Array>();
  for (const f of wire.fields(wire.encodeCreateTransactionRequest("books/b", t, "trade-1"))) seen.set(f.num, f.bytes);
  assert.equal(new TextDecoder().decode(seen.get(1)), "books/b");
  assert.deepEqual(wire.decodeTransaction(seen.get(2)!), t);
  assert.equal(new TextDecoder().decode(seen.get(3)), "trade-1");
});

test("accounts carry enums as numbers", () => {
  const a: Account = { name: "books/b/accounts/20", dim: "20", displayName: "Capital", accountType: "ACCOUNT_TYPE_EQUITY", normalSide: "SIDE_CREDIT" };
  const b = wire.encodeAccount(a);
  assert.ok(b.join(",").includes("32,3"), "field 4 = 3 (EQUITY)");
  assert.ok(b.join(",").includes("40,2"), "field 5 = 2 (CREDIT)");
  assert.deepEqual(wire.decodeAccount(Uint8Array.from(b)), a);
});

test("unknown fields are skipped and truncation is an error", () => {
  const extra = [0x48, 0x07, 0x51, ...Array(8).fill(0), 0x5a, 0x02, 0x68, 0x69];
  assert.deepEqual(wire.decodePosting(Uint8Array.from([...wire.encodePosting({ dim: "3", amount: "4" }), ...extra])), { dim: "3", amount: "4" });
  assert.throws(() => [...wire.fields(Uint8Array.from([0x12, 0x05, 0x61, 0x62]))]);
});
