//! Required CI live backend fixture: an unset URL refuses rather than skips.
use anyhow::{ensure, Result};
use ratio_postgres_store::PgStore;
use ratio_store::{ObjectStore, SeqLog};
use std::sync::{Arc, Barrier};

#[test]
fn the_native_backend_passes_the_registered_fixture_and_atomic_claims() -> Result<()> {
    let url = std::env::var("RATIO_NATIVE_PG_URL")
        .expect("RATIO_NATIVE_PG_URL is required for native Postgres conformance");
    let root = std::path::PathBuf::from(std::env::var("TEST_TMPDIR")?).join("native-pg-fixture");
    let schema = format!("ratio_objects_{}", std::process::id());
    let alpha = Arc::new(PgStore::connect_local(&url, &schema, "alpha")?);
    let beta = Arc::new(PgStore::connect_local(&url, &schema, "beta")?);
    let expected = backend_fixture::exercise(&root.join("local"), None);
    ensure!(
        backend_fixture::exercise(&root.join("remote"), Some(alpha.clone())) == expected,
        "native journal/six-plane fixture differs"
    );
    ensure!(
        backend_fixture::exercise(&root.join("remote"), Some(beta.clone())) == expected,
        "independent namespace fixture differs"
    );
    ensure!(
        alpha.put_if_absent("namespace-private", b"alpha")?
            && beta.put_if_absent("namespace-private", b"beta")?,
        "namespace claim collided"
    );
    ensure!(
        alpha.get("namespace-private")?.as_deref() == Some(b"alpha".as_ref())
            && beta.get("namespace-private")?.as_deref() == Some(b"beta".as_ref()),
        "namespace bytes leaked"
    );
    let binary = vec![0, 255, 1, 128, 10, 0];
    ensure!(
        alpha.put_if_absent("binary", &binary)? && alpha.get("binary")? == Some(binary),
        "binary object changed"
    );
    ensure!(
        PgStore::connect_local(&url, &"x".repeat(64), "refused").is_err(),
        "schema identifier truncation permitted"
    );
    let rival = Arc::new(PgStore::connect_local(&url, &schema, "alpha")?);
    let barrier = Arc::new(Barrier::new(2));
    let workers = [
        (alpha.clone(), b"first".to_vec()),
        (rival, b"second".to_vec()),
    ]
    .into_iter()
    .map(|(store, body)| {
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            (store.put_if_absent("race", &body).unwrap(), body)
        })
    })
    .collect::<Vec<_>>();
    let winners = workers
        .into_iter()
        .map(|w| w.join().unwrap())
        .filter(|(won, _)| *won)
        .collect::<Vec<_>>();
    ensure!(
        winners.len() == 1 && alpha.get("race")? == Some(winners[0].1.clone()),
        "conditional transaction lost an acknowledged writer"
    );
    for plane in [
        "journal",
        "deliveries",
        "entities",
        "facts",
        "actions",
        "explanations",
        "closes",
    ] {
        let log = SeqLog::new(alpha.clone(), format!("gap/{plane}/"));
        ensure!(log.claim(2, b"later")?, "fresh gap");
        let error = log
            .for_each_since(0, &mut |_, _| Ok(()))
            .unwrap_err()
            .to_string();
        ensure!(
            error.contains("missing") && error.contains("gap"),
            "wrong hole refusal: {error}"
        );
        ensure!(
            log.claim(1, b"first")? && !log.claim(1, b"overwrite")?,
            "conditional claim changed"
        );
        let mut rows = Vec::new();
        ensure!(
            log.for_each_since(0, &mut |_, body| {
                rows.push(body.to_vec());
                Ok(())
            })? == 2
                && rows == vec![b"first".to_vec(), b"later".to_vec()],
            "ordered stream differs"
        );
        ensure!(
            log.for_each_since(0, &mut |_, _| anyhow::bail!("consumer refused"))
                .unwrap_err()
                .to_string()
                .contains("consumer refused"),
            "consumer error swallowed"
        );
        ensure!(log.height()? == 2, "refused reader damaged connection");
    }
    let mut direct = postgres::Client::connect(&url, postgres::NoTls)?;
    let mut transaction = direct.transaction()?;
    transaction.execute(
        &format!(
            "INSERT INTO {schema}.immutable_objects (namespace,object_key,body) VALUES ($1,$2,$3)"
        ),
        &[&"alpha", &"aborted", &b"private".as_ref()],
    )?;
    ensure!(
        alpha.get("aborted")?.is_none(),
        "uncommitted row became visible"
    );
    transaction.rollback()?;
    ensure!(alpha.get("aborted")?.is_none(), "aborted row survived");
    for multithread in [false, true] {
        let mut builder = if multithread {
            tokio::runtime::Builder::new_multi_thread()
        } else {
            tokio::runtime::Builder::new_current_thread()
        };
        builder.enable_all().build()?.block_on(async {
            let store = PgStore::connect_local(&url, &schema, "runtime")?;
            store.put_if_absent("works", b"committed")?;
            ensure!(
                store.get("works")?.as_deref() == Some(b"committed".as_ref()),
                "runtime path failed"
            );
            drop(store);
            anyhow::Ok(())
        })?;
    }
    ensure!(
        PgStore::connect_local("postgres://ratio@192.0.2.1/db", &schema, "refused").is_err(),
        "non-TLS remote silently allowed"
    );
    eprintln!("native_pg_conformance journal_prefixes=4 side_planes=6 claim_winners=1 gap_planes=7 namespaces=2 passed");
    Ok(())
}
