/**
 * ratio-sdk — the TypeScript SDK for the Ratio accounting kernel.
 *
 * ```ts
 * import { Ratio, posting } from "ratio-sdk";
 *
 * const ratio = Ratio.rest({ endpoint: "http://127.0.0.1:50051", book: "fund-1" });
 * const txn = await ratio.transactions.create([posting(2, -125_000), posting(1, 125_000)]);
 * console.log(txn.name);                       // books/fund-1/transactions/txn-0
 * const tb = await ratio.trialBalance();
 * tb.difference;                               // "0" — by theorem, not by check
 * ```
 *
 * An entry that does not conserve value rejects with a {@link RatioError} whose
 * `status` is `"FAILED_PRECONDITION"`, carrying the store's own message; nothing
 * reaches the journal. That is the whole API.
 */
export { Ratio, Accounts, Transactions, type RatioOptions } from "./client.js";
export { RatioError, CODE_NAMES } from "./errors.js";
export { RestTransport, type FetchLike } from "./rest.js";
export type { Transport } from "./transport.js";
export * from "./types.js";
