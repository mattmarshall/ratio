# OPFS ownership protocol — proof gate, October 24, 2026

Related: #365. The adapter is not implemented yet. This records the safety
argument before code; browser execution and conformance remain required.

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

## Planned realization

One dedicated worker must own each explicit backend namespace. It will hold
a [Web Lock](https://www.w3.org/TR/web-locks/) and an exclusive
[OPFS synchronous access handle](https://fs.spec.whatwg.org/) for its lifetime.
Other tabs submit work to the owner; they do not become independent writers.
There is no heartbeat-based lease theft or `steal` option. Missing support,
ambiguous I/O, or unavailable ownership must refuse. Actual second-tab and
reload behavior must be tested rather than inferred from API names.

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
