//! Byte-level capture and restore for a quiescent object namespace.
//!
//! ⭐ The manifest is published last. It describes immutable source bytes,
//! not a journal checkpoint or permission to serve figures. An operator must
//! stop every writer and verify replay, access, and figures after restore.

use std::collections::BTreeSet;

use anyhow::{ensure, Context, Result};
use prost::Message;
use ratio_proto::ratio::storage::v1::{RecoveryManifest, RecoveryObject};
use sha2::{Digest, Sha256};

use crate::ObjectStore;

const FORMAT_VERSION: i32 = 1;

fn safe_key(key: &str) -> Result<()> {
    ensure!(
        !key.is_empty() && !key.starts_with('/') && !key.contains('\\') && !key.contains('\0'),
        "unsafe recovery object key {key:?}"
    );
    ensure!(
        key.split('/')
            .all(|part| !part.is_empty() && part != "." && part != ".."),
        "unsafe recovery object key {key:?}"
    );
    Ok(())
}

fn digest(bytes: &[u8]) -> Vec<u8> {
    Sha256::digest(bytes).to_vec()
}

fn checked_keys(store: &dyn ObjectStore, prefix: &str) -> Result<Vec<String>> {
    let mut keys = store.list(prefix)?;
    keys.sort();
    ensure!(!keys.is_empty(), "recovery source namespace is empty");
    for key in &keys {
        safe_key(key)?;
        ensure!(
            key.starts_with(prefix),
            "store listed a key outside the recovery prefix"
        );
    }
    ensure!(
        keys.windows(2).all(|pair| pair[0] != pair[1]),
        "duplicate recovery object key"
    );
    Ok(keys)
}

fn assert_empty(store: &dyn ObjectStore) -> Result<()> {
    ensure!(
        store.list("")?.is_empty(),
        "recovery destination must be empty"
    );
    Ok(())
}

/// Copy all source keys under `prefix` to an empty, independent destination.
/// The manifest key lives in the destination only and must not collide with a
/// source object. A failed capture leaves a partial destination to discard.
pub fn capture_object_namespace(
    source: &dyn ObjectStore,
    destination: &dyn ObjectStore,
    prefix: &str,
    manifest_key: &str,
) -> Result<RecoveryManifest> {
    safe_key(manifest_key)?;
    ensure!(
        prefix.is_empty() || prefix.ends_with('/'),
        "capture prefix must name a namespace"
    );
    assert_empty(destination)?;
    let keys = checked_keys(source, prefix)?;
    ensure!(
        !keys.iter().any(|key| key == manifest_key),
        "manifest key collides with a source object"
    );
    let mut objects = Vec::with_capacity(keys.len());
    for key in &keys {
        let bytes = source
            .get(key)?
            .with_context(|| format!("source object disappeared: {key}"))?;
        ensure!(
            destination.put_if_absent(key, &bytes)?,
            "backup key was already claimed: {key}"
        );
        ensure!(
            destination.get(key)?.as_deref() == Some(bytes.as_slice()),
            "backup readback differs: {key}"
        );
        objects.push(RecoveryObject {
            key: key.clone(),
            size: bytes.len().try_into().context("object size exceeds i64")?,
            sha256: digest(&bytes),
        });
    }
    // This is a detection fence, not a snapshot protocol: writers must still
    // be quiescent. Recheck bytes as well as names before publishing success.
    ensure!(
        checked_keys(source, prefix)? == keys,
        "source key set changed during capture"
    );
    for object in &objects {
        let bytes = source
            .get(&object.key)?
            .with_context(|| format!("source object disappeared: {}", object.key))?;
        ensure!(
            i64::try_from(bytes.len()).ok() == Some(object.size) && digest(&bytes) == object.sha256,
            "source object changed during capture: {}",
            object.key
        );
    }
    let manifest = RecoveryManifest {
        format_version: FORMAT_VERSION,
        source_prefix: prefix.into(),
        objects,
    };
    let manifest_bytes = manifest.encode_to_vec();
    ensure!(
        destination.put_if_absent(manifest_key, &manifest_bytes)?,
        "recovery manifest was already claimed"
    );
    ensure!(
        destination.get(manifest_key)?.as_deref() == Some(manifest_bytes.as_slice()),
        "recovery manifest readback differs"
    );
    Ok(manifest)
}

/// Verify a captured manifest and every object, then copy into an empty
/// destination. The caller must discard a partial target after any failure.
pub fn restore_object_namespace(
    backup: &dyn ObjectStore,
    destination: &dyn ObjectStore,
    manifest_key: &str,
) -> Result<RecoveryManifest> {
    safe_key(manifest_key)?;
    assert_empty(destination)?;
    let bytes = backup
        .get(manifest_key)?
        .context("recovery manifest is missing")?;
    let manifest =
        RecoveryManifest::decode(bytes.as_slice()).context("recovery manifest is corrupt")?;
    ensure!(
        manifest.format_version == FORMAT_VERSION,
        "unsupported recovery manifest version"
    );
    ensure!(
        manifest.source_prefix.is_empty() || manifest.source_prefix.ends_with('/'),
        "manifest source prefix is not a namespace"
    );
    ensure!(
        !manifest.objects.is_empty(),
        "recovery manifest has no objects"
    );
    let mut seen = BTreeSet::new();
    for object in &manifest.objects {
        safe_key(&object.key)?;
        ensure!(
            object.key.starts_with(&manifest.source_prefix),
            "manifest key is outside source prefix"
        );
        ensure!(
            seen.insert(&object.key),
            "duplicate key in recovery manifest"
        );
        ensure!(object.sha256.len() == 32, "invalid recovery object digest");
        let body = backup
            .get(&object.key)?
            .with_context(|| format!("backup object is missing: {}", object.key))?;
        ensure!(
            i64::try_from(body.len()).ok() == Some(object.size) && digest(&body) == object.sha256,
            "backup object differs from manifest: {}",
            object.key
        );
    }
    let listed: BTreeSet<_> = backup
        .list(&manifest.source_prefix)?
        .into_iter()
        .filter(|key| key != manifest_key)
        .collect();
    ensure!(
        listed == seen.into_iter().cloned().collect(),
        "backup object set differs from manifest"
    );
    for object in &manifest.objects {
        let body = backup
            .get(&object.key)?
            .context("verified backup object disappeared")?;
        ensure!(
            i64::try_from(body.len()).ok() == Some(object.size) && digest(&body) == object.sha256,
            "backup object changed during restore: {}",
            object.key
        );
        ensure!(
            destination.put_if_absent(&object.key, &body)?,
            "restore key was already claimed"
        );
        ensure!(
            destination.get(&object.key)?.as_deref() == Some(body.as_slice()),
            "restored object readback differs: {}",
            object.key
        );
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DirStore, MemoryStore};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_ROOT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn a_manifest_restores_exact_objects_and_refuses_incomplete_backup() {
        let source = MemoryStore::new();
        for (key, body) in [
            (
                "journals/_bootstrap/publications/alpha",
                b"bootstrap".as_slice(),
            ),
            (
                "journals/alpha/journal/00000000000000000001",
                b"journal".as_slice(),
            ),
            (
                "journals/alpha/closes/00000000000000000001",
                b"close".as_slice(),
            ),
            (
                "journals/beta/journal/00000000000000000001",
                b"second book".as_slice(),
            ),
        ] {
            assert!(source.put_if_absent(key, body).unwrap());
        }
        let backup = MemoryStore::unconditional();
        let manifest =
            capture_object_namespace(&source, &backup, "journals/", "_recovery/capture.pb")
                .unwrap();
        assert_eq!(manifest.objects.len(), 4);
        let restored = MemoryStore::new();
        restore_object_namespace(&backup, &restored, "_recovery/capture.pb").unwrap();
        assert_eq!(
            restored.list("journals/").unwrap(),
            source.list("journals/").unwrap()
        );
        for object in &manifest.objects {
            assert_eq!(
                restored.get(&object.key).unwrap(),
                source.get(&object.key).unwrap()
            );
        }
        assert!(
            restore_object_namespace(&backup, &restored, "_recovery/capture.pb")
                .unwrap_err()
                .to_string()
                .contains("destination must be empty")
        );
        let key = "journals/alpha/closes/00000000000000000001";
        assert!(backup.put_if_absent("journals/unlisted", b"extra").unwrap());
        assert!(
            restore_object_namespace(&backup, &MemoryStore::new(), "_recovery/capture.pb")
                .unwrap_err()
                .to_string()
                .contains("object set differs")
        );
        // The test double's overwrite dial simulates damaged backup media.
        // The production store must retain conditional writes.
        // Rebuild a clean capture before testing changed bytes.
        let clean_backup = MemoryStore::unconditional();
        for object in &manifest.objects {
            assert!(clean_backup
                .put_if_absent(&object.key, &source.get(&object.key).unwrap().unwrap())
                .unwrap());
        }
        assert!(clean_backup
            .put_if_absent("_recovery/capture.pb", &manifest.encode_to_vec())
            .unwrap());
        assert!(clean_backup.put_if_absent(key, b"tampered").unwrap());
        let empty = MemoryStore::new();
        assert!(
            restore_object_namespace(&clean_backup, &empty, "_recovery/capture.pb")
                .unwrap_err()
                .to_string()
                .contains("differs from manifest")
        );
    }

    #[test]
    fn a_configured_store_can_capture_its_entire_relative_namespace() {
        let root = std::env::temp_dir().join(format!(
            "ratio-recovery-{}-{}",
            std::process::id(),
            NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let source_root = root.join("source");
        let source = DirStore::at(&source_root);
        assert!(source
            .put_if_absent("_bootstrap/publications/alpha", b"bootstrap")
            .unwrap());
        assert!(source
            .put_if_absent("alpha/journal/00000000000000000001", b"entry")
            .unwrap());
        let backup_root = root.join("backup");
        let backup = DirStore::at(&backup_root);
        capture_object_namespace(&source, &backup, "", "_recovery/capture.pb").unwrap();
        std::fs::remove_dir_all(source_root).unwrap();
        let restored = DirStore::at(root.join("restored"));
        restore_object_namespace(&backup, &restored, "_recovery/capture.pb").unwrap();
        assert_eq!(
            restored
                .get("_bootstrap/publications/alpha")
                .unwrap()
                .unwrap(),
            b"bootstrap"
        );
        assert_eq!(
            restored
                .get("alpha/journal/00000000000000000001")
                .unwrap()
                .unwrap(),
            b"entry"
        );
        std::fs::remove_file(backup_root.join("alpha/journal/00000000000000000001")).unwrap();
        let empty = DirStore::at(root.join("empty-restore"));
        assert!(
            restore_object_namespace(&backup, &empty, "_recovery/capture.pb")
                .unwrap_err()
                .to_string()
                .contains("backup object is missing")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
