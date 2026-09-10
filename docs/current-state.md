# Ratio — current state

Updated September 9, 2026. This is the current implementation summary. Update
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
| Read projection | In-process by default. Setting `RATIO_PG_URL` selects the live Postgres projection for lots, positions, and Current aggregates. The journal remains authoritative; stale watermarks refuse. | `crates/ratio-sql-project/src/reads.rs`, `crates/ratio-console/src/store.rs` |
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
- **Connect activation:** the LP portal's read grant is proven. Its hosted
  walkthrough and drip elections remain on issue 161. Other apps need their
  own registration. The API client-template allowlist is tracked in issue
  260, and the minimal read-only reference app in issue 270; neither should
  be confused with a grant helper or an app's local template checks.
- **Demonstration data:** `deploy/seed-demo-funds.sh` seeds eight synthetic
  Investment funds and `deploy/seed_test.sh` verifies that count. Independent
  book creation supports all four kinds. A fixture, seed, or green render
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
  workload details.
- **Recovery:** persistent journal hydration, immutable book bootstrap, and
  post-create configuration/membership transitions are built. NAVs,
  reports/proposals, CHANGELOG, complete backup coverage, and the external
  restore drill remain separate recovery work.

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
