![Ratio Logo](images/ratio.png)

**A fund accounting kernel whose core properties are machine-checked.**

Ratio keeps a fund's books as an append-only journal of conserved postings, and
proves the things that must be true of them: that value is conserved on every
dimension, that a trial balance ties because the kernel says so rather than
because a check passed, that a NAV cites the journal prefix it was folded from,
and that a sale gives up the lots the fund's administration agreement says it
does.

The proofs are Lean 4; the concurrency and staleness arguments are TLA+; the
running system is Rust. Parts of the Rust are **emitted from the Lean**, so the
decision a theorem is about and the decision the code makes are the same
decision.

## Etymology

The name draws on the Latin, where *ratio* meant not only "proportion" or
"reason" but "calculation", "account", and "reckoning". Roman citizens presented
their accounts in symmetrical tablets to achieve *parem rationem* — credits and
debits in balance. The balance is the subject and the ethic at once.

## Quickstart

⛔ **Bazel is the only build path. `cargo build` does not work here** — the
workspace members and several crate dependency lists are stale, and the Lean
emission step has no Cargo equivalent. `Cargo.lock` exists because
`rules_rust`'s crate universe reads it, not because Cargo drives anything.

```bash
git clone https://github.com/mattmarshall/ratio.git
cd ratio

bazel test //...                     # the proofs, the specs, the crates, the demos
bazel run //crates/ratio -- --help   # the CLI

# Generate a fund and measure a period end, both curves.
bazel run //crates/ratio -- bench --securities 20 --lots-per 40

# Serve the operations console over a book, on loopback.
bazel run //crates/ratio -- watch --book <dir>   # then open /app
```

No database, no daemon, no container. The journal is a file; the configuration
is content-addressed beside it. See [DEVELOPING.md](DEVELOPING.md).

## Documentation

- **[HANDOFF.md](HANDOFF.md)** — where the work stands, what is load-bearing, and
  the traps. **Read this before changing the lot engine, the projection, or
  anything emitted from Lean.** It is the most accurate document here.
- **[PLAN.md](PLAN.md)** — the path to revenue, and what is deliberately not
  being built. ⚠ Written 2026-08-07 and since overtaken in places: tax lots,
  corporate actions and multi-currency are listed there as *not building* and
  have since been built.
- **[DEVELOPING.md](DEVELOPING.md)** — build, layout, and workflow.
- **[CONTRIBUTING.md](CONTRIBUTING.md)** — how changes are made here.
- **[Whitepaper](paper/)** — *Ratio: A Formally Grounded Accounting Kernel*.
  `bazel build //paper:ratio`.
- **[Positioning brief](marketing/)** — `bazel build //marketing:positioning`.
- **[Marketing site](site/)** — two self-contained HTML pages; `python3
  site/build.py`. See [site/README.md](site/README.md) for why it is not Bazel.
- **[Competitive specs](competitive/specs/)** — one per advertised wealth-tech
  component. `bazel build //competitive/specs:all_specs`.

⚠ **[specs/](specs/) describes a different program.** It is the specification
set for the original personal-finance tool this repository began as — a TUI over
PostgreSQL with Python extensions — and almost none of it describes what is
here now. It is kept for history. Nothing in it should be treated as a
requirement.

## Where things are

| | |
|---|---|
| `lean/Ratio/` | the proofs. `Core`, `Bounded`, `Chart/Dimensions`, `Lots/*`, `Actions/Factor`, `Closure`, `Exec` |
| `tla/` | `Projection`, `Executor`, `ReliefEngine`, `LotEngine`, `Actions`, `Valuation`, `ControlPlane` — each with `manual` probes that must go RED |
| `crates/ratio-kernel` · `-chart` · `-common` | the conserved core, emitted from Lean |
| `crates/ratio-store` | the append-only journal and the content-addressed config seam |
| `crates/ratio-project` | the read model, the lot book, the relief engine |
| `crates/ratio-rules` | posting rules: parsed, checked, compiled |
| `crates/ratio-recon` | the shadow run — a prospect's file, reconciled |
| `crates/ratio-console` + `web/` | the operations console and its backend-for-frontend |
| `crates/ratio-mcp` | the MCP server, with a fence between proposing and approving |
| `crates/ratio-gen` | a fund with realistic shape, generated the same way every time |
| `proto/` | the wire types. AIP-linted |

## Two things worth knowing before reading the code

**Every `tla_check` tagged `manual` is a probe that must FAIL.** It flips one
dial and asserts an invariant breaks. `tla/probes.sh` runs all of them and checks
each went red *for the reason its config names* — a probe that goes green means
an invariant stopped checking, and a probe that goes red for the wrong reason
looks identical to one doing its job.

**Every theorem here is over Lean's `Int`, which is unbounded; every emitted
function runs on `i64`, which is not.** `Ratio.Bounded` is where that hypothesis
is written down and `ratio_common::checked` is the Rust side. A guard asked
about a wrapped number answers about a product that never happened.

## License

⛔ **`LICENSE` and the stated position do not agree, and this needs resolving.**
The file says MIT. [site/README.md](site/README.md) records the position as of
2026-08-06 as *source-available under strong copyleft, dual licensed with a
commercial license for operators*, says the phrase "open source" must not appear,
and names MIT specifically as removed copy — and `//marketing:language_test`
enforces that ban across `marketing/`, `paper/` and `competitive/`.

So a reader is currently told two different things, and the permissive one is the
one with legal effect. The copyleft license is recorded as "still to be
confirmed"; until it is chosen and `LICENSE` replaced, MIT is what has been
granted.
