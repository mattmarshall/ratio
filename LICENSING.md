# Licensing

**Ratio is licensed under the GNU Affero General Public License, version 3**
([LICENSE](LICENSE)), **and is also available under a commercial license.**

Copyright © 2025–2026 Matt Marshall.

## Why copyleft

Two reasons, and they pull in the same direction.

**Verification.** The argument for Ratio is that a figure can be checked — the
proofs are published, the replay is deterministic, and a client, an auditor or a
regulator can re-derive any number from the journal prefix and configuration
digest that produced it. That argument is much weaker if the code they are
invited to check is not code they can read. The AGPL guarantees they can.

**Nobody takes the work private.** Under §13, anyone who lets users interact
with a modified Ratio over a network has to offer those users the source of
their modifications. A fix to the lot engine or the conservation kernel made by
someone operating this software comes back; it does not become somebody's
private advantage over the people relying on it to be correct.

## What this means for you

**Reading, auditing, re-deriving a figure** — no obligations at all. Read the
proofs, run `bazel test //...`, replay a NAV strike, check the arithmetic. The
license is only engaged by conveying the software or by operating it for others.

**Running Ratio internally on your own books** — the AGPL permits this. If you
modify it and your users interact with it over a network, §13 applies to those
users.

**Building a product or a service on Ratio** — the AGPL applies to the whole
combined work, and §13 reaches your users over the network. If that is
incompatible with how you need to ship, take the commercial license instead.

**The hosted service** is the path with none of this on it. It is operated by us,
the obligations are ours, and you get the verification argument without taking
on a copyleft obligation of your own.

For a commercial license: <mateomm@gmail.com>.

## ⚠ Two things stated plainly

**AGPL-3.0 is open source.** It is OSI-approved and FSF-approved free software.
Ratio's earlier positioning described it as "source-available, not open source",
and that description does not survive this choice — the accurate phrase is *open
source under a strong network copyleft, dual licensed commercially*. What changed
is not openness but reciprocity: MIT let anyone take the work private, and this
does not.

**The AGPL does not stop a competitor operating Ratio as a service.** It requires
them to publish their modifications; it does not forbid the service. A license
that actually forbids it — BUSL-1.1, Elastic v2 — is not copyleft and is not open
source, and would trade the verification argument for the exclusion. That trade
has not been made. If exclusivity of operation later matters more than openness,
that is a different license and a deliberate decision, not a wording change.

## iOS distribution and contribution policy

On October 24, 2026, the owner approved private, all-rights-reserved source
for the separate `mattmarshall/ratio-ios` shell and prohibited outside
contributions to both repositories. Ratio's existing AGPL/commercial licenses
and third-party licenses remain in force. The contribution policy does not
restrict rights already granted under those licenses.

An App Store or TestFlight build linking Ratio depends on the copyright
holder's applicable commercial permission and a separate distribution
decision. Private visibility and a successful build do not grant distribution
approval. The shell retains an exact reviewed kernel pin; this policy record
does not change its pinned code or authorize TestFlight.

Outside contributions are not accepted in Ratio or the iOS shell. Do not merge
an outside pull request or import an outside patch, including through an
owner-authored PR. Before changing that policy, the owner must settle the
rights instrument and enforce the new merge gate. No CLA or assignment is
implied while outside contributions are prohibited. Existing third-party
components retain their separately granted rights.
