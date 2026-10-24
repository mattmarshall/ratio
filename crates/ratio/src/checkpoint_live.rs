//! Manual provider evidence for #357. Uses the deployed S3 adapter and seeded
//! demo geometry; fixtures never touch production or the retained recovery cut.
use anyhow::{ensure, Result};
use prost::Message;
use ratio_project::{
    checkpoint::{
        follow_with_checkpoint_store, prefix_digest_book, Checkpoint, CheckpointMiss,
        CheckpointPin, CheckpointStore, ObjectCheckpointStore,
    },
    Projection,
};
use ratio_proto::ratio::storage::v1::{BackendConformanceCheck, BackendConformanceEvidence};
use ratio_store::{DirStore, FileBook, Journal, ObjectStore};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};

fn elapsed(start: Instant) -> Option<ratio_proto::duration_proto::google::protobuf::Duration> {
    let d = start.elapsed();
    Some(ratio_proto::duration_proto::google::protobuf::Duration {
        seconds: d.as_secs() as i64,
        nanos: d.subsec_nanos() as i32,
    })
}

struct Counted {
    inner: Arc<dyn ObjectStore>,
    reads: AtomicU64,
    lists: AtomicU64,
    writes: AtomicU64,
}
impl Counted {
    fn new(inner: Arc<dyn ObjectStore>) -> Self {
        Self {
            inner,
            reads: 0.into(),
            lists: 0.into(),
            writes: 0.into(),
        }
    }
    fn reset(&self) {
        self.reads.store(0, Ordering::Relaxed);
        self.lists.store(0, Ordering::Relaxed);
        self.writes.store(0, Ordering::Relaxed);
    }
    fn check(&self, id: &str, start: Instant, detail: String) -> BackendConformanceCheck {
        BackendConformanceCheck {
            check_id: id.into(),
            passed: true,
            elapsed: elapsed(start),
            detail,
            object_reads: self.reads.load(Ordering::Relaxed) as i64,
            object_lists: self.lists.load(Ordering::Relaxed) as i64,
            conditional_writes: self.writes.load(Ordering::Relaxed) as i64,
        }
    }
}
impl ObjectStore for Counted {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.inner.get(key)
    }
    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        self.lists.fetch_add(1, Ordering::Relaxed);
        self.inner.list(prefix)
    }
    fn put_if_absent(&self, key: &str, body: &[u8]) -> Result<bool> {
        self.writes.fetch_add(1, Ordering::Relaxed);
        self.inner.put_if_absent(key, body)
    }
}
fn fingerprint(root: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    fn visit(root: &Path, base: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) -> Result<()> {
        for item in fs::read_dir(root)? {
            let path = item?.path();
            if path.is_dir() {
                visit(&path, base, out)?;
            } else {
                out.push((path.strip_prefix(base)?.into(), fs::read(&path)?));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    visit(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

#[test]
#[ignore = "requires explicitly selected isolated live AWS lab"]
fn the_seeded_shape_loads_a_checkpoint_from_live_s3() -> Result<()> {
    let bucket = std::env::var("RATIO_CONFORMANCE_BUCKET")?;
    let prefix = std::env::var("RATIO_CONFORMANCE_PREFIX")?;
    ensure!(
        bucket.starts_with("ratio-recovery-lab-")
            && prefix.starts_with("conformance-lab/")
            && prefix.ends_with('/'),
        "isolated lab coordinates required"
    );
    ensure!(
        !std::env::vars().any(|(k, _)| k.starts_with("AWS_ENDPOINT_URL")),
        "an emulated endpoint is not live evidence"
    );
    let root = PathBuf::from(std::env::var("RATIO_CONFORMANCE_ROOT")?);
    let phase = std::env::var("RATIO_CONFORMANCE_PHASE")?;
    let evidence_path = root.join(format!("{phase}.pb"));
    ensure!(!evidence_path.exists(), "preserve prior attempt");
    let remote: Arc<dyn ObjectStore> = Arc::new(crate::scale::S3::open(&bucket, &prefix)?);
    let count = Arc::new(Counted::new(remote.clone()));
    let mut evidence=BackendConformanceEvidence { run_id:prefix.clone(), kernel_revision:std::env::var("RATIO_CONFORMANCE_REVISION")?, backend_location:format!("s3://{bucket}/{prefix}"),
        limitations:vec!["Seeded synthetic demo, same account/region. Cold processes retain baked configuration, as deployed seeds do. Timings include serial body verification; object counters are adapter calls, not paginated HTTP request counts.".into()], ..Default::default() };
    let result = (|| -> Result<()> {
        let book_dir = root.join("seeded-demo");
        if phase == "seed" {
            ensure!(!root.exists(), "fresh operator root required");
            fs::create_dir_all(&root)?;
            ensure!(
                remote.list("")?.is_empty(),
                "fresh provider namespace required"
            );
            ratio_gen::generate(
                &book_dir,
                ratio_gen::Shape {
                    securities: 20,
                    lots_per: 40,
                    currencies: 3,
                    seed: 1,
                    ..Default::default()
                },
            )?;
            let local = Arc::new(DirStore::at(root.join("object-fixture")));
            let seeded = FileBook::open_with(&book_dir, Some(local.clone()))?;
            let entries = seeded.entries()?;
            ensure!(
                entries.len() >= 4000,
                "nontrivial deployed geometry required"
            );
            evidence.journal_height = entries.len() as i64;
            evidence.journal_digest = ratio_nav::prefix_digest(&entries)?;
            let keys = local.list("")?;
            let start = Instant::now();
            std::thread::scope(|scope| -> Result<()> {
                let mut joins = Vec::new();
                for shard in 0..16 {
                    let keys = &keys;
                    let local = &local;
                    let remote = &count;
                    joins.push(scope.spawn(move || -> Result<()> {
                        for key in keys.iter().skip(shard).step_by(16) {
                            ensure!(
                                remote
                                    .put_if_absent(key, &local.get(key)?.expect("fixture key"))?,
                                "seed occupied key"
                            );
                        }
                        Ok(())
                    }));
                }
                for join in joins {
                    join.join().expect("upload worker")?;
                }
                Ok(())
            })?;
            evidence.checks.push(count.check(
                "publish-seeded-object-fixture",
                start,
                format!(
                    "{} immutable objects; bounded 16-worker upload, excluded from fold timings",
                    keys.len()
                ),
            ));
            // The later processes must actually read S3 journal objects, never a local log.
            fs::remove_file(book_dir.join("journal.jsonl"))?;
            return Ok(());
        }
        let baseline =
            BackendConformanceEvidence::decode(fs::read(root.join("seed.pb"))?.as_slice())?;
        evidence.journal_height = baseline.journal_height;
        evidence.journal_digest = baseline.journal_digest;
        let mut local = FileBook::open_attached_with(
            &book_dir,
            Some(Arc::new(DirStore::at(root.join("object-fixture")))),
        )?;
        let mut book = FileBook::open_attached_with(&book_dir, Some(count.clone()))?;
        if phase == "tail" {
            let mut tail = local
                .entries()?
                .into_iter()
                .find(|e| e.id.starts_with("cap-"))
                .expect("generator capital");
            tail.id = "provider-tail-capital".into();
            book.append(&tail)?;
            local.append(&tail)?;
            evidence.journal_height += 1;
            evidence.journal_digest = ratio_nav::prefix_digest(&local.entries()?)?;
        }
        if phase == "negative" {
            return negatives(&local, remote.clone(), &mut evidence);
        }
        let before = fingerprint(&book_dir)?;
        let checkpoint = ObjectCheckpointStore::new(count.clone(), "seeded-demo");
        count.reset();
        let start = Instant::now();
        let (projection, tel) = follow_with_checkpoint_store(&book, &checkpoint)?;
        ensure!(
            projection.prefix() as i64 == evidence.journal_height,
            "projection height"
        );
        if phase == "cold" {
            ensure!(
                !tel.hit && tel.miss == Some(CheckpointMiss::Missing),
                "first process must miss"
            );
        } else {
            ensure!(
                (phase == "checkpoint" || phase == "tail")
                    && tel.hit
                    && tel.tail_length == if phase == "tail" { 1 } else { 0 },
                "new process must load the checkpoint and exact tail"
            );
        }
        ensure!(
            before == fingerprint(&book_dir)?,
            "read inserted checkpoint into book tree"
        );
        evidence.checks.push(count.check(&format!("live-{phase}-fold"),start,format!("hit={} miss={:?} checkpoint_prefix={} journal_height={} tail_length={}; exact book fingerprint unchanged",tel.hit,tel.miss,tel.checkpoint_prefix,tel.journal_height,tel.tail_length)));
        // A separately computed prefix pin and full fold must agree with the checkpoint.
        count.reset();
        let start = Instant::now();
        ensure!(
            prefix_digest_book(&local, projection.prefix())? == evidence.journal_digest,
            "NAV prefix digest equivalence"
        );
        ensure!(
            checkpoint
                .latest()
                .map_err(|m| anyhow::anyhow!("{}", m.as_str()))?
                .expect("published checkpoint")
                .pin()
                .digest
                == evidence.journal_digest,
            "live checkpoint pin differs from NAV digest"
        );
        let mut full = Projection::new();
        full.follow_book(&local)?;
        for view in full.views() {
            ensure!(
                full.balances(view)?.value == projection.balances(view)?.value,
                "checkpoint balances diverged for {view}"
            );
        }
        evidence.checks.push(count.check("full-fold-and-nav-prefix-equivalence",start,"All declared-view trial balances equal a fresh full fold; prefix pin equals ratio_nav digest".into()));
        Ok(())
    })();
    if let Err(error) = &result {
        evidence.checks.push(BackendConformanceCheck {
            check_id: format!("{phase}-failure"),
            passed: false,
            detail: format!("{error:#}"),
            ..Default::default()
        });
    }
    fs::create_dir_all(&root)?;
    fs::write(evidence_path, evidence.encode_to_vec())?;
    for check in &evidence.checks {
        eprintln!("provider_checkpoint check={} passed={} elapsed_ms={} gets={} lists={} conditional_puts={}",check.check_id,check.passed,check.elapsed.as_ref().map(|d|d.seconds*1000+i64::from(d.nanos)/1_000_000).unwrap_or(0),check.object_reads,check.object_lists,check.conditional_writes);
    }
    result
}

fn negatives(
    local: &FileBook,
    remote: Arc<dyn ObjectStore>,
    evidence: &mut BackendConformanceEvidence,
) -> Result<()> {
    // Small local journal for the corruption cases; the checkpoint publication
    // and retrieval still use live S3. Large-shape timings remain separate.
    let mini =
        std::env::temp_dir().join(format!("ratio-checkpoint-negative-{}", std::process::id()));
    ensure!(!mini.exists(), "fresh negative fixture required");
    ratio_gen::generate(
        &mini,
        ratio_gen::Shape {
            securities: 1,
            lots_per: 1,
            currencies: 1,
            capital_txns: 1,
            ..Default::default()
        },
    )?;
    let small = FileBook::open_with(&mini, None)?;
    let mut projection = Projection::new();
    projection.follow_book(&small)?;
    let pin = CheckpointPin::of_entries(&small.entries()?)?;
    let good = Checkpoint::capture(&projection, pin.clone())?.encode()?;
    for (id, expected) in [
        ("version", CheckpointMiss::VersionRefused),
        ("truncated", CheckpointMiss::Corrupt),
        ("digest", CheckpointMiss::DigestMismatch),
        ("pin", CheckpointMiss::PrefixMismatch),
    ] {
        let start = Instant::now();
        let checkpoint = ObjectCheckpointStore::new(remote.clone(), id);
        let mut bytes = good.clone();
        if id == "version" {
            let mut v: serde_json::Value = serde_json::from_slice(&bytes)?;
            v["version"] = serde_json::json!(999);
            bytes = serde_json::to_vec(&v)?;
        }
        if id == "truncated" {
            bytes = b"{truncated".to_vec();
        }
        if id == "pin" {
            let mut wrong = pin.clone();
            wrong.digest = "0".repeat(64);
            bytes = Checkpoint::capture(&projection, wrong)?.encode()?;
        }
        let blob_hash = ratio_store::Digest::of(&bytes).to_string();
        let base = format!("_checkpoints/{}/", ratio_store::Digest::of(id.as_bytes()));
        ensure!(
            remote.put_if_absent(
                &format!("{base}blobs/{blob_hash}"),
                if id == "digest" { b"corrupt" } else { &bytes }
            )?,
            "fresh negative blob"
        );
        ensure!(
            remote.put_if_absent(
                &format!("{base}heads/{:020}", pin.prefix),
                blob_hash.as_bytes()
            )?,
            "fresh negative head"
        );
        let (replayed, tel) = follow_with_checkpoint_store(&small, &checkpoint)?;
        ensure!(
            !tel.hit && tel.miss == Some(expected),
            "{id}: wrong miss reason {:?}",
            tel.miss
        );
        for view in projection.views() {
            ensure!(
                replayed.balances(view)?.value == projection.balances(view)?.value,
                "negative fallback figure"
            );
        }
        evidence.checks.push(BackendConformanceCheck{check_id:format!("live-checkpoint-{id}-fallback"),passed:true,elapsed:elapsed(start),detail:format!("Live S3 checkpoint rejected for {}; small local journal full replay reproduced every view",expected.as_str()),..Default::default()});
    }
    // Retain the large shape's existence as a guard against accidentally
    // replacing its performance result with this small negative fixture.
    ensure!(local.entries()?.len() >= 4000, "large baseline retained");
    Ok(())
}
