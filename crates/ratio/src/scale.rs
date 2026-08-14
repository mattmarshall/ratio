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
    /// How long before this shape may be run again.
    pub cooldown: i64,
    /// Rounded-up estimate of what one run costs, in cents.
    pub cents: i64,
}

/// ⛔ THE FULL SHAPE IS ONCE A DAY, AND THAT IS THE BUDGET SPEAKING. At ~8 cents
/// a run, daily is ~$2.40 a month against a $5 ceiling that also has to cover
/// the small runs, the function, the gateway and the storage.
pub const SHAPES: [Shape; 3] = [
    Shape { name: "small", cooldown: 10 * 60, cents: 1 },
    Shape { name: "medium", cooldown: 60 * 60, cents: 2 },
    Shape { name: "full", cooldown: 24 * 60 * 60, cents: 8 },
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

/// The record as a directory. Used by `ratio watch` on a laptop, and by every
/// test in this file.
pub struct Files {
    root: PathBuf,
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
