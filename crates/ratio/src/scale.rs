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
                Err(e) => Err(anyhow::anyhow!("taking the run lock in S3: {e}")),
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
                        .map_err(|e| anyhow::anyhow!("reading {k}: {e}"))?
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
                Err(e) => Err(anyhow::anyhow!("reading {k}: {e}")),
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
                .map_err(|e| anyhow::anyhow!("writing {k}: {e}"))?;
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
                .map_err(|e| anyhow::anyhow!("releasing {k}: {e}"))?;
            Ok(())
        })
    }
}

/// How a fold is actually set going, once the policy has allowed it.
///
/// ⛔ SEPARATE FROM THE POLICY ON PURPOSE. Whether a visitor MAY start a fold is
/// decided by `Runs::start` over a `Store`, and both are testable. WHERE the
/// fold then happens is a deployment fact — a thread on a laptop, a Fargate task
/// on the demo — and nothing about the decision should depend on which.
pub trait Launcher {
    /// Returns a token identifying the run: a task arn, a thread name, a word.
    fn launch(&self, size: &str) -> Result<String>;
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
    fn launch(&self, size: &str) -> Result<String> {
        let token = format!("thread:{size}");
        let size = size.to_string();
        let books = self.books.clone();
        let root = self.root.clone();
        std::thread::Builder::new()
            .name(format!("scale-{size}"))
            .spawn(move || {
                let store = Files::at(&root);
                if let Err(e) = run(&store, &size, &books) {
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
    fn launch(&self, size: &str) -> Result<String> {
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
                                .build(),
                        )
                        .build(),
                )
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("starting the fold task: {e}"))?;
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
pub fn run<S: Store>(store: &S, size: &str, book_root: &Path) -> Result<()> {
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
    runs.finish(size, published)?;
    outcome.map(|_| ())
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
    if ratio_gen::shape_of(&dir).is_err() {
        store.put(&format!("progress-{size}"), "generating")?;
        let dials = ratio_gen::Shape {
            securities: shape.securities,
            lots_per: shape.lots_per,
            currencies: 3,
            ..Default::default()
        };
        ratio_gen::generate(&dir, dials).context("generating the book")?;
    }

    store.put(&format!("progress-{size}"), "folding 0")?;
    let started = std::time::Instant::now();
    // ⚠ THE PROGRESS WRITE IS THROTTLED, and the reason is the one
    // `of_book_with_progress` states: whatever the callback does is charged to
    // the cold build. A `PutObject` every 65,536 entries is 2,136 writes at the
    // full shape — enough to bill for and enough to distort what it reports.
    let mut last = std::time::Instant::now();
    let proj = ratio_project::Projection::of_book_with_progress(&dir, &mut |n| {
        if last.elapsed() >= std::time::Duration::from_secs(5) {
            last = std::time::Instant::now();
            let _ = store.put(&format!("progress-{size}"), &format!("folding {n}"));
        }
    })
    .context("folding the book")?;
    let cold_ns = started.elapsed().as_nanos() as i64;

    let entries = proj.prefix() as i64;
    let open_lots = proj.open_lots();
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

    /// A run ended — with a result, or without one.
    ///
    /// ⚠ THE SPEND IS NOT REFUNDED ON FAILURE. A fold that died after fifteen
    /// minutes consumed fifteen minutes of compute, and a ceiling that gave the
    /// money back for it would let a failing run be retried without limit.
    pub fn finish(&self, size: &str, result: Option<&str>) -> Result<()> {
        if let Some(r) = result {
            self.store.put(&format!("result-{size}.json"), r)?;
        }
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
        let outcome = run(&r.store, "small", &bad);

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
}
