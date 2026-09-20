# Using Ratio from your code

Ratio is a ledger kernel with an API: post transactions to a book, read them
back, keep a chart of accounts, and ask for the trial balance. One process
serves it over **gRPC and REST on one port**, from one implementation, over
one book of record — the same journal `ratio post`, the MCP server and the
console read.

```sh
ratio init --kind investment --book fund-1      # a book with a chart and a promoted config
ratio server --book fund-1                       # gRPC + REST on http://127.0.0.1:50051
```

| | Rust | Python | TypeScript |
|---|---|---|---|
| Package | [`crates/ratio-client`](../crates/ratio-client) | [`sdk/python`](python) — `ratio-sdk` | [`sdk/typescript`](typescript) — `ratio-sdk` |
| Transport | gRPC | REST (no dependency) or gRPC (`grpcio`) | REST (no dependency) or gRPC (`@grpc/grpc-js`) |
| Post | `ratio.post(vec![posting(2, -125_000), posting(1, 125_000)]).await?` | `client.transactions.create([Posting(2, -125_000), Posting(1, 125_000)])` | `await ratio.transactions.create([posting(2, -125_000), posting(1, 125_000)])` |
| Refusal | `Status` with `Code::FailedPrecondition` | `RatioError` with `.status == "FAILED_PRECONDITION"` | `RatioError` with `.status === "FAILED_PRECONDITION"` |

Any other language generates a client from the contract — see
[Other languages](#other-languages).

## The API

Seven calls on two services, `ratio.v1.Ledger` and `ratio.v1.Chart`
([`proto/ratio/v1/`](../proto/ratio/v1)). Every resource lives under
`books/{book}`, where `{book}` is the directory name the server was started on.

| gRPC | REST | What |
|---|---|---|
| `Ledger.CreateTransaction` | `POST /v1/books/{book}/transactions[?transactionId=…]` | Post. Body is a `Transaction`. |
| `Ledger.GetTransaction` | `GET /v1/books/{book}/transactions/{id}` | One entry. |
| `Ledger.ListTransactions` | `GET /v1/books/{book}/transactions[?pageSize=…&pageToken=…]` | The journal, in posting order. |
| `Chart.CreateAccount` | `POST /v1/books/{book}/accounts` | Add an account. Body is an `Account`. |
| `Chart.GetAccount` | `GET /v1/books/{book}/accounts/{dim}` | One account. The id is its dimension. |
| `Chart.ListAccounts` | `GET /v1/books/{book}/accounts` | The chart. |
| `Chart.GetTrialBalance` | `GET /v1/books/{book}/trialBalance` | Totals per side, the difference, a row per (account, currency). |

A `Transaction` is its postings — `{dim, amount, currencyCode?, instrument?,
quantity?}` — and `controlPlaneHash`, the digest of the configuration it was
posted under. `dim` is the account; `amount` is exact minor units; `currencyCode`
is the conservation law the amount is under (two currencies are two laws, not
one law over a sum; absent is the book's untyped group); `instrument`
partitions further; `quantity` is measured, not conserved.

**The door is the store's.** A post goes through `FileBook::append`, the same
check `ratio post` uses. An entry that does not net to zero on every conserved
dimension is refused with `FAILED_PRECONDITION` and the store's own message —
nothing reaches the journal, and there is no second implementation of the rule
that could disagree. A post that pins `controlPlaneHash` to a digest that is not
the active configuration is refused the same way: a client that says which rules
it expects is told when they have moved.

### Errors

Every refusal is a [`google.rpc.Status`](https://google.aip.dev/193): over gRPC
as the status; over REST as JSON under the canonical HTTP mapping.

```json
{"error": {"code": 9, "message": "entry \"txn-1\" does not conserve value: postings net to 1, not 0", "status": "FAILED_PRECONDITION"}}
```

| Status | HTTP | When |
|---|---|---|
| `FAILED_PRECONDITION` | 400 | does not conserve value · no configuration promoted · stale `controlPlaneHash` · closed period |
| `INVALID_ARGUMENT` | 400 | a malformed name, body or enum |
| `NOT_FOUND` | 404 | no such entry or account — or a book this process does not serve |
| `ALREADY_EXISTS` | 409 | a client-assigned id or a dimension already in use |

### JSON

The REST body is proto3's canonical JSON: lowerCamelCase keys, enums as their
names (`"ACCOUNT_TYPE_ASSET"`), and **every int64 as a decimal string** —
`"amount": "-125000"` — because JavaScript numbers lose integers past 2^53 and an
amount is exact or it is wrong. The server accepts a number on input.

```sh
curl -s -X POST localhost:50051/v1/books/fund-1/transactions \
  -H 'content-type: application/json' \
  -d '{"postings":[{"dim":2,"amount":-125000},{"dim":1,"amount":125000}]}'
```

## Other languages

The contract is standard protobuf with `google.api` annotations, so any
language with a protobuf toolchain gets a typed gRPC client, and any HTTP
client gets REST. [`proto/buf.gen.yaml`](../proto/buf.gen.yaml) generates Go,
Java, Kotlin, Python and TypeScript stubs with [buf](https://buf.build):

```sh
cd proto && buf generate          # writes gen/<language>/…
```

For gRPC without a plugin at all — the shape the Python and TypeScript SDKs
take — the wire format is small enough to write by hand: two wire types,
seven messages ([`sdk/python/ratio/_wire.py`](python/ratio/_wire.py) is a
hundred lines).

## What is checked

- `//crates/ratio-api:rest_routes_test` — the REST route table is exactly the
  contract's `google.api.http` rules, read from the compiled descriptor set.
- `//proto:sdk_mirrors_test` — the Rust JSON codec, the Python types and the
  TypeScript types each carry exactly the contract's fields under their
  canonical names.
- `//crates/ratio-api:ratio-api_test` — both protocols against a real book;
  `//crates/ratio-client:ratio-client_test` — the Rust SDK over a real socket;
  `//sdk/python:rest_e2e_test` — the Python SDK against the built binary.
- `//proto:ratio_aip_lint` — the contract lints clean under AIP.

## Not yet

- **One book per process.** `ratio server --book DIR` serves that directory as
  `books/<dirname>`; any other book is `NOT_FOUND`. A root of many books is the
  console's job, and guessing a directory from a URL segment is how one client's
  book would be answered from another's.
- **No authentication.** The server binds `127.0.0.1` by default and carries
  no identity; put it behind the same authorizer the demo uses before exposing
  it. The console API's WorkOS session and Connect JWT paths are separate.
- **Reads stream the journal.** `Get` and `List` walk the log, so a lookup is
  O(journal). Indexed reads are what `RATIO_PG_URL` and the projection are for.
- **Bazel-native Go and Java stubs** are not in the graph; `buf generate` is the
  path today.
