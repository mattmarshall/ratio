# Durable journal startup

`ratio watch` binds and accepts HTTP while journal storage initializes.
Startup readiness applies to every `/v1/` RPC, public book JSON endpoint,
terminal, chat, and HTTP MCP request. The scale-run service uses its own
store; its job controls and reports do not open this book.

| Startup state | Book routes | `/healthz`, `/version`, `/authconfig.json` |
|---|---|---|
| Pending | Wait up to 200 ms, then 503 with `Retry-After: 2` and `the journal is still hydrating` | Available |
| Ready | Normal authorization and book handling | Available |
| Failed with a configured durable store | 503 with `journal startup failed; correct the storage configuration and restart`; no `Retry-After` | Available |

An unauthenticated `/v1/` request in required-auth mode still gets 401
without waiting for storage. Health reports that HTTP is alive, not that
book reads or writes are ready. Static public pages can load while their
book-data requests are unavailable.

Both nonempty `RATIO_JOURNAL_BUCKET` and nonempty `RATIO_JOURNAL_LOCAL`
select durable storage; the bucket takes precedence when both are set.
Installing that store must succeed before the first `FileBook::open`.
A failed installation cannot select the local-file fallback. A failed
book open or hydration cannot release requests to try opening again.

Failure is terminal for that process. Consult the server's detailed
startup error, correct the configured store's credentials, access,
location, or failed book data as appropriate, and restart `ratio watch`
(replace the failed runtime for a deployment). There is no in-process
retry or reset of a failed gate. Do not unset a durable-store setting to
clear the error: that deliberately selects different, local storage.
Clients receive the stable failure sentence, not private store details.

When both durable-store settings are unset or empty, local-only mode
remains supported: reads and writes use `journal.jsonl`. A bad local
book path causes `watch` to exit after startup fails.

This boundary addresses [#293](https://github.com/mattmarshall/ratio/issues/293).
It does not establish complete cold-start recovery of book metadata.
Metadata durability remains on [#291](https://github.com/mattmarshall/ratio/issues/291),
and the operational recovery drill remains on
[#264](https://github.com/mattmarshall/ratio/issues/264).

`//crates/ratio:ratio_test` injects both installation and hydration failures,
exercises HTTP reads and a valid `applyEvent` write against a local book,
and checks that the journal is unchanged. The identical write succeeds
after an explicit local startup. Every current console route and every
public book transport is checked for startup refusal. The former ignored
installation error and failed-as-ready behavior must each make the
corresponding regression fail.
