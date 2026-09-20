import assert from "node:assert/strict";
import { test } from "node:test";
import { RatioError } from "./errors.js";
import { int64, posting, toBigInt } from "./types.js";

test("int64 takes a bigint, a safe number or a digit string, and nothing else", () => {
  assert.equal(int64(-125_000), "-125000");
  assert.equal(int64(9_007_199_254_740_993n), "9007199254740993");
  assert.equal(int64(" 42 "), "42");
  assert.throws(() => int64(2 ** 53), RangeError);
  assert.throws(() => int64("1.5"), RangeError);
  assert.throws(() => int64("1e3"), RangeError);
  assert.equal(toBigInt("-1"), -1n);
  assert.equal(toBigInt(undefined), 0n);
});

test("a posting carries only what was given", () => {
  assert.deepEqual(posting(2, -125_000), { dim: "2", amount: "-125000" });
  assert.deepEqual(posting(1n, 7n, "USD"), { dim: "1", amount: "7", currencyCode: "USD" });
});

test("errors are google.rpc.Status", () => {
  const e = RatioError.fromBody({ error: { code: 9, message: "nope", status: "FAILED_PRECONDITION" } }, 400);
  assert.equal(e.code, 9);
  assert.equal(e.status, "FAILED_PRECONDITION");
  assert.equal(e.message, "FAILED_PRECONDITION: nope");
  assert.equal(new RatioError(5, "gone").status, "NOT_FOUND");
  assert.equal(RatioError.fromBody("<html>", 502).code, 2);
});
