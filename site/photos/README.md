# photos

Square headshots, one per person, named for the `data-photo` slug on their card
in `team.src.html` — e.g. `corrine-duhamel.jpg`. `build.py` inlines whichever
are present as `data:` URIs and leaves an initials monogram where one is
missing, so the page is never broken by an absent photo.

**They must be inlined, not linked.** An external image URL is blocked by the
Artifact CSP and is failed by `verify.py`. That is why they live here rather
than on a CDN.

⛔ **Do not take these from LinkedIn.** It refuses automated requests (HTTP 999),
and a profile photo is the subject's or the photographer's to license regardless
— ask each person for one they are happy to have published. Around 400×400 is
plenty; the card renders at 56px.
