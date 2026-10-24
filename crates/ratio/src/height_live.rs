//! Manual #358 provider measurements; no projection or figure is decided here.
use anyhow::{ensure, Result};
use prost::Message;
use ratio_proto::ratio::storage::v1::{BackendConformanceCheck, BackendConformanceEvidence};
use ratio_store::{DirStore, FileBook, Journal, ObjectStore, SeqLog};
use std::{fs, path::PathBuf, sync::Arc, time::Instant};

struct Listed(Arc<dyn ObjectStore>);
impl ObjectStore for Listed {
    fn put_if_absent(&self, k: &str, b: &[u8]) -> Result<bool> {
        self.0.put_if_absent(k, b)
    }
    fn get(&self, k: &str) -> Result<Option<Vec<u8>>> {
        self.0.get(k)
    }
    fn list(&self, p: &str) -> Result<Vec<String>> {
        self.0.list(p)
    }
    // Intentionally use the default full-list height: the before curve.
}
fn check(id: &str, start: Instant, detail: String) -> BackendConformanceCheck {
    let d = start.elapsed();
    BackendConformanceCheck {
        check_id: id.into(),
        passed: true,
        elapsed: Some(ratio_proto::duration_proto::google::protobuf::Duration {
            seconds: d.as_secs() as i64,
            nanos: d.subsec_nanos() as i32,
        }),
        detail,
        ..Default::default()
    }
}
#[test]
#[ignore = "requires an isolated live S3 lab"]
fn height_and_append_cost_are_measured_on_live_s3() -> Result<()> {
    let bucket = std::env::var("RATIO_CONFORMANCE_BUCKET")?;
    let prefix = std::env::var("RATIO_CONFORMANCE_PREFIX")?;
    ensure!(
        bucket.starts_with("ratio-recovery-lab-")
            && prefix.starts_with("conformance-lab/")
            && prefix.ends_with("/358/"),
        "isolated #358 namespace required"
    );
    ensure!(
        !std::env::vars().any(|(k, _)| k.starts_with("AWS_ENDPOINT_URL")),
        "live provider only"
    );
    let phase = std::env::var("RATIO_CONFORMANCE_PHASE")?;
    let root = PathBuf::from(std::env::var("RATIO_CONFORMANCE_ROOT")?);
    let output = root.join(format!("{phase}.pb"));
    ensure!(!output.exists(), "preserve prior phase");
    let store: Arc<dyn ObjectStore> = Arc::new(crate::scale::S3::open(&bucket, &prefix)?);
    let mut evidence=BackendConformanceEvidence{run_id:prefix.clone(),kernel_revision:std::env::var("RATIO_CONFORMANCE_REVISION")?,backend_location:format!("s3://{bucket}/{prefix}"),limitations:vec!["Live S3 timings, synthetic immutable sequence fixtures. This measures height/append, not full projection. Request-count bounds are algorithmic, not instrumented HTTP retries. No production namespace is touched.".into()],..Default::default()};
    let result = (|| -> Result<()> {
        if phase == "seed" {
            ensure!(
                !root.exists() && store.list("")?.is_empty(),
                "new lab namespace and operator root required"
            );
            fs::create_dir_all(&root)?;
            let source = PathBuf::from(std::env::var("RATIO_HEIGHT_SOURCE_ROOT")?);
            let local = FileBook::open_attached_with(
                source.join("seeded-demo"),
                Some(Arc::new(DirStore::at(source.join("object-fixture")))),
            )?;
            let template = local
                .entries()?
                .into_iter()
                .find(|e| e.id.starts_with("cap-"))
                .expect("capital fixture");
            let start = Instant::now();
            std::thread::scope(|scope| -> Result<()> {
                let mut jobs = Vec::new();
                for shard in 0..32 {
                    let store = &store;
                    let template = &template;
                    jobs.push(scope.spawn(move || -> Result<()> {
                        for height in [5396usize, 65536] {
                            for seq in (shard + 1..=height).step_by(32) {
                                let mut entry = template.clone();
                                entry.id = format!("height-{height}-{seq}");
                                let key = format!("n-{height}/journal/{seq:020}");
                                ensure!(
                                    store.put_if_absent(&key, &serde_json::to_vec(&entry)?)?,
                                    "occupied fixture key"
                                );
                            }
                        }
                        Ok(())
                    }));
                }
                for job in jobs {
                    job.join().expect("upload worker")?;
                }
                Ok(())
            })?;
            evidence.checks.push(check("publish-height-fixtures",start,"Two complete contiguous journals: 5396 and 65536 entries; bounded 32-worker upload excluded from append timing".into()));
            return Ok(());
        }
        ensure!(
            phase == "measure" || phase == "adaptive",
            "known phase required"
        );
        if phase == "adaptive" {
            let prior =
                BackendConformanceEvidence::decode(fs::read(root.join("measure.pb"))?.as_slice())?;
            ensure!(
                prior.checks.iter().all(|c| c.passed),
                "preserved original measurement must pass"
            );
        }
        for height in [5396u64, 65536] {
            let key = format!("n-{height}/journal/");
            let expected = height + if phase == "adaptive" { 2 } else { 0 };
            let before = SeqLog::new(Arc::new(Listed(store.clone())), &key);
            let after = SeqLog::new(store.clone(), &key);
            for sample in 0..3 {
                let t = Instant::now();
                ensure!(before.height()? == expected, "baseline height");
                evidence.checks.push(check(
                    &format!("listing-height-{height}-{sample}"),
                    t,
                    "Full listing default; actual provider timing".into(),
                ));
                let t = Instant::now();
                ensure!(after.height()? == expected, "optimized height");
                evidence.checks.push(check(
                    &format!("bounded-height-{height}-{sample}"),
                    t,
                    "Production S3 max_sequence; actual provider timing".into(),
                ));
            }
            let body = store
                .get(&format!("{key}{:020}", 1))?
                .expect("fixture entry");
            let t = Instant::now();
            ensure!(
                before.append(&body)? == expected + 1,
                "baseline next conditional slot"
            );
            evidence.checks.push(check(
                &format!("listing-append-{height}"),
                t,
                "Default listing height plus real conditional append".into(),
            ));
            let t = Instant::now();
            ensure!(
                after.append(&body)? == expected + 2,
                "optimized next conditional slot"
            );
            evidence.checks.push(check(
                &format!("bounded-append-{height}"),
                t,
                "Production height plus real conditional append".into(),
            ));
            ensure!(
                !store.put_if_absent(&format!("{key}{:020}", expected + 2), b"overwrite")?,
                "occupied conditional slot overwritten"
            );
            ensure!(
                store.get(&format!("{key}{:020}", expected + 2))? == Some(body),
                "acknowledged body changed"
            );
        }
        let hole = SeqLog::new(store.clone(), format!("hole-{phase}/journal/"));
        ensure!(hole.claim(2, b"later")?, "fresh hole fixture");
        let error = hole
            .for_each_since(0, &mut |_, _| Ok(()))
            .unwrap_err()
            .to_string();
        ensure!(
            error.contains("missing") && error.contains("gap"),
            "wrong hole refusal: {error}"
        );
        evidence.checks.push(check("live-conditional-claim-and-hole-refusal",Instant::now(),"Occupied conditional claims refused; acknowledged bytes unchanged; a missing first slot refuses replay".into()));
        Ok(())
    })();
    if let Err(e) = &result {
        evidence.checks.push(BackendConformanceCheck {
            check_id: format!("{phase}-failure"),
            passed: false,
            detail: format!("{e:#}"),
            ..Default::default()
        });
    }
    fs::create_dir_all(&root)?;
    fs::write(output, evidence.encode_to_vec())?;
    for c in &evidence.checks {
        let d = c.elapsed.as_ref();
        eprintln!(
            "provider_height check={} passed={} elapsed_ms={}",
            c.check_id,
            c.passed,
            d.map(|d| d.seconds * 1000 + i64::from(d.nanos) / 1_000_000)
                .unwrap_or(0)
        );
    }
    result
}
