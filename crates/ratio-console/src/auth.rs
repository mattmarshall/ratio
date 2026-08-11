//! Who is asking, and which funds they may open.
//!
//! ⛔ AUTHORIZATION LIVES HERE AND AT `Console::book_path`, NOT AT THE GATEWAY.
//! The API Gateway's JWT authorizer proves a token is real, unexpired and ours;
//! it cannot know which funds a person administers. That is a property of the
//! book set, and it is enforced at the one place a fund id becomes a path, where
//! the test suite can break it. A boundary that lived only in CloudFormation
//! would be a boundary this crate's tests could not see — and the repository has
//! recorded three separate cases where a "boundary" turned out to be a comment
//! or a naming convention rather than an enforced constraint.

use std::collections::BTreeSet;
use std::path::Path;

/// The identity a request carries, resolved from the gateway's verified claims.
///
/// `Local` is the CLI, `ratio watch --book`, and the MCP transport — a person at
/// a terminal, who sees every book and whose writes are attributed to
/// `RATIO_ACTOR`. It is NOT a tenant, and it is the only variant that reaches a
/// `Console` without having passed the authenticated `/v1` path: the network
/// server builds a `Member`, or refuses the request before a `Console` exists.
#[derive(Clone, Debug)]
pub enum Subject {
    /// The CLI and loopback surfaces. Unrestricted; not a tenant.
    Local,
    /// An authenticated caller, identified by the claims the gateway verified.
    Member {
        /// The Cognito `sub` — an opaque, stable identifier.
        sub: String,
        /// The verified email, used as a human-readable fallback identity and as
        /// an alternate membership key.
        email: String,
        /// The `cognito:groups` claim, carried for future use. Membership is
        /// deliberately NOT keyed on it — see `funds_for`.
        groups: Vec<String>,
    },
}

impl Subject {
    /// The stable identifier recorded as the actor on a write.
    ///
    /// ⛔ THE COGNITO `sub`, NOT A SESSION TOKEN. A NAV is signed once and read
    /// for years; the signature must outlive the session that produced it, so
    /// what is recorded is the subject and the moment — an opaque, stable id
    /// (falling back to the email only if a `sub` claim were ever absent).
    /// `Local` has no authenticated identity; a caller records `RATIO_ACTOR`
    /// instead, which is why this is an `Option` rather than a string with a
    /// made-up default. An audit trail that invents an actor is worse than one
    /// that admits it does not know.
    pub fn actor(&self) -> Option<&str> {
        match self {
            Subject::Local => None,
            Subject::Member { sub, email, .. } => {
                Some(if !sub.is_empty() { sub } else { email })
            }
        }
    }
}

/// The funds `who` may open, read from `<root>/MEMBERSHIP.tsv`.
///
/// Lines are `<subject-id>\t<fund-id>`, where `<subject-id>` is matched against
/// the caller's `sub` OR their email — an administrator is provisioned by
/// whichever the operator knows. A missing file grants nothing, which is a valid
/// empty answer (`ListFunds` returns `[]`), NOT an error: an operator with no
/// funds yet and an operator whose grants failed to load must not look alike,
/// and the file simply not being there is the former.
///
/// ⚠ TSV, KEYED ON SCALAR CLAIMS. A `cognito:groups` claim serializes in the
/// request context as a bracketed, space-joined string — brittle to parse and
/// capped per user. Keying membership on the scalar `sub`/`email` keeps the
/// grant out of the token's shape, and out of the identity provider entirely: a
/// fund's administration agreement is not something to re-express as an IdP
/// group.
pub fn funds_for(root: &Path, who: &Subject) -> BTreeSet<String> {
    let (sub, email) = match who {
        // `Local` is unrestricted and never consults this file; returning the
        // empty set here would be read as "sees nothing", the exact opposite.
        Subject::Local => return BTreeSet::new(),
        Subject::Member { sub, email, .. } => (sub.as_str(), email.as_str()),
    };
    let text = std::fs::read_to_string(root.join("MEMBERSHIP.tsv")).unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let mut it = line.split('\t');
            let holder = it.next()?.trim();
            let fund = it.next()?.trim();
            let matches = !holder.is_empty() && (holder == sub || holder == email);
            (matches && !fund.is_empty()).then(|| fund.to_string())
        })
        .collect()
}
