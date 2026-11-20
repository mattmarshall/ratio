<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/mattmarshall/ratio/main/images/wordmark-dark.png">
  <img alt="ratio" width="240" src="https://raw.githubusercontent.com/mattmarshall/ratio/main/images/wordmark-light.png">
</picture>

**An accounting book whose core properties are machine-checked.**

Ratio keeps the books as an append-only journal of conserved postings and
proves what must be true of them: value is conserved on every dimension, a
trial balance ties because the kernel says so, a figure cites the journal
prefix it was folded from, and a sale gives up the lots the terms dictate. One
kernel serves investment funds, personal finance, project finance and
operating businesses — different charts, not forked products. The proofs are
Lean 4, the concurrency arguments are TLA+, the running system is Rust, and
parts of the Rust are **emitted from the Lean**, so the theorem and the code
make the same decision.

### ▶ [Try the live demo](https://ratio.marsh.build/)

Sign in to inspect synthetic books — a fund blocked by a break nobody has
explained, one struck because somebody did, one keeping two books of record
over a single journal. Or skip the sign-in and read a
[public trial balance](https://1h4q8av2gb.execute-api.us-east-1.amazonaws.com/balance).

## Quickstart

Bazel is the only build path; `cargo build` does not work here, because the
kernel is emitted from the Lean at build time.

```bash
git clone https://github.com/mattmarshall/ratio.git && cd ratio
bazel test //...                                    # proofs, specs, crates, demos
.agents/skills/run-ratio/scripts/smoke.sh           # build the CLI and drive a real book
```

No database or service is needed: the journal is a file and the configuration
is content-addressed beside it. In a sandbox that refuses GitHub archive
downloads, run
`python3 .agents/skills/restricted-network-bazel/scripts/override_github_archives.py`
first.

## From your code

`ratio server` serves the kernel API — `ratio.v1.Ledger` and `ratio.v1.Chart` —
over gRPC and REST on one port. An entry that does not conserve value on every
dimension is refused with `FAILED_PRECONDITION` and never reaches the journal.

```bash
ratio init --kind investment --book fund-1 && ratio server --book fund-1
curl -s -X POST localhost:50051/v1/books/fund-1/transactions \
  -H 'content-type: application/json' \
  -d '{"postings":[{"dim":2,"amount":-125000},{"dim":1,"amount":125000}]}'
```

SDKs: [Rust](crates/ratio-client) · [Python](sdk/python) (`pip install ratio-sdk`)
· [TypeScript](sdk/typescript) (`npm install ratio-sdk`) · anything else with
`cd proto && buf generate`. The error model is in [sdk/README.md](sdk/README.md).

## Working on Ratio

Start with **[AGENTS.md](AGENTS.md)** — the rules that are not negotiable and
the failure shapes to look for, written for coding agents and just as useful
to people. Task-level instructions are skills in
**[`.agents/skills/`](.agents/skills/)**: building and testing, running the
CLI, proofs, the console, and shipping a change, each with scripts that check
what they claim. [CONTRIBUTING.md](CONTRIBUTING.md) has the owner-only policy;
[DEVELOPING.md](DEVELOPING.md) the full build reference.

## Documentation

- **[Current state](docs/current-state.md)** — what is built, demonstrated, and still open.
- **[HANDOFF.md](HANDOFF.md)** — what is load-bearing and what will bite.
- **[Roadmap project](https://github.com/users/mattmarshall/projects/1)** and
  **[index #258](https://github.com/mattmarshall/ratio/issues/258)** — the queue.
- **[PLAN.md](PLAN.md)** — scope decisions and dated amendments.
- **[console/](console/)** — the operations console (Next.js, on Vercel).
- **[connect/](connect/README.md)** — first-party Connect apps and the
  [scope catalog](docs/connect-scopes.md).
- **[deploy/](deploy/README.md)** — the hosted demo, sign-in and tenancy.
- **[Whitepaper](paper/)** · **[Site](https://mattmarshall.github.io/ratio/)**

## License

**AGPL-3.0** ([LICENSE](LICENSE)), and available under a commercial license
([LICENSING.md](LICENSING.md)). Copyleft because the argument for Ratio is that
a figure can be *checked*, and that argument is weak if the code cannot be
read. The hosted service is the path with no copyleft obligation.
