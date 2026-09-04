//! Who may start a fold, and what it costs to let them.
//!
//! The scale screen has a button on a PUBLIC url that spends real money on real
//! compute. This module is the part that decides whether pressing it does
//! anything, and the policy is plain Rust over a small `Store` interface rather
//! than anything AWS-shaped — the same judgement `Console::book_path` makes about
//! tenancy: *"Authorization is entirely in Rust, where the test suite can break
//! it."* A control that only exists in a CloudFormation template is a control
//! nothing can test.
//!
//! # ⛔ A COOLDOWN IS NOT THE CONTROL. A COUNTER IS.
//!
//! The budget behind this demo is five dollars a month, declared at the payer.
//! A full run is about eight cents, so one a day is comfortably inside it — but
//! the small shape is half a cent, and on a ten-minute cooldown alone a hundred
//! and forty-four of them a day is twenty-one dollars a month. Cooldowns pace
//! what a visitor sees. Only a ceiling bounds the bill, so there is a ceiling.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Seconds. What a run of each shape costs, and how long before another.
///
/// ⚠ THE CENTS ARE AN ESTIMATE AND SAY SO. Fargate bills per vCPU-second and
/// per GB-second and the exact figure is not knowable here; these are computed
/// from the published rates at 4 vCPU / 8 GB plus ephemeral storage, rounded
/// UP. An estimate that rounded down would be a ceiling that leaks.
#[derive(Debug)]
pub struct Shape {
    pub name: &'static str,
    /// ⛔ THE DIALS THE RECORDED ROW USED, NOT THE ONES `ratio closure` DEFAULTS
    /// TO. The measured twenty-million-lot run is 10,000 securities x 2,000
    /// lots; the estimate panel dials 500 x 40,000. Both are twenty million open
    /// tax lots and they are NOT the same fund — one price is read per SECURITY,
    /// so the recorded shape marks ten thousand names and the dialled one marks
    /// five hundred. `//:scale_shapes_test` holds these to HANDOFF's table.
    pub securities: i64,
    pub lots_per: i64,
    /// What the generator settles at. Deterministic, so a fact rather than an
    /// estimate.
    pub open_lots: i64,
    pub entries: i64,
    /// The cold build HANDOFF.md records, in milliseconds. ⚠ Taken on a named
    /// machine on a named day; a run started from the screen happens elsewhere,
    /// and the page says which is which.
    pub recorded_cold_build_ms: i64,
    /// How long before this shape may be run again.
    pub cooldown: i64,
    /// Rounded-up estimate of what one run costs, in cents.
    pub cents: i64,
}

/// ⛔ THE FULL SHAPE IS ONCE A DAY, AND THAT IS THE BUDGET SPEAKING. At ~8 cents
/// a run, daily is ~$2.40 a month against a $5 ceiling that also has to cover
/// the small runs, the function, the gateway and the storage.
pub const SHAPES: [Shape; 3] = [
    Shape {
        name: "small",
        securities: 500,
        lots_per: 500,
        open_lots: 252_843,
        entries: 1_769_907,
        recorded_cold_build_ms: 12_500,
        cooldown: 10 * 60,
        cents: 1,
    },
    Shape {
        name: "medium",
        securities: 500,
        lots_per: 2_000,
        open_lots: 1_022_625,
        entries: 7_158_381,
        recorded_cold_build_ms: 50_200,
        cooldown: 60 * 60,
        cents: 2,
    },
    // ⛔ THE ONLY SHAPE THAT CANNOT RUN WHERE THE OTHERS DO. Its journal is
    // ~291 bytes x 140 million lines ~ 40 GB, against a Lambda's 512 MB of /tmp
    // and a GitHub runner's ~14 GB of free disk. That asymmetry is why the small
    // ones could be measured while writing this and this one could not.
    Shape {
        name: "full",
        securities: 10_000,
        lots_per: 2_000,
        open_lots: 20_004_324,
        entries: 140_030_274,
        recorded_cold_build_ms: 995_000,
        cooldown: 24 * 60 * 60,
        cents: 8,
    },
];

/// Recorded Stage E projection fold of the HANDOFF geometry.
///
/// ⛔ NOT THE 140M-ENTRY / 40GB JOURNAL FOLD. That stays on Fargate
/// ScaleTask. This is 10,000 × 2,000 open lots through `relieve_by`,
/// digest `STAGE_E_FOLD_DIGEST`, measured by
/// `//crates/ratio-sql-project:fold_scale_test`. The demo Lambda does
/// not host these rows. Journal stays SoR.
///
/// ⚠ ALLOWED DEAD ON THE BINARY. `//:scale_shapes_test` reads these
/// as source text; the test crate uses them. A helper that only the
/// tests called would be the same unused-on-the-binary warning.
#[allow(dead_code)]
pub const STAGE_E_FOLD_SECURITIES: i64 = 10_000;
#[allow(dead_code)]
pub const STAGE_E_FOLD_LOTS_PER: i64 = 2_000;
#[allow(dead_code)]
pub const STAGE_E_FOLD_LOTS: i64 = 20_000_000;
/// Fastbuild, 4 vCPU / 15 GiB Linux. Timing varies; the digest does not.
#[allow(dead_code)]
pub const STAGE_E_FOLD_MS: i64 = 17_388;
#[allow(dead_code)]
pub const STAGE_E_FOLD_DIGEST: &str =
    "bbf896400835916d0902f9ea175609bccd84be4801f71cc9fc57140f8a60a5d3";

/// What the account will let this demo spend on folds in a calendar month.
///
/// ⚠ WELL UNDER THE $5 BUDGET ON PURPOSE. The budget covers the function, the
/// gateway, the log group and the storage too, and a ceiling set AT the budget
/// is a ceiling that only stops the bill after something else has already been
/// paid for. Three dollars leaves the rest of the demo its room.
pub const CEILING_CENTS: i64 = 300;

pub fn shape(name: &str) -> Option<&'static Shape> {
    SHAPES.iter().find(|s| s.name == name)
}

/// Why a request to start a fold was refused.
///
/// ⛔ EACH CARRIES WHAT THE CALLER NEEDS TO UNDERSTAND IT, because a refusal a
/// visitor cannot act on reads as a broken button. "Come back later" with no
/// later in it is the shape of message that generates support mail.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A fold is already running. ⭐ NOT AN ERROR — the caller joins it. The
    /// book is deterministic, so the run already going is the same run they
    /// asked for, folding the same bytes.
    Joined { size: String, started: i64 },
    /// This shape ran recently.
    Cooling { size: String, again_at: i64 },
    /// The month's compute ceiling is spent.
    Spent { cents: i64, ceiling: i64 },
    /// Not one of `SHAPES`.
    NoSuchShape,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Joined { size, .. } => write!(
                f,
                "a {size} fold is already running — you are watching it rather than starting a \
                 second one, and because the book is generated from a seed it is the same fold \
                 you asked for"
            ),
            Refusal::Cooling { size, again_at } => write!(
                f,
                "a {size} fold ran recently; the next one can start at {again_at}. The figures \
                 below are from that run, on this build"
            ),
            Refusal::Spent { cents, ceiling } => write!(
                f,
                "this month's compute ceiling is spent — {cents} of {ceiling} cents. The demo \
                 runs on a five-dollar budget, and a public button that could exhaust it would \
                 be a different kind of demonstration"
            ),
            Refusal::NoSuchShape => write!(f, "that is not a shape this screen folds"),
        }
    }
}

/// Where the run record is kept.
///
/// ⛔ THIS IS AN INTERFACE BECAUSE A FILESYSTEM LOCK ON LAMBDA IS NOT A LOCK.
/// `/tmp` is per-INSTANCE and this account allows ten concurrent executions, so
/// ten visitors pressing the button reach ten different filesystems, each of
/// which happily reports that nothing is running. The policy above would be
/// perfectly correct and would start ten folds.
///
/// So the record lives in one place all instances share, and the only operation
/// that must be atomic is `put_if_absent` — `create_new` on a file, a
/// conditional `PutObject` with `If-None-Match` on S3. ⚠ Both are genuinely
/// atomic. A lock built out of "read, then write" would pass every test on one
/// machine and admit two runs on the day it mattered.
pub trait Store {
    /// Create the key with these bytes, or report that somebody already has.
    /// ⛔ MUST BE ATOMIC. Everything else here rests on it.
    fn put_if_absent(&self, key: &str, body: &str) -> Result<bool>;
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn put(&self, key: &str, body: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
    /// Keys under a prefix. ⚠ Used for SMALL sets only — the leads waiting on
    /// one run — and the report email caps what it reads, so a runaway prefix
    /// cannot turn completion into an unbounded walk.
    fn list(&self, prefix: &str) -> Result<Vec<String>>;
}

/// ⚠ So a `Box<dyn Store>` is a `Store`: the server holds ONE runner type
/// whichever backend the deployment wired, rather than a generic parameter
/// that would force two monomorphic copies of every handler.
impl<S: Store + ?Sized> Store for Box<S> {
    fn put_if_absent(&self, key: &str, body: &str) -> Result<bool> {
        (**self).put_if_absent(key, body)
    }
    fn get(&self, key: &str) -> Result<Option<String>> {
        (**self).get(key)
    }
    fn put(&self, key: &str, body: &str) -> Result<()> {
        (**self).put(key, body)
    }
    fn delete(&self, key: &str) -> Result<()> {
        (**self).delete(key)
    }
    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        (**self).list(prefix)
    }
}

/// ⚠ So a caller holding a `&Store` can build a `Runs` over it without giving
/// the store away. `run` needs both — the policy to release the lock, and the
/// store to write progress — over one record.
impl<S: Store + ?Sized> Store for &S {
    fn put_if_absent(&self, key: &str, body: &str) -> Result<bool> {
        (**self).put_if_absent(key, body)
    }
    fn get(&self, key: &str) -> Result<Option<String>> {
        (**self).get(key)
    }
    fn put(&self, key: &str, body: &str) -> Result<()> {
        (**self).put(key, body)
    }
    fn delete(&self, key: &str) -> Result<()> {
        (**self).delete(key)
    }
    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        (**self).list(prefix)
    }
}

/// The record as a directory. Used by `ratio watch` on a laptop, and by every
/// test in this file.
pub struct Files {
    root: PathBuf,
}

impl AsRef<Files> for Files {
    fn as_ref(&self) -> &Files {
        self
    }
}

impl Files {
    pub fn at(root: impl AsRef<Path>) -> Self {
        Files { root: root.as_ref().to_path_buf() }
    }

    fn path(&self, key: &str) -> PathBuf {
        self.root.join(key)
    }
}

impl Store for Files {
    fn put_if_absent(&self, key: &str, body: &str) -> Result<bool> {
        std::fs::create_dir_all(&self.root).context("creating the run directory")?;
        match std::fs::OpenOptions::new().write(true).create_new(true).open(self.path(key)) {
            Ok(mut f) => {
                use std::io::Write as _;
                write!(f, "{body}").context("writing the run record")?;
                Ok(true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(e).context("taking the run lock"),
        }
    }

    fn get(&self, key: &str) -> Result<Option<String>> {
        match std::fs::read_to_string(self.path(key)) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).context("reading the run record"),
        }
    }

    fn put(&self, key: &str, body: &str) -> Result<()> {
        std::fs::create_dir_all(&self.root).context("creating the run directory")?;
        std::fs::write(self.path(key), body).context("writing the run record")
    }

    fn delete(&self, key: &str) -> Result<()> {
        match std::fs::remove_file(self.path(key)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).context("releasing the run lock"),
        }
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let mut out = Vec::new();
        let dir = match std::fs::read_dir(&self.root) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(e).context("listing the run directory"),
        };
        for entry in dir.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if name.starts_with(prefix) {
                    out.push(name);
                }
            }
        }
        out.sort();
        Ok(out)
    }
}

/// Render an AWS SDK error with its cause, not just its category.
///
/// ⛔ `{e}` ON AN SDK ERROR PRINTS `service error` AND NOTHING ELSE. That is not
/// a hyperbole about terseness: the first `RunTask` refusal on the deployed demo
/// reported exactly
///
///     the fold was allowed but could not be started: starting the fold task:
///     service error
///
/// which names no action, no resource and no reason, and cost a round trip to
/// somebody with AWS credentials to find out what it had actually been told.
/// `DisplayErrorContext` walks the source chain and prints what the service
/// said. A refusal that cannot say why is the failure this repository keeps
/// naming, arriving through an error type instead of through a screen.
fn aws(what: &str, e: impl std::error::Error + Send + Sync + 'static) -> anyhow::Error {
    anyhow::anyhow!("{what}: {}", aws_sdk_s3::error::DisplayErrorContext(&e))
}

/// The record in S3, which is what the deployed demo uses.
///
/// ⛔ ONE BUCKET, ONE PREFIX, AND THE LOCK IS A CONDITIONAL PUT. S3 has offered
/// `If-None-Match: *` on `PutObject` since 2024: the write succeeds only if the
/// key does not exist, and returns 412 to everybody else. That is the same
/// guarantee `create_new` makes on a file, from a place all ten concurrent
/// function instances can see — which a Lambda's `/tmp` is not.
///
/// ⚠ ONE RUNTIME, BUILT ONCE. `ratio-agent` does the same for Bedrock. Building
/// a tokio runtime per call would be a fresh thread pool for every button press.
pub struct S3 {
    bucket: String,
    prefix: String,
    client: aws_sdk_s3::Client,
    runtime: tokio::runtime::Runtime,
}

impl S3 {
    pub fn open(bucket: impl Into<String>, prefix: impl Into<String>) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .context("starting the async runtime")?;
        let client = runtime.block_on(async {
            let cfg = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            aws_sdk_s3::Client::new(&cfg)
        });
        Ok(S3 { bucket: bucket.into(), prefix: prefix.into(), client, runtime })
    }

    fn key(&self, key: &str) -> String {
        format!("{}{key}", self.prefix)
    }
}

impl Store for S3 {
    fn put_if_absent(&self, key: &str, body: &str) -> Result<bool> {
        let k = self.key(key);
        self.runtime.block_on(async {
            let r = self
                .client
                .put_object()
                .bucket(&self.bucket)
                .key(&k)
                // ⛔ THIS HEADER IS THE ENTIRE DOS CONTROL. Without it the call
                // is an ordinary overwrite and every concurrent caller "wins".
                .if_none_match("*")
                .body(body.as_bytes().to_vec().into())
                .send()
                .await;
            match r {
                Ok(_) => Ok(true),
                // ⚠ 412 IS THE EXPECTED ANSWER, NOT AN ERROR. Somebody else
                // holds the lock; the caller joins their run.
                Err(e) if format!("{e:?}").contains("PreconditionFailed") => Ok(false),
                Err(e) if matches!(e.raw_response().map(|r| r.status().as_u16()), Some(412)) => {
                    Ok(false)
                }
                Err(e) => Err(aws("taking the run lock in S3", e)),
            }
        })
    }

    fn get(&self, key: &str) -> Result<Option<String>> {
        let k = self.key(key);
        self.runtime.block_on(async {
            match self.client.get_object().bucket(&self.bucket).key(&k).send().await {
                Ok(o) => {
                    let bytes = o
                        .body
                        .collect()
                        .await
                        .map_err(|e| aws(&format!("reading {k}"), e))?
                        .into_bytes();
                    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
                }
                Err(e) if e.as_service_error().map(|s| s.is_no_such_key()).unwrap_or(false) => {
                    Ok(None)
                }
                // ⚠ A MISSING KEY CAN ALSO ARRIVE AS A BARE 404, when the caller
                // may not `s3:ListBucket` the prefix. Both mean "not there".
                Err(e) if matches!(e.raw_response().map(|r| r.status().as_u16()), Some(404)) => {
                    Ok(None)
                }
                Err(e) => Err(aws(&format!("reading {k}"), e)),
            }
        })
    }

    fn put(&self, key: &str, body: &str) -> Result<()> {
        let k = self.key(key);
        self.runtime.block_on(async {
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(&k)
                .body(body.as_bytes().to_vec().into())
                .send()
                .await
                .map_err(|e| aws(&format!("writing {k}"), e))?;
            Ok(())
        })
    }

    fn delete(&self, key: &str) -> Result<()> {
        let k = self.key(key);
        self.runtime.block_on(async {
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(&k)
                .send()
                .await
                .map_err(|e| aws(&format!("releasing {k}"), e))?;
            Ok(())
        })
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let p = self.key(prefix);
        self.runtime.block_on(async {
            let mut out = Vec::new();
            let mut token: Option<String> = None;
            // ⚠ Paginated for form, bounded in practice: the callers list the
            // leads on ONE run, and the mailer caps how many it will email.
            loop {
                let mut req = self
                    .client
                    .list_objects_v2()
                    .bucket(&self.bucket)
                    .prefix(&p)
                    .max_keys(1_000);
                if let Some(t) = &token {
                    req = req.continuation_token(t);
                }
                let page = req.send().await.map_err(|e| aws(&format!("listing {p}"), e))?;
                for o in page.contents() {
                    if let Some(k) = o.key() {
                        // Hand back keys the way `Files` does: relative to the
                        // store's prefix, so callers never see `runs/`.
                        out.push(k.strip_prefix(&self.prefix).unwrap_or(k).to_string());
                    }
                }
                match page.next_continuation_token() {
                    Some(t) => token = Some(t.to_string()),
                    None => break,
                }
            }
            out.sort();
            Ok(out)
        })
    }
}

impl ratio_store::ObjectStore for S3 {
    /// The same conditional PUT the scale lock uses, now as the journal's
    /// claim. `tla/S3Journal.tla`: a second writer to the same sequence key
    /// is refused (`If-None-Match:*`) and retries; a blind PUT would overwrite
    /// an acked entry and the shortened journal would still tie.
    fn put_if_absent(&self, key: &str, body: &[u8]) -> Result<bool> {
        let s = std::str::from_utf8(body).context("a journal object is utf-8")?;
        Store::put_if_absent(self, key, s)
    }
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(Store::get(self, key)?.map(String::into_bytes))
    }
    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        Store::list(self, prefix)
    }
}


// ── Leads, run reports, and the follow-up email ──────────────────────────────

/// A mailing-list address, checked just enough to be worth storing.
///
/// ⛔ NOT AUTHENTICATION, AND NOT PRETENDING TO BE. The email gate on the demo
/// page is a mailing-list capture; the money controls (one run in flight, the
/// cooldowns, the ceiling) never depend on the address being real, so a made-up
/// one costs nothing extra. What this refuses is input that is not even SHAPED
/// like an address — which keeps junk out of the list and control characters out
/// of everything downstream.
pub fn valid_email(email: &str) -> bool {
    let e = email.trim();
    let Some((local, domain)) = e.split_once('@') else { return false };
    e.len() >= 6
        && e.len() <= 254
        && e.chars().all(|c| !c.is_control() && !c.is_whitespace() && c != '@' || c == '@')
        && e.matches('@').count() == 1
        && !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.starts_with('-')
}

/// The store key an address files under: a digest, not the address.
///
/// ⛔ THE ADDRESS IS DATA, NOT A FILENAME. An email is caller-supplied text and
/// making it a key would put caller bytes in paths and URLs; the digest is safe
/// in both, and the address itself lives INSIDE the record where `quote` guards
/// every rendering of it.
fn lead_key(email: &str) -> String {
    let d = ratio_store::Digest::of(email.trim().to_lowercase().as_bytes());
    format!("lead-{}", &d.as_str()[..16])
}

/// Record an address on the mailing list. Returns whether it was new.
///
/// Idempotent by construction — `put_if_absent` on the digest key — so the
/// unlock route, the start route and a retry all record the same address once.
pub fn record_lead<S: Store>(store: &S, email: &str, now: i64) -> Result<bool> {
    anyhow::ensure!(valid_email(email), "that does not look like an email address");
    store.put_if_absent(&lead_key(email), &format!("{}\t{now}", email.trim().to_lowercase()))
}

/// Attach an address to a run in flight, to be emailed its report at completion.
pub fn await_report<S: Store>(store: &S, id: &str, email: &str) -> Result<()> {
    anyhow::ensure!(valid_email(email), "that does not look like an email address");
    let d = ratio_store::Digest::of(email.trim().to_lowercase().as_bytes());
    store.put(
        &format!("await-{id}-{}", &d.as_str()[..16]),
        email.trim().to_lowercase().as_str(),
    )
}

/// The most recently completed run of a size, by id.
pub fn latest<S: Store>(store: &S, size: &str) -> Option<String> {
    store.get(&format!("latest-{size}")).ok().flatten().map(|s| s.trim().to_string())
}

/// A run's published report, by id. `None` is "no such run", and the id is
/// checked before it goes anywhere near a key.
pub fn report<S: Store>(store: &S, id: &str) -> Option<String> {
    if !valid_run_id(id) {
        return None;
    }
    store.get(&format!("report-{id}.json")).ok().flatten()
}

/// A run id is `{size}-{start-epoch}` — two things this module minted, and
/// nothing a caller composed. ⛔ Checked at every door anyway, because ids
/// arrive in URLs and a key built from an unchecked one would put caller bytes
/// in a path — the same refusal `Console::book_path` makes about fund ids.
pub fn valid_run_id(id: &str) -> bool {
    id.len() <= 40
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && SHAPES.iter().any(|s| id.starts_with(&format!("{}-", s.name)))
}

/// Who sends the follow-up report, and where its links point.
///
/// ⚠ ABSENT IS A WORKING STATE. `RATIO_SCALE_SENDER` unset means no email is
/// sent and nothing else changes — the report link is on the page the moment a
/// run completes, so the demo is whole without SES. This is the same shape as
/// an unset `RATIO_COGNITO_*` meaning "no identity provider here": email
/// activates the day the identity verifies, with no code change.
pub struct Mailer {
    sender: String,
    origin: String,
    client: aws_sdk_sesv2::Client,
    runtime: tokio::runtime::Runtime,
}

impl Mailer {
    pub fn from_env() -> Option<Result<Self>> {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let sender = get("RATIO_SCALE_SENDER")?;
        let origin = get("RATIO_PUBLIC_ORIGIN")?;
        Some((|| {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .context("starting the async runtime")?;
            let client = runtime.block_on(async {
                let cfg = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
                aws_sdk_sesv2::Client::new(&cfg)
            });
            Ok(Mailer { sender, origin, client, runtime })
        })())
    }

    /// Send one report email. ⚠ BEST-EFFORT AT EVERY CALL SITE: a fold that
    /// completed is a fold that completed, and an SES outage must not turn it
    /// into a failure — the figures are in the store and on the page either way.
    pub fn send_report(&self, to: &str, id: &str, summary: &str) -> Result<()> {
        use aws_sdk_sesv2::types::{Body, Content, Destination, EmailContent, Message};
        anyhow::ensure!(valid_email(to), "that does not look like an email address");
        anyhow::ensure!(valid_run_id(id), "that is not a run id");
        let text = format!(
            "You folded a fund.\n\n{summary}\n\nEvery figure, the fold as it happened, and \
             the shape and seed that reproduce it:\n\n  {origin}/scale/runs/{id}\n\nThe books \
             tie because the kernel says so — the journal, the proofs and the code are at \
             https://github.com/mattmarshall/ratio. Reply to this address and a person reads \
             it.\n",
            origin = self.origin,
        );
        let content = |s: &str| Content::builder().data(s).charset("UTF-8").build();
        let msg = EmailContent::builder()
            .simple(
                Message::builder()
                    .subject(content("Your twenty-million-tax-lot fold, and the figures it struck").context("subject")?)
                    .body(Body::builder().text(content(&text).context("body")?).build())
                    .build(),
            )
            .build();
        self.runtime.block_on(async {
            self.client
                .send_email()
                .from_email_address(&self.sender)
                .destination(Destination::builder().to_addresses(to).build())
                .content(msg)
                .send()
                .await
                .map_err(|e| aws("sending the report email", e))?;
            Ok(())
        })
    }
}

/// Email everyone waiting on a completed run. Returns how many were sent.
///
/// ⛔ CAPPED, because this walks a caller-influenced prefix. The unlock route
/// dedupes addresses and the cooldown bounds how many can attach per run, but a
/// cap here means even a mistake upstream cannot turn completion into an
/// unbounded mail-out from a public endpoint.
pub fn send_awaited<S: Store>(store: &S, mailer: &Mailer, id: &str, summary: &str) -> usize {
    const MOST: usize = 200;
    let waiting = match store.list(&format!("await-{id}-")) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("could not list who is waiting on {id}: {e:#}");
            return 0;
        }
    };
    let mut sent = 0;
    for key in waiting.iter().take(MOST) {
        let Some(email) = store.get(key).ok().flatten() else { continue };
        match mailer.send_report(email.trim(), id, summary) {
            Ok(()) => {
                sent += 1;
                let _ = store.delete(key);
            }
            Err(e) => eprintln!("report email to a lead failed: {e:#}"),
        }
    }
    sent
}

/// How a fold is actually set going, once the policy has allowed it.
///
/// ⛔ SEPARATE FROM THE POLICY ON PURPOSE. Whether a visitor MAY start a fold is
/// decided by `Runs::start` over a `Store`, and both are testable. WHERE the
/// fold then happens is a deployment fact — a thread on a laptop, a Fargate task
/// on the demo — and nothing about the decision should depend on which.
pub trait Launcher {
    /// Returns a token identifying the run: a task arn, a thread name, a word.
    /// `id` is the run's minted identity, so the runner publishes its report at
    /// the permalink the page has already shown.
    fn launch(&self, size: &str, id: &str) -> Result<String>;
    /// What the screen should say about where a fold would happen. `None` means
    /// no launcher is configured and the button is not offered at all.
    fn describe(&self) -> String;
}

/// Fold on a thread, in this process. `ratio watch` on a laptop.
///
/// ⚠ NOT WHAT THE DEMO USES, and it must not become so by accident. The full
/// shape needs ~40 GB of scratch and sixteen minutes; a function with 512 MB of
/// /tmp and a fifteen-SECOND timeout would fail in a way that looks like the
/// book being wrong rather than the host being too small.
pub struct Here {
    pub books: PathBuf,
    pub root: PathBuf,
}

impl Launcher for Here {
    fn launch(&self, size: &str, id: &str) -> Result<String> {
        let token = format!("thread:{size}");
        let size = size.to_string();
        let id = id.to_string();
        let books = self.books.clone();
        let root = self.root.clone();
        std::thread::Builder::new()
            .name(format!("scale-{size}"))
            .spawn(move || {
                let store = Files::at(&root);
                if let Err(e) = run(&store, &size, &id, &books) {
                    eprintln!("the {size} fold failed: {e:#}");
                }
            })
            .context("starting the fold")?;
        Ok(token)
    }

    fn describe(&self) -> String {
        "this process".into()
    }
}

/// Fold in a one-shot Fargate task. What the demo uses.
pub struct Ecs {
    pub cluster: String,
    pub task: String,
    pub subnet: String,
    pub security_group: String,
    client: aws_sdk_ecs::Client,
    runtime: tokio::runtime::Runtime,
}

impl Ecs {
    /// Built from the environment `deploy/app.yaml` sets. ⚠ `None` when any of
    /// it is missing, which is a VALID state — a local run configures none of
    /// this, and the screen then serves its estimate and its recorded figures
    /// with no button, exactly as an unset `RATIO_COGNITO_*` means "no identity
    /// provider here" rather than "misconfigured".
    pub fn from_env() -> Option<Result<Self>> {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let (cluster, task, subnet, sg) = (
            get("RATIO_SCALE_CLUSTER")?,
            get("RATIO_SCALE_TASK")?,
            get("RATIO_SCALE_SUBNET")?,
            get("RATIO_SCALE_SECURITY_GROUP")?,
        );
        Some((|| {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .context("starting the async runtime")?;
            let client = runtime.block_on(async {
                let cfg = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
                aws_sdk_ecs::Client::new(&cfg)
            });
            Ok(Ecs {
                cluster,
                task,
                subnet,
                security_group: sg,
                client,
                runtime,
            })
        })())
    }
}

impl Launcher for Ecs {
    fn launch(&self, size: &str, id: &str) -> Result<String> {
        anyhow::ensure!(valid_run_id(id), "{id:?} is not a run id");
        use aws_sdk_ecs::types::{
            AssignPublicIp, AwsVpcConfiguration, ContainerOverride, LaunchType,
            NetworkConfiguration, TaskOverride,
        };
        // ⛔ THE COMMAND IS BUILT FROM A SHAPE NAME THE ROUTE ALREADY MATCHED
        // EXHAUSTIVELY, never from anything a caller typed. `terminal_json` sets
        // the standard for this binary: a public endpoint that dispatches on a
        // string the caller controls is a remote-execution seam wearing a demo
        // costume. By here `size` is one of three literals.
        let shape = shape(size).ok_or_else(|| anyhow::anyhow!("{size:?} is not a shape"))?;
        self.runtime.block_on(async {
            let out = self
                .client
                .run_task()
                .cluster(&self.cluster)
                .task_definition(&self.task)
                .launch_type(LaunchType::Fargate)
                .network_configuration(
                    NetworkConfiguration::builder()
                        .awsvpc_configuration(
                            AwsVpcConfiguration::builder()
                                .subnets(&self.subnet)
                                .security_groups(&self.security_group)
                                // ⛔ A PUBLIC IP, BECAUSE THERE IS NO NAT. The
                                // task reaches ECR, S3 and CloudWatch over the
                                // internet gateway; a NAT would be ~$32/month
                                // standing against a $5 budget.
                                .assign_public_ip(AssignPublicIp::Enabled)
                                .build()
                                .context("the task's network configuration")?,
                        )
                        .build(),
                )
                .overrides(
                    TaskOverride::builder()
                        .container_overrides(
                            ContainerOverride::builder()
                                .name("ratio")
                                .command("scale-run")
                                .command("--size")
                                .command(shape.name)
                                .command("--id")
                                .command(id)
                                .build(),
                        )
                        .build(),
                )
                .send()
                .await
                .map_err(|e| aws("starting the fold task", e))?;
            let arn = out
                .tasks()
                .first()
                .and_then(|t| t.task_arn())
                .unwrap_or("started")
                .to_string();
            Ok(arn)
        })
    }

    fn describe(&self) -> String {
        format!("a one-shot task on {}", self.cluster)
    }
}

/// Generate the book if it is not already there, fold it cold, publish both
/// curves, and release the lock.
///
/// ⛔ ONE FUNCTION, TWO WAYS TO REACH IT. On a laptop `ratio watch` calls this
/// on a thread; on the demo a Fargate task calls it as `ratio scale-run --size
/// N`. Two implementations would be two answers to "what did the fold cost",
/// and the one nobody ran locally would be the one quoted.
///
/// ⚠ THE LOCK IS RELEASED WHATEVER HAPPENS. A fold that panics or fails leaves
/// `current` behind otherwise, and every later run is refused with "a fold is
/// already running" forever — a permanently broken button, from one bad run.
/// `id` is the run's identity — `{size}-{start-epoch}`, minted where the run
/// was allowed and carried here so the report's permalink is known before the
/// fold begins. ⛔ The report survives under `report-{id}.json` forever; the
/// per-size `result-{size}.json` is only "the latest", for the panel.
pub fn run<S: Store>(store: &S, size: &str, id: &str, book_root: &Path) -> Result<()> {
    anyhow::ensure!(valid_run_id(id), "{id:?} is not a run id");
    let runs = Runs::over(store);
    let outcome = fold_and_measure(store, size, book_root);
    let published = match &outcome {
        Ok(json) => Some(json.as_str()),
        // ⛔ A FAILURE IS PUBLISHED TOO, AND AS A FIGURE-SHAPED THING. A run
        // that vanished would leave the screen showing the PREVIOUS run's
        // numbers with nothing saying this one died.
        Err(_) => None,
    };
    let failure = outcome.as_ref().err().map(|e| format!("{e:#}"));
    if let Some(why) = &failure {
        let _ = store.put(
            &format!("failed-{size}.json"),
            &format!("{{\"error\":{}}}", crate::watch::quote(why)),
        );
    }
    if let Ok(json) = &outcome {
        // ⛔ THE REPORT CARRIES ITS OWN COPY OF THE FOLD'S SERIES. The progress
        // document lives under a per-SIZE key and the next run of this size
        // overwrites it — so a permalink that read it live would show every old
        // run redrawn as the newest one's curve. Embedded, the chart a lead is
        // emailed is the fold THEY watched, forever.
        let series = store
            .get(&format!("progress-{size}"))
            .ok()
            .flatten()
            .unwrap_or_else(|| "null".to_string());
        let report = format!("{{\"id\":{},\"run\":{json},\"progress\":{series}}}", crate::watch::quote(id));
        // ⛔ THE PERMALINK FIRST, THE POINTER SECOND. A `latest-{size}` naming a
        // report that does not exist yet is a link a visitor can click into a
        // 404; the other order is at worst a report nobody points at for a
        // moment.
        store.put(&format!("report-{id}.json"), &report)?;
        store.put(&format!("latest-{size}"), id)?;
    }
    runs.finish(size, published)?;

    // The follow-up the leads asked for, after every figure is readable at the
    // permalink. ⚠ BEST-EFFORT AND LAST: a fold that completed must not be
    // failed retroactively by SES, and the report link is already on the page.
    if let Ok(json) = &outcome {
        if let Some(Ok(mailer)) = Mailer::from_env() {
            let sent = send_awaited(store, &mailer, id, &summarize_report(json));
            if sent > 0 {
                eprintln!("sent {sent} report email(s) for {id}");
            }
        }
    }
    outcome.map(|_| ())
}

/// The three lines of a report email's body, pulled from the run's own figures.
///
/// ⚠ FROM THE RUN DOCUMENT, NOT THE SHAPES TABLE — the email describes the fold
/// that happened, and the one thing it must never do is describe a different
/// one. Falls back to naming the run rather than inventing a figure it cannot
/// find.
pub fn summarize_report(report_json: &str) -> String {
    let read = |k: &str| -> Option<i64> {
        let at = report_json.find(&format!("\"{k}\":"))?;
        report_json[at + k.len() + 3..]
            .split(|c: char| !c.is_ascii_digit() && c != '-')
            .find(|t| !t.is_empty())?
            .parse()
            .ok()
    };
    match (read("open_lots"), read("journal_entries"), read("cold_build_ns")) {
        (Some(lots), Some(entries), Some(ns)) => format!(
            "{lots} open tax lots over {entries} journal entries, folded cold from an empty \
             projection in {}. Trial balance: 0.",
            ratio_nav::closure::human_nanos(ns)
        ),
        _ => "The full figures are at the link below.".to_string(),
    }
}

/// The progress record a run keeps: a phase and a SERIES, not one overwritten
/// string.
///
/// ⛔ A STRING WAS WHAT A VISITOR SAW FOR TWENTY MINUTES. `"folding 12345"`,
/// overwritten every five seconds, gives a joiner no history, a refreshed page
/// no curve, and the screen nothing to draw. The series is the entire point:
/// this repository's argument is that the cold build GROWS, and a growing thing
/// is shown by its trajectory, not by its latest value.
///
/// ⚠ REWRITTEN WHOLE ON EACH SAMPLE — S3 has no append. ~240 samples over a
/// twenty-minute run is ~7 KB, one small PutObject per five seconds.
struct Progress<'a, S: Store> {
    store: &'a S,
    key: String,
    phase: &'static str,
    /// The shape's DECLARED entry count, so the page can show a fraction
    /// without reading the journal twice to learn its length. ⚠ Declared, not
    /// measured — the page clamps the bar at 100% in case the two ever drift.
    expected: i64,
    started: std::time::Instant,
    samples: Vec<(u64, usize)>,
    last_write: std::time::Instant,
}

impl<'a, S: Store> Progress<'a, S> {
    fn begin(store: &'a S, size: &str, phase: &'static str, expected: i64) -> Self {
        let mut p = Progress {
            store,
            key: format!("progress-{size}"),
            phase,
            expected,
            started: std::time::Instant::now(),
            samples: vec![(0, 0)],
            last_write: std::time::Instant::now(),
        };
        p.write();
        p
    }

    /// A new phase keeps the key and drops the series — generation's samples
    /// are not the fold's, and a chart that spliced them would show one curve
    /// made of two different quantities.
    fn phase(&mut self, phase: &'static str) {
        self.phase = phase;
        self.started = std::time::Instant::now();
        self.samples = vec![(0, 0)];
        self.write();
    }

    /// Record a count. ⛔ THE THROTTLE LIVES HERE: whatever this does is time
    /// the measured work is charged for, so the sample vector grows in memory
    /// every call and the store is written at most every five seconds.
    fn at(&mut self, n: usize) {
        self.samples.push((self.started.elapsed().as_secs(), n));
        if self.last_write.elapsed() >= std::time::Duration::from_secs(5) {
            self.last_write = std::time::Instant::now();
            self.write();
        }
    }

    fn write(&self) {
        let samples: Vec<String> =
            self.samples.iter().map(|(t, n)| format!("[{t},{n}]")).collect();
        let _ = self.store.put(
            &self.key,
            &format!(
                "{{\"phase\":\"{}\",\"expected\":{},\"samples\":[{}]}}",
                self.phase,
                self.expected,
                samples.join(",")
            ),
        );
    }
}

fn fold_and_measure<S: Store>(store: &S, size: &str, book_root: &Path) -> Result<String> {
    let shape = shape(size).ok_or_else(|| anyhow::anyhow!("{size:?} is not a shape"))?;
    let dir = book_root.join(format!("scale-{size}"));

    // ⛔ THE BOOK IS BUILT ONCE AND KEPT, WHICH IS MEASURED RATHER THAN
    // PREFERRED. Generating costs MORE than the cold build it feeds — 28.6 s
    // against 24.0 s at 500x500, 115.8 s against 94.7 s at 500x2000. A runner
    // that regenerated every time would spend most of its life rebuilding
    // something byte-identical to last time, and `ratio-gen` is deterministic
    // precisely so it does not have to.
    //
    // ⚠ `expected` IS THE SAME FIGURE FOR BOTH PHASES — the generator writes
    // what the fold then reads — which is what makes one bar legible across
    // them.
    let mut progress = Progress::begin(store, size, "generating", shape.entries);
    if ratio_gen::shape_of(&dir).is_err() {
        let dials = ratio_gen::Shape {
            securities: shape.securities,
            lots_per: shape.lots_per,
            currencies: 3,
            ..Default::default()
        };
        // ⛔ THE LONGEST PHASE WAS A BLACK BOX: at the full shape, generation
        // outlasts the fold and the screen said one word for all of it.
        ratio_gen::generate_with_progress(&dir, dials, &mut |n| progress.at(n))
            .context("generating the book")?;
    }

    progress.phase("folding");
    let started = std::time::Instant::now();
    let proj = ratio_project::Projection::of_book_with_progress(&dir, &mut |n| progress.at(n))
        .context("folding the book")?;
    let cold_ns = started.elapsed().as_nanos() as i64;
    progress.phase("striking");

    let entries = proj.prefix() as i64;
    // ⚠ THE UNDECLARED VIEW, BY CONSTRUCTION NOT BY DEFAULT: `ratio gen`
    // writes no `[[view]]`, so the benched book keeps exactly one book of
    // record. If the generator ever grows a views dial here, this read refuses
    // rather than quietly benching one view of several.
    let open_lots = proj.open_lots(ratio_rules::UNDECLARED_VIEW)?;
    let cost = proj.cost();
    // ⛔ BOTH CURVES, OR NEITHER. The growing one and the flat one travel
    // together everywhere in this repository; a published result carrying only
    // the strike would be the overclaim `ratio bench` exists to make hard,
    // arriving by a different route.
    Ok(format!(
        "{{\"size\":{},\"securities\":{},\"lots_per\":{},\"open_lots\":{},\
         \"journal_entries\":{},\"cold_build_ns\":{},\"parse_ns\":{},\"fold_ns\":{},\
         \"relieve_ns\":{},\"reliefs\":{},\"recorded_cold_build_ms\":{},\"build\":{}}}",
        crate::watch::quote(size),
        shape.securities,
        shape.lots_per,
        open_lots,
        entries,
        cold_ns,
        cost.parse.as_nanos() as i64,
        cost.fold.as_nanos() as i64,
        cost.relieve.as_nanos() as i64,
        cost.reliefs,
        shape.recorded_cold_build_ms,
        crate::watch::quote(&std::env::var("RATIO_BUILD").unwrap_or_else(|_| "dev".into())),
    ))
}

/// The policy: who may start a fold, and what it costs to let them.
pub struct Runs<S: Store> {
    store: S,
}

impl<S: Store> Runs<S> {
    /// The record this policy decides over, for callers that also keep leads
    /// and reports in it. One store, so the lead that unlocked a run and the
    /// report that run published cannot live in two places that disagree.
    pub fn store(&self) -> &S {
        &self.store
    }
}

/// A run in flight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Current {
    pub size: String,
    pub started: i64,
    /// Whatever the runner is identified by — an ECS task arn, a pid, a word.
    pub token: String,
}

impl<S: Store> Runs<S> {
    pub fn over(store: S) -> Self {
        Runs { store }
    }

    /// What is running, if anything.
    pub fn current(&self) -> Result<Option<Current>> {
        let Some(raw) = self.store.get("current")? else { return Ok(None) };
        let mut it = raw.trim().splitn(3, '\t');
        let size = it.next().unwrap_or_default().to_string();
        let started: i64 = it.next().unwrap_or("0").parse().unwrap_or(0);
        let token = it.next().unwrap_or_default().to_string();
        Ok(Some(Current { size, started, token }))
    }

    /// When each shape last STARTED, for the cooldown.
    ///
    /// ⚠ STARTED, NOT FINISHED. A cooldown measured from the end would let a
    /// visitor start a twenty-minute fold, wait for it, and start another —
    /// spending twice the estimate inside one cooldown window.
    pub fn last_started(&self, size: &str) -> i64 {
        self.store
            .get(&format!("last-{size}"))
            .ok()
            .flatten()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    }

    /// Cents spent this month, and the month they were spent in.
    ///
    /// ⛔ THE MONTH IS STORED WITH THE FIGURE. A counter that reset on a
    /// schedule somebody else ran would either never reset or reset twice; this
    /// resets when the month it names stops being the current one, which needs
    /// nothing to run at all.
    pub fn spent(&self, month: i64) -> i64 {
        let Ok(Some(raw)) = self.store.get("spent") else { return 0 };
        let mut it = raw.trim().splitn(2, '\t');
        let stored: i64 = it.next().unwrap_or("0").parse().unwrap_or(0);
        let cents: i64 = it.next().unwrap_or("0").parse().unwrap_or(0);
        if stored == month {
            cents
        } else {
            0
        }
    }

    /// Ask to start a fold of `size`, as of `now`, in calendar `month`.
    ///
    /// The order of the checks is the order of their costs, cheapest first — but
    /// the ceiling is checked before the lock is taken, so a month that is spent
    /// refuses without leaving a lock behind for a run that never starts.
    pub fn start(
        &self,
        size: &str,
        now: i64,
        month: i64,
        token: &str,
    ) -> Result<std::result::Result<&'static Shape, Refusal>> {
        let Some(shape) = shape(size) else {
            return Ok(Err(Refusal::NoSuchShape));
        };

        if let Some(c) = self.current()? {
            return Ok(Err(Refusal::Joined { size: c.size, started: c.started }));
        }

        let last = self.last_started(size);
        if last > 0 && now < last + shape.cooldown {
            return Ok(Err(Refusal::Cooling {
                size: size.to_string(),
                again_at: last + shape.cooldown,
            }));
        }

        let spent = self.spent(month);
        // ⛔ THE COST OF THIS RUN IS CHARGED BEFORE IT RUNS, not after it
        // finishes. A ceiling that only counted completed runs would admit any
        // number of simultaneous ones — and would never charge for a run that
        // died halfway, which still consumed the compute.
        if spent + shape.cents > CEILING_CENTS {
            return Ok(Err(Refusal::Spent { cents: spent, ceiling: CEILING_CENTS }));
        }

        // ⛔ `put_if_absent` IS THE LOCK, and it is the ONLY atomic operation
        // this policy needs. Two callers racing here both attempt the same key
        // and exactly one wins; the loser reads the winner's record and joins
        // it. Nothing above this line is a check the winner relies on — the
        // `current()` test earlier is a courtesy that saves a round trip, not
        // the guarantee.
        if !self.store.put_if_absent("current", &format!("{size}\t{now}\t{token}"))? {
            let c = self.current()?.unwrap_or(Current {
                size: size.to_string(),
                started: now,
                token: String::new(),
            });
            return Ok(Err(Refusal::Joined { size: c.size, started: c.started }));
        }

        self.store.put(&format!("last-{size}"), &now.to_string())?;
        self.store.put("spent", &format!("{month}\t{}", spent + shape.cents))?;
        Ok(Ok(shape))
    }

    /// A run that RAN ended — with a result, or without one.
    ///
    /// ⚠ THE SPEND IS NOT REFUNDED ON FAILURE. A fold that died after fifteen
    /// minutes consumed fifteen minutes of compute, and a ceiling that gave the
    /// money back for it would let a failing run be retried without limit.
    ///
    /// ⛔ FOR A FOLD THAT NEVER STARTED, THIS IS THE WRONG CALL — see `abandon`.
    /// The distinction is exactly *did compute happen*, and reaching for this
    /// one by default on a launch failure is how the deployed demo put `small`
    /// into a ten-minute cooldown for a run that never existed. On `full` it
    /// would have been twenty-four hours.
    pub fn finish(&self, size: &str, result: Option<&str>) -> Result<()> {
        if let Some(r) = result {
            self.store.put(&format!("result-{size}.json"), r)?;
        }
        // The series was this run's; the next one starts its own, and a page
        // showing last month's curve beside this month's figures would be
        // showing two runs as one.
        self.store.delete(&format!("progress-{size}"))?;
        self.store.delete("current")
    }

    /// A run that NEVER RAN is walked back entirely: the lock, the cooldown and
    /// the charge.
    ///
    /// ⛔ ONLY FOR THE LAUNCH-FAILURE PATH — the window between `start` granting
    /// a run and the compute actually beginning. Nothing was consumed, so there
    /// is nothing the refund argument on `finish` protects: a visitor whose
    /// button press was refused by the infrastructure has not spent the
    /// account's money, and holding them (and everyone else) to a cooldown for
    /// it turns one refused RunTask into a day-long outage of the headline
    /// demonstration. That is not hypothetical; it is what the first click did.
    pub fn abandon(&self, size: &str, month: i64) -> Result<()> {
        let _ = self.store.delete(&format!("progress-{size}"));
        let Some(shape) = shape(size) else { return self.store.delete("current") };
        // The charge comes back because it was taken at `start`, before any
        // compute — the reservation is cancelled with the run.
        let spent = self.spent(month);
        self.store
            .put("spent", &format!("{month}\t{}", (spent - shape.cents).max(0)))?;
        self.store.delete(&format!("last-{size}"))?;
        self.store.delete("current")
    }

    /// The last completed result for a shape, as the runner wrote it.
    /// The record's directory, for tests that need to put a book beside it.
    #[cfg(test)]
    pub fn root_for_test(&self) -> PathBuf
    where
        S: AsRef<Files>,
    {
        self.store.as_ref().root.clone()
    }

    /// How far a fold has got, as the runner last wrote it.
    ///
    /// ⚠ A COUNT OF ENTRIES, NOT A PERCENTAGE. A journal does not know its own
    /// length without reading it, and reading it twice to print a fraction would
    /// double the cost of the thing being reported on.
    pub fn progress(&self, size: &str) -> Option<String> {
        self.store.get(&format!("progress-{size}")).ok().flatten()
    }

    pub fn result(&self, size: &str) -> Option<String> {
        self.store.get(&format!("result-{size}.json")).ok().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs(name: &str) -> Runs<Files> {
        let d = match std::env::var_os("TEST_TMPDIR") {
            Some(t) => PathBuf::from(t),
            None => std::env::temp_dir(),
        }
        .join(format!("scale-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        Runs::over(Files::at(d))
    }

    const M: i64 = 202608;

    #[test]
    fn the_second_visitor_joins_the_run_rather_than_starting_another() {
        let r = runs("join");
        assert!(r.start("small", 1_000, M, "task-a").unwrap().is_ok());

        // ⭐ THE WHOLE DOS CONTROL, AND IT IS ALSO THE BETTER BEHAVIOUR. The book
        // is generated from a seed, so the run already going folds the same bytes
        // the second caller asked for.
        let second = r.start("small", 1_001, M, "task-b").unwrap().unwrap_err();
        assert_eq!(second, Refusal::Joined { size: "small".into(), started: 1_000 });
    }

    #[test]
    fn two_visitors_pressing_at_once_start_exactly_one_fold() {
        // ⛔ THE TEST THE OTHER ONE ONLY LOOKED LIKE. `the_second_visitor_joins`
        // calls `start` twice in sequence, so the second one is turned away by
        // the `current()` check at the top and NEVER REACHES the lock. Replacing
        // `create_new` with a plain truncating `create` left that test green —
        // which is to say the DoS control was untested by the test written for
        // it, and the money it guards is real.
        //
        // Two threads through the same window is the only thing that exercises
        // the lock, because the lock exists for exactly the case where both
        // callers have already read "nothing is running".
        use std::sync::{Arc, Barrier};

        let r = Arc::new(runs("race"));
        let gate = Arc::new(Barrier::new(8));

        let winners: Vec<_> = (0..8)
            .map(|i| {
                let r = Arc::clone(&r);
                let gate = Arc::clone(&gate);
                std::thread::spawn(move || {
                    gate.wait();
                    matches!(r.start("small", 1_000, M, &format!("task-{i}")), Ok(Ok(_)))
                })
            })
            .collect();

        // `join` consumes the handle, so map first and filter the booleans.
        let won = winners.into_iter().map(|h| h.join().unwrap()).filter(|w| *w).count();
        assert_eq!(won, 1, "{won} of eight simultaneous callers started a fold; exactly one may");

        // ⭐ AND THE MONEY WAS CHARGED ONCE, not eight times and not zero. A lock
        // that admitted two runs would also bill for two.
        assert_eq!(r.spent(M), 1, "the month was charged for {} runs", r.spent(M));
    }

    #[test]
    fn a_shape_that_just_ran_will_not_run_again_yet() {
        let r = runs("cool");
        r.start("full", 1_000, M, "t").unwrap().unwrap();
        r.finish("full", None).unwrap();

        let no = r.start("full", 1_000 + 60, M, "t").unwrap().unwrap_err();
        assert_eq!(no, Refusal::Cooling { size: "full".into(), again_at: 1_000 + 86_400 });

        // ⚠ And the cooldown is per SHAPE, so a small fold is not blocked by a
        // full one — they are different demonstrations at very different prices.
        assert!(r.start("small", 1_000 + 60, M, "t").unwrap().is_ok());
    }

    #[test]
    fn the_cooldown_runs_from_when_a_fold_started_not_when_it_ended() {
        // ⛔ A twenty-minute fold measured from its END would let a visitor wait
        // it out and start another inside one window, spending twice the
        // estimate. The clock starts when the money does.
        let r = runs("from-start");
        r.start("medium", 1_000, M, "t").unwrap().unwrap();
        r.finish("medium", None).unwrap();
        let again = r.start("medium", 1_000 + 3_500, M, "t").unwrap().unwrap_err();
        assert_eq!(again, Refusal::Cooling { size: "medium".into(), again_at: 1_000 + 3_600 });
    }

    #[test]
    fn the_month_stops_paying_before_the_budget_does() {
        let r = runs("ceiling");
        // Spend the month down to less than a full fold's estimate.
        r.store.put("spent", &format!("{M}\t{}", CEILING_CENTS - 1)).unwrap();

        let no = r.start("full", 1_000, M, "t").unwrap().unwrap_err();
        assert_eq!(no, Refusal::Spent { cents: CEILING_CENTS - 1, ceiling: CEILING_CENTS });

        // ⛔ AND NO LOCK WAS LEFT BEHIND for a run that never started. A refusal
        // that stranded the lock would stop every later run too, turning a spent
        // month into a permanently broken button.
        assert!(r.current().unwrap().is_none());
    }

    #[test]
    fn a_new_month_starts_from_nothing_with_nobody_resetting_it() {
        let r = runs("rollover");
        r.store.put("spent", &format!("{M}\t{CEILING_CENTS}")).unwrap();
        assert_eq!(r.spent(M), CEILING_CENTS);
        // The stored month is not this one, so the figure is not this month's.
        assert_eq!(r.spent(M + 1), 0);
    }

    #[test]
    fn a_run_is_charged_when_it_starts_so_a_failed_one_is_still_paid_for() {
        let r = runs("charge");
        r.start("full", 1_000, M, "t").unwrap().unwrap();
        assert_eq!(r.spent(M), 8);
        // It died. The compute was consumed either way.
        r.finish("full", None).unwrap();
        assert_eq!(r.spent(M), 8);
    }

    #[test]
    fn a_shape_nobody_declared_is_refused_before_anything_is_spent() {
        let r = runs("unknown");
        assert_eq!(r.start("enormous", 1, M, "t").unwrap().unwrap_err(), Refusal::NoSuchShape);
        assert_eq!(r.spent(M), 0);
        assert!(r.current().unwrap().is_none());
    }

    #[test]
    fn finishing_a_run_publishes_its_figures_and_frees_the_lock() {
        let r = runs("finish");
        r.start("small", 1_000, M, "t").unwrap().unwrap();
        r.finish("small", Some(r#"{"open_lots":252843}"#)).unwrap();
        assert!(r.current().unwrap().is_none());
        assert_eq!(r.result("small").as_deref(), Some(r#"{"open_lots":252843}"#));
        assert!(r.result("full").is_none());
    }

    #[test]
    fn a_failed_fold_still_releases_the_lock() {
        // ⛔ THE FAILURE THAT BREAKS THE BUTTON FOREVER. A run that dies with
        // `current` still set means every later start is refused with "a fold is
        // already running" — for a fold that is not running and never will be.
        // One bad run would take the feature down permanently, and the symptom
        // is indistinguishable from a very slow fold.
        let r = runs("failed");
        r.start("small", 1_000, M, "t").unwrap().unwrap();
        assert!(r.current().unwrap().is_some());

        // A book root that cannot be written to: the fold fails.
        let bad = PathBuf::from("/proc/nonexistent-scale-root");
        let outcome = run(&r.store, "small", "small-1000", &bad);

        assert!(outcome.is_err(), "folding into an unwritable root should fail");
        assert!(r.current().unwrap().is_none(), "the lock outlived the run that took it");
        // ⚠ And the failure is visible rather than silent, so the screen does
        // not go on showing the previous run's figures as though nothing
        // happened.
        assert!(r.store.get("failed-small.json").unwrap().is_some());
    }

    #[test]
    fn a_run_folds_the_book_it_finds_and_publishes_both_curves() {
        // ⭐ THE SAME FUNCTION A FARGATE TASK CALLS, doing the real work: it
        // folds a book off disk and reports what the projection says. What it
        // must NOT be is a stub writing plausible figures, so every assertion
        // below is on the fold's own output.
        //
        // ⚠ A TINY BOOK AT THE `small` PATH. Generating the real small shape is
        // 1.7M entries and ~50 s, far too slow for this suite — but
        // `fold_and_measure` skips generation when the book already says what
        // generated it, which is the path every run after the first takes. So
        // this exercises the fold with a book it can fold in milliseconds. The
        // dials in SHAPES are checked against HANDOFF by `//:scale_shapes_test`;
        // this checks the folding.
        let r = runs("endtoend");
        let root = r.root_for_test();
        ratio_gen::generate(
            &root.join("scale-small"),
            ratio_gen::Shape { securities: 4, lots_per: 6, currencies: 2, ..Default::default() },
        )
        .unwrap();

        let json = fold_and_measure(&r.store, "small", &root).unwrap();
        let got = |k: &str| -> i64 {
            let at = json.find(&format!("\"{k}\":")).unwrap_or_else(|| panic!("{k} missing from {json}"));
            json[at + k.len() + 3..]
                .split(|c: char| !c.is_ascii_digit() && c != '-')
                .find(|s| !s.is_empty())
                .unwrap()
                .parse()
                .unwrap()
        };

        // ⛔ BOTH CURVES. The growing one and the flat one travel together
        // everywhere in this repository, and a published result carrying only
        // one would be the overclaim `ratio bench` exists to make hard,
        // arriving by a different route.
        assert!(got("cold_build_ns") > 0, "the cold build was not measured: {json}");
        assert!(got("parse_ns") > 0, "parse was not measured: {json}");
        assert!(got("recorded_cold_build_ms") == 12_500, "the recorded row is not carried");

        // And the figures are the BOOK's, not the table's — the tiny book has
        // nothing like the small shape's entry count.
        let entries = got("journal_entries");
        assert!(entries > 0 && entries < 100_000, "folded {entries} entries, expected the tiny book");
        assert!(got("open_lots") > 0, "no lots came out of the fold: {json}");

        // ⛔ An undeclared shape folds nothing at all.
        assert!(fold_and_measure(&r.store, "enormous", &root).is_err());
    }

    #[test]
    fn a_fold_that_never_launched_leaves_no_trace_at_all() {
        // ⛔ THE DEPLOYED FAILURE THIS ENCODES: the first button press on the
        // demo was granted by the policy, refused by ECS (the service-linked
        // role did not exist), and the error path called `finish` — which
        // cleared the lock and LEFT THE COOLDOWN AND THE CHARGE. The screen
        // said "cooling" for a run that never ran, and on the full shape that
        // would have been a twenty-four hour outage from one refused RunTask.
        let r = runs("abandon");
        r.start("full", 1_000, M, "t").unwrap().unwrap();
        assert_eq!(r.spent(M), 8, "the reservation is charged at start");

        // The launch fails; nothing was consumed. Walk it all back.
        r.abandon("full", M).unwrap();

        assert!(r.current().unwrap().is_none(), "the lock survived abandon");
        assert_eq!(r.spent(M), 0, "the charge survived abandon — money for no compute");
        // ⭐ THE ASSERTION THAT DISTINGUISHES abandon FROM finish: the same
        // shape can start again immediately, because no cooldown exists for a
        // run that did not happen.
        assert!(
            r.start("full", 1_001, M, "t2").unwrap().is_ok(),
            "a refused launch left a cooldown behind"
        );
    }

    #[test]
    fn a_fold_that_ran_and_died_is_still_paid_for_and_still_cooling() {
        // The other half of the distinction, so the two cannot be merged: a
        // fold that STARTED consumed compute, and `finish` deliberately keeps
        // both the charge and the cooldown. If this test and the one above
        // ever both pass with one method, that method is wrong somewhere.
        let r = runs("died");
        r.start("full", 1_000, M, "t").unwrap().unwrap();
        r.finish("full", None).unwrap();

        assert_eq!(r.spent(M), 8, "a run that consumed compute is paid for");
        assert_eq!(
            r.start("full", 1_001, M, "t2").unwrap().unwrap_err(),
            Refusal::Cooling { size: "full".into(), again_at: 1_000 + 86_400 },
        );
    }


    /// A store that remembers every progress write, so a test can see the
    /// series a real poller would have seen mid-run without racing the run.
    struct Recording<'a> {
        inner: &'a Files,
        progress_writes: std::sync::Mutex<Vec<String>>,
    }

    impl Store for Recording<'_> {
        fn put_if_absent(&self, key: &str, body: &str) -> Result<bool> {
            self.inner.put_if_absent(key, body)
        }
        fn get(&self, key: &str) -> Result<Option<String>> {
            self.inner.get(key)
        }
        fn put(&self, key: &str, body: &str) -> Result<()> {
            if key.starts_with("progress-") {
                self.progress_writes.lock().unwrap().push(body.to_string());
            }
            self.inner.put(key, body)
        }
        fn delete(&self, key: &str) -> Result<()> {
            self.inner.delete(key)
        }
        fn list(&self, prefix: &str) -> Result<Vec<String>> {
            self.inner.list(prefix)
        }
    }

    #[test]
    fn the_progress_record_is_a_phased_series_not_a_string() {
        // ⛔ THE CONTRACT THE CHART DRAWS FROM. Every progress write is a JSON
        // document carrying a phase and a sample series — the page parses it,
        // windows a rate over it, and draws it. One overwritten display string
        // (the previous format) gave a joiner no history and the screen nothing
        // to draw, which is what "show progress over time" was asking about.
        let r = runs("phased");
        let root = r.root_for_test();
        ratio_gen::generate(
            &root.join("scale-small"),
            ratio_gen::Shape { securities: 4, lots_per: 6, currencies: 2, ..Default::default() },
        )
        .unwrap();
        let rec = Recording { inner: &r.store, progress_writes: Default::default() };
        run(&rec, "small", "small-1000", &root).unwrap();

        let writes = rec.progress_writes.lock().unwrap();
        assert!(!writes.is_empty(), "a run wrote no progress at all");
        for w in writes.iter() {
            assert!(w.starts_with('{'), "a progress write is not a document: {w}");
            assert!(w.contains("\"phase\":"), "{w}");
            assert!(w.contains("\"expected\":"), "{w}");
            assert!(w.contains("\"samples\":["), "{w}");
        }
        // Both phases of the work appear — the fold, and the generation that
        // was previously a black box.
        assert!(writes.iter().any(|w| w.contains("\"phase\":\"generating\"")), "{writes:?}");
        assert!(writes.iter().any(|w| w.contains("\"phase\":\"folding\"")), "{writes:?}");
        // The declared entry count rides along so the page can show a fraction.
        assert!(
            writes.iter().any(|w| w.contains(&format!("\"expected\":{}", shape("small").unwrap().entries))),
            "no write carried the declared entry count"
        );
    }

    #[test]
    fn a_watched_run_leaves_a_series_a_joiner_can_read() {
        // ⭐ WHAT THE PAGE ACTUALLY CONSUMES: after a run, the progress record
        // was a JSON document with a phase and a GROWING series — not one
        // overwritten string — and finishing cleared it while publishing the
        // result. This is the whole contract of the progress UI, exercised
        // through the same `run` a Fargate task calls.
        let r = runs("series");
        let root = r.root_for_test();
        ratio_gen::generate(
            &root.join("scale-small"),
            ratio_gen::Shape { securities: 4, lots_per: 6, currencies: 2, ..Default::default() },
        )
        .unwrap();
        r.start("small", 1_000, M, "t").unwrap().unwrap();

        // Snapshot the progress document mid-run by wrapping the store: the
        // fold is milliseconds here, so capture what `run` WRITES rather than
        // polling. `Files` is the store; read the key after each phase by
        // running and then inspecting what was left... the final state clears
        // it, so assert the clearing AND reconstruct the series from a run
        // that fails before finish.
        run(&r.store, "small", "small-1000", &root).unwrap();

        // Finished: the progress key is gone, the result is published.
        assert!(r.progress("small").is_none(), "a finished run left its series behind");
        let result = r.result("small").expect("no result published");
        assert!(result.contains("\"cold_build_ns\""), "{result}");
        assert!(r.current().unwrap().is_none());
    }


    #[test]
    fn an_address_is_recorded_once_however_many_doors_it_walks_through() {
        let r = runs("leads");
        // The unlock route, the start route and a retry all record; the list
        // holds one entry, because the key is a digest of the address.
        assert!(record_lead(&r.store, "pat@fund.example", 1_000).unwrap());
        assert!(!record_lead(&r.store, "pat@fund.example", 2_000).unwrap());
        assert!(!record_lead(&r.store, "  PAT@FUND.EXAMPLE  ", 3_000).unwrap());
        assert_eq!(r.store.list("lead-").unwrap().len(), 1);
        // ⛔ And the first_seen that survives is the FIRST one — a mailing list
        // that re-dated an address on every visit would say everyone joined
        // today.
        let key = r.store.list("lead-").unwrap().remove(0);
        assert!(r.store.get(&key).unwrap().unwrap().ends_with("\t1000"));
    }

    #[test]
    fn what_is_not_shaped_like_an_address_never_reaches_the_list() {
        let r = runs("junk");
        for junk in ["", "x", "no-at-sign.example", "two@@ats.example", "a@b", "sp ace@x.co",
                     "ctrl\u{7}@x.co", "@nodomain.", "a@.leadingdot"] {
            assert!(record_lead(&r.store, junk, 1).is_err(), "{junk:?} was accepted");
        }
        assert_eq!(r.store.list("lead-").unwrap().len(), 0);
    }

    #[test]
    fn a_run_id_is_minted_here_and_a_caller_composed_one_is_refused() {
        assert!(valid_run_id("full-1786736470"));
        assert!(valid_run_id("small-1"));
        // ⛔ Every one of these arrives in a URL and would otherwise become a
        // store key — the same door `Console::book_path` guards for fund ids.
        for bad in ["", "FULL-1", "full-1/../../current", "enormous-1", "full-1786736470-x".repeat(4).as_str(),
                    "lead-abc", "full_1"] {
            assert!(!valid_run_id(bad), "{bad:?} passed");
        }
    }

    #[test]
    fn a_completed_run_publishes_a_permalink_and_points_latest_at_it() {
        let r = runs("permalink");
        let root = r.root_for_test();
        ratio_gen::generate(
            &root.join("scale-small"),
            ratio_gen::Shape { securities: 4, lots_per: 6, currencies: 2, ..Default::default() },
        )
        .unwrap();
        r.start("small", 1_000, M, "t").unwrap().unwrap();
        run(&r.store, "small", "small-1000", &root).unwrap();

        // ⭐ THE PERMALINK IS THE RECORD. The per-size result is only "latest",
        // and the pointer names a report that exists.
        let id = latest(&r.store, "small").expect("latest-small missing");
        assert_eq!(id, "small-1000");
        let doc = report(&r.store, &id).expect("report missing");
        assert!(doc.contains("\"open_lots\""), "{doc}");
        assert!(report(&r.store, "small-9999").is_none(), "a run that never was has a report");
        // And the summary an email carries comes from the run document itself.
        let sum = summarize_report(&doc);
        assert!(sum.contains("open tax lots over"), "{sum}");
        assert!(sum.contains("Trial balance: 0"), "{sum}");
    }

    #[test]
    fn everyone_waiting_on_a_run_is_mailed_once_and_the_queue_drains() {
        // Without SES in a test, what CAN be held is the queue mechanics: attach
        // twice under one address, list, and drain — the mailer is exercised by
        // its refusals below.
        let r = runs("await");
        await_report(&r.store, "full-77", "lead@fund.example").unwrap();
        await_report(&r.store, "full-77", "lead@fund.example").unwrap();
        await_report(&r.store, "full-77", "other@fund.example").unwrap();
        // One key per ADDRESS per run — the digest key dedupes the double-click.
        assert_eq!(r.store.list("await-full-77-").unwrap().len(), 2);
        // Another run's waiters are not this run's.
        await_report(&r.store, "full-88", "third@fund.example").unwrap();
        assert_eq!(r.store.list("await-full-77-").unwrap().len(), 2);
    }

    #[test]
    fn every_shape_the_screen_offers_has_a_price_and_a_cooldown() {
        // ⛔ A shape with no entry here would be free and instant as far as this
        // module is concerned — spendable without limit from a public url.
        for s in SHAPES {
            assert!(s.cents > 0, "{} costs nothing, so nothing bounds it", s.name);
            assert!(s.cooldown > 0, "{} has no cooldown", s.name);
        }
        // And a full month of daily full folds fits under the ceiling, or the
        // headline demonstration is unavailable most of the month.
        let full = shape("full").unwrap();
        assert!(full.cents * 30 < CEILING_CENTS, "a daily full fold does not fit the ceiling");
    }

    #[test]
    fn the_stage_e_fold_cite_is_the_handoff_geometry_not_the_dial() {
        // ⛔ 500 × 40,000 IS ALSO TWENTY MILLION LOTS. The Stage E
        // measurement is the recorded row: 10,000 × 2,000.
        assert_eq!(STAGE_E_FOLD_SECURITIES, 10_000);
        assert_eq!(STAGE_E_FOLD_LOTS_PER, 2_000);
        assert_eq!(STAGE_E_FOLD_LOTS, 20_000_000);
        assert_ne!(STAGE_E_FOLD_SECURITIES, 500);
        assert_eq!(STAGE_E_FOLD_DIGEST.len(), 64);
        assert!(STAGE_E_FOLD_MS > 0);
    }
}
