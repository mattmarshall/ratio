# OPFS ownership protocol — proof gate, October 24, 2026

Related: #365. The proof was committed as `2e520dc` before adapter source.
The implementation below has simulation evidence; actual browser execution
and final conformance remain required.

`OPFSJournal.tla` separates ownership, preparing an append position, writing,
flushing/acknowledging, and crash/reload. An acknowledged write remains in the
flushed log. A partial unacknowledged tail can be discarded; a complete record
whose reply was lost can survive. This is not an exactly-once retry promise.
Unknown outcomes must be reconciled before retrying.

The positive model explored **91 distinct states** and preserved `NoWriteIsLost`,
single-writer exclusion, and a contiguous durable prefix. Removing exclusion
fails `NoWriteIsLost`: two workers prepare the same position, one overwrites
the other, and a flush acknowledges data that is no longer present. The first
draft allowed a dead worker to roll back a different live worker; that shortcut
was removed before committing the model. Tail recovery now occurs only after
the last owner is gone. [Logs](evidence/opfs-2026-10-24/) retain the corrected
trace and the full **49-probe** executable sweep, all red for their named reason.

## Implementation and pending browser realization

One dedicated worker must own each explicit backend namespace. It will hold
a [Web Lock](https://www.w3.org/TR/web-locks/) and an exclusive
[OPFS synchronous access handle](https://fs.spec.whatwg.org/) for its lifetime.
Other tabs submit work to the owner; they do not become independent writers.
There is no heartbeat-based lease theft or `steal` option. Missing support,
ambiguous I/O, or unavailable ownership must refuse. Actual second-tab and
reload behavior must be tested rather than inferred from API names.

The Rust `OpfsStore` passes an explicit binding through every wasm import;
zero, missing, and poisoned bindings refuse. Immutable canonical protobuf
frames protect key/body/version by digest. The synchronous host appends a
length/framed object/complemented length, flushes, then updates its index and
acknowledges. Complete corruption and duplicate keys refuse. Only an incomplete
trailing frame can be discarded after ownership, and ambiguous writes poison
the owner until recovery. A cached per-prefix integer height avoids journal
body reads and key-list allocation; `u64::MAX` remains exact across the ABI.

The required backend gate now includes `//browser/opfs:adapter_simulation_test`.
It runs the actual Rust wasm adapter over an fs-backed synchronous-handle shim:
all native fixture bytes/digests/planes/figures, binary objects, UTF-8 ordering,
u64 heights, conditional claims, holes, distinct bindings, cold reopening,
partial-write refusal/recovery, complete corruption, and duplicate frames.
This is explicitly **simulation**, not live OPFS ownership evidence.
The shared fixture's closes plane contains opaque records, so it cannot accept
a later dated posting; the write/crash drill uses a separate valid book instead
of weakening the close guard or changing the shared fixture.

`bazel run //browser/opfs:verification_server` serves a clean-revision,
loopback-only synthetic lab. The page/worker use protobuf correlation messages
and never turn a correlation ID into an exactly-once claim. The namespace is
preserved in the URL across reload. Actual second-tab and mid-append reload
steps are still pending; do not close #365 from simulation alone.

Because OPFS name/handle lookup is asynchronous, the proposed synchronous Rust
adapter will address opaque protobuf object frames in one append-only file per
backend namespace. Frames name their key and body and protect both by digest.
The worker checks the immutable-key index, appends, flushes, then acknowledges.
A complete corrupt or duplicate frame refuses; recovery can discard only an
incomplete unacknowledged tail after ownership is acquired. Backend handles
must be explicit, including in wasm imports; no process-global default store
may replace the per-book binding proved by #360.

The in-memory index must not retain every journal body. Its own metadata memory
and cold-scan cost are separate from #364's kernel capacity measurements; no
browser-scale performance claim follows from those measurements. The default
module remains capped at 1 GiB. A trapped or timed-out instance must be discarded.

Browser eviction, off-device backup, identity, membership authorization, provider
credentials, a full browser Console transport, and distribution approval are
not supplied by this storage protocol. iOS continues using its independent
APFS claim path and exact kernel pin.
