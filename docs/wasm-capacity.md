# Wasm core conformance and capacity — October 24, 2026

Related: #364. The existing Console/core builds for `wasm32-unknown-unknown`
using Rust 1.85.0 and message-only protobuf generation. Its target runtime
dependencies exclude tonic, the AWS SDK, and tokio. Native gRPC retains tonic.
This is a kernel target and test transport, not a complete browser console.

`//crates/ratio-project:wasm_conformance_test` runs the same fixture as the
native JSONL/object-store gate. The host compares every journal byte, all six
side planes, four prefix digests, and trial-balance figures. The final digest
is `923bede0abad250c1ea53a8b12f2fe8fa7fbc17abf77de476b535545bbe04502`.
Conditional overwrite and gap refusals execute in the actual wasm instance.
Both recorded views also retain 10,000 lots in the small runtime check.

## Measured ceiling

The measured profile has **1,073,741,824 bytes (1 GiB) maximum linear memory**.
Each fresh optimized instance streams synthetic buys across 10,000 securities,
one currency, one dated buy/lot per entry, and either one or two recorded views.
The journal generator retains no second in-memory copy of all JSON bodies.
The existing kernel parses and folds every entry. Successful runs verify each
view's exact open-lot count, position quantity, and cited prefix.

| Views | Last successful journal height | First refusing height | Peak linear memory at successful height | Successful elapsed time |
| --- | ---: | ---: | ---: | ---: |
| 1 | 20,486,308 | 20,486,309 | 1,073,741,824 bytes | 48,811 ms |
| 2 | 10,246,267 | 10,246,268 | 1,073,676,288 bytes | 34,843 ms |

The initial runs requested 100,000,000 entries and trapped at those boundaries.
Separate fresh instances confirmed the last successful height and the very next
refusing height. A callback before each generated entry records the preceding
completed height. Allocation failure traps the call; it returns no figures.
The refusing calls took 49,407 ms and 34,899 ms, respectively.

An independent **2 GiB control**, compiled from identical accounting source,
successfully folded both formerly refusing heights. Its peaks were 1,073,938,432
bytes (one view) and 1,073,807,360 bytes (two views), above the 1 GiB profile.
That distinguishes an allocation-limit failure from an accounting assertion.
The larger profile is a diagnostic control, not another supported ceiling.

These are **peak allocated wasm linear-memory bytes**, measured from
`WebAssembly.Memory.buffer.byteLength`, not RSS. Linear memory grows and does
not shrink during these calls, so its final size records its allocation high
water mark, including allocator slack and the module's stack/static storage.
Host V8 metadata, browser processes, and storage caches are outside that metric.
The measurements do not claim universal browser capacity, a total-device-memory
limit, or interchangeable capacity for different security counts, strings,
currencies, lot elections, sales, corporate actions, or additional views.
The old one-view native estimate cannot be applied to two wasm views.

## Reproduction and evidence

Build with Bazel:

```sh
bazel test //crates/ratio-project:wasm_conformance_test -c opt
bazel build //crates/ratio-project:wasm_conformance -c opt
```

Resolve the module path with `bazel cquery` (shared convenience symlinks can
change when another output base is used). From a clean checkout, run
`node deploy/wasm-capacity.cjs MODULE REVISION NEW_OUTPUT.pb HEIGHT VIEWS`.
The operator refuses an existing output or a dirty/mismatched revision and
reads the memory maximum from the compiled module itself. The runtime gate
requires Node.js on PATH. The measurement runtime was Node v26.8.2, V8
14.6.202.34-node.28, on the operator's Apple Silicon host.

The six 1 GiB measurements came from clean revision
`0717f9c1eaf92071529d331cdf06beb926800102`; module SHA-256:
`7d3b71939827fdc9eea5fcabf3351008b3d1397ca8a183f6a71f78953e3c58ba`.
The two controls came from clean revision
`408fc43803c70b2586b6f5184ef2bf271faf192a`, which adds the control target and
reads the compiled memory limit without changing accounting source.
[Eight protobuf measurements, decoded text, logs, and hashes](evidence/wasm-2026-10-24/)
are retained in this repository. They contain synthetic workload measurements,
not customer book content. No live S3 or Postgres operational evidence is
inferred from wasm execution.

The filesystem-free FileBook door shares the existing immutable metadata,
configuration, journal, and side-plane protocols. Its native parity test also
checks chart/configuration/history recovery and missing-metadata refusal.
Browser OPFS ownership, reload exclusion, UI transport, and authentication
remain separate work. No production client should treat these boundary values
as safe operating targets without headroom and its own workload policy.
