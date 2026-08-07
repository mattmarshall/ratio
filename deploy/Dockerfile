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
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/ratio /usr/local/bin/entrypoint.sh

# The adapter polls this until the server answers, so a cold start does not
# race the first request.
ENV AWS_LWA_PORT=8080 \
    AWS_LWA_READINESS_CHECK_PATH=/healthz \
    RATIO_BOOK=/tmp/book

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
