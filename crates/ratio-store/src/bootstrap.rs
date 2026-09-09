//! The complete bootstrap precedes its conditional book-ID publication.
//! `tla/BookPublication.tla`: staged content is neither a book nor a grant.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{bail, ensure, Context, Result};
use prost::Message;
pub use ratio_proto::ratio::storage::v1::{BookBootstrap, BookPublication, CreatorGrant};

use crate::{Account, Digest, ObjectStore};

const PUBLICATIONS: &str = "_bootstrap/publications/";
const BLOBS: &str = "_bootstrap/blobs/";
pub const MARKER: &str = "BOOTSTRAP.pb";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub fn valid_book_id(id: &str) -> bool {
    !id.is_empty()
        && id != "_bootstrap"
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn kind_name(kind: i32) -> Result<&'static str> {
    match kind {
        1 => Ok("personal"),
        2 => Ok("investment"),
        3 => Ok("project"),
        4 => Ok("operating"),
        _ => bail!("bootstrap has an unsupported book kind"),
    }
}

pub fn validate(bootstrap: &BookBootstrap) -> Result<()> {
    ensure!(
        bootstrap.format_version == 1,
        "unsupported bootstrap version"
    );
    ensure!(
        valid_book_id(&bootstrap.book_id),
        "invalid bootstrap book ID"
    );
    kind_name(bootstrap.kind)?;
    ensure!(
        !bootstrap.display_name.trim().is_empty(),
        "bootstrap display name is absent"
    );
    ensure!(!bootstrap.chart.is_empty(), "bootstrap chart is absent");
    ensure!(
        Digest::of(&bootstrap.chart).as_str() == bootstrap.chart_digest,
        "bootstrap chart digest mismatch"
    );
    let accounts: Vec<Account> =
        serde_json::from_slice(&bootstrap.chart).context("invalid bootstrap chart")?;
    ensure!(!accounts.is_empty(), "bootstrap chart is empty");
    ensure!(!bootstrap.config.is_empty(), "bootstrap config is absent");
    std::str::from_utf8(&bootstrap.config).context("bootstrap config is not UTF-8")?;
    ensure!(
        Digest::of(&bootstrap.config).as_str() == bootstrap.config_digest,
        "bootstrap config digest mismatch"
    );
    ensure!(
        bootstrap.active == bootstrap.config_digest,
        "bootstrap ACTIVE mismatch"
    );
    ensure!(
        bootstrap.history == [bootstrap.config_digest.clone()],
        "bootstrap HISTORY mismatch"
    );
    let grant = bootstrap
        .creator_grant
        .as_ref()
        .context("bootstrap creator grant is absent")?;
    ensure!(
        bootstrap.creator_subject == grant.subject,
        "bootstrap creation attribution mismatch"
    );
    ensure!(
        grant.book_id == bootstrap.book_id,
        "bootstrap creator grant book mismatch"
    );
    ensure!(
        !grant.subject.trim().is_empty() && !grant.subject.chars().any(char::is_control),
        "bootstrap creator subject is invalid"
    );
    Ok(())
}

/// An explicit store makes independent containers testable without replacing
/// the process-wide OnceLock after the first book was opened.
#[derive(Clone)]
pub struct BootstrapStore {
    objects: Arc<dyn ObjectStore>,
}

impl BootstrapStore {
    pub fn new(objects: Arc<dyn ObjectStore>) -> Self {
        Self { objects }
    }

    pub fn publish(&self, bootstrap: &BookBootstrap) -> Result<BookPublication> {
        validate(bootstrap)?;
        // A legacy journal is not an empty ID available for a new generation.
        ensure!(
            self.objects
                .list(&format!("{}/", bootstrap.book_id))?
                .is_empty(),
            "unregistered book data already exists"
        );
        let bytes = bootstrap.encode_to_vec();
        let digest = Digest::of(&bytes).as_str().to_string();
        let blob_key = format!("{BLOBS}{digest}");
        self.objects.put_if_absent(&blob_key, &bytes)?;
        ensure!(
            self.objects.get(&blob_key)?.as_deref() == Some(bytes.as_slice()),
            "staged bootstrap bytes could not be verified"
        );
        let publication = BookPublication {
            format_version: 1,
            book_id: bootstrap.book_id.clone(),
            bootstrap_digest: digest,
        };
        // ⭐ Only this conditional claim exposes a book. The losing creator
        // has written no local book and has obtained no membership.
        ensure!(
            self.objects.put_if_absent(
                &format!("{PUBLICATIONS}{}", bootstrap.book_id),
                &publication.encode_to_vec()
            )?,
            "book already has a durable publication"
        );
        Ok(publication)
    }

    pub fn get(&self, id: &str) -> Result<Option<(BookPublication, BookBootstrap)>> {
        ensure!(valid_book_id(id), "invalid book ID");
        let Some(bytes) = self.objects.get(&format!("{PUBLICATIONS}{id}"))? else {
            return Ok(None);
        };
        let publication =
            BookPublication::decode(bytes.as_slice()).context("invalid book publication")?;
        ensure!(
            publication.format_version == 1,
            "unsupported publication version"
        );
        ensure!(publication.book_id == id, "publication book ID mismatch");
        let digest = Digest::parse(&publication.bootstrap_digest)?;
        let bytes = self
            .objects
            .get(&format!("{BLOBS}{}", digest.as_str()))?
            .context("published bootstrap is missing")?;
        ensure!(
            Digest::of(&bytes) == digest,
            "published bootstrap digest mismatch"
        );
        let bootstrap =
            BookBootstrap::decode(bytes.as_slice()).context("invalid published bootstrap")?;
        validate(&bootstrap)?;
        ensure!(
            bootstrap.book_id == id,
            "published bootstrap book ID mismatch"
        );
        Ok(Some((publication, bootstrap)))
    }

    pub fn ids(&self) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        for key in self.objects.list(PUBLICATIONS)? {
            let id = key
                .strip_prefix(PUBLICATIONS)
                .context("catalog returned an unrelated key")?;
            ensure!(valid_book_id(id), "catalog contains an invalid book ID");
            ids.push(id.to_string());
        }
        ids.sort();
        ids.dedup();
        Ok(ids)
    }
}

fn local_files(
    publication: &BookPublication,
    bootstrap: &BookBootstrap,
) -> Result<Vec<(String, Vec<u8>)>> {
    let sidecar = format!(
        "kind = {:?}\ndisplay_name = {}\n",
        kind_name(bootstrap.kind)?,
        serde_json::to_string(&bootstrap.display_name)?
    );
    Ok(vec![
        ("accounts.json".into(), bootstrap.chart.clone()),
        ("book.toml".into(), sidecar.into_bytes()),
        (
            format!("config/{}", bootstrap.config_digest),
            bootstrap.config.clone(),
        ),
        ("config/ACTIVE".into(), bootstrap.active.as_bytes().to_vec()),
        (
            "config/HISTORY".into(),
            format!("{}\n", bootstrap.history.join("\n")).into_bytes(),
        ),
        (MARKER.into(), publication.encode_to_vec()),
    ])
}

/// Materialize a verified immutable cache in one rename. Existing data is
/// checked, never repaired with opening defaults over a later local state.
pub fn materialize(
    path: &Path,
    publication: &BookPublication,
    bootstrap: &BookBootstrap,
) -> Result<()> {
    validate(bootstrap)?;
    ensure!(
        publication.format_version == 1 && publication.book_id == bootstrap.book_id,
        "publication identity mismatch"
    );
    ensure!(
        publication.bootstrap_digest == Digest::of(&bootstrap.encode_to_vec()).as_str(),
        "publication bootstrap digest mismatch"
    );
    let files = local_files(publication, bootstrap)?;
    let verify = || -> Result<()> {
        ensure!(
            path.join(MARKER).is_file(),
            "local book conflicts with durable publication"
        );
        for (name, expected) in &files {
            let got = fs::read(path.join(name))
                .with_context(|| format!("bootstrap cache file {name} is missing"))?;
            ensure!(
                &got == expected,
                "bootstrap cache differs at {}; refusing to reset local state",
                name
            );
        }
        Ok(())
    };
    if path.exists() {
        return verify();
    }
    let parent = path.parent().context("a book cache needs a parent")?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(
        ".bootstrap-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&temp).context("creating bootstrap staging directory")?;
    let result = (|| -> Result<()> {
        fs::create_dir(temp.join("config"))?;
        for (name, bytes) in &files {
            fs::write(temp.join(name), bytes)?;
        }
        match fs::rename(&temp, path) {
            Ok(()) => Ok(()),
            Err(_) if path.exists() => verify(),
            Err(error) => Err(error).context("publishing the local bootstrap cache"),
        }
    })();
    if temp.exists() {
        let _ = fs::remove_dir_all(&temp);
    }
    result
}
