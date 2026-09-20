// End to end against a live `ratio server`, over both transports:
//
//   ratio init --kind investment --book /tmp/fund-1 && ratio server --book /tmp/fund-1
//   RATIO_ENDPOINT=http://127.0.0.1:50051 RATIO_BOOK=fund-1 npm run e2e
//
// Not part of `npm test`: it needs the binary, which Bazel builds and npm does
// not. The Python SDK's `rest_e2e_test` runs this same script's steps under
// Bazel against the built binary.
import assert from "node:assert/strict";
import { Ratio } from "./client.js";
import { RatioError } from "./errors.js";
import { posting } from "./types.js";

const endpoint = process.env.RATIO_ENDPOINT;
const book = process.env.RATIO_BOOK;
if (!endpoint || !book) {
  console.log("set RATIO_ENDPOINT and RATIO_BOOK to run against a live server; skipping");
  process.exit(0);
}

for (const transport of ["rest", "grpc"] as const) {
  const ratio = await Ratio.connect({ endpoint, book, transport });
  const txn = await ratio.transactions.create([posting(2, -125_000), posting(1, 125_000)]);
  assert.ok(txn.name!.startsWith(`books/${book}/transactions/`));
  assert.deepEqual(await ratio.transactions.get(txn.name!.split("/").pop()!), txn);
  await assert.rejects(ratio.transactions.create([posting(2, -125_000), posting(1, 124_999)]), (e: unknown) => e instanceof RatioError && e.status === "FAILED_PRECONDITION");
  const tb = await ratio.trialBalance();
  assert.equal(tb.difference, "0");
  console.log(`${transport}: posted ${txn.name}; trial balance ties at ${tb.debits}`);
  ratio.close();
}
