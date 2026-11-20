---
name: console
description: Develop and test the operations console in console/ — a Next.js 16 app deployed to Vercel and NOT built by Bazel — including running it locally against a real book, its required CI checks, and adding an API field end to end. Use for any change under console/ or to proto/ratio/console/v1/console.proto.
---

# The operations console

`console/` is a Next.js application (Next 16, React 19, pnpm 10, Node 22.11)
deployed to Vercel. It reads the same API the Rust binary serves.

⛔ **Bazel does not build it and has no JavaScript toolchain.** A type error in
`console/` is invisible to `bazel test //...`. `.github/workflows/console.yml`
is the required gate. The one thing Bazel asserts about the console is
`//proto:mirrors_test`, which holds `console/src/wire/types.ts` to the proto
field for field — so do not delete `console/BUILD.bazel`.

⚠ **This is not the Next.js in your training data.** Read the relevant guide
in `console/node_modules/next/dist/docs/` before writing code (see
`console/AGENTS.md`). `next dev` rewrites the marked block in that file; leave
it alone.

## Run it locally

No identity provider, no secrets, no network: with no `RATIO_WORKOS_*`
variables set, the server answers as `Subject::Local` and the console skips
its sign-in gate.

```bash
bazel build //crates/ratio:ratio
bash deploy/seed-demo-funds.sh bazel-bin/crates/ratio/ratio /tmp/books   # nine real books
bazel-bin/crates/ratio/ratio watch --book /tmp/books/ashcombe-global-equity   # API on :7373
cd console && pnpm install && pnpm dev                                      # console on :3000
```

## Check it — what console.yml runs

```bash
.agents/skills/console/scripts/contract-checks.sh   # the six source-text checks; Python only
cd console && pnpm install --frozen-lockfile
pnpm check                                          # tsc --noEmit, vitest, preflight + next build
pnpm exec playwright install chromium               # once; or CHROMIUM_PATH=<an existing chromium>
pnpm test:phone                                     # needs the build: no screen scrolls sideways at 375px
```

`contract-checks.sh` needs no Node, so run it in any container. It covers: the
console calls exactly the contract's routes and every RPC is read by some
screen; fixtures match the proto; screens still read fields that have shipped
missing before; design tokens agree with `site/style.css`; nothing that belongs
in the environment is committed; every `BookKind` is named in `PLAN.md`.

## Adding an API field

Six files, in this order:

1. `proto/ratio/console/v1/console.proto`
2. `crates/ratio-console/src/transcode.rs` — route, dispatch arm, `JsonView`
3. `crates/ratio-console/src/lib.rs`
4. `console/src/wire/types.ts`
5. `console/src/wire/client.ts`
6. the page under `console/src/app/` that reads it

Four checks turn an omission into a failure rather than a 404:
`//proto:mirrors_test`, `//crates/ratio-console:transcode_test`,
`console/scripts/route_manifest_test.py`, and — if a reader looks for the
field — an entry in `console/scripts/fields_test.py` plus a case in
`console/src/app/screens.test.tsx`. A field has already shipped "declared,
transcoded, served, typechecked and mirrored while no component reads it".

⚠ `route_manifest_test.py` makes any new RPC demand a screen. That is how the
console stays buttonless: approval and acceptance are CLI verbs, not routes.
