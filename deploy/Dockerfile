# The demo image: the three screens and the MCP endpoint, one binary, run on
# Lambda behind a Function URL.
#
# The AWS Lambda Web Adapter is what makes this work without a Lambda-shaped
# rewrite. It runs as a Lambda extension, receives the invocation, and forwards
# it to an ordinary HTTP server on 127.0.0.1 inside the container — so
# `ratio watch` runs exactly as it does on a laptop. That matters beyond
# convenience: the thing demoed is the thing that ships, not a port of it.
#
# `ratio watch` binds loopback only. Under the adapter that is correct rather
# than limiting: the only client is the adapter in the same container.
FROM public.ecr.aws/awsguru/aws-lambda-adapter:0.9.1 AS adapter

# ubuntu:24.04, matched deliberately to the `ubuntu-24.04` runner that builds
# the binary. Bazel links `ratio` against the runner's glibc; a smaller base on
# an older glibc (debian:bookworm-slim is 2.36 against the runner's 2.39) fails
# at start with a symbol-version error that reads like a corrupt image. Both
# are pinned so the pair moves together or not at all.
FROM ubuntu:24.04

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY --from=adapter /lambda-adapter /opt/extensions/lambda-adapter

# Built by Bazel on the runner and passed in — see .github/workflows/deploy.yml.
COPY ratio /usr/local/bin/ratio
COPY demo-book /opt/demo-book
COPY demo-funds /opt/demo-funds
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/ratio /usr/local/bin/entrypoint.sh

# Fail the build on an empty demo book rather than shipping one, and say what
# was actually there when it does.
#
# This is not hypothetical: a .dockerignore that re-included `demo-book` but
# not its contents produced exactly that, twice. The image built, the container
# started, the adapter reported ready, cold start was 95ms, and the demo served
# a blank ledger. Every signal was green. An assertion here turns a blank
# screen in front of a customer into a red build — and printing the tree means
# the next failure names itself instead of needing a bisect.
#
# ⚠ THE FUND COUNT IS EXACT, NOT A MINIMUM, AND THAT IS THE POINT — it has to be
# edited deliberately when the demo changes. Adding a fourth fund turned this
# red, which is the assertion working: the alternative is `-ge 3`, which would
# have gone on passing while a fund silently vanished from the image.
#
# ⚠ AND THE ENTRY COUNT IS HERE FOR THE SAME REASON AS EVERYTHING ELSE ABOVE.
# `//deploy:seed_test` asserts the seed SCRIPT produces a fund with lots; this
# asserts the IMAGE contains one. The two are different failures — a correct
# script whose output never reached the image is exactly the blank-ledger bug
# this block was written for.
#
# ⛔ THE BLOCKED FUND MUST HAVE NO `NAVS`, AND THAT IS NOT A TYPO. This used to
# assert the opposite, because the seeder used to declare harbourline blocked
# and then strike it anyway. `ratio strike` now refuses a fund with an
# unexplained blocking break or an unresolved fact, so the image asserts the
# refusal held — and asserts, on the sixth fund, that explaining the break lets
# the NAV through. A gate nobody has watched open is indistinguishable from one
# that is stuck. `deploy/seed_test.sh` makes the same pair of assertions against
# the script; these two files have to move together.
#
# ⛔ THE EIGHTH FUND IS THE AVERAGE-COST POOL, AND THE COUNT ALONE CANNOT
# SAY SO. A slot that is not kestrel, or a kestrel that wrote no
# `average_cost = true`, is the same silence #9 already refused as a
# Method. `deploy/seed_test.sh` asserts the election against the script;
# this asserts the IMAGE carries the cite. `lot_method = "average_cost"`
# stays refused — this is `average_cost = true`, not a Method, and
# min-tax must stay unset on the same blob.
#
# ⚠ NOTHING ON A PULL REQUEST RUNS THIS FILE. `deploy.yml` fires on push to
# main, so a stale assertion here is invisible until it is merged and the deploy
# goes red. Edit it in the commit that changes the seeder, not afterwards.
RUN set -eu; \
    echo "== /opt/demo-book =="; \
    find /opt/demo-book -maxdepth 2 -print | sed 's|^|  |' || true; \
    for f in accounts.json journal.jsonl; do \
      test -s "/opt/demo-book/$f" || { echo "FAIL: $f is missing or empty"; exit 1; }; \
    done; \
    for d in reports proposals config; do \
      test -d "/opt/demo-book/$d" || { echo "FAIL: $d/ is missing"; exit 1; }; \
    done; \
    test -n "$(ls -A /opt/demo-book/reports)" || { echo "FAIL: no break report"; exit 1; }; \
    test -n "$(ls -A /opt/demo-book/proposals)" || { echo "FAIL: no pending proposal"; exit 1; }; \
    echo "demo book ok: $(find /opt/demo-book -type f | wc -l) file(s)"; \
    echo "== /opt/demo-funds =="; \
    ls -1 /opt/demo-funds | sed 's|^|  |'; \
    test "$(ls -1 /opt/demo-funds | wc -l)" -eq 8 || { echo "FAIL: expected 8 funds"; exit 1; }; \
    for f in /opt/demo-funds/*/; do \
      test -s "$f/accounts.json" || { echo "FAIL: $f has no chart"; exit 1; }; \
    done; \
    test ! -s /opt/demo-funds/harbourline-global-value/NAVS || { echo "FAIL: a blocked fund struck a NAV"; exit 1; }; \
    test -s /opt/demo-funds/pennington-select-income/NAVS || { echo "FAIL: the explained fund has no NAV — the gate did not open"; exit 1; }; \
    test -s /opt/demo-funds/pennington-select-income/explanations.jsonl || { echo "FAIL: no explanation on the explained fund"; exit 1; }; \
    test -s /opt/demo-funds/marlowe-dual-basis/NAVS || { echo "FAIL: the dual-basis fund has no NAV"; exit 1; }; \
    test "$(wc -l < /opt/demo-funds/marlowe-dual-basis/NAVS)" -ge 2 || { echo "FAIL: the dual-basis fund struck fewer than two views"; exit 1; }; \
    ashcombe=$(wc -l < /opt/demo-funds/ashcombe-global-equity/journal.jsonl); \
    test "$ashcombe" -gt 1000 || { echo "FAIL: the generated fund has $ashcombe entries — the demo shows no scale"; exit 1; }; \
    test -d /opt/demo-funds/kestrel-pooled-basis || { echo "FAIL: kestrel is missing"; exit 1; }; \
    test -s /opt/demo-funds/kestrel-pooled-basis/config/ACTIVE || { echo "FAIL: kestrel has no ACTIVE config"; exit 1; }; \
    kestrel_active=$(cat /opt/demo-funds/kestrel-pooled-basis/config/ACTIVE); \
    grep -q "average_cost = true" "/opt/demo-funds/kestrel-pooled-basis/config/$kestrel_active" \
      || { echo "FAIL: kestrel elects no average-cost pool — the walk-through cannot cite one"; exit 1; }; \
    grep -q "min_tax_short_weight" "/opt/demo-funds/kestrel-pooled-basis/config/$kestrel_active" \
      && { echo "FAIL: kestrel wrote min-tax — two elections for one sale"; exit 1; }; \
    echo "demo funds ok: 8, kestrel elects the pool, and $ashcombe entries on the generated one"

# The adapter polls this until the server answers, so a cold start does not
# race the first request.
ENV AWS_LWA_PORT=8080 \
    AWS_LWA_READINESS_CHECK_PATH=/healthz \
    RATIO_BOOK=/tmp/book

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
