# Ratio — current state

Deployment dependency authentication (#382, October 23): the workflow logs
into ECR Public before pulling the Lambda adapter, separately from its private
ECR push login. The versioned bootstrap adds the required two token actions.
The approved bootstrap grant was applied and read back;
[deployment and smoke passed](https://github.com/mattmarshall/ratio/actions/runs/39958721035)
at `ba7bd9fa37884ff02a7d22def629f02f1a4d33ff`.

The [October 22 recovery readiness audit](recovery-readiness.md) distinguishes
the initial state from the now-merged #264/#300 tooling and remaining
unmerged storage stack. The approved [synthetic AWS drill](recovery-drill-2026-10-23.md)
restored 33 objects with matching figures/digests, measured RPO 0 and RTO
275.748 seconds against 60 minutes. Seven-day retained evidence includes all
failures and passing CLI/HTTP/MCP writer continuations. Matt Marshall accepted
the documented synthetic results; #264/#300 are complete for that scope.
The [live checkpoint provider run](storage-provider-conformance.md) passed,
including cold/checkpoint/tail measurements and negative cases. #358–#360
conformance remains the gate for #362.

Native iOS build preparation (October 22): crate-universe includes device
and simulator triples so the separate shell can compile a pinned kernel.
The shell pins merged revision `7b81c1d6f5648ba902d69f30401bf18eedcaf6e4`.
[Shell PR #4](https://github.com/mattmarshall/ratio-ios/pull/4) and
[PR #5](https://github.com/mattmarshall/ratio-ios/pull/5) have passing
device/simulator archive, simulator APFS/digest, and C ABI checks. Physical
device execution remains unproved. Both shell PRs are merged. The owner has
selected private, all-rights-reserved shell source with no outside contributions;
that record merged in shell PR #6. Kernel contribution-policy scope and its
licensing record remain under iOS #3. Distribution remains a separate gate.

Updated October 23, 2026. This is the current implementation summary. Update
it when behavior lands; keep the reasoning and earlier measurements in
[HANDOFF](../HANDOFF.md) and [PLAN's dated amendments](../PLAN.md).
The [GitHub project](https://github.com/users/mattmarshall/projects/1) and
[roadmap index #258](https://github.com/mattmarshall/ratio/issues/258) own
readiness, sequencing, and issue acceptance. A merged PR is evidence for its
slice, not proof that every item in its parent issue is complete.

## Architecture and verification

| Surface | Current behavior | Source of truth |
|---|---|---|
| Book of record | Append-only journal plus content-addressed configuration and provenance-bearing facts. Investment, Personal, Project, and Operating are book kinds over one kernel. | `crates/ratio-store`, `proto/ratio/console/v1/console.proto` |
| Arithmetic and temporal rules | Lean proofs, TLA+ models, Rust execution; some Rust is emitted from Lean. Money is integral, with explicitly accounted FX/per-share residues where defined. | `lean/Ratio`, `tla`, `crates/ratio-common` |
| Read projection | In-process by default. Setting `RATIO_PG_URL` selects the live Postgres projection for lots, positions, and Current aggregates. The journal remains authoritative; stale watermarks refuse. Verified journal-prefix checkpoints use the configured object store when present and a local directory otherwise. They resume the fold at the newest valid pin; corrupt/mismatch falls back to full replay. Prefix verification still reads every journal body, so remote latency is not established by the local benchmark. | `crates/ratio-project/src/checkpoint.rs`, `crates/ratio-sql-project/src/reads.rs`, `crates/ratio-console/src/lib.rs` |
| Multiple views | Declared views fold their own cuts and explain reconciliation differences; this is implemented, not an unresolved per-view design. | `crates/ratio-console/src/lib.rs`, `lean/Ratio/Views.lean`, `tla/Projection.tla` |
| Operations interface | Next.js console on Vercel, Rust API on Lambda. Kind selects the chart and available screens. Direct-route coverage is being completed under issue 26. | `console/src/lib/screens.ts`, `crates/ratio-console`, `deploy/app.yaml` |
| Connect | Python first-party applications use the authenticated Connect API. A scaffold or local allowlist is not a live provider integration or an API permission boundary. | `connect/`, [scope catalog](connect-scopes.md) |

Use Bazel for core, proofs, Connect, and source-contract tests; never Cargo.
The console additionally needs its own workflow (types, rendering, build,
and source checks). The site has a Python build and verification workflow.
See [DEVELOPING](../DEVELOPING.md) for commands. PR descriptions and commit
messages also pass the [issue-completion check](issue-completion.md).

## What has evidence, and what remains

- **Authentication:** AuthKit session and Connect JWTs use separate gateway
  authorizers. Session cookie redirects, the custom-domain session issuer,
  the header client/server boundary, and transient hydration retries have
  fixes on main. Book membership uses the subject; an organization claim
  alone grants no book. `RATIO_DEMO_OPEN` defaults off in the deployed demo.
  This does not establish every operator or app's activation. Issue 22 owns
  the remaining membership/registration and signed-in walkthrough evidence.
- **Post-create control:** Published books derive ACTIVE, newest-first HISTORY,
  and explicit AuthKit-subject / organization membership from one verified,
  predecessor-enforced protobuf transition stream on the configured object
  backend. Membership is resolved at each public operation boundary. Connect
  client/template grants remain separate and conjunctive; a Connect token
  inherits neither the bootstrap creator grant nor an organization grant.
- **Connect activation:** the LP portal's read grant is proven. Personal bank
  and calendar apps have a local runner that binds public WorkOS PKCE to one
  subject/Personal book, then keeps the separate Plaid or Google credential and
  retry cut in AES-256-GCM per-membership custody. Plaid pending-to-posted and
  Google refresh/410 recovery are restart-safe. Production registration,
  credentials, and redacted signed-in provider evidence remain on issues 22,
  163, 165, and 324. The LP portal's hosted walkthrough and drip elections
  remain on issue 161. Exact client-template grants are enforced. The minimal
  read-only reference app remains on issue 270; neither should be
  confused with local provider activation.
- **Personal goals status (October 2 audit):** book-scoped cited account sheets
  and the goals app's live reader are on main. The deployed independent-Book
  PKCE walkthrough required by issues 355 and 168 has no completion evidence
  in those issues. Neither has an active PR or assignee; both now carry
  `status:ready` in the GitHub project. Their remaining acceptance is live
  operator evidence, not another account-sheet implementation.
- **Demonstration data:** `deploy/seed-demo-funds.sh` seeds nine synthetic
  books: eight Investment funds and one Personal tax walkthrough.
  `deploy/seed_test.sh` verifies that inventory. Deployment
  conditionally publishes every baked journal/append-only plane and its
  versioned whole-seed marker before updating the Lambda image. Cold serving
  processes verify those markers and attach without seed-entry PUTs. Seeds use
  an explicit fixed clock; the one-time legacy-clock adoption permits only the
  three documented receipt/acceptance fields and cannot bypass a v2 marker.
  Occupied seed bodies validate with at most 32 readers; result/progress output
  streams before traffic shifts, and missing suffix writes remain ordered and
  conditional.
  Independent book creation supports all four kinds. A fixture, seed, or green render
  test is not a completed signed-in customer workflow or an accepted customer
  period; those gates remain on issues 27 and 262.
- **Figures:** the lot elections, partner cut, unit movements, capital
  notices, fee receivable, period-close records, kind-native statements,
  and multi-view reconciliation have implementations. Read current issue
  acceptance before re-creating work from an old amendment's leftover list.
- **Scale:** the recorded 20-million-lot projection fold (10,000 securities
  × 2,000 lots) took 17.4 seconds. That is not the full 140-million-entry /
  approximately 40 GB journal fold or a measurement of multiple views;
  those measurements remain in issue 268. See HANDOFF for the digest and
  workload details. Verified journal-prefix checkpoints (#310) report cold
  full replay, checkpoint load, and tail replay for the large demo shape
  (~5,400 entries); the journal remains the book of record.
- **Object-store append (#358):** `SeqLog` asks the store for
  `max_sequence` before its conditional append claim. The S3 adapter uses
  streaming pages for smaller logs and bounded search for larger logs; the
  default scans all keys. Live S3 paired height/append curves and the initial
  smaller-shape regression are recorded in the provider report. The measured
  implementation is merged; both the benefit and tradeoff remain explicit.
- **Storage conformance (#359, branch-local):** a Bazel gate compares
  journal prefixes, trial balances, and all six append-only side planes across
  local JSONL, MemoryStore, DirStore, and the S3 adapter against an in-process
  S3 endpoint. It refuses holes and overwrites and checks that an unregistered
  ObjectStore implementation fails the build, including a new-crate probe.
  The identical fixture passed against live S3, with all prefix and side-plane
  pins retained; final merge and #360 isolation remain separate.
- **Recovery:** published journal/configuration/membership and operational
  evidence are durable. Matt Marshall accepted the representative two-book
  live S3 backup/loss/restore/replay/isolation drill: RPO 0, RTO 275.748 seconds,
  separate encrypted/versioned/seven-day Object-Locked backup. Original
  deliveries and explicitly synthetic Connect custody are inventoried. Every
  configured CLI/MCP/HTTP writer and unavailable-backend refusal was checked.
  #264/#300 are complete within that accepted synthetic scope. Customer backup
  cadence, cross-account disaster recovery and real OAuth resumption are not
  implied; see the recovery and provider reports for exact limits.

- **Per-book storage (#360, branch-local):** the console can resolve two
  independent object stores by book ID in one process, including cold
  published-book discovery, journal writes, and active configuration. An unregistered ID
  refuses. CLI, watch, API, MCP, projection, and NAV entry points now pass
  explicit store handles. `FileBook::open` and `open_attached` are local-only.
  The serving binary still selects one store at startup; a hosted per-book
  router and provider recovery evidence remain before #360 is complete.

## Scope decisions that remain in force

Investment performance/attribution and composites are **not committed
features**. PLAN's existing refusal still applies. The site's phase-three
list is a destination proposal pending the explicit decision in issue 274;
it does not authorize an implementation. Portals and adjacent workflows
remain Connect-shaped unless an explicit PLAN amendment says otherwise.

The model proposes; a person approves configuration, accepts explanations,
and strikes NAV through the CLI. A catalog reservation such as `nav:strike`
does not authorize an RPC or model tool. `partners:write` has no mutation
route. `breaks:explain` currently maps to `MarkPositions`, not explanation
acceptance. `rules:approve` and `config:promote` remain absent. Human
explanation acceptance is built; model-authored `propose_explanation` is
issue 265 and is optional for the first accepted customer period.
See [ORCHESTRATION](../ORCHESTRATION.md) for the fence and its historical
analysis, and [the catalog](connect-scopes.md) for current route meanings.
