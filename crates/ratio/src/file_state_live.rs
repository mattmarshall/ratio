//! Manual actual S3 proof for unpublished chart/config/promotion custody.
use anyhow::{ensure, Result};
use prost::Message;
use ratio_proto::ratio::storage::v1::{BackendConformanceCheck, BackendConformanceEvidence};
use ratio_store::{
    Account, AccountTypeRecord, ConfigStore, Digest, FileBook, Journal, JournalEntry, ObjectStore,
};
use std::{fs, path::PathBuf, sync::Arc, time::Instant};
const FIRST: &[u8] = b"rules=[]\n";
const SECOND: &[u8] = b"# reviewed\nrules=[]\n";
const DRAFT: &[u8] = b"# draft\nrules=[]\n";
fn chart() -> Vec<Account> {
    vec![
        Account {
            dim: 1,
            display_name: "Cash".into(),
            account_type: AccountTypeRecord::Asset,
        },
        Account {
            dim: 2,
            display_name: "Capital".into(),
            account_type: AccountTypeRecord::Equity,
        },
    ]
}
fn entry(id: &str, config: &Digest, amount: i64) -> JournalEntry {
    serde_json::from_value(serde_json::json!({"id":id,"config":config.as_str(),"postings":[{"dim":1,"amount":amount},{"dim":2,"amount":-amount}]})).unwrap()
}
#[test]
#[ignore = "requires isolated actual S3"]
fn unpublished_metadata_recovers_from_s3_and_conflicting_cache_refuses() -> Result<()> {
    let bucket = std::env::var("RATIO_CONFORMANCE_BUCKET")?;
    let prefix = std::env::var("RATIO_CONFORMANCE_PREFIX")?;
    ensure!(
        bucket.starts_with("ratio-recovery-lab-")
            && prefix.starts_with("conformance-lab/")
            && prefix.ends_with("/363/"),
        "isolated #363 namespace required"
    );
    ensure!(
        !std::env::vars().any(|(k, _)| k.starts_with("AWS_ENDPOINT_URL")),
        "live provider required"
    );
    let root = PathBuf::from(std::env::var("RATIO_CONFORMANCE_ROOT")?);
    let phase = std::env::var("RATIO_CONFORMANCE_PHASE")?;
    let objects: Arc<dyn ObjectStore> = Arc::new(crate::scale::S3::open(&bucket, &prefix)?);
    let path = root.join("serving/book");
    let start = Instant::now();
    let first = Digest::of(FIRST);
    let second = Digest::of(SECOND);
    let draft = Digest::of(DRAFT);
    if phase == "seed" {
        ensure!(
            !root.exists() && objects.list("")?.is_empty(),
            "fresh namespace required"
        );
        fs::create_dir_all(&root)?;
        let mut book = FileBook::open_with(&path, Some(objects.clone()))?;
        book.put_accounts(&chart())?;
        ensure!(book.put(FIRST)? == first, "first config");
        book.set_active(&first)?;
        ensure!(
            book.put(DRAFT)? == draft && book.active()? == Some(first.clone()),
            "draft must not promote"
        );
        ensure!(book.put(SECOND)? == second, "second config");
        book.set_active(&second)?;
        book.append(&entry("first", &first, 12345))?;
        book.append(&entry("second", &second, 3655))?;
        drop(book);
        fs::remove_dir_all(&path)?;
    } else {
        ensure!(
            phase == "cold" && root.exists() && !path.exists(),
            "cold process requires removed cache"
        );
        let book = FileBook::open_with(&path, Some(objects.clone()))?;
        ensure!(
            book.accounts()? == chart()
                && book.active()? == Some(second.clone())
                && book.history()? == vec![second.clone(), first.clone()],
            "durable metadata differs"
        );
        ensure!(book.get(&draft)? == DRAFT, "unpromoted bytes lost");
        let expected = vec![
            entry("first", &first, 12345),
            entry("second", &second, 3655),
        ];
        ensure!(
            ratio_nav::prefix_digest(&book.entries()?)? == ratio_nav::prefix_digest(&expected)?,
            "journal digest differs"
        );
        ensure!(
            book.trial_balance()?.debits == 16000 && book.trial_balance()?.credits == 16000,
            "restored figures differ"
        );
        ensure!(
            !path.join("book.toml").exists() && !root.join("serving/MEMBERSHIP.tsv").exists(),
            "unpublished metadata invented identity"
        );
        drop(book);
        fs::write(path.join("config/ACTIVE"), first.as_str())?;
        let error = FileBook::open_with(&path, Some(objects.clone()))
            .err()
            .unwrap()
            .to_string();
        ensure!(error.contains("conflicts"), "wrong cache refusal: {error}");
        ensure!(
            fs::read_to_string(path.join("config/ACTIVE"))? == first.as_str(),
            "conflicting cache silently reset"
        );
        fs::rename(&path, root.join("quarantined-conflicting-cache"))?;
        ensure!(
            FileBook::open_with(&path, Some(objects))?.active()? == Some(second.clone()),
            "fresh cache repair did not recover durable state"
        );
    }
    let elapsed = start.elapsed();
    let evidence=BackendConformanceEvidence{run_id:prefix.clone(),kernel_revision:std::env::var("RATIO_CONFORMANCE_REVISION")?,backend_location:format!("s3://{bucket}/{prefix}"),journal_height:2,journal_digest:ratio_nav::prefix_digest(&[entry("first",&first,12345),entry("second",&second,3655)])?,checks:vec![BackendConformanceCheck{check_id:format!("unpublished-metadata-{phase}"),passed:true,elapsed:Some(ratio_proto::duration_proto::google::protobuf::Duration{seconds:elapsed.as_secs() as i64,nanos:elapsed.subsec_nanos() as i32}),detail:if phase=="seed" {"Seeded two cited postings; staging a draft did not promote it; serving cache removed".into()} else {"Cold chart/config/history/draft bytes, journal digest and figures exact; conflicting cache refused unchanged and quarantined before fresh-root repair; no invented identity".into()},..Default::default()}],limitations:vec!["Small synthetic metadata proof against actual S3. Published bootstrap/control and customer recovery guarantees remain separately scoped.".into()],..Default::default()};
    fs::write(root.join(format!("{phase}.pb")), evidence.encode_to_vec())?;
    eprintln!(
        "live_filebook_metadata phase={phase} passed elapsed_ms={}",
        elapsed.as_millis()
    );
    Ok(())
}
