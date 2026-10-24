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

The physical-footprint supplement reran both successful boundaries using
macOS `/usr/bin/time -l`'s **peak memory footprint** counter. Peak physical
footprint was **981,079,368 bytes** for one view and **1,031,656,704 bytes** for
two. These are separate from allocated linear memory and are not RSS. The
supplement includes the Node/V8 process and worker, but no browser UI or OPFS
index. Its elapsed times were 48,859 ms and 31,397 ms. Raw counter output and
the unchanged child protobuf are retained beside each augmented measurement.

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

## Address-space ceiling and a retained failure

The 1 GiB profile is a deliberate smaller limit, not wasm32's architectural
maximum. A separate module capped at **4,294,901,760 bytes** (4 GiB minus one
64 KiB wasm page) measured the near-address-space ceiling:

| Views | Last successful journal height | First refusing height | Peak linear allocation | Peak physical footprint |
| --- | ---: | ---: | ---: | ---: |
| 1 | 81,926,356 | 81,926,357 | 4,294,574,080 bytes | 3,582,336,344 bytes |
| 2 | 40,966,350 | 40,966,351 | 4,294,639,616 bytes | 3,649,920,296 bytes |

Fresh instances confirmed each successful boundary and its adjacent refusal.
The physical-footprint reruns also returned exact figures at both successful
heights; they took 192,283 ms and 127,092 ms. This larger profile is diagnostic,
not the default module or a browser/device memory-budget promise.

**The exact 4 GiB module is unsupported.** Its one-view boundary returned, but
the two-view call at 40,966,351 reached the post-fold validation callback and
did not return. The first attempt was sampled and stopped after about eight
minutes; its partial log is retained, and no successful measurement is invented
for it. A watchdog-backed repeat recorded completed height 40,966,351, allocated
linear memory 4,294,967,296 bytes, and no figures returned after 180,004 ms.
The location of the callback implies a failure while leaving the function;
the underlying allocator/runtime cause has not been diagnosed. A page below
4 GiB, that entry traps normally instead, and the preceding entry returns.
Do not infer a successful query from a completed progress callback.

The capacity operator now runs the kernel in a worker with a bounded watchdog
and preserves timeout evidence as protobuf. Small success and deliberately
forced-timeout checks passed. A trapped or timed-out instance is discarded;
it must never serve figures from a partially completed fold. Exact-4-GiB
diagnostics must use this watchdog, never an unbounded application call.

Linear-memory columns are **peak allocated wasm linear-memory bytes**, measured from
`WebAssembly.Memory.buffer.byteLength`, not RSS. Linear memory grows and does
not shrink during these calls, so its final size records its allocation high
water mark, including allocator slack and the module's stack/static storage.
Host V8 metadata, browser processes, and storage caches are outside that metric.
Physical-footprint columns instead use the process's peak-footprint counter;
the raw time output also contains RSS, but RSS is not used in these claims.
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

For a physical-footprint measurement on macOS, use
`python3 deploy/wasm-footprint.py MODULE REVISION NEW_OUTPUT.pb HEIGHT VIEWS`.
It retains the original child protobuf and raw time output, then writes a new
protobuf with the peak-footprint fields. It refuses to replace any evidence.

The six 1 GiB measurements came from clean revision
`0717f9c1eaf92071529d331cdf06beb926800102`; module SHA-256:
`7d3b71939827fdc9eea5fcabf3351008b3d1397ca8a183f6a71f78953e3c58ba`.
The two controls came from clean revision
`408fc43803c70b2586b6f5184ef2bf271faf192a`, which adds the control target and
reads the compiled memory limit without changing accounting source.
[Protobuf measurements, decoded text, logs, and hashes](evidence/wasm-2026-10-24/)
are retained in this repository. They contain synthetic workload measurements,
not customer book content. No live S3 or Postgres operational evidence is
inferred from wasm execution.

The full-address diagnostic was pinned at `07c3a8d27d9ee22655d8e8d4d13ec29979b919c4`.
Watchdog and page-below-limit evidence came from
`68f44f17d2edb300f47df8789723bb93a6cfd94a`. Physical-footprint evidence came from
`93005fa2ed8ff3d1ab104bcbf5d62e7d193b6786`; its core module hashes remained
unchanged. The page-below-limit module SHA-256 is
`471dded95d8c54ce12c68fa008c51b419d843281e5725ad989f98946d304d5c5`.
There are 25 canonical measurement protobufs, including the operator checks,
plus four unchanged child protobufs from the footprint supplement. Earlier
allocation-only records have no physical-footprint field and do not claim one.

The filesystem-free FileBook door shares the existing immutable metadata,
configuration, journal, and side-plane protocols. Its native parity test also
checks chart/configuration/history recovery and missing-metadata refusal.
Browser OPFS ownership, reload exclusion, UI transport, and authentication
remain separate work. No production client should treat these boundary values
as safe operating targets without headroom and its own workload policy.
