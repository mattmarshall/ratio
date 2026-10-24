//! #359 manual provider companion to the mandatory hermetic conformance gate.
use anyhow::{ensure, Result};
use prost::Message;
use ratio_proto::ratio::storage::v1::{BackendConformanceCheck, BackendConformanceEvidence};
use ratio_store::{DigestBuilder, ObjectStore, SeqLog};
use std::{fs, path::PathBuf, sync::Arc, time::Instant};

#[test]
#[ignore = "requires an isolated live S3 lab"]
fn the_registered_fixture_conforms_against_live_s3() -> Result<()> {
    let bucket = std::env::var("RATIO_CONFORMANCE_BUCKET")?;
    let prefix = std::env::var("RATIO_CONFORMANCE_PREFIX")?;
    ensure!(
        bucket.starts_with("ratio-recovery-lab-")
            && prefix.starts_with("conformance-lab/")
            && prefix.ends_with("/359/"),
        "isolated #359 namespace required"
    );
    ensure!(
        !std::env::vars().any(|(k, _)| k.starts_with("AWS_ENDPOINT_URL")),
        "emulation is not provider evidence"
    );
    let root = PathBuf::from(std::env::var("RATIO_CONFORMANCE_ROOT")?);
    ensure!(!root.exists(), "preserve previous attempt");
    fs::create_dir_all(&root)?;
    let objects: Arc<dyn ObjectStore> = Arc::new(crate::scale::S3::open(&bucket, &prefix)?);
    ensure!(
        objects.list("")?.is_empty(),
        "new provider namespace required"
    );
    let mut evidence=BackendConformanceEvidence{run_id:prefix.clone(),kernel_revision:std::env::var("RATIO_CONFORMANCE_REVISION")?,backend_location:format!("s3://{bucket}/{prefix}"),limitations:vec!["Actual S3, no endpoint override. This is the identical reusable journal/six-SeqLog fixture used by the cheap mandatory gate; retained operational evidence and customer guarantees are separately covered by accepted recovery.".into()],..Default::default()};
    let started = Instant::now();
    let local = backend_fixture::exercise(&root.join("local-book"), None);
    let remote = backend_fixture::exercise(&root.join("remote-book"), Some(objects.clone()));
    ensure!(
        local == remote,
        "live provider differs from the registered local fixture"
    );
    evidence.journal_height = 3;
    evidence.journal_digest = remote.2.last().expect("final pin").clone();
    evidence.checks.push(BackendConformanceCheck{check_id:"live-registered-fixture-equivalence".into(),passed:true,elapsed:Some(duration(started)),detail:"Exact journal bytes, every prefix (including empty), all six side-plane records, and trial balance match local JSONL; every prefix equals ratio_nav digest".into(),..Default::default()});
    for (index, records) in remote.1.iter().enumerate() {
        let mut digest = DigestBuilder::new();
        for record in records {
            digest.update(record.as_bytes());
            digest.update(b"\n");
        }
        evidence.checks.push(BackendConformanceCheck {
            check_id: format!("live-side-plane-{index}-digest"),
            passed: true,
            detail: format!("records={} sha256={}", records.len(), digest.finish()),
            ..Default::default()
        });
    }
    for plane in [
        "journal",
        "deliveries",
        "entities",
        "facts",
        "actions",
        "explanations",
        "closes",
    ] {
        let start = Instant::now();
        let log = SeqLog::new(objects.clone(), format!("adversarial/{plane}/"));
        ensure!(log.claim(2, b"later")?, "fresh gap fixture");
        let error = log
            .for_each_since(0, &mut |_, _| Ok(()))
            .unwrap_err()
            .to_string();
        ensure!(
            error.contains("missing") && error.contains("gap"),
            "{plane}: wrong gap refusal {error}"
        );
        ensure!(log.claim(1, b"first")?, "first claim");
        ensure!(
            !log.claim(1, b"loser")?,
            "{plane}: occupied conditional slot overwritten"
        );
        ensure!(
            log.get(1)?.as_deref() == Some(b"first".as_ref()),
            "{plane}: acknowledged bytes changed"
        );
        let mut seen = Vec::new();
        log.for_each_since(0, &mut |_, bytes| {
            seen.push(bytes.to_vec());
            Ok(())
        })?;
        ensure!(
            seen == vec![b"first".to_vec(), b"later".to_vec()],
            "{plane}: replay order"
        );
        evidence.checks.push(BackendConformanceCheck{check_id:format!("live-{plane}-claim-and-no-hole"),passed:true,elapsed:Some(duration(start)),detail:"Missing first sequence refused; after filling it, conditional overwrite refused and both acknowledged bodies replayed in order".into(),..Default::default()});
    }
    fs::write(root.join("evidence.pb"), evidence.encode_to_vec())?;
    eprintln!("live_provider_conformance passed journal_prefixes={} side_planes=6 conditional_and_gap_planes=7",remote.2.len());
    Ok(())
}
fn duration(start: Instant) -> ratio_proto::duration_proto::google::protobuf::Duration {
    let d = start.elapsed();
    ratio_proto::duration_proto::google::protobuf::Duration {
        seconds: d.as_secs() as i64,
        nanos: d.subsec_nanos() as i32,
    }
}
