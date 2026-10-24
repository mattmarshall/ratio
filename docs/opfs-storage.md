# OPFS ownership protocol and browser evidence, October 24, 2026

Related: #365. The proof was committed as `2e520dc` before adapter source.
The implementation has distinct simulation and actual-browser evidence.
[Final exact-head CI passed](https://github.com/mattmarshall/ratio/actions/runs/40070261423);
PR #388 merged as `cdc2a30d80f63515118b3517cfa01db6ae923b97`.
[Post-merge deployment/smoke passed](https://github.com/mattmarshall/ratio/actions/runs/40070769322).

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

## Implementation

One dedicated worker must own each explicit backend namespace. It holds
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
steps below passed against actual browser APIs, separately from simulation.

Because OPFS name/handle lookup is asynchronous, the synchronous Rust
adapter addresses opaque protobuf object frames in one append-only file per
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

## Actual browser proof

The clean implementation revision was
`b77029e83db9ad3c00525271d1b7a7a8881ad11f`; module SHA-256 was
`b4e7c41f5037e2ea64ba8002798f3cc5ccb649610e390f185fe4ed1efe4f73bd`.
The Codex in-app browser on macOS used the loopback lab origin and namespaces
`ratio365lab-20261024-a1` and `ratio365lab-20261024-b1`. This is actual
OPFS/Web Locks execution, not the Node fs shim.

1. The first worker acquired one namespace lock and an exclusive file handle.
   The reusable fixture matched native JSONL: every journal byte, all four
   prefix pins, six side planes, 21/21 trial balance, binary objects, UTF-8
   ordering, exact u64 height, conditional claims, and gap refusals.
2. A second tab became a client and reached the same owner. An injected
   partial append produced no write acknowledgment and no readable figures.
   Reloading the client still did not steal the live poisoned owner's lease.
3. Reloading the owner produced a different sole owner, which recovered only
   the **159-byte incomplete unacknowledged tail**. The fixture digest stayed
   `923bede0abad250c1ea53a8b12f2fe8fa7fbc17abf77de476b535545bbe04502`;
   all planes and figures agreed. The valid drill book retained its first
   posting, 7/7 trial balance, and digest
   `f45b0b3718cac52e34d52b73be692f5b6573c3fedfd394bd28ba3e40200f5b35`.
4. The client appended through that owner. Another owner reload retained both
   drill postings, **14/14**, and digest
   `b0a5c4d1b8d84f0bd44c9d0f616e2cc01edc9cf23b495f9c9fd8af960d921ec0`.
   Both common fixture books stayed unchanged.
5. An independent OPFS namespace reproduced the base fixture and 7/7 drill
   while the first retained 14/14. Each namespace had one owner; there was
   no cross-namespace mutation or process-global store substitution.

[Evidence](evidence/opfs-2026-10-24/) retains 15 raw protobuf receipts,
decoded text, screenshots, hashes, and a `BackendConformanceEvidence` record
whose 15 assertions were independently checked from those receipts. Native
fixture payloads were compared again offline. The initial modeling shortcut
and invalid dated-append fixture attempt are recorded above; neither counted
as passing evidence. Lab tabs and the server were closed after preservation;
no namespace file was deleted.

The simulation additionally checks complete corruption/duplicate-frame refusal
and a fully flushed write whose acknowledgment is lost: reopening retains it,
and the drill refuses a blind duplicate retry until reconciliation. These are
simulated faults, not browser power-loss evidence. No browser timing or
performance claim is made. The host index consumes O(object-count) metadata
memory in addition to kernel linear memory. The positive TLA check uses two
writers/two slots and checks safety, not availability under a frozen owner.
A frozen owner can block clients; it cannot be displaced by a heartbeat.

A later review found that host catch blocks could mask a nested wasm trap as
an ordinary I/O refusal. VM traps now propagate so the owner discards the
instance. A real nested wasm-trap regression verifies propagation, no frame
write, and intact replay in a fresh instance; this added fault test is part
of the simulation gate, not a claim of browser trap injection.

After the trap guard, clean revision `1a8bbf7d61deb1468f407532f24bb4e9f8f2e0e1`
reopened the retained actual-browser namespace after all lab tabs had closed.
Replay retained the full fixture and 14/14 drill state; a second tab again
remained a client with one held lock. Three supplemental raw receipts and a
screenshot preserve this final-source check. Module SHA-256:
`d2761063bad4410be4944183a4a10a97a0e0e71c144088535a998e4726316abf`.
The server and tabs closed again after preservation.
