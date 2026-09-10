# Durable journal publication and startup

The deploy workflow owns baked-book publication. Before CloudFormation receives
the new image digest, CI runs `ratio publish-seeds` against the configured
object store for `/opt/demo-book`, every `/opt/demo-funds/*` book, and all seven
append-only planes (journal, deliveries, entities, facts, actions,
explanations, and closes). Only after that command publishes, validates, and
opens every durable journal may the workflow update the Lambda image.

Publication claims each missing line at its baked one-based sequence with the
same conditional `put_if_absent` contract as a journal append. It compares every
occupied seed slot byte-for-byte first. The final conditional object is
`journals/_seed/publications-v2/<book-id>`: format version 2, a digest over every
baked plane (including empty planes), and each plane's baked length. An
unchanged deploy reads that marker and performs no sequence LIST or PUT. A
different marker or occupied sequence fails deployment with the book, plane,
sequence, and expected/actual digest where applicable.

### The reviewed legacy-clock adoption

The first format-v1 rollout failed correctly on the old Harbourline delivery:
the durable and newly baked records differed. Source review identified the
cause rather than declaring the bytes equivalent by fiat: the seeder used the
build runner's wall clock for `Delivery.received`, copied it to
`Fact.provenance.received`, used another clock for
`BreakExplanation.accept_time`, and anchored generated settlement tails to the
build day. The local determinism check in #327 rebuilt twice within one second,
so it could not expose that cross-deploy drift.

The seed scripts now fix their explicit source-of-truth clock at Unix epoch
`1788393600` (2026-09-03 00:00:00 UTC), the day the durable journals first
published successfully in deploy #130. The deployment invokes the named
`legacy-volatile-times-v1` migration while a v2 marker is absent. That one-time
door parses and compares each occupied JSON record after removing only:

- `received` from a delivery;
- `provenance.received` from a fact; and
- `accept_time` from a break explanation.

Every other field and every other plane must still match. Adoption does not
overwrite the durable timestamp or any sequence object; the v2 marker records
the migration name and the deterministic baked digest. Once that v2 marker
exists, the migration cannot be reused: later publication compares the marker
normally and any changed seed fails closed. Old format-v1 markers are retained
as rollout evidence but are not the serving marker.

### Migration I/O and progress

Deploy `34505523090` established the failure curve: 41 minutes in publication
before the 45-minute job budget cancelled it. The publisher performed one
serial S3 GET for every occupied seed sequence, then `entries()` downloaded all
16,245 journal bodies a second time to print a count. Including side planes,
that was more than 32,000 sequential body reads. Shell command substitution
captured all successful stdout until exit, leaving no progress in Actions.

Occupied-prefix validation now runs with a hard ceiling of 32 concurrent GET
readers. The write side is unchanged: missing suffix sequences are claimed in
order through conditional PUT, so an interrupted initial publication still
resumes from a contiguous prefix. Once bodies have been validated and the
marker published, the attached-open proof reads journal height with LIST rather
than parsing the same bodies again. The command writes each book and
plane/sequence count to the live log; the workflow streams successful result
lines through `tee` before applying smoke assertions.

`ratio watch` binds and accepts HTTP while durable attachment initializes.
It verifies every baked marker and uses `FileBook::open_attached`; startup has
no seed-publication call and performs no seed-entry PUT.
Startup readiness applies to every `/v1/` RPC, public book JSON endpoint,
terminal, chat, and HTTP MCP request. The scale-run service uses its own
store; its job controls and reports do not open this book.

| Startup state | Book routes | `/healthz`, `/version`, `/authconfig.json` |
|---|---|---|
| Pending | Wait up to 2 seconds, then 503 with `Retry-After: 2` and the compatibility message `the journal is still hydrating` | Available |
| Ready | Normal authorization and book handling | Available |
| Failed with a configured durable store | 503 with `journal startup failed; correct the storage configuration and restart`; no `Retry-After` | Available |

An unauthenticated `/v1/` request in required-auth mode still gets 401
without waiting for storage. Health reports that HTTP is alive, not that
book reads or writes are ready. Static public pages can load while their
book-data requests are unavailable.

Both nonempty `RATIO_JOURNAL_BUCKET` and nonempty `RATIO_JOURNAL_LOCAL`
select durable storage; the bucket takes precedence when both are set.
Installing that store and verifying the seed markers must succeed before the
first attached book open.
A failed installation cannot select the local-file fallback. A failed
marker check or book attachment cannot release requests to try opening again.

Failure is terminal for that process. Consult the server's detailed
startup error, correct the configured store's credentials, access, location,
or failed book data as appropriate, and restart `ratio watch`
(replace the failed runtime for a deployment). There is no in-process
retry or reset of a failed gate. Do not unset a durable-store setting to
clear the error: that deliberately selects different, local storage.
Clients receive the stable failure sentence, not private store details.

## Publication failure procedure

The Platform deployment owner handles publication failures while the previous
application version continues serving the same journals:

1. Read the `publish-seeds` diagnostic. A missing marker or interrupted suffix
   can be retried unchanged; conditional claims resume safely.
2. For `seed mismatch`, do not delete a marker, overwrite a sequence, truncate
   a journal, or change the baked files merely to make deployment green.
   Determine whether the image contains the wrong seed or the destination
   prefix/book ID names different durable data.
3. Do not add another normalization field or generic bypass to
   `legacy-volatile-times-v1`. That reviewed migration is valid only while a v2
   marker is absent and only for the three paths above. Any other mismatch is a
   real divergence for this procedure.
4. Rebuild/redeploy the intended seed, or select the correct untouched durable
   prefix. Escalate unexplained durable-byte changes through the recovery
   procedure before allowing traffic to shift.
5. Rerun deployment. The publisher validates all books, including the small
   Northstar seed and large Ashcombe seed, before CloudFormation updates the
   Function.

Rollback changes only the Lambda image. It does not replace, delete, or fork
`RATIO_JOURNAL_BUCKET` / `RATIO_JOURNAL_PREFIX`; the prior application version
therefore attaches to the same durable journals. It ignores the new marker and
its legacy hydration path sees seed sequence heights already satisfied.

When both durable-store settings are unset or empty, local-only mode
remains supported: reads and writes use `journal.jsonl`. A bad local
book path causes `watch` to exit after startup fails.

This boundary addresses [#293](https://github.com/mattmarshall/ratio/issues/293)
and [#309](https://github.com/mattmarshall/ratio/issues/309).
It does not establish complete cold-start recovery of book metadata.
Metadata durability remains on [#291](https://github.com/mattmarshall/ratio/issues/291),
and the operational recovery drill remains on
[#264](https://github.com/mattmarshall/ratio/issues/264).

`//crates/ratio:ratio_test` injects both installation and attachment failures,
exercises HTTP reads and a valid `applyEvent` write against a local book,
and checks that the journal is unchanged. The identical write succeeds
after an explicit local startup. `//crates/ratio-store:ratio-store_test`
proves conditional suffix publication, all-plane coverage, cheap unchanged
markers, mismatch refusal, and a serving attachment with no seed PUTs. Every
current console route and every
public book transport is checked for startup refusal. The former ignored
installation error and failed-as-ready behavior must each make the
corresponding regression fail.
