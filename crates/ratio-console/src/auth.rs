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
    ///
    /// ⚠ A WORKOS CONNECT ACCESS TOKEN IS THIS VARIANT WITH `connect: true`.
    /// The catalog is `docs/connect-scopes.md` (#150). Frozen names in
    /// `scopes` open the matching `/v1` door after membership. A Connect
    /// token never takes `RATIO_DEMO_OPEN` and never matches `org:{id}`
    /// (#151). Do not treat a session JWT as a Connect grant, and do not
    /// mint `rules:approve` or `config:promote`.
    Member {
        /// The IdP `sub` — an opaque, stable identifier (WorkOS user id).
        sub: String,
        /// The verified email, used as a human-readable fallback identity and as
        /// an alternate membership key.
        email: String,
        /// Optional WorkOS organization id. Membership may also grant
        /// `org:{organization}` — an org is a tenant around books, not a
        /// required parent of one. A Connect token never matches that line.
        organization: String,
        /// Legacy Cognito groups claim. Membership is deliberately NOT keyed
        /// on it — see `membership_for`.
        groups: Vec<String>,
        /// True when the verified claims look like a WorkOS Connect (OAuth /
        /// M2M) token rather than an AuthKit session. The actor is still
        /// `sub`. The grant is still membership. The open demo is not.
        connect: bool,
        /// Frozen catalog scopes this Connect token carries. Empty on an
        /// AuthKit session. Aliases and hard non-scopes never land here.
        scopes: BTreeSet<String>,
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

    /// Whether this subject is a Connect token, not an AuthKit session.
    pub fn is_connect(&self) -> bool {
        matches!(self, Subject::Member { connect: true, .. })
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
    let connect = is_connect_claims(claims, &claim);
    let scopes = if connect {
        catalog_grants(&claim("scope"))
    } else {
        BTreeSet::new()
    };
    Some(Subject::Member { sub, email, organization, groups, connect, scopes })
}

/// Whether verified JWT claims look like a WorkOS Connect token.
///
/// ⛔ AUTHKIT SESSION TOKENS ARE NOT CONNECT. They carry `sid` and a
/// `client_id` equal to this deployment's AuthKit app. Connect OAuth / M2M
/// tokens carry an OAuth `scope` or `azp`, or a `client_id` that is not
/// this app. The API Gateway audience still rejects most of those today;
/// this is the in-process fence for when one arrives, so
/// `RATIO_DEMO_OPEN` cannot become a book-ACL bypass.
fn is_connect_claims(claims: &serde_json::Value, claim: &dyn Fn(&str) -> String) -> bool {
    if claims.get("azp").and_then(serde_json::Value::as_str).is_some_and(|s| !s.is_empty()) {
        return true;
    }
    if claims.get("scope").and_then(serde_json::Value::as_str).is_some_and(|s| !s.is_empty()) {
        return true;
    }
    let client_id = claim("client_id");
    let ours = std::env::var("RATIO_WORKOS_CLIENT_ID").unwrap_or_default();
    !ours.is_empty() && !client_id.is_empty() && client_id != ours
}

/// Frozen grantable scopes from `docs/connect-scopes.md`. A string that is
/// not in this list is not a grant — including aliases and hard non-scopes.
pub const FROZEN_SCOPES: &[&str] = &[
    "books:read",
    "books:write",
    "books:ingest",
    "journals:read",
    "journals:post",
    "statements:read",
    "views:read",
    "positions:read",
    "lots:read",
    "lots:elect",
    "nav:read",
    "nav:strike",
    "partners:read",
    "partners:write",
    "capital:read",
    "commits:read",
    "calls:post",
    "fees:read",
    "fees:accrue",
    "budget:read",
    "billing:read",
    "breaks:read",
    "breaks:explain",
    "closes:read",
    "config:read",
    "audit:export",
    "deliveries:write",
    "facts:admit",
    "webhooks:journal",
];

/// Named so they stop being tempting. Absence is the fence.
pub const HARD_NON_SCOPES: &[&str] = &[
    "rules:approve",
    "config:promote",
    "portal:impersonate",
    "impersonate",
    "payments:initiate",
];

/// Near-misses the catalog refuses. Granting these would be two names for
/// one door.
pub const ALIAS_SCOPES: &[&str] = &[
    "journal:read",
    "journal:append",
    "projects:budget:read",
    "projects:billing:read",
];

/// Catalog scopes a `scope` claim actually grants.
///
/// ⛔ ALIASES AND HARD NON-SCOPES NEVER ENTER THE SET. `journal:read` is
/// not `journals:read`. `rules:approve` is not a permission check that
/// could later be relaxed. OIDC discovery scopes (`openid`, `email`)
/// are ignored — they are not doors.
pub fn catalog_grants(scope_claim: &str) -> BTreeSet<String> {
    scope_claim
        .split_whitespace()
        .filter(|s| FROZEN_SCOPES.contains(s))
        .map(|s| s.to_string())
        .collect()
}

/// Whether `held` contains a frozen name. Hard non-scopes and aliases
/// answer false even if the caller listed them.
pub fn holds_scope(held: &BTreeSet<String>, scope: &str) -> bool {
    FROZEN_SCOPES.contains(&scope) && held.contains(scope)
}

/// The Connect grant a `/v1` route needs. `None` is "this path is not a
/// catalog door" — `transcode::serve` still 404s it. AuthKit sessions
/// skip this table; membership is their grant.
///
/// Any one of the returned names is enough. Write scopes that name a
/// template (`journals:post`, `calls:post`, `fees:accrue`, `lots:elect`)
/// share `ApplyEvent`; they are a tighter grant of the same verb, not
/// a second RPC.
pub fn required_connect_scopes(method: &str, path: &str) -> Option<&'static [&'static str]> {
    let rest = path.strip_prefix("/v1/")?.trim_start_matches('/');
    if method == "POST" {
        return if rest == "books" {
            Some(&["books:write"])
        } else if rest.ends_with(":applyEvent") {
            Some(&["journals:post", "calls:post", "fees:accrue", "lots:elect"])
        } else if rest.ends_with(":ingest") {
            Some(&["books:ingest", "deliveries:write"])
        } else if rest.ends_with(":admit") {
            Some(&["facts:admit"])
        } else if rest.ends_with(":mark") {
            Some(&["breaks:explain"])
        } else {
            None
        };
    }
    if method != "GET" {
        return None;
    }
    let segs: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    Some(match segs.as_slice() {
        ["books"] | ["books", _] | ["funds"] | ["funds", _] => &["books:read"],
        ["funds", _, "views"] => &["views:read"],
        ["funds", _, "views", v] if v.ends_with(":reconcile") => &["views:read"],
        ["funds", _, "views", v] if v.ends_with(":projectProgress") => {
            &["budget:read", "billing:read"]
        }
        ["funds", _, "views", v] if v.ends_with(":operatingAging") => &["statements:read"],
        ["funds", _, "views", _] => &["views:read"],
        ["funds", _, "views", _, "breaks"] | ["funds", _, "views", _, "breaks", _] => {
            &["breaks:read"]
        }
        ["funds", _, "changeLogEntries"] | ["funds", _, "changeLogEntries", _] => {
            &["audit:export"]
        }
        ["funds", _, "configVersions"] | ["funds", _, "configVersions", _] => &["config:read"],
        ["funds", _, "deliveries"] | ["funds", _, "deliveries", _] => &["books:read"],
        ["funds", _, "pendingFacts"]
        | ["funds", _, "pendingFacts", _]
        | ["funds", _, "facts"]
        | ["funds", _, "facts", _] => &["books:read"],
        ["funds", _, "views", _, "accounts", ..] => &["statements:read"],
        ["funds", _, "corporateActions"] | ["funds", _, "corporateActions", _] => {
            &["lots:read"]
        }
        ["funds", _, "views", _, "navStrikes"] | ["funds", _, "views", _, "navStrikes", _] => {
            &["nav:read"]
        }
        ["funds", _, "views", _, "periodCloses"]
        | ["funds", _, "views", _, "periodCloses", _] => &["closes:read"],
        ["funds", _, "views", _, "positions", _, "lots"]
        | ["funds", _, "views", _, "positions", _, "lots", _] => &["lots:read"],
        ["funds", _, "views", _, "positions"] | ["funds", _, "views", _, "positions", _] => {
            &["positions:read"]
        }
        ["funds", _, "templates"]
        | ["funds", _, "templates", _]
        | ["funds", _, "rules"]
        | ["funds", _, "rules", _] => &["config:read"],
        ["funds", _, "entries"] | ["funds", _, "entries", _] => &["journals:read"],
        _ => return None,
    })
}

/// Enforce catalog scopes on a Connect token.
///
/// `None` grants are an AuthKit session or `Local` — membership is the
/// door, not an OAuth scope. A Connect token with an empty grant set
/// is authorized-empty for every route: silence is not "all scopes".
pub fn authorize_connect(
    grants: Option<&BTreeSet<String>>,
    method: &str,
    path: &str,
) -> Result<()> {
    let Some(held) = grants else {
        return Ok(());
    };
    let Some(need) = required_connect_scopes(method, path) else {
        return Ok(());
    };
    if need.iter().any(|scope| holds_scope(held, scope)) {
        return Ok(());
    }
    bail!("scope `{need}` is required", need = need[0])
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
    let (sub, email, org, connect) = match who {
        // `Local` is unrestricted and never consults this file; returning the
        // empty set here would be read as "sees nothing", the exact opposite.
        Subject::Local => return Ok(BTreeSet::new()),
        Subject::Member { sub, email, organization, connect, .. } => {
            (sub.as_str(), email.as_str(), organization.as_str(), *connect)
        }
    };
    let path = root.join("MEMBERSHIP.tsv");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(e) => bail!("membership could not be read: {e}"),
    };
    Ok(grants_in(&text, sub, email, org, connect))
}

/// Parse grants from TSV text. `funds_for` uses this after a successful read.
///
/// ⛔ A CONNECT TOKEN NEVER MATCHES `org:{id}`. That line is an operator
/// grant for an AuthKit session sitting in the org, not a third-party app
/// inheriting every book the org administers. Connect matches `sub` / email
/// only — unset org access, not an implied org.
fn grants_in(text: &str, sub: &str, email: &str, org: &str, connect: bool) -> BTreeSet<String> {
    let org_key = if connect || org.is_empty() {
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
            connect: false,
            scopes: BTreeSet::new(),
        }
    }

    fn connect_member(sub: &str, email: &str, org: &str) -> Subject {
        connect_with_scopes(sub, email, org, &[])
    }

    fn connect_with_scopes(sub: &str, email: &str, org: &str, scopes: &[&str]) -> Subject {
        Subject::Member {
            sub: sub.into(),
            email: email.into(),
            organization: org.into(),
            groups: vec![],
            connect: true,
            scopes: catalog_grants(&scopes.join(" ")),
        }
    }

    #[test]
    fn verified_claims_become_a_member_and_the_actor_is_the_stable_sub() {
        let header = r#"{"authorizer":{"jwt":{"claims":{"sub":"abc-123","email":"a@x.test","org_id":"org_01x","cognito:groups":"[admins ops]"}}}}"#;
        let s = from_request_context(header).expect("claims present");
        match &s {
            Subject::Member { sub, email, organization, groups, connect, scopes } => {
                assert_eq!(sub.as_str(), "abc-123");
                assert_eq!(email.as_str(), "a@x.test");
                assert_eq!(organization.as_str(), "org_01x");
                // The bracketed, space-joined group string is parsed, but it is
                // NOT what authorization keys on.
                assert_eq!(groups, &vec!["admins".to_string(), "ops".to_string()]);
                assert!(
                    !connect,
                    "an AuthKit-shaped session (no azp/scope) is not a Connect token"
                );
                assert!(scopes.is_empty(), "an AuthKit session carries no Connect grants");
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
            connect: false,
            scopes: BTreeSet::new(),
        };
        let granted = funds_for(&dir, &in_a);
        assert!(granted.contains("ashcombe") && granted.len() == 1);

        let in_none = member("user_1", "a@x.test");
        assert!(funds_for(&dir, &in_none).is_empty());
    }

    #[test]
    fn a_connect_token_is_detected_from_azp_or_scope_and_is_not_an_authkit_session() {
        let azp = r#"{"authorizer":{"jwt":{"claims":{"sub":"user_c","email":"c@x.test","azp":"client_connect_app"}}}}"#;
        let s = from_request_context(azp).expect("claims present");
        assert!(s.is_connect(), "azp marks a Connect token");
        assert_eq!(s.actor(), Some("user_c"));

        let scope = r#"{"authorizer":{"jwt":{"claims":{"sub":"user_c","email":"c@x.test","scope":"books:read"}}}}"#;
        assert!(from_request_context(scope).expect("scope").is_connect());

        // AuthKit session shape: sub, sid, client_id, no azp/scope.
        let session = r#"{"authorizer":{"jwt":{"claims":{"sub":"user_a","email":"a@x.test","sid":"session_1","client_id":"client_authkit"}}}}"#;
        assert!(
            !from_request_context(session).expect("session").is_connect(),
            "an AuthKit session is not a Connect token"
        );
    }

    #[test]
    fn a_connect_token_does_not_inherit_an_org_grant() {
        // ⛔ IMPLIED-ORG IS THE BYPASS. An AuthKit operator in org_01a may
        // hold `org:org_01a`. A Connect token carrying the same org_id must
        // not see those books — membership is the subject's `sub`, not the
        // org the token happened to name.
        let dir = std::env::temp_dir().join("ratio-auth-connect-no-org");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("MEMBERSHIP.tsv"),
            "org:org_01a\tashcombe\nuser_c\tmine\n",
        )
        .unwrap();

        let connect = connect_member("user_c", "c@x.test", "org_01a");
        let granted = funds_for(&dir, &connect);
        assert!(
            granted.contains("mine") && granted.len() == 1,
            "Connect matches its sub and not the org: {granted:?}"
        );

        let stranger = connect_member("user_other", "o@x.test", "org_01a");
        assert!(
            funds_for(&dir, &stranger).is_empty(),
            "a Connect token with only an org_id sees authorized-empty"
        );
    }

    #[test]
    fn a_connect_token_grants_only_frozen_catalog_scopes() {
        let header = r#"{"authorizer":{"jwt":{"claims":{"sub":"user_c","email":"c@x.test","scope":"books:read audit:export journal:read rules:approve openid"}}}}"#;
        let s = from_request_context(header).expect("claims present");
        match &s {
            Subject::Member { connect, scopes, .. } => {
                assert!(*connect);
                assert!(scopes.contains("books:read") && scopes.contains("audit:export"));
                assert_eq!(scopes.len(), 2, "alias, hard non-scope, and openid must not grant: {scopes:?}");
            }
            Subject::Local => panic!("Connect claims must not resolve to Local"),
        }
        assert!(holds_scope(
            match &s {
                Subject::Member { scopes, .. } => scopes,
                Subject::Local => panic!("member"),
            },
            "books:read"
        ));
        assert!(
            !holds_scope(&catalog_grants("journal:read journal:append"), "journals:read"),
            "an alias is not the canonical grant"
        );
        assert!(
            !holds_scope(&catalog_grants("rules:approve config:promote impersonate"), "rules:approve"),
            "a hard non-scope is not a grant"
        );
    }

    #[test]
    fn a_connect_token_is_accepted_with_the_matching_scope_and_refused_without() {
        let books = catalog_grants("books:read");
        authorize_connect(Some(&books), "GET", "/v1/books").expect("books:read opens ListBooks");
        authorize_connect(Some(&books), "GET", "/v1/books/alpha").expect("books:read opens GetBook");
        authorize_connect(Some(&books), "GET", "/v1/funds/alpha")
            .expect("books:read opens GetFund");
        let missing = authorize_connect(Some(&books), "GET", "/v1/funds/alpha/entries")
            .unwrap_err()
            .to_string();
        assert!(
            missing.contains("scope `journals:read` is required"),
            "a books:read token must not read the journal: {missing}"
        );

        let none = BTreeSet::new();
        let empty = authorize_connect(Some(&none), "GET", "/v1/books")
            .unwrap_err()
            .to_string();
        assert!(
            empty.contains("scope `books:read` is required"),
            "silence is not every scope: {empty}"
        );

        // AuthKit / Local skip the table.
        authorize_connect(None, "GET", "/v1/funds/alpha/entries")
            .expect("an AuthKit session is not a Connect grant");
    }

    #[test]
    fn hard_non_scopes_and_aliases_do_not_open_a_door() {
        let poison = catalog_grants(
            "rules:approve config:promote portal:impersonate impersonate payments:initiate journal:read journal:append",
        );
        assert!(poison.is_empty(), "none of those strings are grants: {poison:?}");
        let err = authorize_connect(Some(&poison), "GET", "/v1/books")
            .unwrap_err()
            .to_string();
        assert!(err.contains("scope `books:read` is required"), "{err}");

        let alias_journal = catalog_grants("journal:read");
        let journal = authorize_connect(
            Some(&alias_journal),
            "GET",
            "/v1/funds/alpha/entries",
        )
        .unwrap_err()
        .to_string();
        assert!(
            journal.contains("scope `journals:read` is required"),
            "journal:read is an alias: {journal}"
        );
    }

    #[test]
    fn write_routes_need_the_named_write_scope() {
        let read = catalog_grants("books:read journals:read");
        let post = authorize_connect(
            Some(&read),
            "POST",
            "/v1/funds/alpha:applyEvent",
        )
        .unwrap_err()
        .to_string();
        assert!(
            post.contains("scope `journals:post` is required"),
            "read does not imply write: {post}"
        );
        authorize_connect(
            Some(&catalog_grants("journals:post")),
            "POST",
            "/v1/funds/alpha:applyEvent",
        )
        .expect("journals:post opens ApplyEvent");
        authorize_connect(
            Some(&catalog_grants("calls:post")),
            "POST",
            "/v1/funds/alpha:applyEvent",
        )
        .expect("calls:post is a tighter grant of the same verb");
        authorize_connect(
            Some(&catalog_grants("books:write")),
            "POST",
            "/v1/books",
        )
        .expect("books:write opens CreateBook");
        let create = authorize_connect(Some(&read), "POST", "/v1/books")
            .unwrap_err()
            .to_string();
        assert!(create.contains("scope `books:write` is required"), "{create}");
    }
}
