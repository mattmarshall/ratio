# ECB reference rates

A first-party WorkOS Connect app for Personal books. It reads daily exchange
rate observations from the European Central Bank Data Portal, derives each
declared foreign currency's value in the book's declared reporting base, and
delivers a non-posting `ecb-reference-rates` CSV through `ConnectApiUrl`.

The ECB series reports currency units per euro. Cross rates use Python
`Decimal`, then quantize once to Ratio's existing two-decimal rate-fact shape
with `ROUND_HALF_EVEN`. Each cited delivery retains both raw ECB observations,
the requested date, the source base, and the normalized factor. This is the
explicit approximation boundary: Ratio's existing rate facts are hundredths.

Scopes are `books:read`, `books:ingest`, and `facts:admit`. There is no
`journals:post`; a rate changes a translated read only after it is recorded on
the fact plane. Membership remains enforced by Ratio.

## Refusals

- A book that is not Personal, lacks a reporting base, or names no currencies.
- An undeclared base or malformed currency code.
- A future requested day, absent/duplicate observation, mismatched day,
  mismatched ECB denomination, malformed CSV, or nonpositive decimal.
- A missing fact for any declared foreign currency.
- DemoUrl in place of ConnectApiUrl, a missing verified token, or missing
  membership through the shared grant path.

## Live activation

The adapter and provider contract are tested with retained ECB-shaped bytes.
WorkOS dashboard registration, a live OAuth grant, and a live observation
against a permitted Personal book remain operator evidence on #22 and #178.
No credential belongs in this repository.
