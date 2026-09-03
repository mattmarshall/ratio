//! Who is asking, and which books they may open.
//!
//! ⛔ AUTHORIZATION LIVES HERE AND AT `Console::open_book`, NOT AT THE GATEWAY.
//! The API Gateway's JWT authorizer proves a token is real, unexpired and ours;
//! it cannot know which books a person administers. That is a property of the
//! book set, and it is enforced at the one place a book id becomes a `FileBook`,
//! where the test suite can break it. A boundary that lived only in a handler
//! (or only in CloudFormation) would be a boundary one forgotten `FileBook::open`
//! away from being bypassed — the same shape as a config that is read by nobody.
//!
//! ⭐ A BOOK IS THE TENANT, NOT A FUND OR A WORKOS ORG. CreateBook writes an
//! independent journal: no fund, no organization. `MEMBERSHIP.tsv` grants a
//! WorkOS `sub` (or email) that one book. An `org:{id}` line is a separate
//! operator grant, never implied by the creator sitting in an org, and never
//! implied by optional `fund` / `organization` keys on `book.toml`.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use anyhow::{bail, Result};

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
        /// The IdP `sub` — an opaque, stable identifier (WorkOS user id).
        sub: String,
        /// The verified email, used as a human-readable fallback identity and as
        /// an alternate membership key.
        email: String,
        /// Optional WorkOS organization id. Membership may also grant
        /// `org:{organization}` — an org is a tenant around books, not a
        /// required parent of one.
        organization: String,
        /// Legacy Cognito groups claim. Membership is deliberately NOT keyed
        /// on it — see `membership_for`.
        groups: Vec<String>,
    },
}

impl Subject {
    /// The stable identifier recorded as the actor on a write.
    ///
    /// ⛔ THE WORKOS `sub`, NOT A SESSION TOKEN. A NAV is signed once and read
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

/// The subject a request carries, from the API Gateway's verified claims.
///
/// ⛔ THE CLAIMS ARRIVE VERIFIED, AND THE CLIENT CANNOT FORGE THEM. On the
/// deployed surface the request reaches a loopback HTTP server through the AWS
/// Lambda Web Adapter, which serializes the Lambda event's `requestContext` into
/// the `x-amzn-request-context` header. For a route with a JWT authorizer, API
/// Gateway populates `requestContext.authorizer.jwt.claims` only AFTER it has
/// verified the token's signature, issuer, audience and expiry — so the server
/// does no crypto, and this header is synthesized from the event rather than
/// copied from the caller's headers, so a client cannot supply its own claims.
///
/// Returns `None` when there are no verified claims: an unauthenticated request,
/// or a local `ratio watch` with no such header. The caller decides what that
/// means — refuse (deployed) or fall back to `Local` (a developer's machine).
pub fn from_request_context(header: &str) -> Option<Subject> {
    let v: serde_json::Value = serde_json::from_str(header).ok()?;
    let claims = v.get("authorizer")?.get("jwt")?.get("claims")?;
    let claim = |k: &str| claims.get(k).and_then(serde_json::Value::as_str).unwrap_or("").to_string();
    let sub = claim("sub");
    let email = claim("email");
    // A verified context carrying neither a stable id nor an email is not an
    // identity anything here will attribute a figure to.
    if sub.is_empty() && email.is_empty() {
        return None;
    }
    // ⚠ `cognito:groups` serializes as a bracketed, space-joined string
    // (`[a b]`), not a JSON array. Parsed defensively and NOT used for
    // authorization — membership is data (`MEMBERSHIP.tsv`), not a token claim.
    let groups = claims
        .get("cognito:groups")
        .and_then(serde_json::Value::as_str)
        .map(|g| {
            g.trim_matches(|c| c == '[' || c == ']')
                .split([' ', ','])
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let organization = claim("org_id");
    Some(Subject::Member { sub, email, organization, groups })
}

/// What a subject may open, resolved from `<root>/MEMBERSHIP.tsv`.
///
/// ⛔ AN EMPTY GRANT AND AN UNREADABLE FILE ARE DIFFERENT ANSWERS. A missing
/// file is "this operator administers nothing" — `ListBooks` / `ListFunds`
/// return `[]`. A file that exists and cannot be read is a refusal: treating it
/// as empty would hide a broken tenant boundary behind an authorized-looking
/// empty list. `Local` is unrestricted and never consults the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// The CLI, loopback, and the open demo. Every book; not a tenant.
    Unrestricted,
    /// An authenticated member. The set may be empty — that is a real answer.
    Granted(BTreeSet<String>),
    /// `MEMBERSHIP.tsv` existed and could not be read. Not an empty grant.
    Unreadable,
}

impl Scope {
    /// Whether `id` is a book this scope may open.
    ///
    /// `Unreadable` is an error, not `false`: a caller who cannot resolve
    /// membership must not be told "no fund", which is also how a missing book
    /// answers.
    pub fn allows(&self, id: &str) -> Result<bool> {
        match self {
            Scope::Unrestricted => Ok(true),
            Scope::Granted(set) => Ok(set.contains(id)),
            Scope::Unreadable => bail!("membership could not be read"),
        }
    }

    /// Drop ids this scope may not see. `Unreadable` refuses rather than
    /// emptying the list.
    pub fn retain(&self, ids: &mut Vec<String>) -> Result<()> {
        match self {
            Scope::Unrestricted => Ok(()),
            Scope::Granted(set) => {
                ids.retain(|id| set.contains(id));
                Ok(())
            }
            Scope::Unreadable => bail!("membership could not be read"),
        }
    }
}

/// Resolve the caller's scope from identity and `MEMBERSHIP.tsv`.
///
/// `Local` is unrestricted. A `Member` is `Granted` from the file, or
/// `Unreadable` when the file exists and cannot be read.
pub fn scope_for(root: &Path, who: &Subject) -> Scope {
    match who {
        Subject::Local => Scope::Unrestricted,
        member => match membership_for(root, member) {
            Ok(set) => Scope::Granted(set),
            Err(_) => Scope::Unreadable,
        },
    }
}

/// The books `who` may open, read from `<root>/MEMBERSHIP.tsv`.
///
/// Lines are `<subject-id>\t<book-id>`, where `<subject-id>` is matched against
/// the caller's `sub` OR their email — an administrator is provisioned by
/// whichever the operator knows. A missing file grants nothing (`Ok` empty),
/// which is a valid empty answer (`ListBooks` / `ListFunds` return `[]`), NOT
/// an error. A file that exists and cannot be read is `Err`.
///
/// ⚠ TSV, KEYED ON SCALAR CLAIMS. A `cognito:groups` claim serializes in the
/// request context as a bracketed, space-joined string — brittle to parse and
/// capped per user. Keying membership on the scalar `sub`/`email` keeps the
/// grant out of the token's shape, and out of the identity provider entirely: a
/// book's administration agreement is not something to re-express as an IdP
/// group. An `org:{organization}` line is an explicit operator grant, never
/// implied by optional fund/org metadata on the book.
pub fn membership_for(root: &Path, who: &Subject) -> Result<BTreeSet<String>> {
    let (sub, email, org) = match who {
        // `Local` is unrestricted and never consults this file; returning the
        // empty set here would be read as "sees nothing", the exact opposite.
        Subject::Local => return Ok(BTreeSet::new()),
        Subject::Member { sub, email, organization, .. } => {
            (sub.as_str(), email.as_str(), organization.as_str())
        }
    };
    let path = root.join("MEMBERSHIP.tsv");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(e) => bail!("membership could not be read: {e}"),
    };
    Ok(grants_in(&text, sub, email, org))
}

/// Parse grants from TSV text. `funds_for` uses this after a successful read.
fn grants_in(text: &str, sub: &str, email: &str, org: &str) -> BTreeSet<String> {
    let org_key = if org.is_empty() {
        String::new()
    } else {
        format!("org:{org}")
    };
    text.lines()
        .filter_map(|line| {
            let mut it = line.split('\t');
            let holder = it.next()?.trim();
            let fund = it.next()?.trim();
            let matches = !holder.is_empty()
                && (holder == sub
                    || holder == email
                    || (!org_key.is_empty() && holder == org_key));
            (matches && !fund.is_empty()).then(|| fund.to_string())
        })
        .collect()
}

/// The funds `who` may open, ignoring a membership-file read failure.
///
/// ⛔ DO NOT USE THIS TO DECIDE A LIST. A read failure becomes the empty set,
/// which is the authorized-empty / refusal collapse `membership_for` exists
/// to refuse. Kept for call sites that already treated a missing file as
/// empty and do not serve a list.
pub fn funds_for(root: &Path, who: &Subject) -> BTreeSet<String> {
    membership_for(root, who).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(sub: &str, email: &str) -> Subject {
        Subject::Member {
            sub: sub.into(),
            email: email.into(),
            organization: String::new(),
            groups: vec![],
        }
    }

    #[test]
    fn verified_claims_become_a_member_and_the_actor_is_the_stable_sub() {
        let header = r#"{"authorizer":{"jwt":{"claims":{"sub":"abc-123","email":"a@x.test","org_id":"org_01x","cognito:groups":"[admins ops]"}}}}"#;
        let s = from_request_context(header).expect("claims present");
        match &s {
            Subject::Member { sub, email, organization, groups } => {
                assert_eq!(sub.as_str(), "abc-123");
                assert_eq!(email.as_str(), "a@x.test");
                assert_eq!(organization.as_str(), "org_01x");
                // The bracketed, space-joined group string is parsed, but it is
                // NOT what authorization keys on.
                assert_eq!(groups, &vec!["admins".to_string(), "ops".to_string()]);
            }
            Subject::Local => panic!("verified claims must not resolve to Local"),
        }
        // ⛔ THE SUB, NOT THE SESSION. A strike signed under this identity is
        // read for years; the recorded actor is the stable id, not a token.
        assert_eq!(s.actor(), Some("abc-123"));
    }

    #[test]
    fn no_verified_claims_is_none_so_the_caller_decides_what_that_means() {
        // A context with no authorizer (an unauthenticated request, or a local
        // `ratio watch` with no gateway) carries no identity.
        assert!(from_request_context("").is_none());
        assert!(from_request_context("{}").is_none());
        assert!(from_request_context(r#"{"authorizer":{}}"#).is_none());
        // Present-but-empty claims are not an identity either.
        assert!(
            from_request_context(r#"{"authorizer":{"jwt":{"claims":{"sub":"","email":""}}}}"#)
                .is_none()
        );
    }

    #[test]
    fn local_has_no_authenticated_actor() {
        // The CLI records `RATIO_ACTOR`, not a made-up one — `actor()` is `None`
        // rather than a default, because an audit trail that invents an actor is
        // worse than one that admits it does not know.
        assert_eq!(Subject::Local.actor(), None);
    }

    #[test]
    fn membership_matches_on_sub_or_email_and_ignores_everyone_else() {
        let dir = std::env::temp_dir().join("ratio-auth-membership");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("MEMBERSHIP.tsv"),
            // by sub, by email, and a line for a different person that must not
            // leak into this subject's set.
            "abc-123\tashcombe\nbob@x.test\tbellwether\nsomeone-else\tcarrington\n",
        )
        .unwrap();

        let by_sub = funds_for(&dir, &member("abc-123", "unused@x.test"));
        assert!(by_sub.contains("ashcombe") && by_sub.len() == 1);

        let by_email = funds_for(&dir, &member("no-such-sub", "bob@x.test"));
        assert!(by_email.contains("bellwether") && by_email.len() == 1);

        // A subject matching nothing gets an empty set — a valid answer, not an
        // error, and NOT another tenant's funds.
        let none = funds_for(&dir, &member("stranger", "stranger@x.test"));
        assert!(none.is_empty());

        // A missing file is the empty set, not a panic — authorized empty.
        assert!(membership_for(&dir.join("nope"), &member("abc-123", "a@x.test"))
            .unwrap()
            .is_empty());
        assert!(funds_for(&dir.join("nope"), &member("abc-123", "a@x.test")).is_empty());

        // `Local` never consults the file and is unrestricted elsewhere; here it
        // is simply the empty set (unused for `Local`, which bypasses the check).
        assert!(funds_for(&dir, &Subject::Local).is_empty());
        assert_eq!(scope_for(&dir, &Subject::Local), Scope::Unrestricted);
    }

    #[test]
    fn an_unreadable_membership_file_is_a_refusal_not_an_empty_grant() {
        // ⛔ A DIRECTORY NAMED MEMBERSHIP.tsv IS THE READ FAILURE THIS DISTINGUISHES.
        // unwrap_or_default on read_to_string would turn it into [], and ListBooks
        // would look like an authorized empty set.
        let dir = std::env::temp_dir().join("ratio-auth-membership-unreadable");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(dir.join("MEMBERSHIP.tsv")).unwrap();

        let who = member("abc-123", "a@x.test");
        let err = membership_for(&dir, &who).unwrap_err().to_string();
        assert!(
            err.contains("membership could not be read"),
            "a broken membership file must refuse, not look empty: {err}"
        );
        assert_eq!(scope_for(&dir, &who), Scope::Unreadable);
        assert!(Scope::Unreadable.allows("ashcombe").is_err());
        let mut ids = vec!["ashcombe".into()];
        let retain = Scope::Unreadable.retain(&mut ids).unwrap_err().to_string();
        assert!(retain.contains("membership could not be read"), "{retain}");
        assert_eq!(ids, vec!["ashcombe".to_string()], "a refusal must not empty the list");
    }

    #[test]
    fn membership_matches_a_workos_organization_and_not_another() {
        let dir = std::env::temp_dir().join("ratio-auth-org-membership");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("MEMBERSHIP.tsv"), "org:org_01a\tashcombe\norg:org_01b\tbellwether\n")
            .unwrap();

        let in_a = Subject::Member {
            sub: "user_1".into(),
            email: "a@x.test".into(),
            organization: "org_01a".into(),
            groups: vec![],
        };
        let granted = funds_for(&dir, &in_a);
        assert!(granted.contains("ashcombe") && granted.len() == 1);

        let in_none = member("user_1", "a@x.test");
        assert!(funds_for(&dir, &in_none).is_empty());
    }
}
