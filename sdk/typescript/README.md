# ratio-sdk (TypeScript)

```sh
npm install ratio-sdk                   # REST: no dependencies (Node 18+, or any runtime with fetch)
npm install ratio-sdk @grpc/grpc-js     # + the gRPC transport
```

```ts
import { Ratio, RatioError, posting } from "ratio-sdk";

const ratio = Ratio.rest({ endpoint: "http://127.0.0.1:50051", book: "fund-1" });
// or: await Ratio.connect({ endpoint, book, transport: "grpc" })

const txn = await ratio.transactions.create([posting(2, -125_000), posting(1, 125_000)]);
console.log(txn.name);                          // books/fund-1/transactions/txn-0

try {
  await ratio.transactions.create([posting(2, -125_000), posting(1, 124_999)]);
} catch (e) {
  if (e instanceof RatioError) e.status;        // "FAILED_PRECONDITION" — nothing reached the journal
}

for await (const t of ratio.transactions) { /* the journal, in posting order */ }
const equity = await ratio.accounts.create(20, "Capital contributions", "ACCOUNT_TYPE_EQUITY");
equity.normalSide;                              // "SIDE_CREDIT" — derived by theorem, never sent
(await ratio.trialBalance()).difference;        // "0"
```

Every int64 is a string on the wire (`Int64`), so amounts stay exact past
2^53; `int64()` and `toBigInt()` convert, and `posting()` takes a `number` or a
`bigint`. The gRPC transport ships its own wire codec (`src/wire.ts`), so
`@grpc/grpc-js` is the only dependency and there are no stubs to regenerate.

```sh
pnpm install && pnpm test                       # unit tests, no server needed
RATIO_ENDPOINT=http://127.0.0.1:50051 RATIO_BOOK=fund-1 pnpm e2e    # both transports, live
```

See [`../README.md`](../README.md) for the API, the error model and what is checked.
