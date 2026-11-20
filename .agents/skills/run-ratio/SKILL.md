---
name: run-ratio
description: Build the ratio CLI and exercise it on a real book — init, post, the conservation refusal, trial balance, NAV strike and replay, the gRPC/REST server, the browser screens, and the nine seeded demo books. Use to see a change working end to end, to reproduce a bug on real data, or to smoke-test the binary after a change.
---

# Run Ratio

## Smoke test

```bash
.agents/skills/run-ratio/scripts/smoke.sh          # add --keep to keep the book
```

Builds `//crates/ratio:ratio`, then on a throwaway investment book asserts:

- a balanced entry posts, and an unbalanced one is refused for conservation;
- the trial balance ties over only the accepted entry;
- `strike --as-of <today>` lands on today, and `--as-of` another day is refused;
- `navs` lists the strike and `replay` re-derives it;
- `ratio server` answers REST: 200 for a balanced post, 400
  `FAILED_PRECONDITION` for an unbalanced one, and a tying trial balance.

Every check fails the script; nothing passes by omission.

## By hand

```bash
bazel build //crates/ratio:ratio
R=$PWD/bazel-bin/crates/ratio/ratio
$R --help                                     # every verb
$R init --kind investment --book /tmp/fund    # investment | personal | project | operating
cat > /tmp/sub.json <<'EOF'
[{"id":"sub-1","postings":[{"dim":2,"amount":1000000},{"dim":20,"amount":-1000000}]}]
EOF
$R post /tmp/sub.json --book /tmp/fund        # amounts are integer minor units; never floats
$R balance --book /tmp/fund
$R strike --book /tmp/fund                    # refuses a position with no price on the day it lands
$R navs --book /tmp/fund
$R replay <ID> --book /tmp/fund
```

The book is a directory: `accounts.json`, `book.toml`, content-addressed
`config/`, and the append-only journal. The book defaults to `./book` or
`$RATIO_BOOK`. A chart's dimensions are in its `accounts.json`; in the
investment chart, `1` is investments at fair value, `2` cash, `20` capital
contributions.

## Serve it

```bash
$R server --book /tmp/fund --addr 127.0.0.1:50051     # gRPC and REST on one port
curl -s -X POST localhost:50051/v1/books/fund/transactions \
  -H 'content-type: application/json' \
  -d '{"postings":[{"dim":2,"amount":-125000},{"dim":1,"amount":125000}]}'
curl -s localhost:50051/v1/books/fund/trialBalance

$R watch --book /tmp/fund                              # the console API and screens on :7373
$R mcp --book /tmp/fund                                # the MCP tools on stdio
```

The resource name is the book directory's basename. SDKs: `crates/ratio-client`,
`sdk/python`, `sdk/typescript`; the error model is in `sdk/README.md`.

## Realistic data

```bash
bash deploy/seed-demo-funds.sh bazel-bin/crates/ratio/ratio /tmp/books   # ~3s, nine books
```

Nine real books built through the normal code path, each in a state the
product must render: a fund blocked by a break nobody explained, one struck
because somebody did, one keeping two books of record over one journal, and so
on. `RATIO_SEED_EPOCH` fixes the clock, so the seeds are byte-identical
(`//deploy:seed_test` and `//deploy:unchanged_seed_source_is_byte_identical`
hold that).

For scale: `$R gen --book /tmp/big --securities 500 --lots-per 40` generates a
fund, `$R bench` measures a period end, and `$R closure` estimates one before
running it. ⚠ Report both curves when there are two; `ratio bench` is shaped
to make an overclaim hard.

## Humans only

`ratio approve`, `ratio accept` and `ratio close` record a person's decision.
An agent proposes; it does not approve its own proposal. See `ORCHESTRATION.md`.
