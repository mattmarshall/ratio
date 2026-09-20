import assert from "node:assert/strict";
import { test } from "node:test";
import { Ratio } from "./client.js";
import { RatioError } from "./errors.js";
import { posting } from "./types.js";

/** A fetch that records the request and answers from a script. */
function fake(answers: Array<{ status: number; body: unknown }>) {
  const calls: Array<{ url: string; method: string; body?: string; contentType?: string }> = [];
  const f = async (url: string, init: { method: string; headers: Record<string, string>; body?: string }) => {
    calls.push({ url, method: init.method, body: init.body, contentType: init.headers["content-type"] });
    const a = answers.shift();
    if (!a) throw new Error("no scripted answer");
    return { status: a.status, text: async () => (typeof a.body === "string" ? a.body : JSON.stringify(a.body)) };
  };
  return { fetch: f, calls };
}

test("the one call, on the contract's path, as canonical JSON", async () => {
  const posted = { name: "books/fund-1/transactions/txn-0", postings: [{ dim: "2", amount: "-125000" }, { dim: "1", amount: "125000" }], controlPlaneHash: "abc" };
  const { fetch, calls } = fake([{ status: 200, body: posted }]);
  const ratio = Ratio.rest({ endpoint: "http://h:1/", book: "fund-1", fetch });
  const txn = await ratio.transactions.create([posting(2, -125_000), posting(1, 125_000)], { id: "trade-1", controlPlaneHash: "abc" });
  assert.deepEqual(txn, posted);
  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].url, "http://h:1/v1/books/fund-1/transactions?transactionId=trade-1");
  assert.equal(calls[0].contentType, "application/json");
  assert.deepEqual(JSON.parse(calls[0].body!), { postings: [{ dim: "2", amount: "-125000" }, { dim: "1", amount: "125000" }], controlPlaneHash: "abc" });
});

test("a refusal is a RatioError with the server's status and message", async () => {
  const { fetch } = fake([{ status: 400, body: { error: { code: 9, message: "entry does not conserve value: postings net to 1, not 0", status: "FAILED_PRECONDITION" } } }]);
  const ratio = Ratio.rest({ endpoint: "http://h:1", book: "b", fetch });
  await assert.rejects(ratio.transactions.create([posting(1, 1)]), (e: unknown) => {
    assert.ok(e instanceof RatioError);
    assert.equal(e.status, "FAILED_PRECONDITION");
    assert.match(e.message, /does not conserve value/);
    return true;
  });
});

test("reads, lists and pages walk the contract's routes", async () => {
  const { fetch, calls } = fake([
    { status: 200, body: { name: "books/b/transactions/t", postings: [] } },
    { status: 200, body: { transactions: [{ name: "books/b/transactions/a", postings: [] }], nextPageToken: "1" } },
    { status: 200, body: { transactions: [{ name: "books/b/transactions/b", postings: [] }] } },
    { status: 200, body: { name: "books/b/accounts/1", dim: "1", displayName: "Cash", accountType: "ACCOUNT_TYPE_ASSET", normalSide: "SIDE_DEBIT" } },
    { status: 200, body: { name: "books/b/trialBalance", debits: "5", credits: "5", difference: "0", accountBalances: [] } },
  ]);
  const ratio = Ratio.rest({ endpoint: "http://h:1", book: "b", fetch });
  assert.equal((await ratio.transactions.get("t")).name, "books/b/transactions/t");
  const seen: string[] = [];
  for await (const t of ratio.transactions) seen.push(t.name!);
  assert.deepEqual(seen, ["books/b/transactions/a", "books/b/transactions/b"]);
  assert.equal((await ratio.accounts.get(1)).normalSide, "SIDE_DEBIT");
  assert.equal((await ratio.trialBalance()).difference, "0");
  assert.deepEqual(calls.map((c) => c.url.replace("http://h:1", "")), [
    "/v1/books/b/transactions/t",
    "/v1/books/b/transactions",
    "/v1/books/b/transactions?pageToken=1",
    "/v1/books/b/accounts/1",
    "/v1/books/b/trialBalance",
  ]);
});

test("a dead endpoint is UNAVAILABLE, not a stack trace", async () => {
  const ratio = Ratio.rest({ endpoint: "http://h:1", book: "b", fetch: async () => { throw new Error("ECONNREFUSED"); } });
  await assert.rejects(ratio.trialBalance(), (e: unknown) => e instanceof RatioError && e.status === "UNAVAILABLE");
});
