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
        /// on it — see `funds_for`.
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
    let (sub, email, org) = match who {
        // `Local` is unrestricted and never consults this file; returning the
        // empty set here would be read as "sees nothing", the exact opposite.
        Subject::Local => return BTreeSet::new(),
        Subject::Member { sub, email, organization, .. } => {
            (sub.as_str(), email.as_str(), organization.as_str())
        }
    };
    let org_key = if org.is_empty() {
        String::new()
    } else {
        format!("org:{org}")
    };
    let text = std::fs::read_to_string(root.join("MEMBERSHIP.tsv")).unwrap_or_default();
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

        // A missing file is the empty set, not a panic.
        assert!(funds_for(&dir.join("nope"), &member("abc-123", "a@x.test")).is_empty());

        // `Local` never consults the file and is unrestricted elsewhere; here it
        // is simply the empty set (unused for `Local`, which bypasses the check).
        assert!(funds_for(&dir, &Subject::Local).is_empty());
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
